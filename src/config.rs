// Konfiguracja z %APPDATA%\Clipper\config.toml.
// Jeśli pliku nie ma, tworzymy go z wartościami domyślnymi.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Obsługiwane enkodery wideo: (nazwa ffmpeg, opis dla użytkownika)
pub const ENKODERY: &[(&str, &str)] = &[
    ("h264_amf", "H.264 — karta AMD (najlepsza zgodność)"),
    ("hevc_amf", "HEVC — karta AMD (mniejsze pliki)"),
    ("av1_amf", "AV1 — karta AMD (najmniejsze pliki)"),
    ("libx264", "H.264 — procesor (x264)"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ile ostatnich sekund trzymać w pamięci
    pub bufor_sekundy: u32,
    /// Stała liczba klatek na sekundę nagrania
    pub fps: u32,
    /// Bitrate wideo w Mb/s (VBR)
    pub bitrate_mbps: u32,
    /// Enkoder wideo (jeden z ENKODERY)
    pub enkoder: String,
    /// Nagrywanie dźwięku systemowego (to, co słychać w głośnikach)
    pub dzwiek_systemowy: bool,
    /// Nagrywanie mikrofonu (domieszanego do dźwięku systemowego przy zapisie)
    pub mikrofon: bool,
    /// Folder, do którego trafiają klipy
    pub folder_klipow: PathBuf,
    /// Skrót zapisu klipu, np. "Alt+F10", "Ctrl+Shift+S"
    pub skrot: String,
    /// Ikona w zasobniku zawsze widoczna przy zegarze (a nie w ukrytych ikonach pod ^)
    pub ikona_przy_zegarze: bool,
    /// Czy Clipper już raz ustawił widoczność ikony (później decyduje użytkownik w Windows)
    pub ikona_ustawiona: bool,
}

impl Default for Config {
    fn default() -> Self {
        let profil = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            bufor_sekundy: 30,
            fps: 60,
            bitrate_mbps: 20,
            enkoder: "h264_amf".into(),
            dzwiek_systemowy: true,
            mikrofon: false,
            folder_klipow: profil.join("Videos").join("Clipper"),
            skrot: "Alt+F10".into(),
            ikona_przy_zegarze: true,
            ikona_ustawiona: false,
        }
    }
}

impl Config {
    fn sciezka() -> Result<PathBuf, String> {
        let appdata = std::env::var_os("APPDATA").ok_or("Brak zmiennej APPDATA")?;
        Ok(PathBuf::from(appdata).join("Clipper").join("config.toml"))
    }

    /// Wczytuje config (albo tworzy domyślny). Błąd składni pliku jest zgłaszany, a nie ignorowany.
    pub fn wczytaj() -> Result<Self, String> {
        let sciezka = Self::sciezka()?;
        if !sciezka.exists() {
            let domyslny = Config::default();
            domyslny.zapisz()?;
            println!("Utworzono domyślny config: {}", sciezka.display());
            return Ok(domyslny);
        }
        let tekst = std::fs::read_to_string(&sciezka).map_err(|e| format!("Nie mogę odczytać {}: {e}", sciezka.display()))?;
        let cfg: Config = toml::from_str(&tekst).map_err(|e| format!("Błąd w {}: {e}", sciezka.display()))?;
        cfg.sprawdz().map_err(|e| format!("Błąd w {}: {e}", sciezka.display()))?;
        println!("Config: {}", sciezka.display());
        Ok(cfg)
    }

    pub fn zapisz(&self) -> Result<(), String> {
        let sciezka = Self::sciezka()?;
        std::fs::create_dir_all(sciezka.parent().unwrap()).map_err(|e| format!("Nie mogę utworzyć folderu configu: {e}"))?;
        let tekst = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&sciezka, tekst).map_err(|e| format!("Nie mogę zapisać {}: {e}", sciezka.display()))
    }

    /// Sprawdza zakresy wartości (dane mogą przyjść z pliku albo z okna ustawień).
    pub fn sprawdz(&self) -> Result<(), String> {
        if !(5..=600).contains(&self.bufor_sekundy) {
            return Err("Długość bufora musi wynosić od 5 do 600 s".into());
        }
        if !(10..=240).contains(&self.fps) {
            return Err("FPS musi wynosić od 10 do 240".into());
        }
        if !(1..=200).contains(&self.bitrate_mbps) {
            return Err("Bitrate musi wynosić od 1 do 200 Mb/s".into());
        }
        if !ENKODERY.iter().any(|(n, _)| *n == self.enkoder) {
            return Err(format!("Nieznany enkoder \"{}\"", self.enkoder));
        }
        if !self.folder_klipow.is_absolute() {
            return Err("Folder klipów musi być pełną ścieżką".into());
        }
        crate::skrot::parsuj(&self.skrot).map(|_| ())
    }
}
