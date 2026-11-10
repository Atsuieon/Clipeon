// Aktualizacje: Clipeon sam sprawdza GitHub Releases (Atsuieon/Clipeon) przy starcie i raz na dobę,
// a po kliknięciu „Aktualizuj” pobiera nowy instalator, sprawdza jego podpis i instaluje go.
// Plik latest.json i podpisy instalatorów tworzy workflow wydania. To jedyne połączenie Clipeona
// z internetem; samo sprawdzanie można wyłączyć w ustawieniach.
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const REPOZYTORIUM: &str = "https://github.com/Atsuieon/Clipeon";

/// Wersja tego programu (z Cargo.toml).
pub const WERSJA: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Serialize)]
pub struct Wydanie {
    pub wersja: String,
}

/// Postęp pobierania dla okna (procent = None, gdy serwer nie podał rozmiaru).
#[derive(Clone, Serialize)]
pub struct Postep {
    pub etap: &'static str,
    pub procent: Option<u8>,
}

/// Znalezione nowsze wydanie i to, czy właśnie trwa instalacja.
#[derive(Default)]
pub struct Stan {
    znaleziona: Mutex<Option<Update>>,
    trwa: AtomicBool,
}

impl Stan {
    pub fn znaleziona(&self) -> Option<Wydanie> {
        self.znaleziona.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|u| Wydanie { wersja: u.version.clone() })
    }

    pub fn trwa(&self) -> bool {
        self.trwa.load(Ordering::SeqCst)
    }
}

/// Pyta GitHuba o najnowsze wydanie. Zwraca je, jeśli jest nowsze od tej wersji.
pub async fn sprawdz(app: &AppHandle, przed_zamknieciem: impl Fn() + Send + Sync + 'static) -> Result<Option<Wydanie>, String> {
    let aktualizacja = app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .on_before_exit(przed_zamknieciem)
        .build()
        .map_err(|e| format!("Nie udało się sprawdzić aktualizacji: {e}"))?
        .check()
        .await
        .map_err(|e| format!("Nie udało się sprawdzić aktualizacji: {e}"))?;
    let wydanie = aktualizacja.as_ref().map(|u| Wydanie { wersja: u.version.clone() });
    *app.state::<Stan>().znaleziona.lock().unwrap_or_else(|e| e.into_inner()) = aktualizacja;
    Ok(wydanie)
}

/// Pobiera znalezioną wersję, sprawdza podpis i uruchamia instalator. Po udanym starcie
/// instalatora Clipeon się zamyka, a instalator uruchamia go ponownie w nowej wersji.
pub async fn zainstaluj(app: &AppHandle) -> Result<(), String> {
    let stan = app.state::<Stan>();
    if stan.trwa.swap(true, Ordering::SeqCst) {
        return Err("Aktualizacja już trwa".into());
    }
    let wynik = pobierz_i_zainstaluj(app).await;
    stan.trwa.store(false, Ordering::SeqCst);
    if wynik.is_err() {
        let _ = app.emit("aktualizacja-postep", Postep { etap: "blad", procent: None });
    }
    wynik
}

async fn pobierz_i_zainstaluj(app: &AppHandle) -> Result<(), String> {
    let Some(mut aktualizacja) = app.state::<Stan>().znaleziona.lock().unwrap_or_else(|e| e.into_inner()).clone() else {
        return Err("Nie ma nowszej wersji do zainstalowania".into());
    };
    // Limit 20 s dotyczył tylko sprawdzania; instalator pobiera się dłużej
    aktualizacja.timeout = None;
    crate::dziennik::info(&format!("Pobieranie aktualizacji {}", aktualizacja.version));

    let (mut pobrano, mut ostatni) = (0u64, None);
    let bajty = aktualizacja
        .download(
            |kawalek, calosc| {
                pobrano += kawalek as u64;
                let procent = calosc.filter(|c| *c > 0).map(|c| (pobrano * 100 / c).min(100) as u8);
                // Zdarzenie tylko przy zmianie procentu albo co 1 MB, gdy rozmiar nie jest znany
                let krok = procent.map(u64::from).unwrap_or(pobrano >> 20);
                if ostatni != Some(krok) {
                    ostatni = Some(krok);
                    let _ = app.emit("aktualizacja-postep", Postep { etap: "pobieranie", procent });
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("Nie udało się pobrać aktualizacji: {e}"))?;

    crate::dziennik::info(&format!("Instalowanie aktualizacji {}", aktualizacja.version));
    let _ = app.emit("aktualizacja-postep", Postep { etap: "instalowanie", procent: None });
    // Podpis jest sprawdzany przed uruchomieniem instalatora; w Windows ta funkcja kończy program
    aktualizacja.install(bajty).map_err(|e| format!("Nie udało się zainstalować aktualizacji: {e}"))
}

/// Otwiera stronę w domyślnej przeglądarce (tylko adresy repozytorium Clipeona).
pub fn otworz(adres: &str) {
    if adres.starts_with(REPOZYTORIUM) {
        let _ = std::process::Command::new("explorer").arg(adres).spawn();
    }
}
