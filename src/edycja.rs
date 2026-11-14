// Edycja klipów: przycinanie (szybkie bez kodowania albo dokładne z kodowaniem), kadrowanie,
// kompresja do docelowego rozmiaru pliku (np. 10 MB na Discorda) i zapis jako zapętlony GIF/WebP.
// Wynik trafia do nowego pliku albo (gdy użytkownik nie zaznaczy „Zachowaj oryginał”) zastępuje
// oryginał, który idzie do Kosza. GIF i WebP nigdy nie zastępują klipu. Wersja przygotowana tylko
// do przeciągnięcia (np. na Discorda) idzie do osobnego folderu i nie trafia do biblioteki.
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Deserialize)]
pub struct Zlecenie {
    /// Początek i koniec w sekundach (cały klip, jeśli nie przycinamy)
    pub start: f64,
    pub koniec: f64,
    /// Czy użytkownik przyciął klip (wpływa na nazwę pliku)
    pub przyciete: bool,
    /// Dokładne cięcie co do klatki (wymaga kodowania); bez tego cięcie jest od klatki kluczowej
    pub dokladnie: bool,
    /// Docelowy rozmiar w MB (1 MB = 1 000 000 B); None = jakość bez zmian
    pub rozmiar_mb: Option<u32>,
    /// Zastąp oryginał (stary plik trafia do Kosza)
    pub zastap: bool,
    /// Głośność (1.0 = bez zmian): [gra, mikrofon] dla klipów z osobnymi ścieżkami albo [dźwięk];
    /// pusta lista = bez zmian
    #[serde(default)]
    pub glosnosci: Vec<f64>,
    /// Wycinek obrazu w pikselach klipu (None = cały obraz)
    #[serde(default)]
    pub kadr: Option<Kadr>,
    /// "mp4" (domyślnie), "gif" albo "webp"
    #[serde(default)]
    pub format: String,
    /// Wersja tylko do przeciągnięcia (np. na Discorda): trafia do `klipy::folder_przeciagania`,
    /// a nie do folderu klipów, i nigdy nie zastępuje oryginału
    #[serde(default)]
    pub do_przeciagniecia: bool,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Kadr {
    pub x: u32,
    pub y: u32,
    pub szer: u32,
    pub wys: u32,
}

/// Wynik eksportu: ścieżka i rozmiar pliku (GIF/WebP nie trafiają do biblioteki, więc rozmiar
/// podajemy od razu).
#[derive(Serialize)]
pub struct Wynik {
    pub sciezka: PathBuf,
    pub rozmiar: u64,
}

/// GIF i WebP: najdłuższy fragment w sekundach (dłuższe ważą za dużo i Discord ich nie pokaże).
pub const ANIMACJA_MAX_S: f64 = 15.0;

/// Kolejne próby zmieszczenia animacji w 10 MB: (FPS, szerokość, kolory GIF / jakość WebP).
const STOPNIE_ANIMACJI: &[(u32, u32, u32)] = &[(15, 480, 256), (12, 420, 192), (10, 360, 128), (10, 320, 96), (8, 280, 64)];

impl Zlecenie {
    fn animacja(&self) -> bool {
        self.format == "gif" || self.format == "webp"
    }
}

/// Filtr kadrowania (wymiary parzyste — H.264 i yuv420p nie przyjmą nieparzystych), dopasowany
/// do obrazu o rozmiarze `w`×`h` (0 = nieznany, wtedy bez dopasowania).
fn filtr_kadru(k: &Kadr, w: u32, h: u32) -> String {
    let (mut x, mut y, mut szer, mut wys) = (k.x, k.y, k.szer.max(16), k.wys.max(16));
    if w > 0 && h > 0 {
        szer = szer.min(w);
        wys = wys.min(h);
        x = x.min(w - szer);
        y = y.min(h - wys);
    }
    format!("crop={}:{}:{}:{}", szer & !1, wys & !1, x & !1, y & !1)
}

/// Parametry kodowania wyliczone z docelowego rozmiaru.
struct Kompresja {
    wideo_bps: u64,
    audio_kbps: u32,
    /// Wysokość obrazu po zmniejszeniu (None = bez zmian)
    wysokosc: Option<u32>,
    /// Ograniczenie FPS przy bardzo niskim bitrate
    fps: Option<u32>,
}

fn kompresja(rozmiar_mb: u32, dlugosc: f64, zapas: f64) -> Result<Kompresja, String> {
    // Mały cel = mniej bitów na dźwięk; kontener MP4 zjada ~3%, stąd zapas
    let audio_kbps = if rozmiar_mb <= 20 { 96 } else { 128 };
    let wszystko = rozmiar_mb as f64 * 1e6 * 8.0 * zapas / dlugosc;
    let wideo = wszystko - audio_kbps as f64 * 1000.0;
    if wideo < 300_000.0 {
        return Err(format!("Klip jest za długi, żeby zmieścić go w {rozmiar_mb} MB — przytnij go najpierw"));
    }
    // Przy niskim bitrate lepiej wygląda mniejszy obraz niż rozmyty 1080p
    let wysokosc = match wideo {
        w if w < 1_200_000.0 => Some(480),
        w if w < 3_500_000.0 => Some(720),
        _ => None,
    };
    let fps = (wideo < 2_000_000.0).then_some(30);
    Ok(Kompresja { wideo_bps: wideo as u64, audio_kbps, wysokosc, fps })
}

/// Plik, do którego trwa eksport (bez rozszerzenia klipu, żeby nie pojawił się w bibliotece): nazwa.mp4.tmp, nazwa.gif.tmp.
fn plik_tymczasowy(wynik: &Path) -> PathBuf {
    let mut nazwa = wynik.as_os_str().to_owned();
    nazwa.push(".tmp");
    PathBuf::from(nazwa)
}

/// Wolna nazwa: klip_cut.mp4, klip_10MB.mp4, klip_cut_kadr.mp4, klip_cut.gif (z numerem, jeśli zajęta).
/// Plik do przeciągnięcia ląduje w `folder`, zwykły wynik obok źródła.
fn nazwa_wyniku(zrodlo: &Path, z: &Zlecenie, folder_przeciagania: &Path) -> PathBuf {
    let mut baza = zrodlo.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    if z.przyciete {
        baza.push_str("_cut");
    }
    if z.kadr.is_some() {
        baza.push_str("_kadr");
    }
    if let Some(mb) = z.rozmiar_mb {
        baza.push_str(&format!("_{mb}MB"));
    }
    if !z.przyciete && z.rozmiar_mb.is_none() && z.kadr.is_none() && !z.animacja() {
        baza.push_str("_audio");
    }
    let rozszerzenie = if z.animacja() { z.format.as_str() } else { "mp4" };
    let folder = if z.do_przeciagniecia { folder_przeciagania } else { zrodlo.parent().unwrap() };
    let mut plik = folder.join(format!("{baza}.{rozszerzenie}"));
    let mut i = 2;
    // Zajęta jest też nazwa, pod którą właśnie trwa inny eksport (jego plik tymczasowy)
    while plik.exists() || plik_tymczasowy(&plik).exists() {
        plik = folder.join(format!("{baza}_{i}.{rozszerzenie}"));
        i += 1;
    }
    plik
}

/// Uruchamia ffmpeg i raportuje postęp (0..1) na podstawie -progress.
fn ffmpeg_z_postepem(args: &[String], dlugosc: f64, postep: &dyn Fn(f64)) -> Result<(), String> {
    uruchom_z_postepem(crate::ffmpeg::komenda(), args, dlugosc, &|p| {
        postep(p);
        true
    })
}

/// Jak wyżej, ale dla podanego polecenia (np. z niższym priorytetem), a `postep` może przerwać pracę,
/// zwracając false — ffmpeg jest wtedy zamykany.
pub fn uruchom_z_postepem(mut cmd: std::process::Command, args: &[String], dlugosc: f64, postep: &dyn Fn(f64) -> bool) -> Result<(), String> {
    let mut proces = cmd
        .args(["-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1", "-y"])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Nie udało się uruchomić ffmpeg: {e}"))?;
    let mut bledy = proces.stderr.take().unwrap();
    // stderr czytamy w tle, żeby ffmpeg nie zablokował się na pełnym potoku
    let watek_bledow = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut bledy, &mut s);
        s
    });
    for linia in BufReader::new(proces.stdout.take().unwrap()).lines().map_while(Result::ok) {
        if let Some(us) = linia.strip_prefix("out_time_us=").and_then(|v| v.parse::<f64>().ok())
            && !postep((us / 1e6 / dlugosc).clamp(0.0, 1.0))
        {
            let _ = proces.kill();
            let _ = proces.wait();
            return Err("Przerwano".into());
        }
    }
    let status = proces.wait().map_err(|e| e.to_string())?;
    let bledy = watek_bledow.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg: {}", bledy.lines().last().unwrap_or("nieznany błąd")))
    }
}

