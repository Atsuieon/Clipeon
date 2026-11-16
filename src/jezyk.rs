// Język dymków i menu przy zegarze (okno tłumaczy się samo — ui/jezyk.js). Teksty w kodzie są po polsku,
// a po angielsku podmienia je słownik z ui/en.js: ten sam, którego używa okno.
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

/// Słownik polski → angielski: skrypt okna `window.SLOWNIK_EN = {…};` — JSON to wszystko od pierwszego
/// „{” do ostatniego „}”.
const SLOWNIK_EN: &str = include_str!("../ui/en.js");

fn json_slownika(skrypt: &str) -> &str {
    match (skrypt.find('{'), skrypt.rfind('}')) {
        (Some(a), Some(b)) if a < b => &skrypt[a..=b],
        _ => "{}",
    }
}

/// Ustawia język z configu: "pl" albo "en".
pub fn ustaw(z_configu: &str) {
    EN.store(z_configu == "en", Ordering::Relaxed);
}

/// Czy dymki i menu mają być po angielsku.
pub fn en() -> bool {
    EN.load(Ordering::Relaxed)
}

/// Język na pierwsze uruchomienie: polski przy polskim Windows, inaczej angielski.
pub fn systemowy() -> &'static str {
    kod_dla_windows(unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() })
}

/// Polski, gdy podstawowy identyfikator języka Windows to 0x15.
fn kod_dla_windows(id: u16) -> &'static str {
    if id & 0x3ff == 0x15 { "pl" } else { "en" }
}

fn slownik() -> &'static HashMap<String, String> {
    static S: OnceLock<HashMap<String, String>> = OnceLock::new();
    S.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Plik {
            teksty: HashMap<String, String>,
        }
        serde_json::from_str::<Plik>(json_slownika(SLOWNIK_EN)).map(|p| p.teksty).unwrap_or_default()
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
    use super::{kod_dla_windows, slownik};

    #[test]
    fn jezyk_z_windows() {
        assert_eq!(kod_dla_windows(0x0415), "pl"); // polski
        assert_eq!(kod_dla_windows(0x0409), "en"); // angielski (USA)
        assert_eq!(kod_dla_windows(0x0407), "en"); // niemiecki
    }

    #[test]
    fn slownik_sie_wczytuje() {
        assert_eq!(slownik().get("Klip zapisany").map(String::as_str), Some("Clip saved"));
        assert!(slownik().len() > 200);
    }
}
