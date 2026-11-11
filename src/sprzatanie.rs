// Automatyczne sprzątanie folderu klipów (domyślnie wyłączone): klipy starsze niż X dni
// i/lub nadwyżka ponad X GB trafiają do Kosza — od najstarszych.
// Nigdy nie ruszamy świeżych klipów (ostatnie 10 min) ani wersji przyciętych / skompresowanych
// przez użytkownika w ostatniej dobie.
use crate::Zablokuj;
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

/// Usuwa pliki tymczasowe (*.mp4.tmp) po przerwanych zapisach — np. po awarii albo braku miejsca —
/// z folderu klipów i z podfolderów miesięcy; puste podfoldery miesięcy znikają.
/// Świeże (ostatnie 10 min) zostawiamy, bo zapis może właśnie trwać.
pub fn usun_resztki(folder: &Path) {
    resztki_w_folderze(folder);
    let Ok(wpisy) = std::fs::read_dir(folder) else { return };
    for w in wpisy.flatten() {
        if !w.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let podfolder = w.path();
        resztki_w_folderze(&podfolder);
        // Pusty miesiąc (np. po sprzątnięciu starych klipów) nie musi zostawać
        if std::fs::read_dir(&podfolder).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&podfolder);
        }
    }
}

fn resztki_w_folderze(folder: &Path) {
    let Ok(wpisy) = std::fs::read_dir(folder) else { return };
    for w in wpisy.flatten() {
        let sciezka = w.path();
        if !sciezka.to_string_lossy().ends_with(".mp4.tmp") {
            continue;
        }
        let stary = w.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok())
            .is_some_and(|wiek| wiek > Duration::from_secs(600));
        if stary && std::fs::remove_file(&sciezka).is_ok() {
            crate::dziennik::info(&format!("Usunięto resztkę przerwanego zapisu: {}", sciezka.display()));
        }
    }
}

/// Sprząta folder; zwraca liczbę klipów przeniesionych do Kosza.
pub fn sprzataj(folder: &Path, dni: u32, gb: u32) -> usize {
    if dni == 0 && gb == 0 {
        return 0;
    }
    // Sprzątanie po zapisie i cogodzinne nie mogą działać jednocześnie
    static SPRZATANIE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _jedno = SPRZATANIE.zablokuj();
    let teraz = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let wiek = |k: &klipy::Klip| Duration::from_millis(teraz.saturating_sub(k.czas_ms));
    let chroniony = |k: &klipy::Klip| {
        wiek(k) < Duration::from_secs(600)
            || (wynik_edycji(&k.nazwa) && wiek(k) < Duration::from_secs(86_400))
            || klipy::w_uzyciu(&k.sciezka)
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
        usun_puste_miesiace(folder);
    }
    usuniete
}

/// Kasuje puste podfoldery miesięcy, żeby po sprzątaniu nie zostawały same nazwy.
fn usun_puste_miesiace(folder: &Path) {
    let Ok(wpisy) = std::fs::read_dir(folder) else { return };
    for w in wpisy.flatten() {
        let podfolder = w.path();
        if w.file_type().is_ok_and(|t| t.is_dir()) && std::fs::read_dir(&podfolder).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&podfolder);
        }
    }
}

#[cfg(test)]
mod testy {
    use super::wynik_edycji;

    #[test]
    fn rozpoznaje_wyniki_edycji() {
        assert!(wynik_edycji("Clipeon_2026-11-08_18-21-33_cut.mp4"));
        assert!(wynik_edycji("Clipeon_2026-11-08_18-21-33_cut_2.mp4"));
        assert!(wynik_edycji("Clipeon_2026-11-08_18-21-33_10MB.mp4"));
        assert!(wynik_edycji("Clipeon_2026-11-08_18-21-33_cut_100MB.mp4"));
        assert!(wynik_edycji("Clipeon_2026-11-08_18-21-33_audio.mp4"));
        assert!(!wynik_edycji("Clipeon_2026-11-08_18-21-33.mp4"));
        assert!(!wynik_edycji("Clipeon_2026-11-08_18-21-33_2.mp4"));
    }
}