/// Wykonuje zlecenie i zwraca ścieżkę wyniku. `postep` dostaje wartości 0..1.
pub fn eksportuj(zrodlo: &Path, z: &Zlecenie, nazwa_enkodera: &str, postep: &dyn Fn(f64)) -> Result<Wynik, String> {
    let sciezka = eksportuj_plik(zrodlo, z, nazwa_enkodera, postep)?;
    let rozmiar = std::fs::metadata(&sciezka).map(|m| m.len()).unwrap_or(0);
    Ok(Wynik { sciezka, rozmiar })
}

fn eksportuj_plik(zrodlo: &Path, z: &Zlecenie, nazwa_enkodera: &str, postep: &dyn Fn(f64)) -> Result<PathBuf, String> {
    // koniec = 0 oznacza „do końca klipu”; zakres przycinamy do prawdziwej długości pliku
    let info = crate::klipy::info(zrodlo)?;
    let pelna = info.dlugosc;
    let audio = |kopiuj: bool, tylko_pierwsza: bool, kbps: u32| argumenty_audio(z, info.sciezki_audio, kopiuj, tylko_pierwsza, kbps);
    let start = z.start.clamp(0.0, pelna);
    let koniec = if z.koniec <= 0.0 { pelna } else { z.koniec.min(pelna) };
    let dlugosc = koniec - start;
    // NaN (zepsuta długość pliku) też odpada
    if dlugosc.is_nan() || dlugosc < 0.5 {
        return Err("Zaznaczony fragment jest za krótki (minimum 0,5 s)".into());
    }
    if !z.przyciete && z.rozmiar_mb.is_none() && z.glosnosci.is_empty() && z.kadr.is_none() && !z.animacja() {
        return Err("Nic do zrobienia — przytnij klip, wybierz rozmiar albo zmień głośność".into());
    }
    if !matches!(z.format.as_str(), "" | "mp4" | "gif" | "webp") {
        return Err(format!("Nieznany format „{}”", z.format));
    }
    if z.animacja() && dlugosc > ANIMACJA_MAX_S + 0.05 {
        return Err(format!("{} może mieć najwyżej {} s — przytnij klip", z.format.to_uppercase(), ANIMACJA_MAX_S as u32));
    }
    if z.format == "webp" && !crate::ffmpeg::ma_enkoder("libwebp_anim") {
        return Err("Ten ffmpeg nie umie zapisać WebP — wybierz GIF".into());
    }
    let kadr = z.kadr.map(|k| filtr_kadru(&k, info.szerokosc, info.wysokosc));
    // Do końca eksportu porządki i usuwanie nie ruszą źródła
    let _uzywany = crate::klipy::uzywaj(zrodlo);
    let przeciaganie = crate::klipy::folder_przeciagania();
    if z.do_przeciagniecia {
        std::fs::create_dir_all(&przeciaganie).map_err(|e| format!("Nie mogę utworzyć folderu na plik do przeciągnięcia: {e}"))?;
    }
    let wynik = nazwa_wyniku(zrodlo, z, &przeciaganie);
    // Wynik nie będzie większy niż źródło (×2 na faststart) — sprawdzamy, zanim zaczniemy kodować
    if let Some(folder) = wynik.parent() {
        let zrodlowy = std::fs::metadata(zrodlo).map(|m| m.len()).unwrap_or(0);
        crate::zapis::sprawdz_miejsce(folder, zrodlowy * 2 + 32 * 1024 * 1024)?;
    }
    // Najpierw do pliku tymczasowego, żeby przerwany eksport nie zostawił połowy klipu
    let tymczasowy = plik_tymczasowy(&wynik);
    let wyjscie = |args: &mut Vec<String>| {
        args.extend(["-movflags".into(), "+faststart".into(), "-f".into(), "mp4".into()]);
        args.push(tymczasowy.to_string_lossy().into_owned());
    };
    let zakres = |args: &mut Vec<String>| {
        // -ss przed -i: szybkie przewinięcie; przy kodowaniu i tak dokładne co do klatki
        args.extend(["-ss".into(), format!("{start:.3}"), "-t".into(), format!("{dlugosc:.3}"), "-i".into()]);
        args.push(zrodlo.to_string_lossy().into_owned());
        args.extend(["-map".into(), "0:v:0".into()]);
    };

    let rezultat = (|| {
        if z.animacja() {
            return animacja(zrodlo, z, start, dlugosc, kadr.as_deref(), &tymczasowy, postep);
        }
        match z.rozmiar_mb {
            // Kadrowanie zawsze wymaga kodowania obrazu
            None if !z.dokladnie && kadr.is_none() => {
                // Szybkie cięcie bez kodowania: zaczyna się od najbliższej wcześniejszej klatki kluczowej
                let mut a = Vec::new();
                zakres(&mut a);
                a.extend(["-c:v".into(), "copy".into(), "-avoid_negative_ts".into(), "make_zero".into()]);
                a.extend(audio(true, false, 160));
                wyjscie(&mut a);
                ffmpeg_z_postepem(&a, dlugosc, postep)
            }
            None => {
                // Dokładne cięcie: kodowanie w wysokiej jakości (rozmiar zbliżony do oryginału)
                let bps = (std::fs::metadata(zrodlo).map(|m| m.len()).unwrap_or(0) as f64 * 8.0
                    / pelna.max(1.0)) as u64;
                let mut a = Vec::new();
                zakres(&mut a);
                if let Some(k) = &kadr {
                    a.extend(["-vf".into(), k.clone()]);
                }
                a.extend(crate::enkodery::argumenty_eksportu(nazwa_enkodera, bps.clamp(4_000_000, 60_000_000), false));
                a.extend(audio(false, false, 160));
                wyjscie(&mut a);
                ffmpeg_z_postepem(&a, dlugosc, postep)
            }
            Some(mb) => {
                // Kompresja do rozmiaru; jeśli wyjdzie za duży, ponawiamy z mniejszym bitrate
                let limit = mb as u64 * 1_000_000;
                let mut zapas = 0.94;
                for proba in 0..3 {
                    let k = kompresja(mb, dlugosc, zapas)?;
                    let mut filtry = Vec::new();
                    if let Some(kf) = &kadr {
                        filtry.push(kf.clone());
                    }
                    if let Some(h) = k.wysokosc {
                        filtry.push(format!("scale=-2:'min({h},ih)'"));
                    }
                    if let Some(f) = k.fps {
                        filtry.push(format!("fps='min({f},source_fps)'"));
                    }
                    let mut a = Vec::new();
                    zakres(&mut a);
                    if !filtry.is_empty() {
                        a.extend(["-vf".into(), filtry.join(",")]);
                    }
                    a.extend(crate::enkodery::argumenty_eksportu(nazwa_enkodera, k.wideo_bps, true));
                    // Do małego pliku idzie tylko jedna ścieżka (miks)
                    a.extend(audio(false, true, k.audio_kbps));
                    wyjscie(&mut a);
                    // Postęp kolejnych prób zaczyna się od nowa — tak uczciwiej niż udawać
                    ffmpeg_z_postepem(&a, dlugosc, postep)?;
                    let rozmiar = std::fs::metadata(&tymczasowy).map(|m| m.len()).unwrap_or(u64::MAX);
                    if rozmiar <= limit {
                        return Ok(());
                    }
                    crate::dziennik::info(&format!("Kompresja: próba {} dała {:.1} MB > {mb} MB, ponawiam", proba + 1, rozmiar as f64 / 1e6));
                    zapas *= 0.92 * limit as f64 / rozmiar as f64;
                }
                Err(format!("Nie udało się zmieścić klipu w {mb} MB — przytnij go"))
            }
        }
    })();
    if let Err(e) = rezultat {
        let _ = std::fs::remove_file(&tymczasowy);
        return Err(e);
    }

    if z.zastap && !z.animacja() && !z.do_przeciagniecia {
        // Najpierw odsuwamy oryginał pod inną nazwę, potem nowa wersja dostaje jego nazwę, a dopiero
        // na końcu oryginał idzie do Kosza — przy każdym błędzie po drodze nic nie ginie
        let odsuniety = zrodlo.with_extension("mp4.oryginal");
        if let Err(e) = std::fs::rename(zrodlo, &odsuniety) {
            let _ = std::fs::remove_file(&tymczasowy);
            return Err(format!("Nie udało się zastąpić oryginału (plik jest otwarty w innym programie?): {e}"));
        }
        if let Err(e) = std::fs::rename(&tymczasowy, zrodlo) {
            let _ = std::fs::rename(&odsuniety, zrodlo);
            let _ = std::fs::remove_file(&tymczasowy);
            return Err(format!("Nie udało się zastąpić oryginału: {e}"));
        }
        if let Err(e) = trash::delete(&odsuniety) {
            crate::dziennik::blad(&format!("Oryginał zostaje jako {}: {e}", odsuniety.display()));
        }
        return Ok(zrodlo.to_path_buf());
    }
    std::fs::rename(&tymczasowy, &wynik).map_err(|e| format!("Nie udało się zapisać wyniku: {e}"))?;
    Ok(wynik)
}

