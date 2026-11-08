// Warstwa Tauri: ikona w zasobniku z menu, okno (Klipy / Ustawienia) tworzone na żądanie
// i niszczone po zamknięciu (w tle zostaje tylko rdzeń nagrywania), komendy dla UI, powiadomienia.
use crate::config::{Config, ENKODERY};
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
use tauri_plugin_notification::NotificationExt;

const ID_ZASOBNIKA: &str = "glowna";

/// Pozycje menu, których tekst się zmienia.
struct PozycjeMenu {
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
            .title("Clipper")
            .inner_size(1120.0, 740.0)
            .min_inner_size(760.0, 520.0)
            .background_color(Color(0, 0, 0, 255))
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

/// Ustawia widoczność ikony przy zegarze i zapamiętuje, że Clipper już to zrobił.
fn ustaw_ikone(rdzen: &Rdzen, widoczna: bool) {
    let r = rdzen.clone();
    crate::zasobnik::ustaw_w_tle(widoczna, move || r.zmien_bez_restartu(|c| c.ikona_ustawiona = true));
}

/// Odświeża ikonę, podpowiedź i menu zasobnika według stanu rdzenia.
fn pokaz_stan(app: &AppHandle, stan: &Stan) {
    let (kolor, opis) = if let Some(b) = stan.bledy.first() {
        (ikona::CZERWONY, format!("Clipper — błąd: {b}"))
    } else if stan.pauza {
        (ikona::SZARY, "Clipper — bufor wstrzymany".to_string())
    } else {
        (ikona::ZIELONY, "Clipper — nagrywa".to_string())
    };
    if let Some(t) = app.tray_by_id(ID_ZASOBNIKA) {
        let _ = t.set_icon(Some(ikona::ikona(kolor)));
        // Windows przycina podpowiedź do 127 znaków
        let _ = t.set_tooltip(Some(opis.chars().take(120).collect::<String>()));
    }
    if let Some(m) = app.try_state::<PozycjeMenu>() {
        let _ = m.pauza.set_text(if stan.pauza { "Wznów bufor" } else { "Wstrzymaj bufor" });
    }
    let _ = app.emit("stan", stan);
}

fn obsluz_zdarzenie(app: &AppHandle, z: Zdarzenie) {
    match z {
        Zdarzenie::Stan(s) => pokaz_stan(app, &s),
        Zdarzenie::Zapisano(plik) => {
            let nazwa = plik.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let _ = app.notification().builder().title("Klip zapisany").body(&nazwa).show();
            let _ = app.emit("klip-zapisany", plik);
        }
        Zdarzenie::BladZapisu(e) => {
            let _ = app.notification().builder().title("Nie udało się zapisać klipu").body(&e).show();
        }
    }
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
}

#[tauri::command]
fn ustawienia(app: AppHandle, rdzen: State<'_, Rdzen>) -> Ustawienia {
    Ustawienia {
        cfg: rdzen.config(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        enkodery: ENKODERY.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
    }
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
        ustaw_ikone(&rdzen, widoczna);
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

#[tauri::command]
async fn miniatura(app: AppHandle, sciezka: PathBuf) -> Result<klipy::Miniatura, String> {
    let folder = app.state::<Rdzen>().config().folder_klipow;
    let klip = klipy::sprawdz_sciezke(&folder, &sciezka)?;
    tauri::async_runtime::spawn_blocking(move || klipy::miniatura(&klip)).await.map_err(|e| e.to_string())?
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
        crate::edycja::eksportuj(&klip, &zlecenie, &cfg.enkoder, &postep)
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
    let o = okno.clone();
    okno.run_on_main_thread(move || {
        let wynik = drag::start_drag(&o, drag::DragItem::Files(vec![klip]), obraz, |wynik, _| {
            let opis = match wynik {
                drag::DragResult::Dropped => "upuszczono",
                drag::DragResult::Cancel => "anulowano",
            };
            println!("Przeciąganie klipu: {opis}");
        }, Default::default());
        if let Err(e) = wynik {
            eprintln!("Nie udało się rozpocząć przeciągania: {e}");
        }
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn otworz_folder(rdzen: State<'_, Rdzen>) {
    let folder = rdzen.config().folder_klipow;
    let _ = std::fs::create_dir_all(&folder);
    klipy::pokaz_w_eksploratorze(&folder, false);
}

// ---------- Start ----------

pub fn uruchom() {
    tauri::Builder::default()
        // Druga kopia programu tylko pokazuje okno pierwszej (skrót i enkoder są jedne)
        .plugin(tauri_plugin_single_instance::init(|app, _, _| pokaz_okno(app, "klipy")))
        .plugin(tauri_plugin_notification::init())
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
            przeciagnij
        ])
        .setup(|app| {
            let (cfg, blad_cfg) = match Config::wczytaj() {
                Ok(c) => (c, None),
                Err(e) => (Config::default(), Some(e)),
            };

            // Menu zasobnika
            let zapisz = MenuItem::with_id(app, "zapisz", tekst_zapisu(&cfg), true, None::<&str>)?;
            let pauza = MenuItem::with_id(app, "pauza", "Wstrzymaj bufor", true, None::<&str>)?;
            let folder = MenuItem::with_id(app, "folder", "Otwórz folder klipów", true, None::<&str>)?;
            let biblioteka = MenuItem::with_id(app, "biblioteka", "Biblioteka…", true, None::<&str>)?;
            let ustawienia = MenuItem::with_id(app, "ustawienia", "Ustawienia…", true, None::<&str>)?;
            let zamknij = MenuItem::with_id(app, "zamknij", "Zamknij", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let menu = Menu::with_items(
                app,
                &[&zapisz, &pauza, &sep()?, &folder, &biblioteka, &ustawienia, &sep()?, &zamknij],
            )?;
            app.manage(PozycjeMenu { zapisz, pauza });

            TrayIconBuilder::with_id(ID_ZASOBNIKA)
                .icon(ikona::ikona(ikona::ZIELONY))
                .tooltip("Clipper")
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
            pozwol_na_folder(app.handle(), &klipy::folder_miniatur());

            let uchwyt = app.handle().clone();
            let rdzen = Rdzen::nowy(cfg, move |z| obsluz_zdarzenie(&uchwyt, z));
            app.manage(rdzen.clone());
            rdzen.uruchom();
            // Raz (przy pierwszym starcie) wyciągamy ikonę z ukrytych przy zegarze; jeśli użytkownik
            // potem schowa ją w Windows, już tego nie zmieniamy
            let c = rdzen.config();
            if c.ikona_przy_zegarze && !c.ikona_ustawiona {
                ustaw_ikone(&rdzen, true);
            }
            if let Some(e) = blad_cfg {
                rdzen.blad_configu(e);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Nie udało się uruchomić Clippera")
        .run(|_, e| {
            // Zamknięcie okna nie kończy programu — nagrywanie działa dalej w zasobniku
            if let RunEvent::ExitRequested { api, code: None, .. } = e {
                api.prevent_exit();
            }
        });
}
