// Dziennik zdarzeń w %APPDATA%\Clipeon\logs\clipeon.log (z rotacją: najwyżej 3 pliki po 1 MB).
// Trafiają tu błędy, wznowienia nagrywania i awarie wątków — żeby dało się sprawdzić, co się stało.
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAKS_ROZMIAR: u64 = 1_000_000;
const PLIKOW: u32 = 3;

pub fn folder() -> PathBuf {
    let baza = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    baza.join("Clipeon").join("logs")
}

pub fn plik() -> PathBuf {
    folder().join("clipeon.log")
}

fn znacznik_czasu() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

/// Dopisuje linię do dziennika (i na konsolę w wersji debug).
pub fn zapisz(poziom: &str, tekst: &str) {
    static ZAMEK: Mutex<()> = Mutex::new(());
    let _z = ZAMEK.lock().unwrap_or_else(|e| e.into_inner());
    eprintln!("[{poziom}] {tekst}");
    let sciezka = plik();
    let _ = std::fs::create_dir_all(folder());
    // Rotacja: clipeon.log -> clipeon.1.log -> clipeon.2.log
    if std::fs::metadata(&sciezka).is_ok_and(|m| m.len() > MAKS_ROZMIAR) {
        for i in (1..PLIKOW).rev() {
            let z = if i == 1 { sciezka.clone() } else { folder().join(format!("clipeon.{}.log", i - 1)) };
            let _ = std::fs::rename(z, folder().join(format!("clipeon.{i}.log")));
        }
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&sciezka) {
        let _ = writeln!(f, "{} [{poziom}] {tekst}", znacznik_czasu());
    }
}

pub fn info(tekst: &str) {
    zapisz("INFO", tekst);
}

pub fn blad(tekst: &str) {
    zapisz("BŁĄD", tekst);
}

/// Awaria wątku (panic) trafia do dziennika; program działa dalej, a strażnik nagrywania
/// wznowi to, co przestało działać.
pub fn przechwytuj_awarie() {
    std::panic::set_hook(Box::new(|info| {
        let gdzie = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let co = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let watek = std::thread::current().name().unwrap_or("bez nazwy").to_string();
        zapisz("AWARIA", &format!("wątek {watek}: {co} ({gdzie})"));
    }));
}
