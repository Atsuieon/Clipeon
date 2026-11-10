// Warstwa Tauri: ikona w zasobniku z menu, okno (Klipy / Ustawienia) tworzone na żądanie
// i niszczone po zamknięciu (w tle zostaje tylko rdzeń nagrywania), komendy dla UI, powiadomienia.
use crate::config::Config;
use crate::ikona;
use crate::klipy;
use crate::rdzen::{Rdzen, Stan, Zdarzenie};
use serde::Serialize;
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_dialog::DialogExt;

const ID_ZASOBNIKA: &str = "glowna";

/// Pozycje menu, których tekst się zmienia.
struct PozycjeMenu {
    stan: MenuItem<Wry>,
    aktualizacja: MenuItem<Wry>,
    zapisz: MenuItem<Wry>,
    pauza: MenuItem<Wry>,
}

/// Pozwala oknu czytać pliki z folderu przez protokół asset:. Tauri porównuje ścieżki kanoniczne
/// (z przedrostkiem \\?\), więc folder musi istnieć i być podany w tej samej postaci.
fn pozwol_na_folder(app: &AppHandle, folder: &std::path::Path) {
    let _ = std::fs::create_dir_all(folder);
    let kanoniczny = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    let _ = app.asset_protocol_scope().allow_directory(kanoniczny, false);
}

fn tekst_zapisu(cfg: &Config) -> String {
    format!("Zapisz ostatnie {} s\t{}", cfg.bufor_sekundy, cfg.skrot)
}

/// Pokazuje okno na danej zakładce ("klipy" / "ustawienia"); tworzy je, jeśli zostało zamknięte.
fn pokaz_okno(app: &AppHandle, zakladka: &str) {
    if let Some(okno) = app.get_webview_window("main") {
        let _ = okno.emit("zakladka", zakladka);
        let _ = okno.unminimize();
        let _ = okno.show();
        let _ = okno.set_focus();
        return;
    }
    // Tworzenie okna z obsługi zdarzenia potrafi zakleszczyć się na Windows — robimy to z innego wątku
    let (app, url) = (app.clone(), format!("index.html#{zakladka}"));
    std::thread::spawn(move || {
        let wynik = WebviewWindowBuilder::new(&app, "main", WebviewUrl::App(url.into()))
            .title("Clipeon")
            .inner_size(1120.0, 740.0)
            .min_inner_size(760.0, 520.0)
            .background_color(Color(0, 0, 0, 255))
            // Bez sprzętowych nakładek wideo: w oknach z przezroczystością (np. nieaktywne okna
            // w GlazeWM) taka nakładka znika i podgląd klipu jest pusty
            .additional_browser_args(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-direct-composition-video-overlays",
            )
            // Bez systemowej ramki (pasek tytułu w kolorze akcentu Windows); własny pasek jest w UI.
            // Cień zostaje, więc Windows 11 nadal zaokrągla rogi i pozwala zmieniać rozmiar.
            .decorations(false)
            .shadow(true)
            .build();
        match wynik {
            Ok(okno) => ciemna_obwodka(&okno),
            Err(e) => eprintln!("Nie udało się otworzyć okna: {e}"),
        }
    });
}

/// Windows 11 rysuje wokół okna obwódkę w kolorze akcentu (u użytkownika zielonym) — ustawiamy ciemną.
fn ciemna_obwodka(okno: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::{COLORREF, HWND};
    use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmSetWindowAttribute};
    let Ok(uchwyt) = okno.hwnd() else { return };
    let kolor = COLORREF(0x001A_1A1A); // 0x00BBGGRR
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(uchwyt.0 as _),
            DWMWA_BORDER_COLOR,
            &kolor as *const _ as *const _,
            std::mem::size_of::<COLORREF>() as u32,
        );
    }
}

