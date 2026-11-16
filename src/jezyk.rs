// Język dymków i menu przy zegarze (okno tłumaczy się samo — ui/jezyk.js). Teksty w kodzie są po polsku,
// a po angielsku podmienia je słownik z ui/en.json: ten sam, którego używa okno.
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

/// Ustawia język z configu: "pl", "en" albo "auto" (polski tylko przy polskim Windows).
pub fn ustaw(z_configu: &str) {
    EN.store(kod_dla(z_configu, windows_po_polsku()) == "en", Ordering::Relaxed);
}

/// Czy dymki i menu mają być po angielsku.
pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// Kod języka dla okna: "pl" albo "en".
pub fn kod() -> &'static str {
    if en() { "en" } else { "pl" }
}

fn kod_dla(z_configu: &str, windows_po_polsku: bool) -> &'static str {
    match z_configu {
        "pl" => "pl",
        "en" => "en",
        _ if windows_po_polsku => "pl",
        _ => "en",
    }
}

/// Język interfejsu Windows to polski (podstawowy identyfikator języka 0x15).
fn windows_po_polsku() -> bool {
    let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
    id & 0x3ff == 0x15
}

fn slownik() -> &'static HashMap<String, String> {
    static S: OnceLock<HashMap<String, String>> = OnceLock::new();
    S.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Plik {
            teksty: HashMap<String, String>,
        }
        serde_json::from_str::<Plik>(include_str!("../ui/en.json")).map(|p| p.teksty).unwrap_or_default()
    })
}

/// Tekst w wybranym języku: po angielsku ze słownika (gdy go tam nie ma — bez zmian).
pub fn t(tekst: &str) -> String {
    if !en() {
        return tekst.to_string();
    }
    slownik().get(tekst).cloned().unwrap_or_else(|| tekst.to_string())
}

/// Jeden z dwóch gotowych tekstów (dla tekstów z liczbami i nazwami, których nie ma w słowniku).
pub fn wybierz(pl: String, en: String) -> String {
    if self::en() { en } else { pl }
}

#[cfg(test)]
mod testy {
    use super::{kod_dla, slownik};

    #[test]
    fn jezyk_z_configu_i_z_windows() {
        assert_eq!(kod_dla("pl", false), "pl");
        assert_eq!(kod_dla("en", true), "en");
        assert_eq!(kod_dla("auto", true), "pl");
        assert_eq!(kod_dla("auto", false), "en");
    }

    #[test]
    fn slownik_sie_wczytuje() {
        assert_eq!(slownik().get("Klip zapisany").map(String::as_str), Some("Clip saved"));
        assert!(slownik().len() > 200);
    }
}
