// Warstwa Tauri: ikona w zasobniku z menu, okno (Klipy / Ustawienia) tworzone na żądanie
// i niszczone po zamknięciu (w tle zostaje tylko rdzeń nagrywania), komendy dla UI, powiadomienia.
use crate::Zablokuj;
use crate::config::Config;
use crate::ikona;
use crate::klipy;
use crate::rdzen::{Rdzen, Stan, Zdarzenie};
use serde::Serialize;
use std::path::PathBuf;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
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
    film: MenuItem<Wry>,
    tryb_gra: CheckMenuItem<Wry>,
    tryb_pulpit: CheckMenuItem<Wry>,
}

/// Pozwala oknu czytać pliki z folderu przez protokół asset:. Tauri porównuje ścieżki kanoniczne
/// (z przedrostkiem \\?\), więc folder musi istnieć i być podany w tej samej postaci.
fn pozwol_na_folder(app: &AppHandle, folder: &std::path::Path) {
    let _ = std::fs::create_dir_all(folder);
    let kanoniczny = folder.canonicalize().unwrap_or_else(|_| folder.to_path_buf());
    // Rekurencyjnie: klipy leżą w podfolderach miesięcy (2026-11), a starsze prosto w folderze
    let _ = app.asset_protocol_scope().allow_directory(kanoniczny, true);
}

fn tekst_zapisu(cfg: &Config) -> String {
    format!("Zapisz ostatnie {} s\t{}", cfg.bufor_sekundy, cfg.skrot)
}

fn tekst_filmu(cfg: &Config, trwa: bool) -> String {
    format!("{}\t{}", if trwa { "Zakończ film" } else { "Nagraj film" }, cfg.skrot_filmu)
}