/// Ustawia widoczność ikony przy zegarze i zapamiętuje, że Clipeon już to zrobił.
/// Windows czyta to ustawienie tylko przy dodawaniu ikony, więc po zapisie dodajemy ją ponownie.
fn ustaw_ikone(app: &AppHandle, rdzen: &Rdzen, widoczna: bool) {
    let (r, app) = (rdzen.clone(), app.clone());
    crate::zasobnik::ustaw_w_tle(widoczna, move || {
        r.zmien_bez_restartu(|c| c.ikona_ustawiona = true);
        if let Some(t) = app.tray_by_id(ID_ZASOBNIKA) {
            let _ = t.set_visible(false);
            std::thread::sleep(std::time::Duration::from_millis(300));
            let _ = t.set_visible(true);
        }
    });
}

/// Odświeża ikonę, podpowiedź i menu zasobnika według stanu rdzenia.
fn pokaz_stan(app: &AppHandle, stan: &Stan) {
    let (kolor, opis) = if let Some(b) = stan.bledy.first() {
        (ikona::CZERWONY, b.clone())
    } else if let Some(p) = &stan.wznawianie {
        (ikona::ZOLTY, format!("Wznawiam nagrywanie ({p})…"))
    } else if stan.pauza {
        (ikona::SZARY, "Nagrywanie wstrzymane".to_string())
    } else {
        (ikona::ZIELONY, "Nagrywa".to_string())
    };
    if let Some(t) = app.tray_by_id(ID_ZASOBNIKA) {
        let _ = t.set_icon(Some(ikona::ikona(kolor)));
        // Windows przycina podpowiedź do 127 znaków
        let _ = t.set_tooltip(Some(format!("Clipeon — {opis}").chars().take(120).collect::<String>()));
    }
    if let Some(m) = app.try_state::<PozycjeMenu>() {
        let znak = if kolor == ikona::ZIELONY { "●" } else { "○" };
        let _ = m.stan.set_text(format!("{znak} {}", opis.chars().take(70).collect::<String>()));
        let _ = m.pauza.set_text(if stan.pauza { "Wznów nagrywanie" } else { "Wstrzymaj nagrywanie" });
    }
    let _ = app.emit("stan", stan);
}

fn obsluz_zdarzenie(app: &AppHandle, z: Zdarzenie) {
    let cfg = app.try_state::<Rdzen>().map(|r| r.config()).unwrap_or_default();
    match z {
        Zdarzenie::Stan(s) => pokaz_stan(app, &s),
        Zdarzenie::Zapisano(plik) => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj();
            }
            if cfg.powiadomienie_zapisu {
                let nazwa = plik.file_name().unwrap_or_default().to_string_lossy().into_owned();
                pokaz_dymek(app, &cfg.monitor, &format!("Zapisano ostatnie {} s", cfg.bufor_sekundy), &nazwa, false, false);
            }
            let _ = app.emit("klip-zapisany", plik);
            // Po zapisie pilnujemy limitów folderu (jeśli sprzątanie jest włączone)
            std::thread::spawn(move || crate::sprzatanie::sprzataj(&cfg.folder_klipow, cfg.sprzatanie_dni, cfg.sprzatanie_gb));
        }
        // Błąd pokazujemy zawsze — inaczej użytkownik myślałby, że klip jest zapisany
        Zdarzenie::BladZapisu(e) => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj_blad();
            }
            pokaz_dymek(app, &cfg.monitor, "Nie udało się zapisać klipu", &e, true, false);
        }
        // Clipeon nie nagrywa: dymek wisi do czasu naprawy albo kliknięcia (kliknięcie otwiera okno)
        Zdarzenie::Alarm(Some(opis)) => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj_blad();
            }
            pokaz_dymek(app, &cfg.monitor, "Clipeon nie nagrywa", &opis, true, true);
        }
        Zdarzenie::Uwaga(opis) => pokaz_dymek(app, &cfg.monitor, "Nagrywanie działa wolniej", &opis, true, false),
        Zdarzenie::Alarm(None) => {
            crate::powiadomienie::ukryj();
            pokaz_dymek(app, &cfg.monitor, "Nagrywanie znów działa", "Clipeon sam wznowił nagrywanie", false, false);
        }
    }
}

