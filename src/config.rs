// Konfiguracja z %APPDATA%\Clipper\config.toml.
// Jeśli pliku nie ma, tworzymy go z wartościami domyślnymi.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ile ostatnich sekund trzymać w pamięci
    pub bufor_sekundy: u32,
    /// Stała liczba klatek na sekundę nagrania
    pub fps: u32,
    /// Bitrate wideo w Mb/s (VBR)
    pub bitrate_mbps: u32,
    /// Folder, do którego trafiają klipy
    pub folder_klipow: PathBuf,
    /// Skrót zapisu klipu, np. "Alt+F10", "Ctrl+Shift+S"
    pub skrot: String,
}

impl Default for Config {
    fn default() -> Self {
        let profil = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            bufor_sekundy: 30,
            fps: 60,
            bitrate_mbps: 20,
            folder_klipow: profil.join("Videos").join("Clipper"),
            skrot: "Alt+F10".into(),
        }
    }
}

impl Config {
    /// Wczytuje config (albo tworzy domyślny). Błąd składni pliku jest zgłaszany, a nie ignorowany.
    pub fn wczytaj() -> Result<Self, String> {
        let appdata = std::env::var_os("APPDATA").ok_or("Brak zmiennej APPDATA")?;
        let sciezka = PathBuf::from(appdata).join("Clipper").join("config.toml");

        if !sciezka.exists() {
            let domyslny = Config::default();
            std::fs::create_dir_all(sciezka.parent().unwrap())
                .map_err(|e| format!("Nie mogę utworzyć folderu configu: {e}"))?;
            let tekst = toml::to_string_pretty(&domyslny).map_err(|e| e.to_string())?;
            std::fs::write(&sciezka, tekst).map_err(|e| format!("Nie mogę zapisać {}: {e}", sciezka.display()))?;
            println!("Utworzono domyślny config: {}", sciezka.display());
            return Ok(domyslny);
        }

        let tekst = std::fs::read_to_string(&sciezka).map_err(|e| format!("Nie mogę odczytać {}: {e}", sciezka.display()))?;
        let cfg: Config = toml::from_str(&tekst).map_err(|e| format!("Błąd w {}: {e}", sciezka.display()))?;
        if cfg.fps == 0 || cfg.bufor_sekundy == 0 || cfg.bitrate_mbps == 0 {
            return Err("fps, bufor_sekundy i bitrate_mbps muszą być większe od zera".into());
        }
        println!("Config: {}", sciezka.display());
        Ok(cfg)
    }
}
