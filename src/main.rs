// Clipper: lekki "instant replay" — trzyma ostatnie N sekund gry w RAM i zapisuje je na skrót.
// W wersji release bez okna konsoli (program żyje w zasobniku).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aplikacja;
mod audio;
mod bufor;
mod config;
mod edycja;
mod ffmpeg;
mod ikona;
mod klipy;
mod rdzen;
mod skrot;
mod wideo;
mod zasobnik;
mod zapis;

fn main() {
    aplikacja::uruchom();
}