/// GIF albo WebP: zapętlony (Discord odtwarza go w kółko), bez dźwięku. Przy `rozmiar_mb` kolejne
/// próby zmniejszają FPS, szerokość i liczbę kolorów, aż plik się zmieści.
fn animacja(zrodlo: &Path, z: &Zlecenie, start: f64, dlugosc: f64, kadr: Option<&str>, wyjscie: &Path, postep: &dyn Fn(f64)) -> Result<(), String> {
    let limit = z.rozmiar_mb.map(|mb| mb as u64 * 1_000_000);
    let stopnie = if limit.is_some() { STOPNIE_ANIMACJI } else { &STOPNIE_ANIMACJI[..1] };
    let mut ostatni = 0u64;
    for &(fps, szer, kolory) in stopnie {
        let a = argumenty_animacji(zrodlo, &z.format, start, dlugosc, kadr, fps, szer, kolory, wyjscie);
        ffmpeg_z_postepem(&a, dlugosc, postep)?;
        ostatni = std::fs::metadata(wyjscie).map(|m| m.len()).unwrap_or(u64::MAX);
        match limit {
            Some(l) if ostatni > l => {
                crate::dziennik::info(&format!("{}: {fps} FPS, {szer} px dało {:.1} MB — zmniejszam", z.format, ostatni as f64 / 1e6));
            }
            _ => return Ok(()),
        }
    }
    Err(format!(
        "{} wychodzi {:.1} MB nawet w najmniejszej wersji — przytnij fragment albo wykadruj mniejszy obraz",
        z.format.to_uppercase(),
        ostatni as f64 / 1e6
    ))
}

