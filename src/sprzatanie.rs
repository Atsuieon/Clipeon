// Automatyczne sprzątanie folderu klipów (domyślnie wyłączone): klipy starsze niż X dni
// i/lub nadwyżka ponad X GB trafiają do Kosza — od najstarszych.
// Nigdy nie ruszamy świeżych klipów (ostatnie 10 min) ani wersji przyciętych / skompresowanych
// przez użytkownika w ostatniej dobie.
use crate::klipy;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Czy plik jest wynikiem edycji (nazwa_cut.mp4, nazwa_10MB.mp4, nazwa_audio.mp4, także z numerem).
fn wynik_edycji(nazwa: &str) -> bool {
    let rdzen = nazwa.trim_end_matches(".mp4");
    // Odcinamy ewentualny numer kolejnej wersji: nazwa_cut_2
    let rdzen = rdzen.rsplit_once('_').filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit())).map_or(rdzen, |(r, _)| r);
    rdzen.ends_with("_cut")
        || rdzen.ends_with("_audio")
        || rdzen.rsplit_once('_').is_some_and(|(_, k)| k.ends_with("MB") && k[..k.len() - 2].chars().all(|c| c.is_ascii_digit()) && k.len() > 2)
}

/// Sprząta folder; zwraca liczbę klipów przeniesionych do Kosza.
pub fn sprzataj(folder: &Path, dni: u32, gb: u32) -> usize {
    if dni == 0 && gb == 0 {
        return 0;
    }
    let teraz = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let wiek = |k: &klipy::Klip| Duration::from_millis(teraz.saturating_sub(k.czas_ms));
    let chroniony = |k: &klipy::Klip| {
        wiek(k) < Duration::from_secs(600) || (wynik_edycji(&k.nazwa) && wiek(k) < Duration::from_secs(86_400))
    };

    // Od najstarszego
    let mut lista = klipy::lista(folder);
    lista.reverse();
    let mut suma: u64 = lista.iter().map(|k| k.rozmiar).sum();
    let limit = gb as u64 * 1_000_000_000;
    let mut usuniete = 0;
    for k in &lista {
        if chroniony(k) {
            continue;
        }
        let za_stary = dni > 0 && wiek(k) > Duration::from_secs(dni as u64 * 86_400);
        let za_duzo = gb > 0 && suma > limit;
        if !(za_stary || za_duzo) {
            continue;
        }
        match klipy::usun(&k.sciezka) {
            Ok(()) => {
                suma = suma.saturating_sub(k.rozmiar);
                usuniete += 1;
            }
            Err(e) => eprintln!("Sprzątanie: {e}"),
        }
    }
    if usuniete > 0 {
        println!("Sprzątanie: {usuniete} klipów przeniesiono do Kosza");
    }
    usuniete
}

#[cfg(test)]
mod testy {
    use super::wynik_edycji;

    #[test]
    fn rozpoznaje_wyniki_edycji() {
        assert!(wynik_edycji("Clipper_2026-11-08_18-21-33_cut.mp4"));
        assert!(wynik_edycji("Clipper_2026-11-08_18-21-33_cut_2.mp4"));
        assert!(wynik_edycji("Clipper_2026-11-08_18-21-33_10MB.mp4"));
        assert!(wynik_edycji("Clipper_2026-11-08_18-21-33_cut_100MB.mp4"));
        assert!(wynik_edycji("Clipper_2026-11-08_18-21-33_audio.mp4"));
        assert!(!wynik_edycji("Clipper_2026-11-08_18-21-33.mp4"));
        assert!(!wynik_edycji("Clipper_2026-11-08_18-21-33_2.mp4"));
    }
}
