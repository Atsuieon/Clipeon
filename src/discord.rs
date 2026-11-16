// Klip na Discorda jednym skrótem: zapisany klip (gdy trzeba) zmniejszony do limitu wysyłania plików
// i położony do schowka jako plik — w Discordzie zostaje tylko Ctrl+V i Enter.
// Oryginał zostaje w bibliotece bez zmian, a zmniejszona kopia trafia do folderu plików do przeciągania
// (sama znika po dobie).
use std::path::{Path, PathBuf};

/// Klip gotowy do wklejenia.
#[derive(Debug, Clone)]
pub struct Gotowy {
    /// Plik w schowku (oryginał albo zmniejszona kopia)
    pub plik: PathBuf,
    /// Rozmiar pliku w MB (1 MB = 1 000 000 B)
    pub mb: f64,
    /// Czy trzeba było go zmniejszyć (inaczej w schowku jest oryginał)
    pub zmniejszony: bool,
    /// Długość wklejanego klipu w sekundach
    pub sekundy: f64,
}

/// Oryginał mieszczący się w tylu procentach limitu idzie bez zmian (kontener i zaokrąglenia Discorda)
const ZAPAS_ORYGINALU: u64 = 97;
/// Najmniejszy bitrate obrazu (b/s), przy którym klip na Discorda wygląda jeszcze dobrze (720p).
/// Dłuższy bufor przycinamy do końcówki, zamiast robić z niego rozmytą papkę.
const NAJMNIEJSZY_BITRATE: f64 = 1_300_000.0;
/// Krótszego fragmentu nie tniemy (przy bardzo małym limicie zmniejszanie samo obniży jakość)
const NAJKROTSZY_FRAGMENT: f64 = 10.0;

/// Najdłuższa końcówka klipu, która zmieści się w `limit_mb` w przyzwoitej jakości.
pub fn najdluzszy_fragment(limit_mb: u32) -> f64 {
    // Tak samo jak edycja: 94% limitu na obraz i dźwięk, dźwięk 96 kb/s do 20 MB
    let audio = if limit_mb <= 20 { 96_000.0 } else { 128_000.0 };
    (limit_mb as f64 * 8e6 * 0.94 / (NAJMNIEJSZY_BITRATE + audio)).max(NAJKROTSZY_FRAGMENT)
}

/// Przygotowuje zapisany klip do wklejenia na Discorda i kładzie go do schowka.
pub fn przygotuj(klip: &Path, limit_mb: u32) -> Result<Gotowy, String> {
    let rozmiar = std::fs::metadata(klip).map(|m| m.len()).map_err(|e| format!("Nie widzę zapisanego klipu: {e}"))?;
    let info = crate::klipy::info(klip)?;
    let limit = limit_mb as u64 * 1_000_000;
    if rozmiar <= limit * ZAPAS_ORYGINALU / 100 {
        crate::schowek::kopiuj_plik(klip)?;
        return Ok(Gotowy { plik: klip.to_path_buf(), mb: rozmiar as f64 / 1e6, zmniejszony: false, sekundy: info.dlugosc });
    }
    let start = (info.dlugosc - najdluzszy_fragment(limit_mb)).max(0.0);
    let zlecenie = crate::edycja::Zlecenie {
        start,
        koniec: 0.0,
        przyciete: start > 0.0,
        dokladnie: false,
        rozmiar_mb: Some(limit_mb),
        zastap: false,
        glosnosci: Vec::new(),
        kadr: None,
        format: String::new(),
        do_przeciagniecia: true,
    };
    let wynik = crate::edycja::eksportuj(klip, &zlecenie, crate::enkodery::do_eksportu(), &|_| {})?;
    crate::schowek::kopiuj_plik(&wynik.sciezka)?;
    crate::dziennik::info(&format!(
        "Klip na Discorda: {} ({:.1} MB) w schowku",
        wynik.sciezka.file_name().unwrap_or_default().to_string_lossy(),
        wynik.rozmiar as f64 / 1e6
    ));
    Ok(Gotowy { plik: wynik.sciezka, mb: wynik.rozmiar as f64 / 1e6, zmniejszony: true, sekundy: info.dlugosc - start })
}

#[cfg(test)]
mod testy {
    use super::najdluzszy_fragment;

    #[test]
    fn dlugi_bufor_przycinamy_do_konca_ktory_sie_zmiesci() {
        // 10 MB: niecała minuta w 720p
        let f = najdluzszy_fragment(10);
        assert!((50.0..65.0).contains(&f), "{f}");
        // Nitro Basic 50 MB: ok. 4,5 minuty
        assert!(najdluzszy_fragment(50) > 250.0);
        // Bardzo mały limit: nie mniej niż 10 s
        assert_eq!(najdluzszy_fragment(1), 10.0);
    }
}