/// Długość filmu słowami: „45 s”, „12 min 5 s”, „1 godz. 3 min”.
fn dlugosc_slowami(sekundy: f64) -> String {
    let s = sekundy.round() as u64;
    match (s / 3600, s % 3600 / 60, s % 60) {
        (0, 0, s) => format!("{s} s"),
        (0, m, s) => format!("{m} min {s} s"),
        (g, m, _) => format!("{g} godz. {m} min"),
    }
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
            .inner_size(1200.0, 760.0)
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
            Err(e) => crate::dziennik::blad(&format!("Nie udało się otworzyć okna: {e}")),
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
    } else if let Some(p) = &stan.przelaczanie {
        (ikona::ZOLTY, format!("Przełączam {p} na nowe urządzenie…"))
    } else if stan.pauza {
        (ikona::SZARY, "Nagrywanie wstrzymane".to_string())
    } else if stan.czeka_na_gre {
        (ikona::SZARY, format!("Czekam na {}", stan.wybrana_gra))
    } else if stan.tryb == crate::config::TRYB_PULPIT {
        (ikona::ZIELONY, format!("Nagrywa pulpit ({})", stan.ekran))
    } else {
        // Przy rozpoznanej grze widać, co dokładnie jest nagrywane
        match &stan.gra {
            Some(g) => (ikona::ZIELONY, format!("Nagrywa: {g}")),
            None => (ikona::ZIELONY, "Nagrywa".to_string()),
        }
    };
    // Trwający film widać w podpowiedzi ikony
    let opis = match stan.film {
        Some(s) if kolor == ikona::ZIELONY => format!("{opis} · film {}", dlugosc_slowami(s as f64)),
        _ => opis,
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
        if let Some(r) = app.try_state::<Rdzen>() {
            let _ = m.film.set_text(tekst_filmu(&r.config(), stan.film.is_some()));
        }
        let _ = m.film.set_enabled(!stan.film_zapis);
        let pulpit = stan.tryb == crate::config::TRYB_PULPIT;
        let _ = m.tryb_gra.set_checked(!pulpit);
        let _ = m.tryb_pulpit.set_checked(pulpit);
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
                pokaz_dymek(app, &format!("Zapisano ostatnie {} s", cfg.bufor_sekundy), &nazwa, false, false);
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
            pokaz_dymek(app, "Nie udało się zapisać klipu", &e, true, false);
        }
        // Clipeon nie nagrywa: dymek wisi do czasu naprawy albo kliknięcia (kliknięcie otwiera okno)
        Zdarzenie::Alarm(Some(opis)) => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj_blad();
            }
            pokaz_dymek(app, "Clipeon nie nagrywa", &opis, true, true);
        }
        Zdarzenie::Uwaga { tytul, opis } => pokaz_dymek(app, &tytul, &opis, true, false),
        Zdarzenie::Alarm(None) => {
            crate::powiadomienie::ukryj();
            pokaz_dymek(app, "Nagrywanie znów działa", "Clipeon sam wznowił nagrywanie", false, false);
        }
        // Film: czerwona kropka = nagrywa się
        Zdarzenie::FilmStart => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj();
            }
            let mut opis = if cfg.skrot_filmu.is_empty() {
                "Zakończysz go w oknie Clipeona albo w menu przy zegarze".to_string()
            } else {
                format!("{} kończy nagrywanie", cfg.skrot_filmu)
            };
            if !cfg.skrot_znacznika.is_empty() {
                opis.push_str(&format!(", {} dodaje znacznik", cfg.skrot_znacznika));
            }
            pokaz_dymek(app, "Nagrywam film", &opis, true, false);
        }
        Zdarzenie::FilmKoniec(powod) => {
            let (tytul, opis) = match powod {
                Some(p) => ("Film zakończony".to_string(), format!("Powód: {p}. Zapisuję plik…")),
                None => ("Koniec filmu".to_string(), "Zapisuję plik…".to_string()),
            };
            pokaz_dymek(app, &tytul, &opis, false, false);
        }
        Zdarzenie::FilmZapisany { plik, sekundy, czesci } => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj();
            }
            let nazwa = plik.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let opis = if czesci > 1 { format!("{nazwa} (i {} kolejne części)", czesci - 1) } else { nazwa };
            pokaz_dymek(app, &format!("Film zapisany ({})", dlugosc_slowami(sekundy)), &opis, false, false);
            let _ = app.emit("film-zapisany", plik);
        }
        Zdarzenie::FilmOdzyskany { plik, sekundy, czesci } => {
            let nazwa = plik.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let opis = if czesci > 1 { format!("{nazwa} (i {} kolejne części)", czesci - 1) } else { nazwa };
            pokaz_dymek(app, &format!("Odzyskano przerwany film ({})", dlugosc_slowami(sekundy)), &opis, false, true);
            let _ = app.emit("film-zapisany", plik);
        }
        Zdarzenie::Znacznik(Some((nr, sekundy))) => {
            pokaz_dymek(app, &format!("Znacznik {nr}"), &format!("{}:{:02} filmu — skoczysz do niego w edytorze", sekundy / 60, sekundy % 60), false, false);
        }
        Zdarzenie::Znacznik(None) => {
            let opis = if cfg.skrot_filmu.is_empty() {
                "Znaczniki działają w trakcie filmu — zacznij go w oknie Clipeona".to_string()
            } else {
                format!("Znaczniki działają w trakcie filmu — zacznij go skrótem {}", cfg.skrot_filmu)
            };
            pokaz_dymek(app, "Nie nagrywasz filmu", &opis, false, false);
        }
        Zdarzenie::FilmZmniejszony { plik, przed, po } => {
            let gb = |b: u64| format!("{:.1} GB", b as f64 / 1e9).replace('.', ",");
            let nazwa = plik.file_name().unwrap_or_default().to_string_lossy().into_owned();
            pokaz_dymek(app, &format!("Film zmniejszony: {} → {}", gb(przed), gb(po)), &nazwa, false, false);
            let _ = app.emit("film-zmniejszony", plik);
        }
        Zdarzenie::FilmNiezmniejszony(e) => {
            pokaz_dymek(app, "Film został w pełnej jakości", &format!("Nie udało się go zmniejszyć: {e}"), true, false);
            let _ = app.emit("film-blad", format!("Film został w pełnej jakości: {e}"));
        }
        Zdarzenie::BladFilmu(e) => {
            if cfg.dzwiek_zapisu {
                crate::dzwiek::zagraj_blad();
            }
            pokaz_dymek(app, "Nie udało się nagrać filmu", &e, true, false);
            let _ = app.emit("film-blad", e);
        }
    }
}

