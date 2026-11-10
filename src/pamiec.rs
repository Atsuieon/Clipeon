// Pamięć sprzętowa w %APPDATA%\Clipeon\enkodery.json: wyniki prób enkoderów i działająca droga
// przechwytywania dla każdego monitora. Ważna tylko dla tych samych kart, sterowników i ffmpeg —
// po zmianie czegokolwiek z tego zaczynamy od zera (stąd „odcisk”).
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Clone, Serialize, Deserialize)]
pub struct WynikProby {
    pub ok: bool,
    /// Przyczyna porażki (ostatnie linie komunikatu ffmpeg)
    #[serde(default)]
    pub powod: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Pamiec {
    #[serde(default)]
    odcisk: String,
    /// Klucz: enkoder@SZERxWYS[/d3d11:karta]
    #[serde(default)]
    pub proby: BTreeMap<String, WynikProby>,
    /// Klucz: nazwa urządzenia monitora, wartość: droga, która ostatnio działała
    #[serde(default)]
    pub drogi: BTreeMap<String, String>,
}

fn plik() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Clipeon").join("enkodery.json"))
}

/// Karty, ich sterowniki i ffmpeg — od tego zależą wyniki prób.
fn odcisk() -> String {
    let karty: Vec<String> = crate::karty::karty().iter().map(|k| format!("{}={}", k.nazwa, k.sterownik)).collect();
    format!("{}|ffmpeg {}", karty.join(";"), crate::ffmpeg::odcisk())
}

fn wspolna() -> &'static Mutex<Pamiec> {
    static P: OnceLock<Mutex<Pamiec>> = OnceLock::new();
    P.get_or_init(|| {
        let teraz = odcisk();
        let wczytana: Option<Pamiec> =
            plik().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok());
        match wczytana {
            Some(p) if p.odcisk == teraz => Mutex::new(p),
            stara => {
                if stara.is_some() {
                    crate::dziennik::info("Zmieniły się karty, sterowniki albo ffmpeg — enkodery będą sprawdzone od nowa");
                }
                Mutex::new(Pamiec { odcisk: teraz, ..Default::default() })
            }
        }
    })
}

/// Dostęp do pamięci (blokada odporna na awarię innego wątku).
pub fn pamiec() -> MutexGuard<'static, Pamiec> {
    wspolna().lock().unwrap_or_else(|e| e.into_inner())
}

/// Zapis na dysk (po każdej zmianie; plik jest mały).
pub fn zapisz(p: &Pamiec) {
    let Some(sciezka) = plik() else { return };
    let _ = std::fs::create_dir_all(sciezka.parent().unwrap());
    if let Ok(t) = serde_json::to_string_pretty(p) {
        let _ = std::fs::write(sciezka, t);
    }
}
