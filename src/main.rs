// Clipeon: lekki "instant replay" — trzyma ostatnie N sekund gry w RAM i zapisuje je na skrót.
// W wersji release bez okna konsoli (program żyje w zasobniku).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aktualizacje;
mod aplikacja;
mod audio;
mod bufor;
mod config;
mod dziennik;
mod dzwiek;
mod edycja;
mod enkodery;
mod ffmpeg;
mod film;
mod gra;
mod ikona;
mod karty;
mod klipy;
mod odzyskiwanie;
mod pamiec;
mod pomiar;
mod powiadomienie;
mod rdzen;
mod skrot;
mod sprzatanie;
mod wideo;
mod zasobnik;
mod zapis;

/// Blokada odporna na „zatrucie”: gdy inny wątek padł w trakcie blokady, dane i tak są do użycia
/// (wszędzie w Clipeonie wolimy działać dalej niż wyłożyć cały program).
pub trait Zablokuj<T> {
    fn zablokuj(&self) -> std::sync::MutexGuard<'_, T>;
}

impl<T> Zablokuj<T> for std::sync::Mutex<T> {
    fn zablokuj(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn main() {
    aplikacja::uruchom();
}
