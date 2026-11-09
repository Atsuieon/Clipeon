// Rdzeń nagrywania: sesja (obraz + dźwięk), pauza, restart po zmianie ustawień, skrót i zapis klipu.
// Działa niezależnie od okna — UI dostaje tylko powiadomienia.
use crate::audio::{self, Zrodlo};
use crate::bufor::{Bufor, WspolnyBufor};
use crate::config::Config;
use crate::{skrot, wideo, zapis};
use std::collections::BTreeMap;
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
}

pub enum Zdarzenie {
    Stan(Stan),
    Zapisano(PathBuf),
    BladZapisu(String),
    /// Trwały alarm „Clipper nie nagrywa” (Some) albo koniec problemu (None)
    Alarm(Option<String>),
}

/// Po tylu sekundach bez nowych klatek strażnik uznaje, że nagrywanie stoi
const BEZ_KLATEK: i64 = 3;
/// Po tylu sekundach nieprzerwanego problemu pokazujemy alarm
const ALARM_PO: Duration = Duration::from_secs(5);

type Powiadom = Arc<dyn Fn(Zdarzenie) + Send + Sync>;

/// Jedna sesja nagrywania. Zniszczenie jej zatrzymuje obraz i dźwięk.
struct Sesja {
    _wideo: wideo::Wideo,
    _audio: Vec<audio::Audio>,
    bufor_wideo: WspolnyBufor,
    bufory_audio: Vec<(&'static str, WspolnyBufor)>,
    /// Format surowego strumienia wideo dla ffmpeg (h264 / hevc / obu)
    format_wideo: &'static str,
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
        Stan {
            nagrywa: w.sesja.is_some(),
            pauza: w.pauza,
            bledy: w.bledy.values().cloned().collect(),
            wznawianie: w.wznawianie.clone(),
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
            match self.nowa_sesja(&w.cfg, pokolenie) {
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

    fn nowa_sesja(&self, cfg: &Config, pokolenie: u64) -> Result<Sesja, String> {
        let bufor_wideo = Bufor::nowy(cfg.bufor_sekundy);
        let zglos_wideo = self.zglaszacz(pokolenie, "wideo");
        let wideo = wideo::uruchom(cfg, bufor_wideo.clone(), move |e| zglos_wideo(Some(e))).map_err(|e| e.to_string())?;

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
        Ok(Sesja { _wideo: wideo, _audio: audio_, bufor_wideo, bufory_audio, format_wideo })
    }

    /// Nowy config: zapis na dysk i restart nagrywania. Zawartość bufora przepada.
    pub fn zmien_config(&self, cfg: Config) -> Result<(), String> {
        cfg.sprawdz()?;
        cfg.zapisz()?;
        {
            let mut w = self.zamek();
            w.cfg = cfg;
            w.bledy.remove("config");
        }
        self.uruchom();
        Ok(())
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
    pub fn zapisz_klip(&self) {
        let (migawka, folder, fps, pauza) = {
            let w = self.zamek();
            let migawka = w.sesja.as_ref().map(|s| {
                let wideo = s.bufor_wideo.lock().unwrap_or_else(|e| e.into_inner()).migawka();
                let audio: Vec<_> =
                    s.bufory_audio.iter().map(|(n, b)| (*n, b.lock().unwrap_or_else(|e| e.into_inner()).migawka())).collect();
                (wideo, audio, s.format_wideo)
            });
            (migawka, w.cfg.folder_klipow.clone(), w.cfg.fps, w.pauza)
        };
        let powiadom = self.powiadom.clone();
        let Some((wideo, audio, format_wideo)) = migawka else {
            let opis = if pauza {
                "Nagrywanie jest wstrzymane — wznów je, żeby zapisywać klipy"
            } else {
                "Nagrywanie teraz nie działa — Clipper próbuje je wznowić"
            };
            powiadom(Zdarzenie::BladZapisu(opis.into()));
            return;
        };
        std::thread::spawn(move || match zapis::zapisz(&folder, fps, format_wideo, wideo, audio) {
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
    pub fn uruchom_straznika(&self) {
        let r = self.clone();
        let wynik = std::thread::Builder::new().name("straznik".into()).spawn(move || {
            let mut przerwa = Duration::from_secs(2);
            let mut nastepna_proba = Instant::now();
            let mut od_problemu: Option<Instant> = None;
            let mut alarm = false;
            let mut poprzedni_obieg = Instant::now();
            loop {
                std::thread::sleep(Duration::from_secs(1));
                // Długa przerwa między obiegami = komputer był uśpiony
                let po_uspieniu = poprzedni_obieg.elapsed() > Duration::from_secs(10);
                poprzedni_obieg = Instant::now();

                let problem = r.problem_nagrywania(po_uspieniu);
                // Inne błędy (skrót zajęty, brak dźwięku) też zasługują na alarm, ale nie na restart
                let inny_blad = r.zamek().bledy.iter().find(|(k, _)| **k != "nagrywanie").map(|(_, v)| v.clone());

                if let Some(powod) = &problem {
                    let od = *od_problemu.get_or_insert_with(Instant::now);
                    if Instant::now() >= nastepna_proba {
                        crate::dziennik::info(&format!("Strażnik: nagrywanie nie działa ({powod}) — uruchamiam ponownie"));
                        {
                            let mut w = r.zamek();
                            if od.elapsed() >= ALARM_PO {
                                w.wznawianie = None;
                                w.bledy.insert("nagrywanie", format!("Nie nagrywa: {powod} — ponawiam…"));
                            } else {
                                w.wznawianie = Some(powod.clone());
                            }
                        }
                        r.uruchom();
                        nastepna_proba = Instant::now() + przerwa;
                        przerwa = (przerwa * 2).min(Duration::from_secs(30));
                    } else if od.elapsed() >= ALARM_PO && !r.zamek().bledy.contains_key("nagrywanie") {
                        {
                            let mut w = r.zamek();
                            w.wznawianie = None;
                            w.bledy.insert("nagrywanie", format!("Nie nagrywa: {powod} — ponawiam…"));
                        }
                        r.wyslij_stan();
                    }
                } else if od_problemu.take().is_some() {
                    crate::dziennik::info("Strażnik: nagrywanie znów działa");
                    przerwa = Duration::from_secs(2);
                    {
                        let mut w = r.zamek();
                        w.wznawianie = None;
                        w.bledy.remove("nagrywanie");
                    }
                    r.wyslij_stan();
                }

                // Alarm: dowolny problem trwający ponad 5 s; zdejmujemy, gdy wszystko wróci do normy
                // Pierwsza litera wielka — tytuł dymka mówi już „Clipper nie nagrywa”
                let wielka = |t: String| {
                    let mut z = t.chars();
                    z.next().map(|c| c.to_uppercase().chain(z).collect::<String>()).unwrap_or_default()
                };
                let opis = problem.map(wielka).or(inny_blad);
                let dlugo = od_problemu.is_some_and(|t| t.elapsed() >= ALARM_PO) || (opis.is_some() && problem_inny_dlugo(&r));
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
            }
        });
        if let Err(e) = wynik {
            crate::dziennik::blad(&format!("Nie udało się uruchomić strażnika: {e}"));
        }
    }
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
