// Biblioteka klipów: lista plików MP4 z folderu klipów, miniatury (z pamięcią podręczną), usuwanie.
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Serialize)]
pub struct Klip {
    pub sciezka: PathBuf,
    pub nazwa: String,
    pub rozmiar: u64,
    /// Czas modyfikacji w ms od 1970 (do sortowania i wyświetlania daty)
    pub czas_ms: u64,
}

#[derive(Serialize)]
pub struct Miniatura {
    pub obraz: PathBuf,
    /// Długość klipu w sekundach (0, jeśli nie udało się odczytać)
    pub dlugosc: f64,
}

/// Klipy z folderu, od najnowszego.
pub fn lista(folder: &Path) -> Vec<Klip> {
    let Ok(wpisy) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut klipy: Vec<Klip> = wpisy
        .flatten()
        .filter(|w| w.path().extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4")))
        .filter_map(|w| {
            let m = w.metadata().ok()?;
            let czas_ms = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
            Some(Klip { sciezka: w.path(), nazwa: w.file_name().to_string_lossy().into_owned(), rozmiar: m.len(), czas_ms })
        })
        .collect();
    klipy.sort_by(|a, b| b.czas_ms.cmp(&a.czas_ms));
    klipy
}

/// Folder pamięci podręcznej miniatur: %LOCALAPPDATA%\Clipper\miniatury
pub fn folder_miniatur() -> PathBuf {
    let baza = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    baza.join("Clipper").join("miniatury")
}

/// Klucz pamięci podręcznej: nazwa pliku + rozmiar (nowy plik o tej samej nazwie dostaje nową miniaturę).
fn klucz(klip: &Path) -> Result<String, String> {
    let rozmiar = std::fs::metadata(klip).map_err(|e| e.to_string())?.len();
    Ok(format!("{}-{rozmiar}", klip.file_stem().unwrap_or_default().to_string_lossy()))
}

/// Miniatura JPG i długość klipu; generowane raz przez ffmpeg, potem brane z dysku.
pub fn miniatura(klip: &Path) -> Result<Miniatura, String> {
    let folder = folder_miniatur();
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let k = klucz(klip)?;
    let (obraz, plik_dlugosci) = (folder.join(format!("{k}.jpg")), folder.join(format!("{k}.txt")));

    if obraz.is_file() {
        let dlugosc = std::fs::read_to_string(&plik_dlugosci).ok().and_then(|t| t.trim().parse().ok()).unwrap_or(0.0);
        return Ok(Miniatura { obraz, dlugosc });
    }

    let wynik = crate::ffmpeg::komenda()
        .args(["-hide_banner", "-y", "-ss", "0.5", "-i"])
        .arg(klip)
        .args(["-frames:v", "1", "-vf", "scale=480:-2", "-q:v", "4"])
        .arg(&obraz)
        .output()
        .map_err(|e| format!("Nie udało się uruchomić ffmpeg: {e}"))?;
    if !obraz.is_file() {
        return Err("Nie udało się wygenerować miniatury".into());
    }
    // Długość z linii "Duration: 00:00:30.18, ..." w opisie wejścia
    let opis = String::from_utf8_lossy(&wynik.stderr);
    let dlugosc = opis
        .split("Duration: ")
        .nth(1)
        .and_then(|r| r.split(',').next())
        .and_then(|t| {
            let c: Vec<f64> = t.trim().split(':').filter_map(|x| x.parse().ok()).collect();
            (c.len() == 3).then(|| c[0] * 3600.0 + c[1] * 60.0 + c[2])
        })
        .unwrap_or(0.0);
    let _ = std::fs::write(&plik_dlugosci, dlugosc.to_string());
    Ok(Miniatura { obraz, dlugosc })
}

/// Sprawdza, że ścieżka z okna to plik MP4 w folderze klipów (okno nie może wskazać dowolnego pliku).
pub fn sprawdz_sciezke(folder: &Path, klip: &Path) -> Result<PathBuf, String> {
    let klip = klip.canonicalize().map_err(|_| "Klip nie istnieje".to_string())?;
    let folder = folder.canonicalize().map_err(|_| "Folder klipów nie istnieje".to_string())?;
    let mp4 = klip.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    if !mp4 || klip.parent() != Some(folder.as_path()) {
        return Err("Ten plik nie jest klipem z folderu klipów".into());
    }
    // canonicalize daje ścieżkę z przedrostkiem \\?\, którego Eksplorator nie rozumie
    let tekst = klip.to_string_lossy();
    Ok(PathBuf::from(tekst.strip_prefix(r"\\?\").unwrap_or(&tekst)))
}

/// Przenosi klip do Kosza (da się go przywrócić) i usuwa jego miniaturę.
pub fn usun(klip: &Path) -> Result<(), String> {
    if let Ok(k) = klucz(klip) {
        let folder = folder_miniatur();
        let _ = std::fs::remove_file(folder.join(format!("{k}.jpg")));
        let _ = std::fs::remove_file(folder.join(format!("{k}.txt")));
    }
    trash::delete(klip).map_err(|e| format!("Nie udało się przenieść klipu do Kosza: {e}"))
}

/// Otwiera Eksplorator z zaznaczonym plikiem albo sam folder.
pub fn pokaz_w_eksploratorze(sciezka: &Path, zaznacz: bool) {
    let mut cmd = std::process::Command::new("explorer");
    if zaznacz {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(sciezka);
        cmd.arg(arg);
    } else {
        cmd.arg(sciezka);
    }
    let _ = cmd.spawn();
}