#[allow(clippy::too_many_arguments)]
fn argumenty_animacji(
    zrodlo: &Path,
    format: &str,
    start: f64,
    dlugosc: f64,
    kadr: Option<&str>,
    fps: u32,
    szer: u32,
    kolory: u32,
    wyjscie: &Path,
) -> Vec<String> {
    let mut filtry: Vec<String> = kadr.map(|k| vec![k.to_string()]).unwrap_or_default();
    filtry.push(format!("fps={fps}"));
    // Nie powiększamy: mały wycinek zostaje w swoim rozmiarze
    filtry.push(format!("scale='min({szer},iw)':-2:flags=lanczos"));
    let mut a: Vec<String> = vec!["-ss".into(), format!("{start:.3}"), "-t".into(), format!("{dlugosc:.3}"), "-i".into()];
    a.push(zrodlo.to_string_lossy().into_owned());
    if format == "gif" {
        // Własna paleta kolorów dla tego fragmentu = dużo ładniejszy GIF niż paleta domyślna
        let graf = format!(
            "[0:v:0]{},split[a][b];[a]palettegen=max_colors={kolory}:stats_mode=diff[p];[b][p]paletteuse=dither=sierra2_4a:diff_mode=rectangle",
            filtry.join(",")
        );
        a.extend(["-filter_complex".into(), graf, "-an".into(), "-loop".into(), "0".into(), "-f".into(), "gif".into()]);
    } else {
        // WebP: `kolory` z tej samej drabinki służy jako jakość (256 → 75, 64 → 45)
        let jakosc = (45 + (kolory.saturating_sub(64)) * 30 / 192).min(75);
        a.extend(["-map".into(), "0:v:0".into(), "-vf".into(), filtry.join(","), "-an".into()]);
        a.extend(["-c:v".into(), "libwebp_anim".into(), "-quality".into(), jakosc.to_string(), "-compression_level".into(), "4".into()]);
        a.extend(["-loop".into(), "0".into(), "-f".into(), "webp".into()]);
    }
    a.push(wyjscie.to_string_lossy().into_owned());
    a
}

