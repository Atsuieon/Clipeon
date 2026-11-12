// Zapis klipu: pakiety z bufora -> MP4 (faststart) bez ponownego kodowania obrazu.
use crate::Zablokuj;
use crate::bufor::Pakiet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

/// Zapisuje klip i zwraca ścieżkę pliku. `audio` to nazwane ścieżki dźwięku (Gra, Mikrofon).
/// Przy dwóch klip dostaje trzy ścieżki: miks (pierwsza — odtwarzacze zwykle grają tylko ją),
/// samą grę i sam mikrofon (do późniejszej zmiany głośności w edytorze).
pub fn zapisz(
    folder: &Path,
    przedrostek: &str,
    fps: u32,
    format_wideo: &str,
    wideo: Vec<Pakiet>,
    audio: Vec<(&'static str, Vec<Pakiet>)>,
    gra: Option<String>,
) -> Result<PathBuf, String> {
    static ZAPIS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _kolejka = ZAPIS.zablokuj();
    let start = Instant::now();

    // Klip zaczyna się od klatki kluczowej
    let pierwsza = wideo.iter().position(|p| p.klucz).ok_or("Nie ma jeszcze czego zapisać — poczekaj kilka sekund")?;
    let wideo = &wideo[pierwsza..];
    let poczatek = wideo[0].czas;

    // Dźwięk z tego samego przedziału czasu (pierwsza ramka nie wcześniej niż obraz). Gdy w obrazie są
    // przerwy (nagrywanie wznawiało się po chwilowej utracie ekranu), obraz w pliku idzie klatka po klatce
    // bez nich — dźwięk z przerw wycinamy, żeby dalej zgadzał się z obrazem.
    let okres = 10_000_000 / fps.max(1) as i64;
    let czasy: Vec<i64> = wideo.iter().map(|p| p.czas).collect();
    let sciezki: Vec<(&str, i64, Vec<&Pakiet>)> = audio
        .iter()
        .filter_map(|(n, a)| {
            let ramka = dlugosc_ramki_aac(a.first()?);
            let (przesuniecie, ramki) = dzwiek_do_obrazu(&czasy, okres, a.iter().collect(), ramka);
            (!ramki.is_empty()).then_some((*n, przesuniecie, ramki))
        })
        .collect();
    let przerwy = przerwy_w_obrazie(&czasy, okres);
    if przerwy > 0 {
        crate::dziennik::info(&format!("Klip ma {przerwy} przerw(y) w obrazie (nagrywanie się wznawiało) — dźwięk dopasowany"));
    }

    // Klipy lądują w podfolderze miesiąca (2026-11), żeby folder nie puchł bez końca
    let znacznik = znacznik_czasu();
    let folder_miesiaca = folder.join(miesiac(&znacznik));
    std::fs::create_dir_all(&folder_miesiaca)
        .map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder_miesiaca.display()))?;
    // Miejsce na dysku: faststart przepisuje plik drugi raz, więc potrzeba ok. 2× rozmiaru klipu
    let rozmiar_danych: u64 = wideo.iter().map(|p| p.dane.len() as u64).sum::<u64>()
        + sciezki.iter().flat_map(|(_, _, a)| a.iter()).map(|p| p.dane.len() as u64).sum::<u64>();
    sprawdz_miejsce(folder, rozmiar_danych * 2 + 64 * 1024 * 1024)?;
    let plik = nowa_nazwa(&folder_miesiaca, przedrostek, &znacznik);
    // Najpierw do pliku tymczasowego (bez .mp4, więc nie pojawi się w bibliotece); nazwa docelowa
    // dopiero po udanym zapisie — przerwany zapis (brak miejsca, awaria) nie zostawi uszkodzonego klipu
    let tymczasowy = plik.with_extension("mp4.tmp");

    // Dźwięk idzie przez małe pliki tymczasowe (ADTS), obraz przez stdin ffmpeg
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-n"])
        .args(["-fflags", "+genpts", "-f", format_wideo, "-framerate", &fps.to_string(), "-i", "-"]);
    let mut tymczasowe = Vec::new();
    let wynik = (|| {
        for (i, (_, przesuniecie, sciezka)) in sciezki.iter().enumerate() {
            let tmp = std::env::temp_dir().join(format!("clipeon-{}-{poczatek}-{i}.aac", std::process::id()));
            let dane: Vec<u8> = sciezka.iter().flat_map(|p| p.dane.iter().copied()).collect();
            std::fs::write(&tmp, dane).map_err(|e| format!("Nie mogę zapisać pliku tymczasowego: {e}"))?;
            // Przesunięcie dźwięku względem pierwszej klatki obrazu (synchronizacja po QPC)
            let przesuniecie = *przesuniecie as f64 / 1e7;
            cmd.args(["-itsoffset", &format!("{przesuniecie:.6}"), "-f", "aac", "-i"]).arg(&tmp);
            tymczasowe.push(tmp);
        }
        cmd.args(["-map", "0:v", "-c:v", "copy"]);
        let nazwy: Vec<&str> = sciezki.iter().map(|(n, _, _)| *n).collect();
        mapuj_dzwiek(&mut cmd, &nazwy);
        // Nazwa gry zostaje w metadanych pliku (biblioteka pokazuje ją przy klipie)
        if let Some(g) = gra.as_deref().filter(|g| !g.is_empty()) {
            cmd.args(["-metadata", &format!("title={g}")]);
        }
        cmd.args(["-movflags", "+faststart", "-f", "mp4"]).arg(&tymczasowy).stdin(Stdio::piped());

        let mut ffmpeg = cmd.spawn().map_err(|e| format!("Nie udało się uruchomić ffmpeg: {e}"))?;
        let mut wejscie = ffmpeg.stdin.take().unwrap();
        for p in wideo {
            wejscie.write_all(&p.dane).map_err(|e| format!("Błąd przesyłania do ffmpeg: {e}"))?;
        }
        drop(wejscie);
        let status = ffmpeg.wait().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err(format!("ffmpeg zakończył się błędem: {status}")) }
    })();
    for t in &tymczasowe {
        let _ = std::fs::remove_file(t);
    }
    let wynik = wynik.and_then(|()| {
        std::fs::rename(&tymczasowy, &plik).map_err(|e| format!("Nie udało się nadać nazwy klipowi: {e}"))
    });
    if let Err(e) = wynik {
        let _ = std::fs::remove_file(&tymczasowy);
        // Brak miejsca w trakcie zapisu: czytelny komunikat zamiast błędu ffmpeg
        if wolne_miejsce(folder).is_some_and(|w| w < 64 * 1024 * 1024) {
            return Err(format!("Za mało miejsca na dysku z folderem {} — zwolnij miejsce", folder.display()));
        }
        return Err(e);
    }

    let rozmiar = std::fs::metadata(&plik).map(|m| m.len()).unwrap_or(0);
    println!(
        "Zapisano klip: {} ({:.1} s, {:.1} MB, ścieżek dźwięku: {}) w {:.2} s",
        plik.display(),
        wideo.len() as f64 / fps as f64,
        rozmiar as f64 / 1e6,
        sciezki.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(plik)
}