/// Dymek w prawym górnym rogu nagrywanego monitora (w trybie Gra i Pulpit) — patrz powiadomienie.rs.
/// `trwaly` = wisi do naprawy problemu, a kliknięcie otwiera okno Clipeona.
fn pokaz_dymek(app: &AppHandle, tytul: &str, opis: &str, blad: bool, trwaly: bool) {
    let klik: Option<Box<dyn FnOnce() + Send>> = if trwaly {
        let app = app.clone();
        Some(Box::new(move || pokaz_okno(&app, "klipy")))
    } else {
        None
    };
    pokaz_dymek_z_klikiem(app, tytul, opis, blad, trwaly, klik);
}

/// Dymek z własną akcją po kliknięciu (`klik` = None: kliknięcia przechodzą do gry).
fn pokaz_dymek_z_klikiem(app: &AppHandle, tytul: &str, opis: &str, blad: bool, trwaly: bool, klik: Option<Box<dyn FnOnce() + Send>>) {
    let cfg = app.try_state::<Rdzen>().map(|r| r.config()).unwrap_or_default();
    let monitor = cfg.aktywny_monitor().0;
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
    /// Formaty filmu do wyboru: (wartość, opis) — mniejsze tylko, gdy karta je umie
    formaty_filmu: Vec<(String, String)>,
    wersja: &'static str,
    repozytorium: &'static str,
}

/// Sprawdza aktualizacje i pokazuje wynik w oknie i menu zasobnika. `w_tle` = sprawdzenie
/// automatyczne: wtedy o nowej wersji mówi też dymek (raz na wersję).
async fn sprawdz_aktualizacje_teraz(app: &AppHandle, w_tle: bool) -> Result<Option<crate::aktualizacje::Wydanie>, String> {
    let h = app.clone();
    let wynik = crate::aktualizacje::sprawdz(app, move || przed_instalacja(&h)).await;
    match &wynik {
        Ok(Some(w)) => {
            crate::dziennik::info(&format!("Dostępna aktualizacja {}", w.wersja));
            if let Some(m) = app.try_state::<PozycjeMenu>() {
                let _ = m.aktualizacja.set_text(format!("Aktualizuj do wersji {}", w.wersja));
            }
            let _ = app.emit("aktualizacja", w);
            static OGLOSZONA: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
            let mut ogloszona = OGLOSZONA.zablokuj();
            if w_tle && *ogloszona != w.wersja {
                *ogloszona = w.wersja.clone();
                let a = app.clone();
                pokaz_dymek_z_klikiem(
                    app,
                    &format!("Nowa wersja Clipeona: {}", w.wersja),
                    "Kliknij, żeby otworzyć Clipeona i zaktualizować",
                    false,
                    false,
                    Some(Box::new(move || pokaz_okno(&a, "klipy"))),
                );
            }
        }
        Ok(None) => {
            if let Some(m) = app.try_state::<PozycjeMenu>() {
                let _ = m.aktualizacja.set_text("Sprawdź aktualizacje");
            }
            let _ = app.emit("aktualizacja", None::<crate::aktualizacje::Wydanie>);
        }
        Err(e) => crate::dziennik::blad(e),
    }
    wynik
}

/// Pobiera i instaluje znalezioną wersję; menu zasobnika pokazuje, co się dzieje.
async fn zainstaluj_aktualizacje_teraz(app: &AppHandle) -> Result<(), String> {
    if let Some(m) = app.try_state::<PozycjeMenu>() {
        let _ = m.aktualizacja.set_text("Pobieranie aktualizacji…");
        let _ = m.aktualizacja.set_enabled(false);
    }
    let wynik = crate::aktualizacje::zainstaluj(app).await;
    // Tu dochodzimy tylko po błędzie (udana instalacja zamyka program)
    if let Err(e) = &wynik {
        crate::dziennik::blad(e);
        if let Some(m) = app.try_state::<PozycjeMenu>() {
            let tekst = match app.state::<crate::aktualizacje::Stan>().znaleziona() {
                Some(w) => format!("Aktualizuj do wersji {}", w.wersja),
                None => "Sprawdź aktualizacje".into(),
            };
            let _ = m.aktualizacja.set_text(tekst);
            let _ = m.aktualizacja.set_enabled(true);
        }
        pokaz_dymek(app, "Aktualizacja się nie udała", e, true, false);
    }
    wynik
}

