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
    /// Klip nagrany w trybie pulpitu (nazwa zaczyna się od „Clipeon_Pulpit_”)
    pub pulpit: bool,
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
            let nazwa = w.file_name().to_string_lossy().into_owned();
            let pulpit = nazwa.starts_with("Clipeon_Pulpit_");
            Some(Klip { sciezka: w.path(), nazwa, rozmiar: m.len(), czas_ms, pulpit })
        })
        .collect();
    klipy.sort_by(|a, b| b.czas_ms.cmp(&a.czas_ms));
    klipy
}

/// Folder pamięci podręcznej miniatur: %LOCALAPPDATA%\pl.clipper.app\miniatury (folder danych
/// aplikacji — instalator może go wyczyścić przy odinstalowaniu; %LOCALAPPDATA%\Clipeon to folder programu).
pub fn folder_miniatur() -> PathBuf {
    let baza = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    baza.join("pl.clipper.app").join("miniatury")
}

/// Usuwa starą pamięć podręczną miniatur z wersji sprzed 1.0 (same miniatury, odtworzą się same).
pub fn usun_stara_pamiec_podreczna() {
    if let Some(baza) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        let stara = baza.join("Clipper").join("miniatury");
        if stara.is_dir() {
            // Plik po pliku: remove_dir_all bywa odrzucany przez system (os error 4395) w AppData
            if let Ok(wpisy) = std::fs::read_dir(&stara) {
                for w in wpisy.flatten() {
                    let _ = std::fs::remove_file(w.path());
                }
            }
            match std::fs::remove_dir(&stara) {
                Ok(()) => crate::dziennik::info("Usunięto starą pamięć podręczną miniatur"),
                Err(e) => crate::dziennik::blad(&format!("Nie udało się usunąć starej pamięci miniatur: {e}")),
            }
        }
    }
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

    // Pusty plik (np. po przerwanym generowaniu) traktujemy jak brak miniatury
    if std::fs::metadata(&obraz).is_ok_and(|m| m.len() > 0) {
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
    if !std::fs::metadata(&obraz).is_ok_and(|m| m.len() > 0) {
        let _ = std::fs::remove_file(&obraz);
        return Err("Nie udało się wygenerować miniatury".into());
    }
    let dlugosc = dlugosc_z_opisu(&String::from_utf8_lossy(&wynik.stderr)).unwrap_or(0.0);
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

/// Pliki, na których trwa właśnie eksport (źródło); znormalizowane do małych liter.
static W_UZYCIU: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn klucz_uzycia(p: &Path) -> String {
    p.to_string_lossy().to_lowercase()
}

/// Oznacza plik jako używany do końca życia zwróconej blokady.
pub struct Uzywany(String);

impl Drop for Uzywany {
    fn drop(&mut self) {
        let mut l = W_UZYCIU.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = l.iter().position(|x| *x == self.0) {
            l.remove(i);
        }
    }
}

pub fn uzywaj(p: &Path) -> Uzywany {
    let k = klucz_uzycia(p);
    W_UZYCIU.lock().unwrap_or_else(|e| e.into_inner()).push(k.clone());
    Uzywany(k)
}

pub fn w_uzyciu(p: &Path) -> bool {
    W_UZYCIU.lock().unwrap_or_else(|e| e.into_inner()).contains(&klucz_uzycia(p))
}

/// Przenosi klip do Kosza (da się go przywrócić) i usuwa jego miniaturę.
pub fn usun(klip: &Path) -> Result<(), String> {
    if w_uzyciu(klip) {
        return Err("Ten klip jest właśnie zapisywany w nowej wersji — spróbuj za chwilę".into());
    }
    if let Ok(k) = klucz(klip) {
        let folder = folder_miniatur();
        let _ = std::fs::remove_file(folder.join(format!("{k}.jpg")));
        let _ = std::fs::remove_file(folder.join(format!("{k}.txt")));
        let _ = std::fs::remove_file(folder.join(format!("{k}.maly.png")));
        let _ = std::fs::remove_file(folder.join(format!("{k}.duch.png")));
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

/// Mały, półprzezroczysty obrazek (180 px) pokazywany pod kursorem przy przeciąganiu klipu.
pub fn obraz_przeciagania(klip: &Path) -> Option<PathBuf> {
    let m = miniatura(klip).ok()?;
    let maly = m.obraz.with_extension("duch.png");
    if !maly.is_file() {
        let _ = crate::ffmpeg::komenda()
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&m.obraz)
            .args(["-vf", "scale=180:-2,format=rgba,colorchannelmixer=aa=0.7"])
            .arg(&maly)
            .status();
    }
    maly.is_file().then_some(maly)
}

#[derive(Serialize)]
pub struct Info {
    pub dlugosc: f64,
    /// Liczba ścieżek dźwięku (3 = miks, gra, mikrofon)
    pub sciezki_audio: u32,
}

/// Długość i liczba ścieżek dźwięku z opisu wejścia ffmpeg.
pub fn info(klip: &Path) -> Result<Info, String> {
    let wynik = crate::ffmpeg::komenda()
        .args(["-hide_banner", "-i"])
        .arg(klip)
        .output()
        .map_err(|e| format!("Nie udało się uruchomić ffmpeg: {e}"))?;
    let opis = String::from_utf8_lossy(&wynik.stderr);
    let dlugosc = dlugosc_z_opisu(&opis).ok_or("Nie udało się odczytać długości klipu")?;
    let sciezki_audio = opis.lines().filter(|l| l.contains("Stream #") && l.contains("Audio:")).count() as u32;
    Ok(Info { dlugosc, sciezki_audio })
}

/// Długość z linii "Duration: 00:00:30.18, ..." w opisie wejścia.
fn dlugosc_z_opisu(opis: &str) -> Option<f64> {
    opis.split("Duration: ").nth(1).and_then(|r| r.split(',').next()).and_then(|t| {
        let c: Vec<f64> = t.trim().split(':').filter_map(|x| x.parse().ok()).collect();
        (c.len() == 3).then(|| c[0] * 3600.0 + c[1] * 60.0 + c[2])
    })
}