/// Odcinki obrazu bez przerw: (czas pierwszej klatki, czas tuż po ostatniej, numer pierwszej klatki w pliku).
/// Przerwa = odstęp między kolejnymi klatkami większy niż półtorej klatki.
fn odcinki(czasy: &[i64], okres: i64) -> Vec<(i64, i64, i64)> {
    let mut wynik: Vec<(i64, i64, i64)> = Vec::new();
    for (i, &t) in czasy.iter().enumerate() {
        match wynik.last_mut() {
            Some(o) if t - (o.1 - okres) <= okres * 3 / 2 => o.1 = t + okres,
            _ => wynik.push((t, t + okres, i as i64)),
        }
    }
    wynik
}

fn przerwy_w_obrazie(czasy: &[i64], okres: i64) -> usize {
    odcinki(czasy, okres).len().saturating_sub(1)
}

/// Długość ramki AAC (1024 próbki) w 100 ns, z częstotliwości zapisanej w nagłówku ADTS.
fn dlugosc_ramki_aac(p: &Pakiet) -> i64 {
    const CZESTOTLIWOSCI: [i64; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
    let indeks = p.dane.get(2).map(|b| ((b >> 2) & 0x0F) as usize).unwrap_or(3);
    1024 * 10_000_000 / CZESTOTLIWOSCI.get(indeks).copied().unwrap_or(48000)
}

/// Ramki dźwięku pasujące do obrazu zapisanego bez przerw i przesunięcie pierwszej z nich względem
/// pierwszej klatki (100 ns). Ramka z czasem t trafia w pliku na pozycję obrazu z tej samej chwili;
/// ramki z przerw w obrazie odpadają, a gdy dźwięk po przerwie wyprzedzałby obraz o ponad pół ramki,
/// pomijamy nadmiarowe ramki (dźwięk w pliku leci ramka po ramce, bez własnych znaczników czasu).
fn dzwiek_do_obrazu<'a>(czasy: &[i64], okres: i64, audio: Vec<&'a Pakiet>, ramka: i64) -> (i64, Vec<&'a Pakiet>) {
    let odc = odcinki(czasy, okres);
    let mut wynik = Vec::new();
    let (mut przesuniecie, mut pozycja) = (0, 0);
    let mut o = 0;
    for p in audio {
        while o < odc.len() && p.czas >= odc[o].1 {
            o += 1;
        }
        let Some(&(od, _, nr)) = odc.get(o) else { break };
        if p.czas < od {
            continue;
        }
        let cel = nr * okres + (p.czas - od);
        if wynik.is_empty() {
            przesuniecie = cel;
            pozycja = cel;
        } else if cel < pozycja - ramka / 2 {
            continue;
        }
        wynik.push(p);
        pozycja += ramka;
    }
    (przesuniecie, wynik)
}

