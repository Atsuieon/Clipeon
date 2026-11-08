// Clipper: lekki "instant replay" — trzyma ostatnie N sekund gry w RAM i zapisuje je na skrót.
mod bufor;
mod config;
mod skrot;
mod wideo;
mod zapis;

use bufor::Bufor;

fn main() {
    if let Err(e) = uruchom() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn uruchom() -> Result<(), String> {
    let cfg = config::Config::wczytaj()?;

    let bufor_wideo = Bufor::nowy(cfg.bufor_sekundy);
    let _wideo = wideo::uruchom(&cfg, bufor_wideo.clone()).map_err(|e| format!("Nie udało się uruchomić wideo: {e}"))?;

    skrot::nasluchuj(&cfg.skrot, || {
        // Migawka bufora trwa chwilę; zapis idzie w osobnym wątku, nagrywanie się nie zatrzymuje
        let wideo = bufor_wideo.lock().unwrap().migawka();
        let (folder, fps) = (cfg.folder_klipow.clone(), cfg.fps);
        std::thread::spawn(move || {
            if let Err(e) = zapis::zapisz(&folder, fps, wideo, Vec::new()) {
                eprintln!("Nie udało się zapisać klipu: {e}");
            }
        });
    })
}
