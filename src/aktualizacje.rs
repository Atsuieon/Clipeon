// Sprawdzanie aktualizacji: jedno zapytanie do GitHub Releases (Atsuieon/Clipeon) przy starcie
// i raz na dobę. To jedyne połączenie Clipeona z internetem; można je wyłączyć w ustawieniach.
use serde::Serialize;
use std::time::Duration;

pub const REPOZYTORIUM: &str = "https://github.com/Atsuieon/Clipeon";
const API: &str = "https://api.github.com/repos/Atsuieon/Clipeon/releases/latest";

/// Wersja tego programu (z Cargo.toml).
pub const WERSJA: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize)]
pub struct Wydanie {
    pub wersja: String,
    /// Strona wydania na GitHubie (zawsze w obrębie repozytorium Clipeona)
    pub strona: String,
}

/// „1.2.3” / „v1.2.3” -> (1, 2, 3); przyrostki typu „-beta” są ignorowane.
fn liczby(wersja: &str) -> Option<(u64, u64, u64)> {
    let w = wersja.trim().trim_start_matches(['v', 'V']);
    let w = w.split(['-', '+']).next()?;
    let mut c = w.split('.').map(|x| x.parse::<u64>().ok());
    Some((c.next()??, c.next().flatten().unwrap_or(0), c.next().flatten().unwrap_or(0)))
}

pub fn nowsza(dostepna: &str, obecna: &str) -> bool {
    matches!((liczby(dostepna), liczby(obecna)), (Some(a), Some(b)) if a > b)
}

/// Najnowsze wydanie, jeśli jest nowsze od tej wersji (None = brak nowszego).
pub fn sprawdz() -> Result<Option<Wydanie>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let tekst = agent
        .get(API)
        .header("User-Agent", &format!("Clipeon/{WERSJA}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Nie udało się sprawdzić aktualizacji: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&tekst).map_err(|e| e.to_string())?;
    let tag = json["tag_name"].as_str().ok_or("Brak numeru wersji w odpowiedzi GitHuba")?;
    // Adres strony bierzemy tylko z naszego repozytorium (nie ufamy dowolnemu linkowi z odpowiedzi)
    let strona = json["html_url"]
        .as_str()
        .filter(|u| u.starts_with(&format!("{REPOZYTORIUM}/releases/")))
        .map(String::from)
        .unwrap_or_else(|| format!("{REPOZYTORIUM}/releases/latest"));
    Ok(nowsza(tag, WERSJA).then(|| Wydanie { wersja: tag.trim_start_matches(['v', 'V']).to_string(), strona }))
}

/// Otwiera stronę w domyślnej przeglądarce (tylko adresy repozytorium Clipeona).
pub fn otworz(adres: &str) {
    if adres.starts_with(REPOZYTORIUM) {
        let _ = std::process::Command::new("explorer").arg(adres).spawn();
    }
}

#[cfg(test)]
mod testy {
    use super::nowsza;

    #[test]
    fn porownuje_wersje() {
        assert!(nowsza("v1.0.1", "1.0.0"));
        assert!(nowsza("1.10.0", "1.9.9"));
        assert!(nowsza("v2.0", "1.9.9"));
        assert!(!nowsza("v1.0.0", "1.0.0"));
        assert!(!nowsza("0.9.0", "1.0.0"));
        assert!(!nowsza("v1.0.0-beta", "1.0.0"));
        assert!(!nowsza("bzdura", "1.0.0"));
    }
}
