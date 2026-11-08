// Clipper: lekki "instant replay" — trzyma ostatnie N sekund gry w RAM i zapisuje je na skrót.
mod bufor;
mod config;
mod wideo;

use bufor::Bufor;
use std::time::Duration;

fn main() {
    let cfg = match config::Config::wczytaj() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let bufor_wideo = Bufor::nowy(cfg.bufor_sekundy);
    let _wideo = wideo::uruchom(&cfg, bufor_wideo.clone()).unwrap_or_else(|e| {
        eprintln!("Nie udało się uruchomić wideo: {e}");
        std::process::exit(1);
    });

    // Tymczasowo: co 5 s wypisz stan bufora
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let b = bufor_wideo.lock().unwrap();
        let p = b.migawka();
        if let (Some(a), Some(z)) = (p.first(), p.last()) {
            println!(
                "Bufor: {} klatek, {:.1} s, {:.1} MB, kluczowych {}",
                p.len(),
                (z.czas - a.czas) as f64 / 1e7,
                b.bajty() as f64 / 1e6,
                p.iter().filter(|x| x.klucz).count()
            );
        }
    }
}
