// Clipper: lekki "instant replay" — trzyma ostatnie N sekund gry w RAM i zapisuje je na skrót.
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
mod ikona;
mod klipy;
mod powiadomienie;
mod rdzen;
mod skrot;
mod sprzatanie;
mod wideo;
mod zasobnik;
mod zapis;

fn main() {
    aplikacja::uruchom();
}
