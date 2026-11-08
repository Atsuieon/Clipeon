// Zapis klipu: pakiety z bufora -> MP4 (faststart) bez ponownego kodowania obrazu.
use crate::bufor::Pakiet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Instant;

/// Zapisuje klip i zwraca ścieżkę pliku. `audio` to ścieżki dźwięku (system, mikrofon), które
/// przy więcej niż jednej są miksowane do jednej (tylko dźwięk jest wtedy kodowany ponownie).
pub fn zapisz(folder: &Path, fps: u32, wideo: Vec<Pakiet>, audio: Vec<Vec<Pakiet>>) -> Result<PathBuf, String> {
    let start = Instant::now();

    // Klip zaczyna się od klatki kluczowej
    let pierwsza = wideo.iter().position(|p| p.klucz).ok_or("Bufor jest jeszcze pusty")?;
    let wideo = &wideo[pierwsza..];
    let (poczatek, koniec) = (wideo[0].czas, wideo[wideo.len() - 1].czas);

    // Dźwięk z tego samego przedziału czasu (pierwsza ramka nie wcześniej niż obraz)
    let sciezki: Vec<Vec<&Pakiet>> = audio
        .iter()
        .map(|a| a.iter().filter(|p| p.czas >= poczatek && p.czas <= koniec).collect::<Vec<_>>())
        .filter(|a| !a.is_empty())
        .collect();

    std::fs::create_dir_all(folder).map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder.display()))?;
    let plik = nowa_nazwa(folder);

    // Dźwięk idzie przez małe pliki tymczasowe (ADTS), obraz przez stdin ffmpeg
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-n"])
        .args(["-fflags", "+genpts", "-f", "h264", "-framerate", &fps.to_string(), "-i", "-"]);
    let mut tymczasowe = Vec::new();
    let wynik = (|| {
        for (i, sciezka) in sciezki.iter().enumerate() {
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
                let wejscia: String = (1..=n).map(|i| format!("[{i}:a]")).collect();
                cmd.args(["-filter_complex", &format!("{wejscia}amix=inputs={n}:duration=longest:normalize=0[a]")])
                    .args(["-map", "[a]", "-c:a", "aac", "-b:a", "160k"]);
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
    let baza = format!("Clipper_{}", znacznik_czasu());
    let mut plik = folder.join(format!("{baza}.mp4"));
    let mut i = 2;
    while plik.exists() {
        plik = folder.join(format!("{baza}_{i}.mp4"));
        i += 1;
    }
    plik
}
