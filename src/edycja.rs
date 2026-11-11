// Edycja klipów: przycinanie (szybkie bez kodowania albo dokładne z kodowaniem) i kompresja
// do docelowego rozmiaru pliku (np. 10 MB na Discorda). Wynik zawsze trafia do nowego pliku,
// chyba że użytkownik wybierze zastąpienie oryginału.
use serde::Deserialize;
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

/// Wolna nazwa: klip_cut.mp4, klip_10MB.mp4, klip_cut_10MB.mp4 (z numerem, jeśli zajęta).
fn nazwa_wyniku(zrodlo: &Path, z: &Zlecenie) -> PathBuf {
    let mut baza = zrodlo.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    if z.przyciete {
        baza.push_str("_cut");
    }
    if let Some(mb) = z.rozmiar_mb {
        baza.push_str(&format!("_{mb}MB"));
    }
    if !z.przyciete && z.rozmiar_mb.is_none() {
        baza.push_str("_audio");
    }
    let folder = zrodlo.parent().unwrap();
    let mut plik = folder.join(format!("{baza}.mp4"));
    let mut i = 2;
    while plik.exists() {
        plik = folder.join(format!("{baza}_{i}.mp4"));
        i += 1;
    }
    plik
}

/// Uruchamia ffmpeg i raportuje postęp (0..1) na podstawie -progress.
fn ffmpeg_z_postepem(args: &[String], dlugosc: f64, postep: &dyn Fn(f64)) -> Result<(), String> {
    let mut proces = crate::ffmpeg::komenda()
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
        if let Some(us) = linia.strip_prefix("out_time_us=").and_then(|v| v.parse::<f64>().ok()) {
            postep((us / 1e6 / dlugosc).clamp(0.0, 1.0));
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
pub fn eksportuj(zrodlo: &Path, z: &Zlecenie, nazwa_enkodera: &str, postep: &dyn Fn(f64)) -> Result<PathBuf, String> {
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
    if !z.przyciete && z.rozmiar_mb.is_none() && z.glosnosci.is_empty() {
        return Err("Nic do zrobienia — przytnij klip, wybierz rozmiar albo zmień głośność".into());
    }
    // Do końca eksportu porządki i usuwanie nie ruszą źródła
    let _uzywany = crate::klipy::uzywaj(zrodlo);
    let wynik = nazwa_wyniku(zrodlo, z);
    // Wynik nie będzie większy niż źródło (×2 na faststart) — sprawdzamy, zanim zaczniemy kodować
    if let Some(folder) = wynik.parent() {
        let zrodlowy = std::fs::metadata(zrodlo).map(|m| m.len()).unwrap_or(0);
        crate::zapis::sprawdz_miejsce(folder, zrodlowy * 2 + 32 * 1024 * 1024)?;
    }
    // Najpierw do pliku tymczasowego (bez .mp4, żeby nie pojawił się w bibliotece),
    // żeby przerwany eksport nie zostawił połowy klipu
    let tymczasowy = wynik.with_extension("mp4.tmp");
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
        match z.rozmiar_mb {
            None if !z.dokladnie => {
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
                    eprintln!("Kompresja: próba {} dała {:.1} MB > {mb} MB, ponawiam", proba + 1, rozmiar as f64 / 1e6);
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

    if z.zastap {
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
            eprintln!("Oryginał zostaje jako {}: {e}", odsuniety.display());
        }
        return Ok(zrodlo.to_path_buf());
    }
    std::fs::rename(&tymczasowy, &wynik).map_err(|e| format!("Nie udało się zapisać wyniku: {e}"))?;
    Ok(wynik)
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