/// Ścieżki dźwięku pliku wynikowego. Wejścia ffmpeg nr 1.. to kolejne ścieżki z `nazwy` (0 = obraz).
/// Jedna ścieżka jest kopiowana; przy dwóch plik dostaje trzy: miks (pierwsza — odtwarzacze zwykle
/// grają tylko ją) i obie oryginalne bez ponownego kodowania (do zmiany głośności w edytorze).
pub fn mapuj_dzwiek(cmd: &mut std::process::Command, nazwy: &[&str]) {
    match nazwy.len() {
        0 => {}
        1 => {
            cmd.args(["-map", "1:a", "-c:a", "copy"]);
        }
        n => {
            let wejscia: String = (1..=n).map(|i| format!("[{i}:a]")).collect();
            cmd.args(["-filter_complex", &format!("{wejscia}amix=inputs={n}:duration=longest:normalize=0[a]")])
                .args(["-map", "[a]", "-c:a:0", "aac", "-b:a:0", "160k"])
                .args(["-metadata:s:a:0", "title=Gra + mikrofon", "-metadata:s:a:0", "handler_name=Gra + mikrofon"])
                .args(["-disposition:a:0", "default"]);
            for (i, nazwa) in nazwy.iter().enumerate() {
                let wyj = i + 1;
                cmd.args(["-map", &format!("{}:a", i + 1), &format!("-c:a:{wyj}"), "copy"])
                    .args([format!("-metadata:s:a:{wyj}"), format!("title={nazwa}")])
                    // MP4 trzyma nazwę ścieżki w handler_name
                    .args([format!("-metadata:s:a:{wyj}"), format!("handler_name={nazwa}")])
                    .args([format!("-disposition:a:{wyj}"), "0".to_string()]);
            }
        }
    }
}

/// Wolne miejsce (bajty) na dysku z danym folderem, dostępne dla bieżącego użytkownika.
pub fn wolne_miejsce(folder: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    let szeroka: Vec<u16> = folder.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let mut wolne = 0u64;
    unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::PCWSTR(szeroka.as_ptr()),
            Some(&mut wolne),
            None,
            None,
        )
        .ok()?;
    }
    Some(wolne)
}

/// Błąd z jasnym opisem, gdy na dysku jest mniej niż `potrzeba` bajtów (nieznane = próbujemy zapisać).
pub fn sprawdz_miejsce(folder: &Path, potrzeba: u64) -> Result<(), String> {
    match wolne_miejsce(folder) {
        Some(w) if w < potrzeba => Err(format!(
            "Za mało miejsca na dysku: wolne {:.1} GB, potrzeba ok. {:.1} GB. Zwolnij miejsce albo wybierz inny folder klipów",
            w as f64 / 1e9,
            potrzeba as f64 / 1e9
        )),
        _ => Ok(()),
    }
}

