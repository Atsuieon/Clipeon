// Rdzeń nagrywania: sesja (obraz + dźwięk), pauza, restart po zmianie ustawień, skrót i zapis klipu.
// Działa niezależnie od okna — UI dostaje tylko powiadomienia.
use crate::audio::{self, Zrodlo};
use crate::bufor::{Bufor, WspolnyBufor};
use crate::config::Config;
use crate::{skrot, wideo, zapis};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Stan widoczny w UI (kropka w zasobniku: zielona = nagrywa, szara = pauza, czerwona = błąd).
#[derive(Clone, serde::Serialize)]
pub struct Stan {
    pub nagrywa: bool,
    pub pauza: bool,
    pub bledy: Vec<String>,
}

pub enum Zdarzenie {
    Stan(Stan),
    Zapisano(PathBuf),
    BladZapisu(String),
}

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
}

#[derive(Clone)]
pub struct Rdzen {
    w: Arc<Mutex<Wnetrze>>,
    powiadom: Powiadom,
}

impl Rdzen {
    pub fn nowy(cfg: Config, powiadom: impl Fn(Zdarzenie) + Send + Sync + 'static) -> Self {
        let w = Wnetrze { cfg, sesja: None, pauza: false, bledy: BTreeMap::new(), _skrot: None, pokolenie: 0 };
        Rdzen { w: Arc::new(Mutex::new(w)), powiadom: Arc::new(powiadom) }
    }

    pub fn config(&self) -> Config {
        self.w.lock().unwrap().cfg.clone()
    }

    pub fn stan(&self) -> Stan {
        let w = self.w.lock().unwrap();
        Stan { nagrywa: w.sesja.is_some(), pauza: w.pauza, bledy: w.bledy.values().cloned().collect() }
    }

    fn wyslij_stan(&self) {
        (self.powiadom)(Zdarzenie::Stan(self.stan()));
    }

    /// Ustawia (Some) albo kasuje (None) błąd danego źródła, jeśli zgłoszenie pochodzi z bieżącego pokolenia.
    fn zglos(&self, pokolenie: u64, klucz: &'static str, blad: Option<String>) {
        {
            let mut w = self.w.lock().unwrap();
            if w.pokolenie != pokolenie {
                return;
            }
            let zmiana = match blad {
                Some(b) => w.bledy.insert(klucz, b.clone()).as_ref() != Some(&b),
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
        self.zatrzymaj(&["config"]);
        let mut w = self.w.lock().unwrap();
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
                Ok(s) => w.sesja = Some(s),
                Err(e) => {
                    w.bledy.insert("wideo", format!("Nie udało się uruchomić nagrywania: {e}"));
                }
            }
        }
        drop(w);
        self.wyslij_stan();
    }

    /// Zatrzymuje skrót i sesję; zostawia tylko błędy z listy `zostaw`.
    fn zatrzymaj(&self, zostaw: &[&str]) {
        let (skrot, sesja) = {
            let mut w = self.w.lock().unwrap();
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
        let format_wideo = wideo::format_strumienia(&cfg.enkoder);
        Ok(Sesja { _wideo: wideo, _audio: audio_, bufor_wideo, bufory_audio, format_wideo })
    }

    /// Nowy config: zapis na dysk i restart nagrywania. Zawartość bufora przepada.
    pub fn zmien_config(&self, cfg: Config) -> Result<(), String> {
        cfg.sprawdz()?;
        cfg.zapisz()?;
        {
            let mut w = self.w.lock().unwrap();
            w.cfg = cfg;
            w.bledy.remove("config");
        }
        self.uruchom();
        Ok(())
    }

    /// Zmiana configu, która nie wymaga restartu nagrywania (zapis na dysk).
    pub fn zmien_bez_restartu(&self, zmiana: impl FnOnce(&mut Config)) {
        let mut w = self.w.lock().unwrap();
        zmiana(&mut w.cfg);
        if let Err(e) = w.cfg.zapisz() {
            eprintln!("{e}");
        }
    }

    /// Błąd wczytania configu (aplikacja działa wtedy na domyślnych ustawieniach).
    pub fn blad_configu(&self, blad: String) {
        self.w.lock().unwrap().bledy.insert("config", blad);
        self.wyslij_stan();
    }

    /// Wstrzymuje albo wznawia bufor (pauza zwalnia też enkoder i RAM bufora).
    pub fn przelacz_pauze(&self) {
        let pauza = {
            let mut w = self.w.lock().unwrap();
            w.pauza = !w.pauza;
            w.pauza
        };
        if pauza {
            // Skrót zostaje, żeby zgłosić "bufor wstrzymany"; zatrzymujemy tylko sesję
            let sesja = {
                let mut w = self.w.lock().unwrap();
                w.bledy.retain(|k, _| matches!(*k, "config" | "skrot"));
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
        let (migawka, folder, fps) = {
            let w = self.w.lock().unwrap();
            let migawka = w.sesja.as_ref().map(|s| {
                let wideo = s.bufor_wideo.lock().unwrap().migawka();
                let audio: Vec<_> = s.bufory_audio.iter().map(|(n, b)| (*n, b.lock().unwrap().migawka())).collect();
                (wideo, audio, s.format_wideo)
            });
            (migawka, w.cfg.folder_klipow.clone(), w.cfg.fps)
        };
        let powiadom = self.powiadom.clone();
        let Some((wideo, audio, format_wideo)) = migawka else {
            powiadom(Zdarzenie::BladZapisu("Bufor jest wstrzymany".into()));
            return;
        };
        std::thread::spawn(move || match zapis::zapisz(&folder, fps, format_wideo, wideo, audio) {
            Ok(plik) => powiadom(Zdarzenie::Zapisano(plik)),
            Err(e) => {
                eprintln!("Nie udało się zapisać klipu: {e}");
                powiadom(Zdarzenie::BladZapisu(e));
            }
        });
    }
}
