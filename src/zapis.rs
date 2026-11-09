// Zapis klipu: pakiety z bufora -> MP4 (faststart) bez ponownego kodowania obrazu.
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
    fps: u32,
    format_wideo: &str,
    wideo: Vec<Pakiet>,
    audio: Vec<(&'static str, Vec<Pakiet>)>,
) -> Result<PathBuf, String> {
    static ZAPIS: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _kolejka = ZAPIS.lock().unwrap_or_else(|e| e.into_inner());
    let start = Instant::now();

    // Klip zaczyna się od klatki kluczowej
    let pierwsza = wideo.iter().position(|p| p.klucz).ok_or("Nie ma jeszcze czego zapisać — poczekaj kilka sekund")?;
    let wideo = &wideo[pierwsza..];
    let (poczatek, koniec) = (wideo[0].czas, wideo[wideo.len() - 1].czas);

    // Dźwięk z tego samego przedziału czasu (pierwsza ramka nie wcześniej niż obraz)
    let sciezki: Vec<(&str, Vec<&Pakiet>)> = audio
        .iter()
        .map(|(n, a)| (*n, a.iter().filter(|p| p.czas >= poczatek && p.czas <= koniec).collect::<Vec<_>>()))
        .filter(|(_, a)| !a.is_empty())
        .collect();

    std::fs::create_dir_all(folder).map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder.display()))?;
    let plik = nowa_nazwa(folder);

    // Dźwięk idzie przez małe pliki tymczasowe (ADTS), obraz przez stdin ffmpeg
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-n"])
        .args(["-fflags", "+genpts", "-f", format_wideo, "-framerate", &fps.to_string(), "-i", "-"]);
    let mut tymczasowe = Vec::new();
    let wynik = (|| {
        for (i, (_, sciezka)) in sciezki.iter().enumerate() {
            let tmp = std::env::temp_dir().join(format!("clipper-{}-{poczatek}-{i}.aac", std::process::id()));
            let dane: Vec<u8> = sciezka.iter().flat_map(|p| p.dane.iter().copied()).collect();
            std::fs::write(&tmp, dane).map_err(|e| format!("Nie mogę zapisać pliku tymczasowego: {e}"))?;
            // Przesunięcie dźwięku względem pierwszej klatki obrazu (synchronizacja po QPC)
            let przesuniecie = (sciezka[0].czas - poczatek) as f64 / 1e7;
            cmd.args(["-itsoffset", &format!("{przesuniecie:.6}"), "-f", "aac", "-i"]).arg(&tmp);
            tymczasowe.push(tmp);
        }
        cmd.args(["-map", "0:v", "-c:v", "copy"]);
        match sciezki.len() {
            0 => {}
            1 => {
                cmd.args(["-map", "1:a", "-c:a", "copy"]);
            }
            n => {
                // Ścieżka 0: miks (kodowany), dalej oryginalne ścieżki bez ponownego kodowania
                let wejscia: String = (1..=n).map(|i| format!("[{i}:a]")).collect();
                cmd.args(["-filter_complex", &format!("{wejscia}amix=inputs={n}:duration=longest:normalize=0[a]")])
                    .args(["-map", "[a]", "-c:a:0", "aac", "-b:a:0", "160k"])
                    .args(["-metadata:s:a:0", "title=Gra + mikrofon", "-metadata:s:a:0", "handler_name=Gra + mikrofon"])
                    .args(["-disposition:a:0", "default"]);
                for (i, (nazwa, _)) in sciezki.iter().enumerate() {
                    let wyj = i + 1;
                    cmd.args(["-map", &format!("{}:a", i + 1), &format!("-c:a:{wyj}"), "copy"])
                        .args([format!("-metadata:s:a:{wyj}"), format!("title={nazwa}")])
                        // MP4 trzyma nazwę ścieżki w handler_name
                        .args([format!("-metadata:s:a:{wyj}"), format!("handler_name={nazwa}")])
                        .args([format!("-disposition:a:{wyj}"), "0".to_string()]);
                }
            }
        }
        cmd.args(["-movflags", "+faststart"]).arg(&plik).stdin(Stdio::piped());

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
    wynik?;

    let rozmiar = std::fs::metadata(&plik).map(|m| m.len()).unwrap_or(0);
    println!(
        "Zapisano klip: {} ({:.1} s, {:.1} MB, ścieżek dźwięku: {}) w {:.2} s",
        plik.display(),
        (koniec - poczatek) as f64 / 1e7 + 1.0 / fps as f64,
        rozmiar as f64 / 1e6,
        sciezki.len(),
        start.elapsed().as_secs_f64()
    );
    Ok(plik)
}

/// Lokalna data i godzina w formacie 2026-11-08_17-45-03.
fn znacznik_czasu() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}_{:02}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Wolna nazwa pliku z datą; drugi zapis w tej samej sekundzie dostaje sufiks, żeby nie nadpisać klipu.
fn nowa_nazwa(folder: &Path) -> PathBuf {
    let baza = format!("Clipeon_{}", znacznik_czasu());
    let mut plik = folder.join(format!("{baza}.mp4"));
    let mut i = 2;
    while plik.exists() {
        plik = folder.join(format!("{baza}_{i}.mp4"));
        i += 1;
    }
    plik
}