/// Lokalna data i godzina w formacie 2026-11-08_17-45-03.
pub fn znacznik_czasu() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Nazwa podfolderu miesiąca („2026-11”) ze znacznika czasu.
pub fn miesiac(znacznik: &str) -> &str {
    &znacznik[..7.min(znacznik.len())]
}

/// Wolna nazwa pliku z datą (np. Clipeon_2026-11-10_18-30-00.mp4, z pulpitu Clipeon_Pulpit_…);
/// drugi zapis w tej samej sekundzie dostaje sufiks, żeby nie nadpisać klipu.
pub fn nowa_nazwa(folder: &Path, przedrostek: &str, znacznik: &str) -> PathBuf {
    let baza = format!("{przedrostek}_{znacznik}");
    let mut plik = folder.join(format!("{baza}.mp4"));
    let mut i = 2;
    while plik.exists() || plik.with_extension("mp4.tmp").exists() {
        plik = folder.join(format!("{baza}_{i}.mp4"));
        i += 1;
    }
    plik
}

#[cfg(test)]
mod testy {
    use super::{Pakiet, dlugosc_ramki_aac, dzwiek_do_obrazu, miesiac, przerwy_w_obrazie};

    fn pakiet(czas: i64) -> Pakiet {
        Pakiet { czas, klucz: true, dane: std::sync::Arc::from(vec![0xFFu8, 0xF1, 0x4C, 0x80]) }
    }

    #[test]
    fn ramka_aac_48_khz() {
        // Indeks częstotliwości 3 (48 kHz) w trzecim bajcie nagłówka ADTS
        assert_eq!(dlugosc_ramki_aac(&pakiet(0)), 1024 * 10_000_000 / 48000);
    }

    #[test]
    fn dzwiek_bez_przerw_w_obrazie_zostaje_caly() {
        let okres = 333_333;
        let czasy: Vec<i64> = (0..90).map(|i| 1_000_000 + i * okres).collect();
        let ramka = 213_333;
        let audio: Vec<Pakiet> = (0..140).map(|i| pakiet(1_050_000 + i * ramka)).collect();
        let (przes, ramki) = dzwiek_do_obrazu(&czasy, okres, audio.iter().collect(), ramka);
        assert_eq!(przes, 50_000);
        // Ramki od pierwszej klatki do końca ostatniej (90 klatek = 3 s ≈ 140 ramek)
        assert_eq!(ramki.len(), audio.iter().filter(|p| p.czas < 1_000_000 + 90 * okres).count());
        assert_eq!(przerwy_w_obrazie(&czasy, okres), 0);
    }

    #[test]
    fn dzwiek_z_przerwy_w_obrazie_odpada() {
        let okres = 333_333;
        // 30 klatek, 2 s przerwy (restart nagrywania), znów 30 klatek
        let mut czasy: Vec<i64> = (0..30).map(|i| i * okres).collect();
        let po = 30 * okres + 20_000_000;
        czasy.extend((0..30).map(|i| po + i * okres));
        assert_eq!(przerwy_w_obrazie(&czasy, okres), 1);
        let ramka = 213_333;
        let audio: Vec<Pakiet> = (0..((po + 30 * okres) / ramka)).map(|i| pakiet(i * ramka)).collect();
        let (przes, ramki) = dzwiek_do_obrazu(&czasy, okres, audio.iter().collect(), ramka);
        assert_eq!(przes, 0);
        // Żadna ramka z przerwy
        assert!(ramki.iter().all(|p| p.czas < 30 * okres || p.czas >= po));
        // Dźwięk trwa tyle, co obraz w pliku (60 klatek = 2 s), z dokładnością do ramki
        let dlugosc = ramki.len() as i64 * ramka;
        assert!((dlugosc - 60 * okres).abs() <= 2 * ramka, "{dlugosc}");
        // Pierwsza ramka po przerwie ląduje w pliku przy 30. klatce
        let k = ramki.iter().position(|p| p.czas >= po).unwrap() as i64;
        assert!((k * ramka - 30 * okres).abs() <= ramka);
    }

    #[test]
    fn podfolder_miesiaca_ze_znacznika() {
        assert_eq!(miesiac("2026-11-10_18-30-00"), "2026-11");
        assert_eq!(miesiac("2027-01-01_00-00-00"), "2027-01");
    }
}