/// Argumenty dźwięku. Klip z osobnymi ścieżkami ma: 0 = miks, 1 = gra, 2 = mikrofon.
/// Zmiana głośności składa nowy miks z gry i mikrofonu (jedna ścieżka w wyniku).
fn argumenty_audio(z: &Zlecenie, sciezki: u32, kopiuj: bool, tylko_pierwsza: bool, kbps: u32) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    if sciezki == 0 {
        return Vec::new();
    }
    let kodowanie = s(&["-c:a", "aac", "-b:a", &format!("{kbps}k")]);
    if !z.glosnosci.is_empty() {
        let g = |i: usize| z.glosnosci.get(i).copied().unwrap_or(1.0).clamp(0.0, 4.0);
        let filtr = if sciezki >= 3 && z.glosnosci.len() >= 2 {
            format!(
                "[0:a:1]volume={:.2}[g];[0:a:2]volume={:.2}[m];[g][m]amix=inputs=2:duration=longest:normalize=0[a]",
                g(0),
                g(1)
            )
        } else {
            format!("[0:a:0]volume={:.2}[a]", g(0))
        };
        let mut a = s(&["-filter_complex", &filtr, "-map", "[a]"]);
        a.extend(kodowanie);
        return a;
    }
    let mut a = if tylko_pierwsza { s(&["-map", "0:a:0"]) } else { s(&["-map", "0:a"]) };
    if kopiuj {
        a.extend(s(&["-c:a", "copy"]));
    } else {
        a.extend(kodowanie);
    }
    a
}
