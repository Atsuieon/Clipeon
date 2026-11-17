// Język dymków i menu przy zegarze (okno tłumaczy się samo — ui/jezyk.js). Teksty w kodzie są po polsku,
// a po angielsku podmienia je słownik z ui/en.js: ten sam, którego używa okno.
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

static EN: AtomicBool = AtomicBool::new(false);

/// Słownik polski → angielski: skrypt okna `window.SLOWNIK_EN = {…};` — JSON to wszystko od pierwszego
/// nawiasu klamrowego po `SLOWNIK_EN =` do ostatniego (komentarze nad nim mogą zawierać nawiasy).
const SLOWNIK_EN: &str = include_str!("../ui/en.js");

fn json_slownika(skrypt: &str) -> &str {
    let od = skrypt.find("SLOWNIK_EN =").unwrap_or(0);
    match (skrypt[od..].find('{').map(|a| od + a), skrypt.rfind('}')) {
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

struct Slownik {
    teksty: HashMap<String, String>,
    /// Wzory dla tekstów złożonych z części (np. błąd z opisem z Windows): `$1`, `$2` są tłumaczone dalej
    wzory: Vec<(regex::Regex, String)>,
}

fn slownik() -> &'static Slownik {
    static S: OnceLock<Slownik> = OnceLock::new();
    S.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Plik {
            teksty: HashMap<String, String>,
            #[serde(default)]
            wzory: Vec<(String, String)>,
        }
        let plik = serde_json::from_str::<Plik>(json_slownika(SLOWNIK_EN)).unwrap_or(Plik { teksty: HashMap::new(), wzory: Vec::new() });
        let wzory = plik.wzory.into_iter().filter_map(|(w, z)| regex::Regex::new(&w).ok().map(|r| (r, z))).collect();
        Slownik { teksty: plik.teksty, wzory }
    })
}

/// Polski tekst po angielsku (tak samo jak `przetlumacz` w ui/jezyk.js); bez zmian, gdy go nie ma w słowniku.
fn przetlumacz(tekst: &str, glebokosc: u8) -> String {
    let s = slownik();
    if let Some(en) = s.teksty.get(tekst) {
        return en.clone();
    }
    if glebokosc < 3 {
        for (wzor, zamiana) in &s.wzory {
            if let Some(m) = wzor.captures(tekst) {
                let mut wynik = String::new();
                let mut znaki = zamiana.chars().peekable();
                while let Some(c) = znaki.next() {
                    match (c, znaki.peek().and_then(|d| d.to_digit(10))) {
                        ('$', Some(i)) => {
                            znaki.next();
                            wynik.push_str(&przetlumacz(m.get(i as usize).map_or("", |g| g.as_str()), glebokosc + 1));
                        }
                        _ => wynik.push(c),
                    }
                }
                return wynik;
            }
        }
    }
    tekst.to_string()
}

/// Tekst w wybranym języku: po angielsku ze słownika (gdy go tam nie ma — bez zmian).
pub fn t(tekst: &str) -> String {
    if !en() {
        return tekst.to_string();
    }
    przetlumacz(tekst, 0)
}

/// Jeden z dwóch gotowych tekstów (dla tekstów z liczbami i nazwami, których nie ma w słowniku).
pub fn wybierz(pl: String, en: String) -> String {
    if self::en() { en } else { pl }
}

#[cfg(test)]
mod testy {
    use super::{SLOWNIK_EN, json_slownika, kod_dla_windows, przetlumacz, slownik};

    #[test]
    fn jezyk_z_windows() {
        assert_eq!(kod_dla_windows(0x0415), "pl"); // polski
        assert_eq!(kod_dla_windows(0x0409), "en"); // angielski (USA)
        assert_eq!(kod_dla_windows(0x0407), "en"); // niemiecki
    }

    #[test]
    fn slownik_sie_wczytuje() {
        // Cały plik musi być poprawnym JSON-em (błąd nie może cicho dać pustego słownika)
        serde_json::from_str::<serde_json::Value>(json_slownika(SLOWNIK_EN)).expect("ui/en.js: zły JSON");
        assert_eq!(slownik().teksty.get("Klip zapisany").map(String::as_str), Some("Clip saved"));
        assert!(slownik().teksty.len() > 200);
        // Każdy wzór z en.js musi działać też w Rust (inaczej dymki i menu zostałyby po polsku)
        let w = serde_json::from_str::<serde_json::Value>(json_slownika(SLOWNIK_EN)).unwrap()["wzory"].as_array().unwrap().len();
        assert_eq!(slownik().wzory.len(), w, "część wzorów z ui/en.js nie kompiluje się w Rust");
    }

    #[test]
    fn bledy_sa_tlumaczone_wzorami() {
        assert_eq!(
            przetlumacz("Nie nagrywa: Nie udało się uruchomić nagrywania: Nie udało się uruchomić ffmpeg: program not found — ponawiam…", 0),
            "Not recording: Couldn't start recording: Couldn't start ffmpeg: program not found — retrying…"
        );
        assert_eq!(przetlumacz("Klip zapisany", 0), "Clip saved");
        assert_eq!(przetlumacz("coś spoza słownika", 0), "coś spoza słownika");
    }
}