/// Tuż przed uruchomieniem instalatora: zatrzymujemy nagrywanie, żeby ffmpeg zwolnił swoje pliki.
fn przed_instalacja(app: &AppHandle) {
    crate::dziennik::info("Zamykanie Clipeona na czas instalacji aktualizacji");
    crate::odzyskiwanie::przed_aktualizacja();
    if let Some(r) = app.try_state::<Rdzen>() {
        r.zakoncz();
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
}

/// Asynchronicznie: przy pierwszym wywołaniu wykrywanie enkoderów może potrwać sekundę-dwie.
#[tauri::command]
async fn ustawienia(app: AppHandle) -> Result<Ustawienia, String> {
    tauri::async_runtime::spawn_blocking(move || Ustawienia {
        cfg: app.state::<Rdzen>().config(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        enkodery: crate::enkodery::do_ustawien(),
        formaty_filmu: crate::film::formaty(),
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
    // Tryb i grę wybiera się na pasku / w zasobniku (okno ustawień mogło mieć stare)
    cfg.tryb = stary.tryb.clone();
    cfg.gra = stary.gra.clone();
    cfg.gra_nazwa = stary.gra_nazwa.clone();
    rdzen.zmien_config(cfg)?;
    if zmiana_ikony {
        ustaw_ikone(&app, &rdzen, widoczna);
    }
    pozwol_na_folder(&app, &folder);
    if let Some(m) = app.try_state::<PozycjeMenu>() {
        let cfg = rdzen.config();
        let _ = m.zapisz.set_text(tekst_zapisu(&cfg));
        let _ = m.film.set_text(tekst_filmu(&cfg, false));
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

/// Zaczyna albo kończy długie nagrywanie (film).
#[tauri::command]
async fn przelacz_film(app: AppHandle) {
    // W tle: start zakłada pliki na dysku, a okno ma reagować
    let _ = tauri::async_runtime::spawn_blocking(move || app.state::<Rdzen>().przelacz_film()).await;
}

/// Okno ustawia nowy skrót: na ten czas skróty Clipeona nie działają (true), potem wracają (false).
#[tauri::command]
async fn wstrzymaj_skroty(app: AppHandle, wstrzymaj: bool) {
    let _ = tauri::async_runtime::spawn_blocking(move || app.state::<Rdzen>().wstrzymaj_skroty(wstrzymaj)).await;
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
    let klip = klipy::sprawdz_wynik(&rdzen.config().folder_klipow, &sciezka)?;
    klipy::pokaz_w_eksploratorze(&klip, true);
    Ok(())
}

/// Klip do schowka jako plik: Ctrl+V w Discordzie (albo w Eksploratorze) wkleja sam plik.
#[tauri::command]
fn kopiuj_klip(rdzen: State<'_, Rdzen>, sciezka: PathBuf) -> Result<(), String> {
    let klip = klipy::sprawdz_wynik(&rdzen.config().folder_klipow, &sciezka)?;
    crate::schowek::kopiuj_plik(&klip)
}

#[tauri::command]
fn usun_klip(rdzen: State<'_, Rdzen>, sciezka: PathBuf) -> Result<(), String> {
    let klip = klipy::sprawdz_sciezke(&rdzen.config().folder_klipow, &sciezka)?;
    klipy::usun(&klip)
}

/// Przycięcie i/lub kompresja klipu. Postęp idzie zdarzeniem "postep" (0..1).
#[tauri::command]
async fn eksportuj(app: AppHandle, sciezka: PathBuf, zlecenie: crate::edycja::Zlecenie) -> Result<crate::edycja::Wynik, String> {
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
    let klip = klipy::sprawdz_wynik(&rdzen.config().folder_klipow, &sciezka)?;
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
            crate::dziennik::blad(&format!("Nie udało się rozpocząć przeciągania: {e}"));
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

/// Jedna ścieżka dźwięku klipu (plik M4A w bajtach) do podglądu głośności na żywo w edytorze.
/// Bajty zamiast adresu `asset:`: okno robi z nich adres `blob:` z tego samego źródła, więc Web Audio
/// może zmieniać ich głośność (z adresu z innego źródła dostałby ciszę).
#[tauri::command]
async fn dzwiek_podgladu(app: AppHandle, sciezka: PathBuf, nr: u32) -> Result<tauri::ipc::Response, String> {
    let folder = app.state::<Rdzen>().config().folder_klipow;
    let klip = klipy::sprawdz_sciezke(&folder, &sciezka)?;
    let dane = tauri::async_runtime::spawn_blocking(move || klipy::dzwiek_podgladu(&klip, nr))
        .await
        .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(dane))
}

/// Wykres głośności jednej ścieżki dźwięku (wartości 0–255) do ścieżek dźwięku w edytorze.
#[tauri::command]
async fn fala_dzwieku(app: AppHandle, sciezka: PathBuf, nr: u32, kubelki: usize) -> Result<Vec<u8>, String> {
    let folder = app.state::<Rdzen>().config().folder_klipow;
    let klip = klipy::sprawdz_sciezke(&folder, &sciezka)?;
    tauri::async_runtime::spawn_blocking(move || klipy::fala(&klip, nr, kubelki)).await.map_err(|e| e.to_string())?
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

/// Podłączone mikrofony do wyboru w ustawieniach.
#[tauri::command]
async fn mikrofony() -> Vec<crate::audio::Mikrofon> {
    tauri::async_runtime::spawn_blocking(crate::audio::mikrofony).await.unwrap_or_default()
}

/// Programy, które można wybrać na pasku jako nagrywaną grę (plus ta wybrana wcześniej, gdy teraz nie działa).
#[tauri::command]
fn gry(rdzen: State<'_, Rdzen>) -> Vec<crate::gra::Gra> {
    let cfg = rdzen.config();
    let mut lista = crate::gra::lista();
    if !cfg.gra.is_empty() && !lista.iter().any(|g| crate::gra::ten_sam(&g.proces, &cfg.gra)) {
        lista.insert(
            0,
            crate::gra::Gra {
                nazwa: if cfg.gra_nazwa.is_empty() { cfg.gra.clone() } else { cfg.gra_nazwa.clone() },
                proces: cfg.gra,
            },
        );
    }
    lista
}

/// Monitory do wyboru w ustawieniach i na pasku (z rozdzielczością, odświeżaniem i HDR).
#[tauri::command]
fn monitory() -> Vec<crate::wideo::InfoMonitora> {
    crate::wideo::monitory()
}

/// Przełącznik „Gra | Pulpit” i wybór ekranu na pasku (None = bez zmiany).
#[tauri::command]
fn ustaw_tryb(rdzen: State<'_, Rdzen>, tryb: Option<String>, monitor: Option<String>) -> Result<(), String> {
    rdzen.ustaw_tryb(tryb, monitor)
}

/// Wybór gry na pasku (pusty proces = automatycznie, co jest na wierzchu).
#[tauri::command]
async fn ustaw_gre(app: AppHandle, proces: String, nazwa: String) {
    // W tle: start nagrywania po wyborze może chwilę trwać, a okno ma reagować
    let _ = tauri::async_runtime::spawn_blocking(move || app.state::<Rdzen>().ustaw_gre(proces, nazwa)).await;
}

/// Tekst „Kopiuj diagnostykę” do zgłoszenia błędu (bez ścieżek do klipów i bez danych z sieci).
#[tauri::command]
fn diagnostyka(rdzen: State<'_, Rdzen>) -> String {
    rdzen.diagnostyka()
}

/// Znalezione nowsze wydanie (okno pyta o nie przy otwarciu).
#[tauri::command]
fn aktualizacja(stan: State<'_, crate::aktualizacje::Stan>) -> Option<crate::aktualizacje::Wydanie> {
    stan.znaleziona()
}

/// Ręczne sprawdzenie z ustawień.
#[tauri::command]
async fn sprawdz_aktualizacje(app: AppHandle) -> Result<Option<crate::aktualizacje::Wydanie>, String> {
    sprawdz_aktualizacje_teraz(&app, false).await
}

/// Przycisk „Aktualizuj”: pobranie, sprawdzenie podpisu, instalacja i ponowne uruchomienie.
#[tauri::command]
async fn zainstaluj_aktualizacje(app: AppHandle) -> Result<(), String> {
    zainstaluj_aktualizacje_teraz(&app).await
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
    let po_awarii = crate::odzyskiwanie::przy_starcie();
    crate::dziennik::info(&format!("Start Clipeona {}", crate::aktualizacje::WERSJA));
    // Windows nie spowalnia Clipeona w tle (oszczędzanie energii zaniżało tempo przechwytywania)
    crate::ffmpeg::bez_dlawienia(unsafe { windows::Win32::System::Threading::GetCurrentProcess() });
    tauri::Builder::default()
        // Druga kopia programu tylko pokazuje okno pierwszej (skrót i enkoder są jedne)
        .plugin(tauri_plugin_single_instance::init(|app, _, _| pokaz_okno(app, "klipy")))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        // Okno zamknięte w trakcie ustawiania skrótu nie może zostawić skrótów wyłączonych
        .on_window_event(|okno, e| {
            if let tauri::WindowEvent::Destroyed = e
                && let Some(r) = okno.app_handle().try_state::<Rdzen>()
            {
                let r = r.inner().clone();
                std::thread::spawn(move || r.wstrzymaj_skroty(false));
            }
        })
        .invoke_handler(tauri::generate_handler![
            stan,
            ustawienia,
            mikrofony,
            gry,
            zapisz_ustawienia,
            wybierz_folder,
            zapisz_klip,
            przelacz_pauze,
            przelacz_film,
            wstrzymaj_skroty,
            klipy,
            miniatura,
            pokaz_w_folderze,
            kopiuj_klip,
            usun_klip,
            otworz_folder,
            eksportuj,
            przeciagnij,
            info_klipu,
            dzwiek_podgladu,
            fala_dzwieku,
            monitory,
            ustaw_tryb,
            ustaw_gre,
            diagnostyka,
            uruchom_ponownie,
            otworz_log,
            aktualizacja,
            sprawdz_aktualizacje,
            zainstaluj_aktualizacje,
            otworz_repozytorium
        ])
        .setup(move |app| {
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
            let film = MenuItem::with_id(app, "film", tekst_filmu(&cfg, false), true, None::<&str>)?;
            let tryb_gra = CheckMenuItem::with_id(app, "tryb_gra", "Tryb: Gra", true, !cfg.pulpit(), None::<&str>)?;
            let tryb_pulpit = CheckMenuItem::with_id(app, "tryb_pulpit", "Tryb: Pulpit", true, cfg.pulpit(), None::<&str>)?;
            let folder = MenuItem::with_id(app, "folder", "Otwórz folder klipów", true, None::<&str>)?;
            let biblioteka = MenuItem::with_id(app, "biblioteka", "Biblioteka…", true, None::<&str>)?;
            let ustawienia = MenuItem::with_id(app, "ustawienia", "Ustawienia…", true, None::<&str>)?;
            let zamknij = MenuItem::with_id(app, "zamknij", "Zamknij", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let menu = Menu::with_items(
                app,
                &[&stan, &sep()?, &zapisz, &film, &pauza, &tryb_gra, &tryb_pulpit, &sep()?, &folder, &biblioteka, &ustawienia, &log, &aktualizacja, &sep()?, &zamknij],
            )?;
            app.manage(PozycjeMenu { stan, aktualizacja, zapisz, pauza, film, tryb_gra, tryb_pulpit });
            app.manage(crate::aktualizacje::Stan::default());

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
                        "film" => {
                            let r = rdzen.inner().clone();
                            std::thread::spawn(move || r.przelacz_film());
                        }
                        "tryb_gra" | "tryb_pulpit" => {
                            let tryb = if e.id().as_ref() == "tryb_pulpit" { "pulpit" } else { "gra" };
                            if let Err(b) = rdzen.ustaw_tryb(Some(tryb.into()), None) {
                                crate::dziennik::blad(&b);
                            }
                            // Pole wyboru przełącza się samo przy kliknięciu — poprawiamy je według stanu
                            pokaz_stan(app, &rdzen.stan());
                        }
                        "folder" => klipy::pokaz_w_eksploratorze(&rdzen.config().folder_klipow, false),
                        "biblioteka" => pokaz_okno(app, "klipy"),
                        "ustawienia" => pokaz_okno(app, "ustawienia"),
                        "log" => otworz_log(),
                        "aktualizacja" => {
                            let stan = app.state::<crate::aktualizacje::Stan>();
                            let app = app.clone();
                            if stan.trwa() {
                                // Pozycja jest wtedy wyłączona; na wszelki wypadek nic nie robimy
                            } else if stan.znaleziona().is_some() {
                                tauri::async_runtime::spawn(async move {
                                    let _ = zainstaluj_aktualizacje_teraz(&app).await;
                                });
                            } else {
                                // Ręczne sprawdzenie; wynik pokazujemy dymkiem
                                tauri::async_runtime::spawn(async move {
                                    let (tytul, opis) = match sprawdz_aktualizacje_teraz(&app, false).await {
                                        Ok(Some(w)) => (
                                            format!("Dostępna wersja {}", w.wersja),
                                            "Kliknij ikonę prawym przyciskiem → „Aktualizuj”".to_string(),
                                        ),
                                        Ok(None) => ("Masz najnowszą wersję".to_string(), format!("Clipeon {}", crate::aktualizacje::WERSJA)),
                                        Err(e) => ("Nie udało się sprawdzić".to_string(), e),
                                    };
                                    pokaz_dymek(&app, &tytul, &opis, false, false);
                                });
                            }
                        }
                        // Trwający film najpierw zapisujemy do końca (w tle, żeby menu nie zamarzło)
                        "zamknij" => {
                            let (r, app) = (rdzen.inner().clone(), app.clone());
                            std::thread::spawn(move || {
                                r.zakoncz();
                                app.exit(0);
                            });
                        }
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
            if po_awarii {
                rdzen.uwaga("Clipeon wrócił po awarii", "Nagrywanie znów działa. Szczegóły są w dzienniku zdarzeń");
            }
            std::thread::spawn(klipy::usun_stara_pamiec_podreczna);
            // Pełna lista enkoderów do ustawień — w tle, po starcie nagrywania
            std::thread::spawn(crate::enkodery::dostepne);

            // Aktualizacje: chwilę po starcie, potem raz na dobę (jeśli włączone w ustawieniach)
            let h = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(10));
                loop {
                    if h.state::<Rdzen>().config().sprawdzaj_aktualizacje {
                        let _ = tauri::async_runtime::block_on(sprawdz_aktualizacje_teraz(&h, true));
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
            // Filmy przerwane awarią: lista teraz (zanim zacznie się nowy film), składanie w tle
            let przerwane = crate::film::przerwane(&c.folder_klipow);
            // Sprzątanie folderu przy starcie i potem co godzinę (jeśli włączone w ustawieniach)
            let r = rdzen.clone();
            std::thread::spawn(move || {
                r.odzyskaj_filmy(przerwane);
                loop {
                    let c = r.config();
                    // Pliki tymczasowe filmów, których nie dało się złożyć ani odzyskać (starsze niż 3 dni)
                    crate::film::usun_resztki(&c.folder_klipow);
                    crate::sprzatanie::usun_resztki(&c.folder_klipow);
                    crate::sprzatanie::sprzataj(&c.folder_klipow, c.sprzatanie_dni, c.sprzatanie_gb);
                    std::thread::sleep(std::time::Duration::from_secs(3600));
                }
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
