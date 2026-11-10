// Rdzeń nagrywania: sesja (obraz + dźwięk), pauza, restart po zmianie ustawień, skrót i zapis klipu,
// tryb gry / pulpitu, wybór drogi przechwytywania (z przejściem na następną, gdy zawiedzie) i strażnik.
// Działa niezależnie od okna — UI dostaje tylko powiadomienia.
use crate::audio::{self, Zrodlo};
use crate::bufor::{Bufor, WspolnyBufor};
use crate::config::Config;
use crate::wideo::Droga;
use crate::{skrot, wideo, zapis};
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::Ordering;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Stan widoczny w UI (kropka w zasobniku: zielona = nagrywa, żółta = wznawianie, szara = pauza,
/// czerwona = nie nagrywa / błąd).
#[derive(Clone, serde::Serialize)]
pub struct Stan {
    pub nagrywa: bool,
    pub pauza: bool,
    pub bledy: Vec<String>,
    /// Strażnik właśnie wznawia nagrywanie (powód)
    pub wznawianie: Option<String>,
    /// "gra" albo "pulpit"
    pub tryb: String,
    /// Krótka nazwa nagrywanego ekranu, np. „Ekran 2”
    pub ekran: String,
    /// Nazwa urządzenia nagrywanego ekranu (pusta = główny)
    pub monitor: String,
    /// Droga przechwytywania bieżącej sesji ("dda", "gfx", "potok")
    pub droga: Option<String>,
}

pub enum Zdarzenie {
    Stan(Stan),
    Zapisano(PathBuf),
    BladZapisu(String),
    /// Trwały alarm „Clipeon nie nagrywa” (Some) albo koniec problemu (None)
    Alarm(Option<String>),
    /// Nagrywanie działa, ale gorzej niż mogłoby (np. za stary sterownik karty) — pokazywane raz
    Uwaga { tytul: String, opis: String },
}

/// Po tylu sekundach bez nowych klatek strażnik uznaje, że nagrywanie stoi
const BEZ_KLATEK: i64 = 3;
/// Po tylu sekundach nieprzerwanego problemu pokazujemy alarm
const ALARM_PO: Duration = Duration::from_secs(5);
/// …a przy chwilowej utracie dostępu do ekranu (okno UAC, ekran blokady) dopiero po tylu
const ALARM_PO_CHWILOWYM: Duration = Duration::from_secs(60);
/// Problem w tym czasie od startu sesji = ta droga przechwytywania nie działa na tym komputerze
const ROZRUCH_DROGI: Duration = Duration::from_secs(15);
/// Po tylu sekundach bez problemów zapamiętujemy drogę jako działającą
const DROGA_DZIALA_PO: Duration = Duration::from_secs(30);

type Powiadom = Arc<dyn Fn(Zdarzenie) + Send + Sync>;

/// Jedna sesja nagrywania. Zniszczenie jej zatrzymuje obraz i dźwięk.
struct Sesja {
    _wideo: wideo::Wideo,
    _audio: Vec<audio::Audio>,
    bufor_wideo: WspolnyBufor,
    bufory_audio: Vec<(&'static str, WspolnyBufor)>,
    /// Format surowego strumienia wideo dla ffmpeg (h264 / hevc / obu)
    format_wideo: &'static str,
    /// Klucz monitora (do pamięci dróg): nazwa urządzenia, pusta = główny
    monitor: String,
    pulpit: bool,
}

struct Wnetrze {
    cfg: Config,
    sesja: Option<Sesja>,
    pauza: bool,
    /// Bieżące błędy według źródła ("wideo", "skrot", "system", "mikrofon", "config")
    bledy: BTreeMap<&'static str, String>,
    _skrot: Option<skrot::Nasluch>,
    /// Numer kolejnej sesji/skrótu; zgłoszenia ze starych wątków są ignorowane
    pokolenie: u64,
    /// Kiedy wystartowała bieżąca sesja (strażnik daje jej chwilę na rozruch)
    start_sesji: Option<Instant>,
    /// Powód trwającego wznawiania (żółta kropka)
    wznawianie: Option<String>,
    /// Drogi przechwytywania, które zawiodły w tym uruchomieniu: (monitor, droga)
    odrzucone: HashSet<(String, Droga)>,
    /// Lżejsze ustawienia enkodera (po wykryciu, że nie nadąża)
    lekki: bool,
}

#[derive(Clone)]
pub struct Rdzen {
    w: Arc<Mutex<Wnetrze>>,
    powiadom: Powiadom,
}

impl Rdzen {
    pub fn nowy(cfg: Config, powiadom: impl Fn(Zdarzenie) + Send + Sync + 'static) -> Self {
        let w = Wnetrze {
            cfg,
            sesja: None,
            pauza: false,
            bledy: BTreeMap::new(),
            _skrot: None,
            pokolenie: 0,
            start_sesji: None,
            wznawianie: None,
            odrzucone: HashSet::new(),
            lekki: false,
        };
        Rdzen { w: Arc::new(Mutex::new(w)), powiadom: Arc::new(powiadom) }
    }

    /// Blokada stanu odporna na „zatrucie” (awarię innego wątku w trakcie blokady).
    fn zamek(&self) -> MutexGuard<'_, Wnetrze> {
        self.w.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn config(&self) -> Config {
        self.zamek().cfg.clone()
    }

    pub fn stan(&self) -> Stan {
        let w = self.zamek();
        let (monitor, _) = w.cfg.aktywny_monitor();
        Stan {
            nagrywa: w.sesja.is_some(),
            pauza: w.pauza,
            bledy: w.bledy.values().cloned().collect(),
            wznawianie: w.wznawianie.clone(),
            tryb: w.cfg.tryb.clone(),
            ekran: wideo::etykieta_ekranu(monitor),
            monitor: monitor.to_string(),
            droga: w.sesja.as_ref().map(|s| s._wideo.droga.nazwa().to_string()),
        }
    }

    fn wyslij_stan(&self) {
        (self.powiadom)(Zdarzenie::Stan(self.stan()));
    }

    /// Ustawia (Some) albo kasuje (None) błąd danego źródła, jeśli zgłoszenie pochodzi z bieżącego pokolenia.
    fn zglos(&self, pokolenie: u64, klucz: &'static str, blad: Option<String>) {
        {
            let mut w = self.zamek();
            if w.pokolenie != pokolenie {
                return;
            }
            let zmiana = match blad {
                Some(b) => {
                    let nowy = w.bledy.insert(klucz, b.clone()).as_ref() != Some(&b);
                    if nowy {
                        crate::dziennik::blad(&b);
                    }
                    nowy
                }
                None => w.bledy.remove(klucz).is_some(),
            };
            if !zmiana {
                return;
            }
        }
        self.wyslij_stan();
    }

    fn zglaszacz(&self, pokolenie: u64, klucz: &'static str) -> impl Fn(Option<String>) + Send + 'static {
        let r = self.clone();
        move |b| r.zglos(pokolenie, klucz, b)
    }

    /// Startuje (albo restartuje) skrót i nagrywanie według bieżącego configu.
    pub fn uruchom(&self) {
        // Najpierw zatrzymujemy stare (poza blokadą — wątek skrótu może właśnie na nią czekać),
        // żeby zwolnić skrót i enkoder
        // Błąd „nagrywanie” należy do strażnika — zostaje do czasu, aż nagrywanie znów ruszy
        self.zatrzymaj(&["config", "nagrywanie"]);
        let mut w = self.zamek();
        let pokolenie = w.pokolenie;

        let r = self.clone();
        match skrot::nasluchuj(&w.cfg.skrot, move || r.zapisz_klip(), self.zglaszacz(pokolenie, "skrot")) {
            Ok(n) => w._skrot = Some(n),
            Err(e) => {
                w.bledy.insert("skrot", e);
            }
        }
        if !w.pauza {
            let (cfg, lekki) = (w.cfg.clone(), w.lekki);
            let mut odrzucone = std::mem::take(&mut w.odrzucone);
            let wynik = self.nowa_sesja(&cfg, pokolenie, &mut odrzucone, lekki);
            w.odrzucone = odrzucone;
            match wynik {
                Ok(s) => {
                    w.sesja = Some(s);
                    w.start_sesji = Some(Instant::now());
                }
                Err(e) => {
                    let opis = format!("Nie udało się uruchomić nagrywania: {e}");
                    crate::dziennik::blad(&opis);
                    w.bledy.insert("wideo", opis);
                }
            }
        }
        drop(w);
        self.wyslij_stan();
        // Poza blokadą: obsługa zdarzenia odczytuje config
        static POKAZANO: std::sync::Once = std::sync::Once::new();
        if let Some(u) = crate::enkodery::ostrzezenie() {
            POKAZANO.call_once(|| {
                (self.powiadom)(Zdarzenie::Uwaga { tytul: "Nagrywanie działa wolniej".into(), opis: u })
            });
        }
    }

    /// Zatrzymuje nagrywanie na stałe (przed instalacją aktualizacji): ffmpeg musi się zamknąć,
    /// żeby instalator mógł podmienić pliki, a strażnik nie może go wznowić.
    pub fn zakoncz(&self) {
        self.zamek().pauza = true;
        self.zatrzymaj(&[]);
    }

    /// Zatrzymuje skrót i sesję; zostawia tylko błędy z listy `zostaw`.
    fn zatrzymaj(&self, zostaw: &[&str]) {
        let (skrot, sesja) = {
            let mut w = self.zamek();
            w.pokolenie += 1;
            w.bledy.retain(|k, _| zostaw.contains(k));
            (w._skrot.take(), w.sesja.take())
        };
        drop(skrot);
        drop(sesja);
    }

    /// Nowa sesja: obraz pierwszą działającą drogą przechwytywania (zapamiętana droga najpierw),
    /// potem dźwięk. Drogi, które nie wystartowały, trafiają do `odrzucone`.
    fn nowa_sesja(&self, cfg: &Config, pokolenie: u64, odrzucone: &mut HashSet<(String, Droga)>, lekki: bool) -> Result<Sesja, String> {
        let monitor = cfg.aktywny_monitor().0.to_string();
        let mut kolejnosc = wideo::drogi(cfg);
        let zapamietana = crate::pamiec::pamiec().drogi.get(&monitor).and_then(|d| Droga::z_nazwy(d));
        if let Some(z) = zapamietana.filter(|z| cfg.metoda_przechwytywania == "auto" && kolejnosc.contains(z)) {
            kolejnosc.retain(|d| *d != z);
            kolejnosc.insert(0, z);
        }
        let mut ostatni_blad = String::new();
        let mut wideo_ = None;
        for droga in kolejnosc {
            if droga != Droga::Potok && odrzucone.contains(&(monitor.clone(), droga)) {
                continue;
            }
            let bufor = Bufor::nowy(cfg.bufor_sekundy);
            let zglos_wideo = self.zglaszacz(pokolenie, "wideo");
            match wideo::uruchom(cfg, droga, lekki, bufor.clone(), move |e| zglos_wideo(Some(e))) {
                Ok(w) => {
                    wideo_ = Some((w, bufor));
                    break;
                }
                Err(e) => {
                    ostatni_blad = e.to_string();
                    crate::dziennik::info(&format!("Droga przechwytywania „{}” nie wystartowała: {e}", droga.nazwa()));
                    if droga != Droga::Potok {
                        odrzucone.insert((monitor.clone(), droga));
                    }
                }
            }
        }
        let Some((wideo, bufor_wideo)) = wideo_ else { return Err(ostatni_blad) };

        // Dźwięk: AAC 160 kb/s; bufor dłuższy o 2 s, żeby na pewno pokrył początek obrazu
        let (mut audio_, mut bufory_audio) = (Vec::new(), Vec::new());
        for (wlaczone, zrodlo, klucz, nazwa) in [
            (cfg.dzwiek_systemowy, Zrodlo::System, "system", "Gra"),
            (cfg.mikrofon, Zrodlo::Mikrofon, "mikrofon", "Mikrofon"),
        ] {
            if wlaczone {
                let b = Bufor::nowy(cfg.bufor_sekundy + 2);
                audio_.push(audio::uruchom(zrodlo, 160, b.clone(), self.zglaszacz(pokolenie, klucz)));
                bufory_audio.push((nazwa, b));
            }
        }
        let format_wideo = wideo.format;
        Ok(Sesja { _wideo: wideo, _audio: audio_, bufor_wideo, bufory_audio, format_wideo, monitor, pulpit: cfg.pulpit() })
    }

    /// Nowy config: zapis na dysk i restart nagrywania. Zawartość bufora przepada.
    pub fn zmien_config(&self, mut cfg: Config) -> Result<(), String> {
        cfg.sprawdz()?;
        {
            // Stałe identyfikatory wybranych monitorów (odporne na przenumerowanie DISPLAYn)
            let stary = self.config();
            cfg.monitor_id = id_monitora(&cfg.monitor, &stary.monitor, &stary.monitor_id);
            cfg.monitor_pulpitu_id = id_monitora(&cfg.monitor_pulpitu, &stary.monitor_pulpitu, &stary.monitor_pulpitu_id);
        }
        cfg.zapisz()?;
        {
            let mut w = self.zamek();
            // Inna metoda albo enkoder: drogi i lekkie ustawienia sprawdzamy od nowa
            if w.cfg.metoda_przechwytywania != cfg.metoda_przechwytywania || w.cfg.enkoder != cfg.enkoder {
                w.odrzucone.clear();
                w.lekki = false;
            }
            w.cfg = cfg;
            w.bledy.remove("config");
        }
        self.uruchom();
        Ok(())
    }

    /// Przełącza tryb (gra / pulpit) i/lub ekran bieżącego trybu (z paska w oknie albo menu przy zegarze).
    pub fn ustaw_tryb(&self, tryb: Option<String>, monitor: Option<String>) -> Result<(), String> {
        let mut cfg = self.config();
        if let Some(t) = tryb {
            cfg.tryb = t;
        }
        if let Some(m) = monitor {
            if cfg.pulpit() {
                cfg.monitor_pulpitu = m;
            } else {
                cfg.monitor = m;
            }
        }
        if cfg.tryb == self.config().tryb && cfg.aktywny_monitor() == self.config().aktywny_monitor() {
            return Ok(());
        }
        crate::dziennik::info(&format!(
            "Tryb: {}, ekran: {}",
            cfg.tryb,
            wideo::etykieta_ekranu(cfg.aktywny_monitor().0)
        ));
        self.zmien_config(cfg)
    }

    /// Zmiana configu, która nie wymaga restartu nagrywania (zapis na dysk).
    pub fn zmien_bez_restartu(&self, zmiana: impl FnOnce(&mut Config)) {
        let mut w = self.zamek();
        zmiana(&mut w.cfg);
        if let Err(e) = w.cfg.zapisz() {
            crate::dziennik::blad(&e);
        }
    }

    /// Błąd wczytania configu (aplikacja działa wtedy na domyślnych ustawieniach).
    pub fn blad_configu(&self, blad: String) {
        self.zamek().bledy.insert("config", blad);
        self.wyslij_stan();
    }

    /// Wstrzymuje albo wznawia bufor (pauza zwalnia też enkoder i RAM bufora).
    pub fn przelacz_pauze(&self) {
        let pauza = {
            let mut w = self.zamek();
            w.pauza = !w.pauza;
            w.pauza
        };
        if pauza {
            // Skrót zostaje, żeby zgłosić "bufor wstrzymany"; zatrzymujemy tylko sesję
            let sesja = {
                let mut w = self.zamek();
                w.bledy.retain(|k, _| matches!(*k, "config" | "skrot"));
                w.wznawianie = None;
                w.sesja.take()
            };
            drop(sesja);
            self.wyslij_stan();
        } else {
            self.uruchom();
        }
    }

    /// Zapisuje bufor do MP4 w osobnym wątku; nagrywanie trwa dalej.
    /// Krótka informacja w dymku (nie alarm).
    pub fn uwaga(&self, tytul: &str, opis: &str) {
        (self.powiadom)(Zdarzenie::Uwaga { tytul: tytul.into(), opis: opis.into() });
    }

    pub fn zapisz_klip(&self) {
        let (migawka, folder, pauza) = {
            let w = self.zamek();
            let migawka = w.sesja.as_ref().map(|s| {
                let wideo = s.bufor_wideo.lock().unwrap_or_else(|e| e.into_inner()).migawka();
                let audio: Vec<_> =
                    s.bufory_audio.iter().map(|(n, b)| (*n, b.lock().unwrap_or_else(|e| e.into_inner()).migawka())).collect();
                (wideo, audio, s.format_wideo, s._wideo.fps, s.pulpit)
            });
            (migawka, w.cfg.folder_klipow.clone(), w.pauza)
        };
        let powiadom = self.powiadom.clone();
        let Some((wideo, audio, format_wideo, fps, pulpit)) = migawka else {
            let opis = if pauza {
                "Nagrywanie jest wstrzymane — wznów je, żeby zapisywać klipy"
            } else {
                "Nagrywanie teraz nie działa — Clipeon próbuje je wznowić"
            };
            powiadom(Zdarzenie::BladZapisu(opis.into()));
            return;
        };
        let przedrostek = if pulpit { "Clipeon_Pulpit" } else { "Clipeon" };
        std::thread::spawn(move || match zapis::zapisz(&folder, przedrostek, fps, format_wideo, wideo, audio) {
            Ok(plik) => powiadom(Zdarzenie::Zapisano(plik)),
            Err(e) => {
                crate::dziennik::blad(&format!("Nie udało się zapisać klipu: {e}"));
                powiadom(Zdarzenie::BladZapisu(e));
            }
        });
    }

    /// Co nie działa w nagrywaniu obrazu (None = działa albo jest wstrzymane).
    fn problem_nagrywania(&self, po_uspieniu: bool) -> Option<String> {
        let w = self.zamek();
        if w.pauza {
            return None;
        }
        let Some(sesja) = &w.sesja else {
            return Some(w.bledy.get("wideo").cloned().unwrap_or_else(|| "nagrywanie nie wystartowało".into()));
        };
        if po_uspieniu {
            return Some("komputer był uśpiony".into());
        }
        if let Some(p) = sesja._wideo.problem() {
            return Some(p);
        }
        if let Some(b) = w.bledy.get("wideo") {
            return Some(b.clone());
        }
        // Rozruch enkodera: nie oceniamy świeżo uruchomionej sesji
        if w.start_sesji.is_some_and(|t| t.elapsed() < Duration::from_secs(4)) {
            return None;
        }
        // Windows Graphics Capture w ffmpeg nie daje klatek, gdy ekran się nie zmienia — wtedy brak
        // klatek to nie awaria (proces ffmpeg pilnuje wątek odczytu, który zgłosi jego koniec)
        if sesja._wideo.droga == Droga::Gfx {
            return None;
        }
        let ostatnia = sesja.bufor_wideo.lock().unwrap_or_else(|e| e.into_inner()).ostatni_czas();
        let teraz = crate::bufor::teraz_100ns();
        match ostatnia {
            Some(t) if teraz - t <= BEZ_KLATEK * 10_000_000 => None,
            _ => Some("brak nowych klatek z enkodera".into()),
        }
    }

    /// Strażnik nagrywania: co sekundę sprawdza, czy przybywają nowe klatki. Gdy nagrywanie stoi
    /// (enkoder padł, zmiana rozdzielczości, uśpienie komputera…), uruchamia je od nowa — coraz rzadziej
    /// (2, 4, 8… do 30 s). Po 5 s problemu wysyła alarm, a po powrocie do normy go zdejmuje.
    /// Pilnuje też drogi przechwytywania (zła -> następna, dobra -> zapamiętana), przeciążenia enkodera,
    /// powrotu odłączonego monitora i co minutę zapisuje statystyki do dziennika.
    pub fn uruchom_straznika(&self) {
        let r = self.clone();
        let wynik = std::thread::Builder::new().name("straznik".into()).spawn(move || {
            let mut przerwa = Duration::from_secs(2);
            let mut nastepna_proba = Instant::now();
            let mut od_problemu: Option<Instant> = None;
            let mut alarm = false;
            let mut poprzedni_obieg = Instant::now();
            let mut obieg: u64 = 0;
            let mut pomiar = crate::pomiar::Licznik::default();
            let mut poprzednie_statystyki: Option<[u64; 6]> = None;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                obieg += 1;
                // Długa przerwa między obiegami = komputer był uśpiony
                let po_uspieniu = poprzedni_obieg.elapsed() > Duration::from_secs(10);
                poprzedni_obieg = Instant::now();

                let problem = r.problem_nagrywania(po_uspieniu);
                // Inne błędy (skrót zajęty, brak dźwięku) też zasługują na alarm, ale nie na restart
                let inny_blad = r.zamek().bledy.iter().find(|(k, _)| **k != "nagrywanie").map(|(_, v)| v.clone());
                let chwilowy = problem.as_deref().is_some_and(wideo::chwilowy);
                let prog_alarmu = if chwilowy { ALARM_PO_CHWILOWYM } else { ALARM_PO };

                if let Some(powod) = &problem {
                    let od = *od_problemu.get_or_insert_with(Instant::now);
                    if Instant::now() >= nastepna_proba {
                        r.odrzuc_droge_po_problemie(powod, chwilowy);
                        crate::dziennik::info(&format!("Strażnik: nagrywanie nie działa ({powod}) — uruchamiam ponownie"));
                        {
                            let mut w = r.zamek();
                            if od.elapsed() >= prog_alarmu {
                                w.wznawianie = None;
                                w.bledy.insert("nagrywanie", format!("Nie nagrywa: {powod} — ponawiam…"));
                            } else {
                                w.wznawianie = Some(if chwilowy {
                                    "ekran chwilowo niedostępny (np. okno UAC albo ekran blokady)".to_string()
                                } else {
                                    powod.clone()
                                });
                            }
                        }
                        r.uruchom();
                        nastepna_proba = Instant::now() + przerwa;
                        przerwa = (przerwa * 2).min(Duration::from_secs(30));
                    } else if od.elapsed() >= prog_alarmu && !r.zamek().bledy.contains_key("nagrywanie") {
                        {
                            let mut w = r.zamek();
                            w.wznawianie = None;
                            w.bledy.insert("nagrywanie", format!("Nie nagrywa: {powod} — ponawiam…"));
                        }
                        r.wyslij_stan();
                    }
                } else {
                    if od_problemu.take().is_some() {
                        crate::dziennik::info("Strażnik: nagrywanie znów działa");
                        przerwa = Duration::from_secs(2);
                        {
                            let mut w = r.zamek();
                            w.wznawianie = None;
                            w.bledy.remove("nagrywanie");
                        }
                        r.wyslij_stan();
                    }
                    r.pilnuj_dzialajacej_sesji(obieg);
                }

                // Alarm: dowolny problem trwający ponad próg; zdejmujemy, gdy wszystko wróci do normy
                // Pierwsza litera wielka — tytuł dymka mówi już „Clipeon nie nagrywa”
                let wielka = |t: String| {
                    let mut z = t.chars();
                    z.next().map(|c| c.to_uppercase().chain(z).collect::<String>()).unwrap_or_default()
                };
                let opis = problem.map(wielka).or(inny_blad);
                let dlugo = od_problemu.is_some_and(|t| t.elapsed() >= prog_alarmu) || (opis.is_some() && problem_inny_dlugo(&r));
                match (&opis, alarm) {
                    (Some(o), false) if dlugo => {
                        alarm = true;
                        (r.powiadom)(Zdarzenie::Alarm(Some(o.clone())));
                    }
                    (None, true) => {
                        alarm = false;
                        (r.powiadom)(Zdarzenie::Alarm(None));
                    }
                    _ => {}
                }

                if obieg.is_multiple_of(60) {
                    r.zapisz_statystyki(&mut pomiar, &mut poprzednie_statystyki);
                }
            }
        });
        if let Err(e) = wynik {
            crate::dziennik::blad(&format!("Nie udało się uruchomić strażnika: {e}"));
        }
    }

    /// Problem tuż po starcie sesji przechwytującej w ffmpeg = ta droga nie działa na tym komputerze;
    /// następny start weźmie kolejną. Chwilowa utrata dostępu do ekranu się nie liczy.
    fn odrzuc_droge_po_problemie(&self, powod: &str, chwilowy: bool) {
        let mut w = self.zamek();
        let Some(s) = &w.sesja else { return };
        let droga = s._wideo.droga;
        let mlody = w.start_sesji.is_some_and(|t| t.elapsed() < ROZRUCH_DROGI);
        if chwilowy || !mlody || !droga.w_ffmpeg() {
            return;
        }
        let monitor = s.monitor.clone();
        crate::dziennik::info(&format!(
            "Droga przechwytywania „{}” nie działa na tym komputerze ({powod}) — przechodzę na następną",
            droga.nazwa()
        ));
        w.odrzucone.insert((monitor.clone(), droga));
        drop(w);
        let mut p = crate::pamiec::pamiec();
        if p.drogi.get(&monitor).is_some_and(|d| d == droga.nazwa()) {
            p.drogi.remove(&monitor);
            crate::pamiec::zapisz(&p);
        }
    }

    /// Sesja działa: zapamiętanie dobrej drogi, przejście na lżejsze ustawienia przy przeciążeniu,
    /// powrót do wybranego monitora, gdy znów jest podłączony.
    fn pilnuj_dzialajacej_sesji(&self, obieg: u64) {
        enum Akcja {
            Nic,
            Lekki,
            Restart(&'static str),
        }
        let akcja = {
            let mut w = self.zamek();
            let dlugo = w.start_sesji.is_some_and(|t| t.elapsed() >= DROGA_DZIALA_PO);
            let lekki = w.lekki;
            let Some(s) = &w.sesja else { return };
            if dlugo {
                let (monitor, droga) = (s.monitor.clone(), s._wideo.droga.nazwa());
                let mut p = crate::pamiec::pamiec();
                if p.drogi.get(&monitor).map(String::as_str) != Some(droga) {
                    p.drogi.insert(monitor, droga.to_string());
                    crate::pamiec::zapisz(&p);
                }
            }
            if s._wideo.przeciazenie.load(Ordering::SeqCst) && !lekki {
                w.lekki = true;
                Akcja::Lekki
            } else if obieg.is_multiple_of(5)
                && s._wideo.zastepczy.as_ref().is_some_and(|(n, id)| wideo::monitor_wrocil(n, id))
            {
                Akcja::Restart("wybrany monitor jest znów podłączony")
            } else {
                Akcja::Nic
            }
        };
        match akcja {
            Akcja::Nic => {}
            Akcja::Lekki => {
                crate::dziennik::info("Enkoder nie nadąża — przechodzę na lżejsze ustawienia kodowania");
                self.uwaga(
                    "Clipeon nagrywa lżej",
                    "Karta albo procesor nie nadążały za grą, więc Clipeon koduje szybciej, kosztem odrobiny jakości",
                );
                self.uruchom();
            }
            Akcja::Restart(powod) => {
                crate::dziennik::info(&format!("Strażnik: {powod} — uruchamiam nagrywanie od nowa"));
                self.uruchom();
            }
        }
    }

    /// Co minutę do dziennika: ile klatek zakodowano, ile powtórzono / pominięto / spóźniło się
    /// i ile procesora zużywa Clipeon z procesami ffmpeg.
    fn zapisz_statystyki(&self, pomiar: &mut crate::pomiar::Licznik, poprzednie: &mut Option<[u64; 6]>) {
        let cpu = pomiar.pomiar();
        let (teraz, droga) = {
            let w = self.zamek();
            match &w.sesja {
                Some(s) => (Some(s._wideo.statystyki.odczyt()), s._wideo.droga.nazwa()),
                None => (None, "-"),
            }
        };
        let Some(teraz) = teraz else {
            *poprzednie = None;
            return;
        };
        let roznica = match poprzednie.replace(teraz) {
            // Nowa sesja ma liczniki od zera
            Some(p) if p.iter().zip(teraz.iter()).all(|(a, b)| b >= a) => {
                let mut r = [0u64; 6];
                for i in 0..6 {
                    r[i] = teraz[i] - p[i];
                }
                r
            }
            _ => return,
        };
        let [przechwycone, pominiete, powtorzone, spoznione, pakiety, bajty] = roznica;
        let mut t = format!(
            "Statystyki (60 s): droga {droga}, zakodowane klatki {pakiety} ({:.1}/s, {:.1} Mb/s)",
            pakiety as f64 / 60.0,
            bajty as f64 * 8.0 / 60.0 / 1e6
        );
        if droga == "potok" {
            t.push_str(&format!(", z ekranu {przechwycone}, pominięte {pominiete}, powtórzone {powtorzone}"));
        }
        t.push_str(&format!(", spóźnione {spoznione}"));
        if let Some(c) = cpu {
            t.push_str(&format!("; procesor: {c}"));
        }
        crate::dziennik::info(&t.replace(". ", ", "));
    }

    /// Opis do zgłoszenia błędu: wersje, sprzęt, ustawienia, wyniki prób i końcówka dziennika.
    pub fn diagnostyka(&self) -> String {
        let (cfg, sesja) = {
            let w = self.zamek();
            (w.cfg.clone(), w.sesja.as_ref().map(|s| s._wideo.opis.clone()))
        };
        let stan = self.stan();
        let mut t = String::new();
        let mut l = |s: String| {
            t.push_str(&s);
            t.push('\n');
        };
        l(format!("Clipeon {}", crate::aktualizacje::WERSJA));
        l(format!("Windows: {}", wersja_windows()));
        l(format!("FFmpeg: {}", crate::ffmpeg::wersja()));
        l(format!(
            "Filtry przechwytywania: ddagrab {}, gfxcapture {}",
            tak_nie(crate::ffmpeg::ma_filtr("ddagrab")),
            tak_nie(crate::ffmpeg::ma_filtr("gfxcapture"))
        ));
        l(String::new());
        l("Karty graficzne:".into());
        for k in crate::karty::karty() {
            l(format!("- {} ({:.1} GB), sterownik {}", k.nazwa, k.pamiec as f64 / (1u64 << 30) as f64, k.sterownik));
        }
        l("Monitory:".into());
        for m in wideo::monitory() {
            let wyj = crate::karty::wyjscie_monitora(&m.id);
            l(format!(
                "- {} {}: {}×{} {} Hz{}{}{}",
                m.id,
                m.nazwa,
                m.szerokosc,
                m.wysokosc,
                m.hz,
                if m.glowny { ", główny" } else { "" },
                if m.hdr { ", HDR" } else { "" },
                wyj.map(|w| format!(", karta {} ({:?}) wyjście {}", w.karta, w.producent, w.wyjscie)).unwrap_or_default()
            ));
        }
        l(String::new());
        l(format!(
            "Ustawienia: tryb {}, ekran „{}”, {} FPS, {} Mb/s, enkoder {}, metoda {}, bufor {} s",
            cfg.tryb,
            cfg.aktywny_monitor().0,
            cfg.aktywne_fps(),
            cfg.aktywny_bitrate(),
            cfg.enkoder,
            cfg.metoda_przechwytywania,
            cfg.bufor_sekundy
        ));
        l(format!("Sesja: {}", sesja.unwrap_or_else(|| "nie nagrywa".into())));
        l(format!(
            "Stan: {}{}",
            if stan.pauza { "wstrzymane" } else if stan.nagrywa { "nagrywa" } else { "nie nagrywa" },
            if stan.bledy.is_empty() { String::new() } else { format!(", błędy: {}", stan.bledy.join("; ")) }
        ));
        l(String::new());
        l("Próby enkoderów:".into());
        for w in crate::enkodery::raport() {
            l(format!("- {w}"));
        }
        l(String::new());
        l("Ostatnie wpisy dziennika:".into());
        let dziennik = std::fs::read_to_string(crate::dziennik::plik()).unwrap_or_default();
        let linie: Vec<&str> = dziennik.lines().collect();
        for linia in &linie[linie.len().saturating_sub(50)..] {
            l(linia.to_string());
        }
        // Bez nazwy użytkownika Windows w ścieżkach
        match std::env::var("USERPROFILE") {
            Ok(u) if !u.is_empty() => t.replace(&u, "%USERPROFILE%"),
            _ => t,
        }
    }
}

fn tak_nie(b: bool) -> &'static str {
    if b { "tak" } else { "nie" }
}

/// Np. „Windows 11 24H2 (kompilacja 26100.2314)”.
fn wersja_windows() -> String {
    let Ok(k) = windows_registry::LOCAL_MACHINE.open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") else {
        return "nieznana".into();
    };
    let kompilacja: u32 = k.get_string("CurrentBuild").ok().and_then(|b| b.parse().ok()).unwrap_or(0);
    let ubr = k.get_u32("UBR").unwrap_or(0);
    let wersja = k.get_string("DisplayVersion").unwrap_or_default();
    // Windows 11 nadal przedstawia się w rejestrze jako „Windows 10” — rozróżniamy po kompilacji
    let nazwa = if kompilacja >= 22000 { "Windows 11" } else { "Windows 10" };
    format!("{nazwa} {wersja} (kompilacja {kompilacja}.{ubr})")
}

/// Stały identyfikator monitora do zapisania w ustawieniach. Odłączony monitor: zostaje poprzedni
/// identyfikator, jeśli to ten sam wybór.
fn id_monitora(nazwa: &str, stara_nazwa: &str, stary_id: &str) -> String {
    if nazwa.is_empty() {
        return String::new();
    }
    wideo::trwaly_id(nazwa).unwrap_or_else(|| if nazwa == stara_nazwa { stary_id.to_string() } else { String::new() })
}

/// Czy inny błąd (skrót, dźwięk) trwa już dłużej niż próg alarmu.
fn problem_inny_dlugo(r: &Rdzen) -> bool {
    use std::sync::OnceLock;
    static OD: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    let od = OD.get_or_init(|| Mutex::new(None));
    let mut od = od.lock().unwrap_or_else(|e| e.into_inner());
    let jest = r.zamek().bledy.keys().any(|k| *k != "nagrywanie");
    if !jest {
        *od = None;
        return false;
    }
    od.get_or_insert_with(Instant::now).elapsed() >= ALARM_PO
}