/// Dymek w prawym górnym rogu monitora z grą (nagrywanego) — patrz powiadomienie.rs.
/// `trwaly` = wisi do naprawy problemu, a kliknięcie otwiera okno Clipeona.
fn pokaz_dymek(app: &AppHandle, monitor: &str, tytul: &str, opis: &str, blad: bool, trwaly: bool) {
    let m = app
        .available_monitors()
        .ok()
        .and_then(|l| l.into_iter().find(|m| !monitor.is_empty() && m.name().is_some_and(|n| *n == monitor)))
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = m else { return };
    let miejsce = crate::powiadomienie::Miejsce {
        prawa: m.position().x + m.size().width as i32,
        gora: m.position().y,
        skala: m.scale_factor() as f32,
    };
    let (tytul, opis) = (tytul.to_string(), opis.to_string());
    let klik: Option<Box<dyn FnOnce() + Send>> = if trwaly {
        let app = app.clone();
        Some(Box::new(move || pokaz_okno(&app, "klipy")))
    } else {
        None
    };
    std::thread::spawn(move || crate::powiadomienie::pokaz(miejsce, &tytul, &opis, blad, trwaly, klik));
}

// ---------- Komendy dla okna ----------

#[tauri::command]
fn stan(rdzen: State<'_, Rdzen>) -> Stan {
    rdzen.stan()
}

#[derive(Serialize)]
struct Ustawienia {
    cfg: Config,
    autostart: bool,
    enkodery: Vec<(String, String)>,
    wersja: &'static str,
    repozytorium: &'static str,
}

/// Znalezione nowsze wydanie (None = brak albo jeszcze nie sprawdzono).
#[derive(Default)]
struct Aktualizacja(std::sync::Mutex<Option<crate::aktualizacje::Wydanie>>);

/// Sprawdza aktualizacje i pokazuje wynik w oknie i menu zasobnika.
fn sprawdz_aktualizacje_teraz(app: &AppHandle) -> Result<Option<crate::aktualizacje::Wydanie>, String> {
    let wynik = crate::aktualizacje::sprawdz();
    match &wynik {
        Ok(Some(w)) => {
            crate::dziennik::info(&format!("Dostępna aktualizacja {}", w.wersja));
            *app.state::<Aktualizacja>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(w.clone());
            if let Some(m) = app.try_state::<PozycjeMenu>() {
                let _ = m.aktualizacja.set_text(format!("Dostępna aktualizacja {} — pobierz", w.wersja));
            }
            let _ = app.emit("aktualizacja", w);
        }
        Ok(None) => {}
        Err(e) => crate::dziennik::blad(e),
    }
    wynik
}

/// Asynchronicznie: przy pierwszym wywołaniu wykrywanie enkoderów może potrwać sekundę-dwie.
#[tauri::command]
async fn ustawienia(app: AppHandle) -> Result<Ustawienia, String> {
    tauri::async_runtime::spawn_blocking(move || Ustawienia {
        cfg: app.state::<Rdzen>().config(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        enkodery: crate::enkodery::do_ustawien(),
        wersja: crate::aktualizacje::WERSJA,
        repozytorium: crate::aktualizacje::REPOZYTORIUM,
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn zapisz_ustawienia(app: AppHandle, rdzen: State<'_, Rdzen>, mut cfg: Config, autostart: bool) -> Result<(), String> {
    let folder = cfg.folder_klipow.clone();
    // Flaga „ikona już ustawiona” należy do rdzenia, nie do okna (okno mogło mieć starą wartość)
    let stary = rdzen.config();
    cfg.ikona_ustawiona = stary.ikona_ustawiona;
    let zmiana_ikony = cfg.ikona_przy_zegarze != stary.ikona_przy_zegarze;
    let widoczna = cfg.ikona_przy_zegarze;
    rdzen.zmien_config(cfg)?;
    if zmiana_ikony {
        ustaw_ikone(&app, &rdzen, widoczna);
    }
    pozwol_na_folder(&app, &folder);
    if let Some(m) = app.try_state::<PozycjeMenu>() {
        let _ = m.zapisz.set_text(tekst_zapisu(&rdzen.config()));
    }
    let a = app.autolaunch();
    let wynik = if autostart { a.enable() } else { a.disable() };
    wynik.map_err(|e| format!("Ustawienia zapisane, ale nie udało się zmienić autostartu: {e}"))
}

#[tauri::command]
async fn wybierz_folder(app: AppHandle) -> Option<String> {
    app.dialog().file().set_title("Folder klipów").blocking_pick_folder().map(|f| f.to_string())
}

#[tauri::command]
fn zapisz_klip(rdzen: State<'_, Rdzen>) {
    rdzen.zapisz_klip();
}

#[tauri::command]
fn przelacz_pauze(rdzen: State<'_, Rdzen>) {
    rdzen.przelacz_pauze();
}

#[tauri::command]
fn klipy(rdzen: State<'_, Rdzen>) -> Vec<klipy::Klip> {
    klipy::lista(&rdzen.config().folder_klipow)
}

/// Miniatura dla okna: obrazek jako data URL (bez protokołu asset:, którego zakres dla folderu
/// miniatur działał zawodnie — część plików dostawała 403) i długość klipu.
#[derive(Serialize)]
struct MiniaturaOkna {
    obraz: String,
    dlugosc: f64,
}

#[tauri::command]
async fn miniatura(app: AppHandle, sciezka: PathBuf) -> Result<MiniaturaOkna, String> {
    let folder = app.state::<Rdzen>().config().folder_klipow;
    let klip = klipy::sprawdz_sciezke(&folder, &sciezka)?;
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine as _;
        let m = klipy::miniatura(&klip)?;
        let bajty = std::fs::read(&m.obraz).map_err(|e| format!("Nie mogę odczytać miniatury: {e}"))?;
        let obraz = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bajty));
        Ok(MiniaturaOkna { obraz, dlugosc: m.dlugosc })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn pokaz_w_folderze(rdzen: State<'_, Rdzen>, sciezka: PathBuf) -> Result<(), String> {
    let klip = klipy::sprawdz_sciezke(&rdzen.config().folder_klipow, &sciezka)?;
    klipy::pokaz_w_eksploratorze(&klip, true);
    Ok(())
}

#[tauri::command]
fn usun_klip(rdzen: State<'_, Rdzen>, sciezka: PathBuf) -> Result<(), String> {
    let klip = klipy::sprawdz_sciezke(&rdzen.config().folder_klipow, &sciezka)?;
    klipy::usun(&klip)
}

/// Przycięcie i/lub kompresja klipu. Postęp idzie zdarzeniem "postep" (0..1).
#[tauri::command]
async fn eksportuj(app: AppHandle, sciezka: PathBuf, zlecenie: crate::edycja::Zlecenie) -> Result<PathBuf, String> {
    let cfg = app.state::<Rdzen>().config();
    let klip = klipy::sprawdz_sciezke(&cfg.folder_klipow, &sciezka)?;
    let wynik = tauri::async_runtime::spawn_blocking(move || {
        let postep = |u: f64| {
            let _ = app.emit("postep", u);
        };
        // Wynik edycji zawsze w H.264 — odtworzy go każdy komunikator i przeglądarka
        let enkoder = crate::enkodery::do_eksportu();
        crate::edycja::eksportuj(&klip, &zlecenie, enkoder, &postep)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(wynik)
}

/// Przeciąganie klipu jako prawdziwego pliku do innych aplikacji (Discord, Eksplorator, przeglądarka).
/// Okno wywołuje to na początku przeciągania; dalej prowadzi system (OLE drag & drop).
#[tauri::command]
fn przeciagnij(okno: tauri::WebviewWindow, rdzen: State<'_, Rdzen>, sciezka: PathBuf) -> Result<(), String> {
    let klip = klipy::sprawdz_sciezke(&rdzen.config().folder_klipow, &sciezka)?;
    let obraz = klipy::obraz_przeciagania(&klip)
        .map(drag::Image::File)
        .unwrap_or_else(|| drag::Image::Raw(include_bytes!("../icons/icon.png").to_vec()));
    let (o, app) = (okno.clone(), okno.app_handle().clone());
    okno.run_on_main_thread(move || {
        let wynik = drag::start_drag(&o, drag::DragItem::Files(vec![klip]), obraz, move |wynik, _| {
            let opis = match wynik {
                drag::DragResult::Dropped => "upuszczono",
                drag::DragResult::Cancel => "anulowano",
            };
            println!("Przeciąganie klipu: {opis}");
            let _ = app.emit("przeciaganie-koniec", opis);
        }, Default::default());
        if let Err(e) = wynik {
            eprintln!("Nie udało się rozpocząć przeciągania: {e}");
        }
    })
    .map_err(|e| e.to_string())
}

/// Długość i liczba ścieżek dźwięku (do suwaków głośności w edytorze).
#[tauri::command]
async fn info_klipu(app: AppHandle, sciezka: PathBuf) -> Result<klipy::Info, String> {
    let folder = app.state::<Rdzen>().config().folder_klipow;
    let klip = klipy::sprawdz_sciezke(&folder, &sciezka)?;
    tauri::async_runtime::spawn_blocking(move || klipy::info(&klip)).await.map_err(|e| e.to_string())?
}

/// Migracja z wersji 1.0 (nazwa „Clipper”): stary wpis autostartu wskazuje na stary program —
/// usuwamy go i, jeśli był włączony, włączamy autostart Clipeona.
fn przenies_autostart_z_clippera(app: &AppHandle) {
    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let Ok(klucz) = windows_registry::CURRENT_USER.options().read().write().open(RUN) else { return };
    if klucz.get_string("Clipper").is_err() {
        return;
    }
    let _ = klucz.remove_value("Clipper");
    match app.autolaunch().enable() {
        Ok(()) => crate::dziennik::info("Przeniesiono autostart z Clippera 1.0 na Clipeona"),
        Err(e) => crate::dziennik::blad(&format!("Nie udało się przenieść autostartu: {e}")),
    }
}

/// Monitory do wyboru w ustawieniach.
#[tauri::command]
fn monitory() -> Vec<(String, String)> {
    crate::wideo::monitory()
}

/// Znalezione nowsze wydanie (okno pyta o nie przy otwarciu).
#[tauri::command]
fn aktualizacja(stan: State<'_, Aktualizacja>) -> Option<crate::aktualizacje::Wydanie> {
    stan.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Ręczne sprawdzenie z ustawień.
#[tauri::command]
async fn sprawdz_aktualizacje(app: AppHandle) -> Result<Option<crate::aktualizacje::Wydanie>, String> {
    tauri::async_runtime::spawn_blocking(move || sprawdz_aktualizacje_teraz(&app)).await.map_err(|e| e.to_string())?
}

/// Otwiera stronę nowego wydania (albo listę wydań) w przeglądarce.
#[tauri::command]
fn otworz_aktualizacje(stan: State<'_, Aktualizacja>) {
    let strona = stan.0.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|w| w.strona.clone());
    crate::aktualizacje::otworz(&strona.unwrap_or_else(|| format!("{}/releases", crate::aktualizacje::REPOZYTORIUM)));
}

#[tauri::command]
fn otworz_repozytorium() {
    crate::aktualizacje::otworz(crate::aktualizacje::REPOZYTORIUM);
}

/// Uruchamia nagrywanie od nowa (przycisk „Spróbuj ponownie”).
#[tauri::command]
fn uruchom_ponownie(rdzen: State<'_, Rdzen>) {
    crate::dziennik::info("Ponowne uruchomienie nagrywania na prośbę użytkownika");
    rdzen.uruchom();
}

/// Pokazuje plik dziennika w Eksploratorze.
#[tauri::command]
fn otworz_log() {
    let plik = crate::dziennik::plik();
    if plik.is_file() {
        klipy::pokaz_w_eksploratorze(&plik, true);
    } else {
        let _ = std::fs::create_dir_all(crate::dziennik::folder());
        klipy::pokaz_w_eksploratorze(&crate::dziennik::folder(), false);
    }
}

#[tauri::command]
fn otworz_folder(rdzen: State<'_, Rdzen>) {
    let folder = rdzen.config().folder_klipow;
    let _ = std::fs::create_dir_all(&folder);
    klipy::pokaz_w_eksploratorze(&folder, false);
}

// ---------- Start ----------

pub fn uruchom() {
    crate::dziennik::przechwytuj_awarie();
    crate::dziennik::info("Start Clipeona");
    tauri::Builder::default()
        // Druga kopia programu tylko pokazuje okno pierwszej (skrót i enkoder są jedne)
        .plugin(tauri_plugin_single_instance::init(|app, _, _| pokaz_okno(app, "klipy")))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            stan,
            ustawienia,
            zapisz_ustawienia,
            wybierz_folder,
            zapisz_klip,
            przelacz_pauze,
            klipy,
            miniatura,
            pokaz_w_folderze,
            usun_klip,
            otworz_folder,
            eksportuj,
            przeciagnij,
            info_klipu,
            monitory,
            uruchom_ponownie,
            otworz_log,
            aktualizacja,
            sprawdz_aktualizacje,
            otworz_aktualizacje,
            otworz_repozytorium
        ])
        .setup(|app| {
            let (cfg, blad_cfg) = match Config::wczytaj() {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(e)),
            };

            // Menu zasobnika
            let stan = MenuItem::with_id(app, "stan", "○ Uruchamiam nagrywanie…", false, None::<&str>)?;
            let zapisz = MenuItem::with_id(app, "zapisz", tekst_zapisu(&cfg), true, None::<&str>)?;
            let log = MenuItem::with_id(app, "log", "Pokaż dziennik zdarzeń", true, None::<&str>)?;
            let aktualizacja = MenuItem::with_id(app, "aktualizacja", "Sprawdź aktualizacje", true, None::<&str>)?;
            let pauza = MenuItem::with_id(app, "pauza", "Wstrzymaj nagrywanie", true, None::<&str>)?;
            let folder = MenuItem::with_id(app, "folder", "Otwórz folder klipów", true, None::<&str>)?;
            let biblioteka = MenuItem::with_id(app, "biblioteka", "Biblioteka…", true, None::<&str>)?;
            let ustawienia = MenuItem::with_id(app, "ustawienia", "Ustawienia…", true, None::<&str>)?;
            let zamknij = MenuItem::with_id(app, "zamknij", "Zamknij", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let menu = Menu::with_items(
                app,
                &[&stan, &sep()?, &zapisz, &pauza, &sep()?, &folder, &biblioteka, &ustawienia, &log, &aktualizacja, &sep()?, &zamknij],
            )?;
            app.manage(PozycjeMenu { stan, aktualizacja, zapisz, pauza });
            app.manage(Aktualizacja::default());

            TrayIconBuilder::with_id(ID_ZASOBNIKA)
                .icon(ikona::ikona(ikona::ZIELONY))
                .tooltip("Clipeon")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| {
                    let rdzen = app.state::<Rdzen>();
                    match e.id().as_ref() {
                        "zapisz" => rdzen.zapisz_klip(),
                        "pauza" => rdzen.przelacz_pauze(),
                        "folder" => klipy::pokaz_w_eksploratorze(&rdzen.config().folder_klipow, false),
                        "biblioteka" => pokaz_okno(app, "klipy"),
                        "ustawienia" => pokaz_okno(app, "ustawienia"),
                        "log" => otworz_log(),
                        "aktualizacja" => {
                            let znana = app.state::<Aktualizacja>().0.lock().unwrap_or_else(|e| e.into_inner()).is_some();
                            if znana {
                                otworz_aktualizacje(app.state::<Aktualizacja>());
                            } else {
                                // Ręczne sprawdzenie; wynik pokazujemy dymkiem
                                let app = app.clone();
                                std::thread::spawn(move || {
                                    let monitor = app.state::<Rdzen>().config().monitor;
                                    let (tytul, opis) = match sprawdz_aktualizacje_teraz(&app) {
                                        Ok(Some(w)) => (format!("Dostępna wersja {}", w.wersja), "Kliknij w menu ikony, żeby ją pobrać".to_string()),
                                        Ok(None) => ("Masz najnowszą wersję".to_string(), format!("Clipeon {}", crate::aktualizacje::WERSJA)),
                                        Err(e) => ("Nie udało się sprawdzić".to_string(), e),
                                    };
                                    pokaz_dymek(&app, &monitor, &tytul, &opis, false, false);
                                });
                            }
                        }
                        "zamknij" => app.exit(0),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        pokaz_okno(tray.app_handle(), "klipy");
                    }
                })
                .build(app)?;

            // Okno może czytać klipy i miniatury przez protokół asset:
            pozwol_na_folder(app.handle(), &cfg.folder_klipow);

            let uchwyt = app.handle().clone();
            let rdzen = Rdzen::nowy(cfg, move |z| obsluz_zdarzenie(&uchwyt, z));
            app.manage(rdzen.clone());
            rdzen.uruchom();
            rdzen.uruchom_straznika();
            std::thread::spawn(klipy::usun_stara_pamiec_podreczna);
            // Pełna lista enkoderów do ustawień — w tle, po starcie nagrywania
            std::thread::spawn(crate::enkodery::dostepne);

            // Aktualizacje: chwilę po starcie, potem raz na dobę (jeśli włączone w ustawieniach)
            let h = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                loop {
                    if h.state::<Rdzen>().config().sprawdzaj_aktualizacje {
                        let _ = sprawdz_aktualizacje_teraz(&h);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(24 * 3600));
                }
            });
            if crate::config::PRZENIESIONO.load(std::sync::atomic::Ordering::SeqCst) {
                przenies_autostart_z_clippera(app.handle());
            }
            // Raz (przy pierwszym starcie) wyciągamy ikonę z ukrytych przy zegarze; jeśli użytkownik
            // potem schowa ją w Windows, już tego nie zmieniamy
            let c = rdzen.config();
            if c.ikona_przy_zegarze && !c.ikona_ustawiona {
                ustaw_ikone(app.handle(), &rdzen, true);
            }
            // Sprzątanie folderu przy starcie i potem co godzinę (jeśli włączone w ustawieniach)
            let r = rdzen.clone();
            std::thread::spawn(move || loop {
                let c = r.config();
                crate::sprzatanie::sprzataj(&c.folder_klipow, c.sprzatanie_dni, c.sprzatanie_gb);
                std::thread::sleep(std::time::Duration::from_secs(3600));
            });
            if let Some(e) = blad_cfg {
                rdzen.blad_configu(e);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Nie udało się uruchomić Clipeona")
        .run(|_, e| {
            // Zamknięcie okna nie kończy programu — nagrywanie działa dalej w zasobniku
            if let RunEvent::ExitRequested { api, code: None, .. } = e {
                api.prevent_exit();
            }
        });
}
