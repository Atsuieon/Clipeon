// Rdzeń nagrywania: sesja (obraz + dźwięk), pauza, restart po zmianie ustawień, skróty, zapis klipu,
// długie nagrywanie (film), tryb gry / pulpitu, wybór drogi przechwytywania (z przejściem na następną,
// gdy zawiedzie, i powrotem, gdy zawiodła tylko chwilowo) i strażnik.
// Działa niezależnie od okna — UI dostaje tylko powiadomienia.
use crate::Zablokuj;
use crate::audio::{self, Zrodlo};
use crate::bufor::{Bufor, WspolnyBufor};
use crate::config::Config;
use crate::film::Film;
use crate::wideo::Droga;
use crate::{skrot, wideo, zapis};
use std::collections::{BTreeMap, HashMap, HashSet};
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
    /// Nazwa gry, która jest teraz na wierzchu nagrywanego ekranu
    pub gra: Option<String>,
    /// Nazwa gry wybranej na pasku (pusta = nagrywaj zawsze)
    pub wybrana_gra: String,
    /// Plik programu wybranej gry, np. „cs2.exe” (pusty = automatycznie)
    pub gra_proces: String,
    /// Czekamy, aż wybrana gra wystartuje (nagrywanie jest wtedy wstrzymane)
    pub czeka_na_gre: bool,
    /// Czy mikrofon jest nagrywany (wyłączony = przekreślony mikrofon przy statusie)
    pub mikrofon: bool,
    /// Windows rysuje teraz żółtą ramkę wokół nagrywanego ekranu (droga przez Windows Graphics Capture
    /// na systemie, który nie umie jej wyłączyć)
    pub ramka: bool,
    /// Trwa długie nagrywanie: ile sekund (None = nie trwa)
    pub film: Option<u64>,
    /// Zakończony film jest właśnie składany w plik
    pub film_zapis: bool,
    /// Zapisany film jest właśnie zmniejszany do formatu z ustawień (procent postępu)
    pub film_zmniejszanie: Option<u8>,
    /// Skrót filmu jest zajęty przez inny program (opis) — film da się wtedy włączyć tylko z okna
    pub skrot_filmu_blad: Option<String>,
}

#[allow(clippy::large_enum_variant)]
pub enum Zdarzenie {
    Stan(Stan),
    Zapisano(PathBuf),
    BladZapisu(String),
    /// Trwały alarm „Clipeon nie nagrywa” (Some) albo koniec problemu (None)
    Alarm(Option<String>),
    /// Nagrywanie działa, ale gorzej niż mogłoby (np. za stary sterownik karty) — pokazywane raz
    Uwaga { tytul: String, opis: String },
    /// Zaczęło się długie nagrywanie
    FilmStart,
    /// Długie nagrywanie się skończyło i plik jest składany; Some = skończyło się samo (powód)
    FilmKoniec(Option<String>),
    FilmZapisany { plik: PathBuf, sekundy: f64, czesci: usize },
    /// Film został zmniejszony do formatu z ustawień (rozmiary w bajtach)
    FilmZmniejszony { plik: PathBuf, przed: u64, po: u64 },
    /// Zmniejszanie się nie udało — film został w pełnej jakości
    FilmNiezmniejszony(String),
    BladFilmu(String),
}

/// Po tylu sekundach bez nowych klatek strażnik uznaje, że nagrywanie stoi
const BEZ_KLATEK: i64 = 3;
/// Po tylu sekundach nieprzerwanego problemu pokazujemy alarm
const ALARM_PO: Duration = Duration::from_secs(5);
/// …a przy chwilowej utracie dostępu do ekranu (okno UAC, ekran blokady) dopiero po tylu
const ALARM_PO_CHWILOWYM: Duration = Duration::from_secs(60);
/// Problem w tym czasie od startu sesji = ta droga przechwytywania nie działa na tym komputerze
const ROZRUCH_DROGI: Duration = Duration::from_secs(15);
/// Tyle czasu wybranej gry nie może być, zanim uznamy, że się zamknęła
const GRA_ZNIKA_PO: Duration = Duration::from_secs(10);
/// Chwilowe awarie drogi w ffmpeg (gra przełącza pełny ekran albo rozdzielczość i Desktop Duplication
/// na moment gubi obraz) to nie powód do zmiany drogi: ponawiamy tę samą. Dopiero gdy przez tyle czasu
/// żadna sesja nie utrzymała się SESJA_STABILNA_PO, bierzemy następną drogę — i to tylko na jakiś czas.
const SERIA_AWARII_DO_ZMIANY: Duration = Duration::from_secs(30);
const SESJA_STABILNA_PO: Duration = Duration::from_secs(10);
/// Po tylu sekundach wracamy na drogę odrzuconą chwilowo (każdy kolejny nieudany powrót dwa razy później)
const POWROT_NA_DROGE_PO: Duration = Duration::from_secs(60);
const POWROT_NAJPOZNIEJ_PO: Duration = Duration::from_secs(1800);
/// Po tylu sekundach bez problemów zapamiętujemy drogę jako działającą
const DROGA_DZIALA_PO: Duration = Duration::from_secs(30);

type Powiadom = Arc<dyn Fn(Zdarzenie) + Send + Sync>;

/// Jedna sesja nagrywania. Zniszczenie jej zatrzymuje obraz i dźwięk.
struct Sesja {
    _wideo: wideo::Wideo,
    _audio: Vec<audio::Audio>,
    bufor_wideo: WspolnyBufor,
    bufory_audio: Vec<(&'static str, WspolnyBufor)>,
    /// Urządzenia ścieżek dźwięku (w kolejności `bufory_audio`) — do ponownego użycia buforów
    urzadzenia_audio: Vec<String>,
    /// Format surowego strumienia wideo dla ffmpeg (h264 / hevc / obu)
    format_wideo: &'static str,
    /// Klucz monitora (do pamięci dróg): nazwa urządzenia, pusta = główny
    monitor: String,
    pulpit: bool,
    /// Czy tę drogę wolno zapamiętać jako działającą (nie, gdy lepsza jest tylko chwilowo odrzucona)
    zapamietaj_droge: bool,
}

impl Sesja {
    fn obraz(&self) -> crate::film::Obraz {
        let w = &self._wideo;
        crate::film::Obraz { format: self.format_wideo, fps: w.fps, rozmiar: w.rozmiar, enkoder: w.enkoder }
    }
}

struct Wnetrze {
    cfg: Config,
    sesja: Option<Sesja>,
    pauza: bool,
    /// Bieżące błędy według źródła ("wideo", "skrot", "system", "mikrofon", "config")
    bledy: BTreeMap<&'static str, String>,
    _skrot: Option<skrot::Nasluch>,
    _skrot_filmu: Option<skrot::Nasluch>,
    /// Okno ustawia właśnie nowy skrót: skróty są wyrejestrowane, żeby wciskane klawisze nie robiły klipów
    skroty_wstrzymane: bool,
    skrot_filmu_blad: Option<String>,
    /// Trwające długie nagrywanie
    film: Option<Film>,
    /// Zakończony film jest składany w plik (w tle)
    zapis_filmu: bool,
    /// Zapisany film jest zmniejszany (procent postępu)
    zmniejszanie_filmu: Option<u8>,
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
    /// Gra na wierzchu nagrywanego ekranu (do pokazania w oknie i w zasobniku)
    gra: Option<crate::gra::Gra>,
    /// Wybrana gra nie działa — nagrywanie czeka na nią
    czeka_na_gre: bool,
    /// Trwająca seria chwilowych awarii drogi w ffmpeg (np. gra zmienia tryb ekranu): od kiedy i która
    /// droga. W czasie serii ponawiamy tylko tę drogę.
    seria_awarii: Option<(Instant, Droga)>,
    /// Drogi odrzucone chwilowo (seria awarii trwała za długo): (monitor, droga) -> kiedy wrócić
    odrzucone_chwilowo: HashMap<(String, Droga), Instant>,
    /// Za ile wracamy na chwilowo odrzuconą drogę (rośnie po każdym nieudanym powrocie)
    przerwa_powrotu: Duration,
    /// Od kiedy wybranej gry nie ma na liście procesów (zatrzymujemy nagrywanie dopiero po chwili)
    gra_znikla_od: Option<Instant>,
    /// Bufory zatrzymanej sesji: następna sesja z takim samym obrazem nagrywa dalej do nich, więc restart
    /// (np. Fightcade przełącza tryb ekranu co kilka sekund) nie kasuje powtórki
    zachowane: Option<Zachowane>,
}

/// Bufory poprzedniej sesji i to, czym była nagrywana.
struct Zachowane {
    /// Opis strumienia obrazu (monitor, rozmiar, FPS, enkoder, droga) — musi się zgadzać co do znaku
    opis: String,
    bufor_wideo: WspolnyBufor,
    /// (nazwa ścieżki, urządzenie, bufor)
    bufory_audio: Vec<(&'static str, String, WspolnyBufor)>,
    format_wideo: &'static str,
    fps: u32,
    pulpit: bool,
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
            _skrot_filmu: None,
            skroty_wstrzymane: false,
            skrot_filmu_blad: None,
            film: None,
            zapis_filmu: false,
            zmniejszanie_filmu: None,
            pokolenie: 0,
            start_sesji: None,
            wznawianie: None,
            odrzucone: HashSet::new(),
            lekki: false,
            gra: None,
            czeka_na_gre: false,
            seria_awarii: None,
            odrzucone_chwilowo: HashMap::new(),
            przerwa_powrotu: POWROT_NA_DROGE_PO,
            gra_znikla_od: None,
            zachowane: None,
        };
        Rdzen { w: Arc::new(Mutex::new(w)), powiadom: Arc::new(powiadom) }
    }

    /// Blokada stanu odporna na „zatrucie” (awarię innego wątku w trakcie blokady).
    fn zamek(&self) -> MutexGuard<'_, Wnetrze> {
        self.w.zablokuj()
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
            gra: w.gra.as_ref().map(|g| g.nazwa.clone()),
            wybrana_gra: nazwa_wybranej(&w.cfg),
            gra_proces: w.cfg.gra.clone(),
            czeka_na_gre: w.czeka_na_gre,
            mikrofon: w.cfg.mikrofon,
            ramka: w.sesja.as_ref().is_some_and(|s| s._wideo.droga.z_ramka()),
            film: w.film.as_ref().map(Film::sekundy),
            film_zapis: w.zapis_filmu,
            film_zmniejszanie: w.zmniejszanie_filmu,
            skrot_filmu_blad: w.skrot_filmu_blad.clone(),
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
        // Wybrana gra nie działa — nagrywanie czeka na nią (bez alarmu i bez zużycia procesora)
        let wybrana = self.config().wybrana_gra().to_string();
        let czeka = !wybrana.is_empty() && !crate::gra::dziala(&wybrana);
        let mut w = self.zamek();
        w.czeka_na_gre = czeka;
        let pokolenie = w.pokolenie;

        self.zarejestruj_skroty(&mut w);
        if !w.pauza && !czeka {
            let (cfg, lekki) = (w.cfg.clone(), w.lekki);
            let monitor = cfg.aktywny_monitor().0.to_string();
            let mut odrzucone = std::mem::take(&mut w.odrzucone);
            // Drogi odrzucone chwilowo, na które jeszcze nie pora wracać
            let teraz = Instant::now();
            let chwilowo: HashSet<Droga> =
                w.odrzucone_chwilowo.iter().filter(|((m, _), kiedy)| *m == monitor && **kiedy > teraz).map(|((_, d), _)| *d).collect();
            // W serii chwilowych awarii ponawiamy tylko drogę, która je miała
            let tylko = w.seria_awarii.map(|(_, d)| d);
            let mut odlozone = Vec::new();
            // Film zaczyna nowy segment od świeżych buforów (inaczej dostałby powtórzone pakiety)
            let zachowane = if w.film.is_some() { None } else { w.zachowane.take() };
            let wynik = self.nowa_sesja(&cfg, pokolenie, &mut odrzucone, &chwilowo, tylko, lekki, &mut odlozone, zachowane.as_ref());
            // Nowa sesja nagrywa do zachowanych buforów albo do własnych — wtedy stare przepadają.
            // Gdy sesja nie ruszyła, bufory czekają na następną próbę (i na Alt+F10 w międzyczasie).
            if wynik.is_err() {
                w.zachowane = zachowane;
            }
            for (m, d) in &odrzucone {
                w.odrzucone_chwilowo.remove(&(m.clone(), *d));
            }
            w.odrzucone = odrzucone;
            if !odlozone.is_empty() {
                let przerwa = w.przerwa_powrotu;
                for d in odlozone {
                    crate::dziennik::info(&format!("Droga przechwytywania „{}”: spróbuję znów za {} s", d.nazwa(), przerwa.as_secs()));
                    w.odrzucone_chwilowo.insert((monitor.clone(), d), teraz + przerwa);
                }
                w.przerwa_powrotu = (przerwa * 2).min(POWROT_NAJPOZNIEJ_PO);
            }
            match wynik {
                Ok(s) => {
                    // Trwający film nagrywa się dalej z nowej sesji (nowy segment)
                    if let Some(f) = &mut w.film
                        && let Err(e) = f.dodaj_segment(s.obraz(), &s.bufor_wideo, &s.bufory_audio)
                    {
                        crate::dziennik::blad(&format!("Film: {e}"));
                    }
                    w.sesja = Some(s);
                    w.start_sesji = Some(Instant::now());
                }
                // Gra właśnie przełącza tryb ekranu: zaraz spróbujemy jeszcze raz, bez czerwonego błędu
                Err(e) if tylko.is_some() => {
                    crate::dziennik::info(&format!("Nagrywanie jeszcze nie ruszyło ({e}) — ponawiam"));
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

    /// Rejestruje skróty (klip i film) według configu — chyba że okno właśnie ustawia nowy skrót.
    fn zarejestruj_skroty(&self, w: &mut Wnetrze) {
        if w.skroty_wstrzymane {
            return;
        }
        let pokolenie = w.pokolenie;
        let r = self.clone();
        match skrot::nasluchuj(&w.cfg.skrot, move || r.zapisz_klip(), self.zglaszacz(pokolenie, "skrot")) {
            Ok(n) => w._skrot = Some(n),
            Err(e) => {
                w.bledy.insert("skrot", e);
            }
        }
        w.skrot_filmu_blad = None;
        if w.cfg.skrot_filmu.is_empty() {
            return;
        }
        // Zajęty skrót filmu to nie alarm (film da się włączyć z okna) — tylko uwaga w ustawieniach
        let (r, r2) = (self.clone(), self.clone());
        let zglos = move |b: Option<String>| {
            {
                let mut w = r2.zamek();
                if w.pokolenie != pokolenie || w.skrot_filmu_blad == b {
                    return;
                }
                if let Some(b) = &b {
                    crate::dziennik::info(b);
                }
                w.skrot_filmu_blad = b;
            }
            r2.wyslij_stan();
        };
        match skrot::nasluchuj(&w.cfg.skrot_filmu, move || r.przelacz_film(), zglos) {
            Ok(n) => w._skrot_filmu = Some(n),
            Err(e) => w.skrot_filmu_blad = Some(e),
        }
    }

    /// Okno ustawia nowy skrót (true): skróty przestają działać, żeby wciskane klawisze nie zapisywały
    /// klipów ani nie włączały filmu. false = skróty wracają.
    pub fn wstrzymaj_skroty(&self, wstrzymaj: bool) {
        let stare = {
            let mut w = self.zamek();
            if w.skroty_wstrzymane == wstrzymaj {
                return;
            }
            w.skroty_wstrzymane = wstrzymaj;
            if wstrzymaj {
                (w._skrot.take(), w._skrot_filmu.take())
            } else {
                self.zarejestruj_skroty(&mut w);
                (None, None)
            }
        };
        // Poza blokadą: wątek skrótu może właśnie na nią czekać
        drop(stare);
        if wstrzymaj {
            let mut w = self.zamek();
            w.bledy.remove("skrot");
            w.skrot_filmu_blad = None;
        }
        self.wyslij_stan();
    }

    /// Zatrzymuje nagrywanie na stałe (przed instalacją aktualizacji albo zamknięciem programu): ffmpeg
    /// musi się zamknąć, żeby instalator mógł podmienić pliki, a strażnik nie może go wznowić.
    /// Trwający film jest najpierw zapisywany do końca.
    pub fn zakoncz(&self) {
        // Zmniejszanie filmu w tle przerywamy (film zostaje w pełnej jakości): ffmpeg musi się zamknąć
        crate::film::przerwij_zmniejszanie();
        let film = {
            let mut w = self.zamek();
            w.pauza = true;
            let f = w.film.take();
            w.zapis_filmu |= f.is_some();
            f
        };
        if let Some(f) = film {
            crate::dziennik::info("Kończę film przed zamknięciem Clipeona");
            self.zapisz_film(f);
        }
        // Film, który właśnie składa się w tle, też musi zdążyć (najwyżej 10 minut)
        let od = Instant::now();
        while self.zamek().zapis_filmu && od.elapsed() < Duration::from_secs(600) {
            std::thread::sleep(Duration::from_millis(100));
        }
        // Przerwane zmniejszanie kończy się w ułamku sekundy (ffmpeg dostaje sygnał przy najbliższym postępie)
        let od = Instant::now();
        while self.zamek().zmniejszanie_filmu.is_some() && od.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(100));
        }
        self.zatrzymaj(&[]);
    }

    /// Zatrzymuje skróty i sesję; zostawia tylko błędy z listy `zostaw`.
    fn zatrzymaj(&self, zostaw: &[&str]) {
        let (skroty, sesja) = {
            let mut w = self.zamek();
            w.pokolenie += 1;
            w.bledy.retain(|k, _| zostaw.contains(k));
            let sesja = w.sesja.take();
            // Bufory zostają na następną sesję — chyba że trwa film (ten dostaje nowy segment)
            if let Some(s) = sesja.as_ref().filter(|_| w.film.is_none()) {
                w.zachowane = Some(Zachowane {
                    opis: s._wideo.opis.clone(),
                    bufor_wideo: s.bufor_wideo.clone(),
                    bufory_audio: s
                        .bufory_audio
                        .iter()
                        .zip(&s.urzadzenia_audio)
                        .map(|((n, b), u)| (*n, u.clone(), b.clone()))
                        .collect(),
                    format_wideo: s.format_wideo,
                    fps: s._wideo.fps,
                    pulpit: s.pulpit,
                });
            }
            ((w._skrot.take(), w._skrot_filmu.take()), sesja)
        };
        drop(skroty);
        drop(sesja);
    }

    /// Nowa sesja: obraz pierwszą działającą drogą przechwytywania (zapamiętana droga najpierw),
    /// potem dźwięk. Drogi, które nie wystartowały, trafiają do `odrzucone`.
    /// `chwilowo` = drogi pomijane tylko na jakiś czas, `tylko` = jedyna droga, którą wolno teraz
    /// spróbować (trwa seria chwilowych awarii tej drogi).
    #[allow(clippy::too_many_arguments)]
    fn nowa_sesja(
        &self,
        cfg: &Config,
        pokolenie: u64,
        odrzucone: &mut HashSet<(String, Droga)>,
        chwilowo: &HashSet<Droga>,
        tylko: Option<Droga>,
        lekki: bool,
        odlozone: &mut Vec<Droga>,
        zachowane: Option<&Zachowane>,
    ) -> Result<Sesja, String> {
        let monitor = cfg.aktywny_monitor().0.to_string();
        let zapamietana = crate::pamiec::pamiec().drogi.get(&monitor).and_then(|d| Droga::z_nazwy(d));
        let wszystkie = wideo::drogi(cfg);
        let kolejnosc = kolejnosc_prob(
            wszystkie.clone(),
            zapamietana.filter(|_| cfg.metoda_przechwytywania == "auto"),
            |d| d != Droga::Potok && (odrzucone.contains(&(monitor.clone(), d)) || chwilowo.contains(&d)),
            tylko,
        );
        // Lepsza droga jest odrzucona tylko chwilowo — tej nie zapamiętujemy jako „działającej tutaj”
        let zapamietaj_droge = !wszystkie.iter().any(|d| chwilowo.contains(d));
        let mut ostatni_blad = String::new();
        let mut wideo_ = None;
        let mut ciag_dalszy = false;
        for droga in kolejnosc {
            let wybrany = std::cell::RefCell::new(None);
            let wybierz_bufor = |opis: &str| {
                // Ten sam obraz co w poprzedniej sesji: nagrywamy dalej do jej bufora (powtórka nie przepada)
                let b = match zachowane.filter(|z| z.opis == opis) {
                    Some(z) => z.bufor_wideo.clone(),
                    None => Bufor::nowy(cfg.bufor_sekundy),
                };
                *wybrany.borrow_mut() = Some((b.clone(), zachowane.is_some_and(|z| z.opis == opis)));
                b
            };
            let zglos_wideo = self.zglaszacz(pokolenie, "wideo");
            match wideo::uruchom(cfg, droga, lekki, wybierz_bufor, move |e| zglos_wideo(Some(e))) {
                Ok(w) => {
                    let (bufor, dalej) = wybrany.into_inner().expect("wideo::uruchom wybiera bufor");
                    ciag_dalszy = dalej;
                    wideo_ = Some((w, bufor));
                    break;
                }
                Err(e) => {
                    ostatni_blad = e.to_string();
                    crate::dziennik::info(&format!("Droga przechwytywania „{}” nie wystartowała: {e}", droga.nazwa()));
                    // W serii chwilowych awarii (gra zmienia tryb ekranu) nieudany start o niczym nie przesądza.
                    // Chwilowa utrata obrazu przy starcie (np. powrót z trybu zgodności w trakcie zmiany trybu
                    // ekranu) odkłada drogę tylko na jakiś czas — dawniej odrzucała ją do końca uruchomienia
                    // i żółta ramka zostawała do restartu Clipeona.
                    if droga != Droga::Potok && tylko.is_none() {
                        if wideo::chwilowy(&ostatni_blad) {
                            odlozone.push(droga);
                        } else {
                            odrzucone.insert((monitor.clone(), droga));
                        }
                    }
                }
            }
        }
        let Some((wideo, bufor_wideo)) = wideo_ else { return Err(ostatni_blad) };

        // Dźwięk: AAC 160 kb/s; bufor dłuższy o 2 s, żeby na pewno pokrył początek obrazu
        // Gdy obraz nagrywa się dalej do starego bufora, dźwięk też (to samo urządzenie = ta sama ścieżka)
        let (mut audio_, mut bufory_audio, mut urzadzenia_audio) = (Vec::new(), Vec::new(), Vec::new());
        for (wlaczone, zrodlo, urzadzenie, klucz, nazwa) in [
            (cfg.dzwiek_systemowy, Zrodlo::System, String::new(), "system", "Gra"),
            (cfg.mikrofon, Zrodlo::Mikrofon, cfg.mikrofon_urzadzenie.clone(), "mikrofon", "Mikrofon"),
        ] {
            if wlaczone {
                let stary = zachowane
                    .filter(|_| ciag_dalszy)
                    .and_then(|z| z.bufory_audio.iter().find(|(n, u, _)| *n == nazwa && *u == urzadzenie))
                    .map(|(_, _, b)| b.clone());
                let b = stary.unwrap_or_else(|| Bufor::nowy(cfg.bufor_sekundy + 2));
                audio_.push(audio::uruchom(zrodlo, urzadzenie.clone(), 160, b.clone(), self.zglaszacz(pokolenie, klucz)));
                bufory_audio.push((nazwa, b));
                urzadzenia_audio.push(urzadzenie);
            }
        }
        if ciag_dalszy {
            crate::dziennik::info("Nagrywanie wznowione — powtórka sprzed przerwy zostaje w buforze");
        }
        let format_wideo = wideo.format;
        Ok(Sesja {
            _wideo: wideo,
            _audio: audio_,
            bufor_wideo,
            bufory_audio,
            urzadzenia_audio,
            format_wideo,
            monitor,
            pulpit: cfg.pulpit(),
            zapamietaj_droge,
        })
    }

    /// Nowy config: zapis na dysk i restart nagrywania. Zawartość bufora przepada.
    pub fn zmien_config(&self, mut cfg: Config) -> Result<(), String> {
        cfg.sprawdz()?;
        // Nowe ustawienia mogą zmienić rozmiar albo format obrazu — trwający film kończymy na starych
        self.zakoncz_film(Some("zmieniły się ustawienia nagrywania".into()));
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
                w.odrzucone_chwilowo.clear();
                w.przerwa_powrotu = POWROT_NA_DROGE_PO;
                w.lekki = false;
            }
            w.seria_awarii = None;
            // Nowe ustawienia (długość bufora, dźwięk, jakość) = nowe bufory
            w.zachowane = None;
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

    /// Wybór gry z paska w oknie (pusty proces = nagrywaj zawsze). Bez restartu nagrywania:
    /// strażnik gry od razu uruchamia albo wstrzymuje bufor, gdy trzeba.
    pub fn ustaw_gre(&self, proces: String, nazwa: String) {
        crate::dziennik::info(&if proces.is_empty() { "Gra: automatycznie".to_string() } else { format!("Gra: {nazwa} ({proces})") });
        self.zmien_bez_restartu(|c| {
            c.gra = proces;
            c.gra_nazwa = nazwa;
        });
        self.pilnuj_gry();
        self.wyslij_stan();
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
            w.zachowane = None;
            w.pauza
        };
        if pauza {
            // Skrót zostaje, żeby zgłosić "bufor wstrzymany"; zatrzymujemy tylko sesję
            let sesja = {
                let mut w = self.zamek();
                w.bledy.retain(|k, _| matches!(*k, "config" | "skrot"));
                w.wznawianie = None;
                w.seria_awarii = None;
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
        let (migawka, folder, brak_sesji, gra) = {
            let w = self.zamek();
            let migawka = w.sesja.as_ref().map(|s| {
                let wideo = s.bufor_wideo.zablokuj().migawka();
                let audio: Vec<_> =
                    s.bufory_audio.iter().map(|(n, b)| (*n, b.zablokuj().migawka())).collect();
                (wideo, audio, s.format_wideo, s._wideo.fps, s.pulpit)
            });
            // Alt+F10 akurat w chwili wznawiania nagrywania: zapisujemy to, co zostało w buforze
            let migawka = migawka.or_else(|| {
                w.zachowane.as_ref().filter(|_| !w.pauza && !w.czeka_na_gre).map(|z| {
                    let wideo = z.bufor_wideo.zablokuj().migawka();
                    let audio: Vec<_> = z.bufory_audio.iter().map(|(n, _, b)| (*n, b.zablokuj().migawka())).collect();
                    (wideo, audio, z.format_wideo, z.fps, z.pulpit)
                })
            });
            (migawka, w.cfg.folder_klipow.clone(), brak_sesji(&w, "zapisywać klipy"), w.gra.as_ref().map(|g| g.nazwa.clone()))
        };
        let powiadom = self.powiadom.clone();
        let Some((wideo, audio, format_wideo, fps, pulpit)) = migawka else {
            powiadom(Zdarzenie::BladZapisu(brak_sesji));
            return;
        };
        let przedrostek = if pulpit { "Clipeon_Pulpit" } else { "Clipeon" };
        std::thread::spawn(move || match zapis::zapisz(&folder, przedrostek, fps, format_wideo, wideo, audio, gra) {
            Ok(plik) => powiadom(Zdarzenie::Zapisano(plik)),
            Err(e) => {
                crate::dziennik::blad(&format!("Nie udało się zapisać klipu: {e}"));
                powiadom(Zdarzenie::BladZapisu(e));
            }
        });
    }

    /// Zaczyna albo kończy długie nagrywanie (skrót, przycisk w oknie, menu przy zegarze).
    pub fn przelacz_film(&self) {
        if self.zamek().film.is_some() {
            self.zakoncz_film(None);
        } else {
            self.zacznij_film();
        }
    }

    fn zacznij_film(&self) {
        let wynik = {
            let mut w = self.zamek();
            if w.zapis_filmu {
                Err("Poprzedni film jeszcze się zapisuje — poczekaj chwilę".to_string())
            } else if w.sesja.is_none() {
                Err(brak_sesji(&w, "nagrać film"))
            } else {
                let gra = w.gra.as_ref().map(|g| g.nazwa.clone());
                Film::nowy(&w.cfg.folder_klipow, gra).and_then(|mut f| {
                    let s = w.sesja.as_ref().unwrap();
                    f.dodaj_segment(s.obraz(), &s.bufor_wideo, &s.bufory_audio)?;
                    w.film = Some(f);
                    Ok(())
                })
            }
        };
        match wynik {
            Ok(()) => {
                crate::dziennik::info("Film: start długiego nagrywania");
                self.wyslij_stan();
                (self.powiadom)(Zdarzenie::FilmStart);
            }
            Err(e) => {
                crate::dziennik::blad(&format!("Nie udało się zacząć filmu: {e}"));
                (self.powiadom)(Zdarzenie::BladFilmu(e));
            }
        }
    }

    /// Kończy film i składa plik w tle. `powod` = film kończy się sam (np. brakuje miejsca na dysku).
    pub fn zakoncz_film(&self, powod: Option<String>) {
        let film = {
            let mut w = self.zamek();
            let f = w.film.take();
            w.zapis_filmu |= f.is_some();
            f
        };
        let Some(film) = film else { return };
        crate::dziennik::info(&match &powod {
            Some(p) => format!("Film: koniec nagrywania ({p}) — składam plik"),
            None => "Film: koniec nagrywania — składam plik".to_string(),
        });
        self.wyslij_stan();
        (self.powiadom)(Zdarzenie::FilmKoniec(powod));
        let r = self.clone();
        std::thread::spawn(move || r.zapisz_film(film));
    }

    /// Składa zakończony film w plik (trwa od ułamka sekundy do minut — zależnie od długości).
    fn zapisz_film(&self, film: Film) {
        let wynik = film.zakoncz();
        self.zamek().zapis_filmu = false;
        self.wyslij_stan();
        match wynik {
            Ok(w) => {
                let czesci = w.pliki.len();
                (self.powiadom)(Zdarzenie::FilmZapisany { plik: w.pliki[0].clone(), sekundy: w.sekundy, czesci });
                self.zmniejsz_filmy(w.pliki);
            }
            Err(e) => {
                crate::dziennik::blad(&format!("Nie udało się zapisać filmu: {e}"));
                (self.powiadom)(Zdarzenie::BladFilmu(e));
            }
        }
    }

    /// Zmniejsza zapisane filmy do formatu z ustawień (kartą graficzną, z niskim priorytetem). Film jest
    /// już w bibliotece w pełnej jakości i tak zostaje, jeśli coś pójdzie nie tak.
    fn zmniejsz_filmy(&self, pliki: Vec<PathBuf>) {
        let format = self.config().format_filmu;
        if format == crate::film::BEZ_ZMIAN || crate::film::zmniejszanie_przerwane() {
            return;
        }
        for plik in pliki {
            crate::dziennik::info(&format!("Film: zmniejszam {} do formatu {format}", plik.display()));
            self.ustaw_zmniejszanie(Some(0));
            let r = self.clone();
            let wynik = crate::film::zmniejsz(&plik, &format, &move |p| r.ustaw_zmniejszanie(Some((p * 100.0) as u8)));
            self.ustaw_zmniejszanie(None);
            match wynik {
                Ok(Some(z)) => {
                    crate::dziennik::info(&format!("Film: zmniejszony z {:.2} GB do {:.2} GB", z.przed as f64 / 1e9, z.po as f64 / 1e9));
                    (self.powiadom)(Zdarzenie::FilmZmniejszony { plik, przed: z.przed, po: z.po });
                }
                Ok(None) => crate::dziennik::info("Film: zostaje bez zmian (i tak jest mały)"),
                // Zamykanie programu: bez dymka, film po prostu zostaje w pełnej jakości
                Err(_) if crate::film::zmniejszanie_przerwane() => crate::dziennik::info("Film: zmniejszanie przerwane, zostaje pełna jakość"),
                Err(e) => {
                    crate::dziennik::blad(&format!("Nie udało się zmniejszyć filmu: {e}"));
                    (self.powiadom)(Zdarzenie::FilmNiezmniejszony(e));
                }
            }
        }
    }

    fn ustaw_zmniejszanie(&self, procent: Option<u8>) {
        {
            let mut w = self.zamek();
            if w.zmniejszanie_filmu == procent {
                return;
            }
            w.zmniejszanie_filmu = procent;
        }
        self.wyslij_stan();
    }

    /// Co kilka sekund ze strażnika: film kończy się sam, gdy nie da się go dalej zapisywać.
    fn pilnuj_filmu(&self) {
        let problem = self.zamek().film.as_ref().and_then(Film::problem);
        if let Some(p) = problem {
            self.zakoncz_film(Some(p));
        }
    }

    /// Rozpoznaje grę na wierzchu nagrywanego ekranu i — gdy w ustawieniach wybrano konkretną grę —
    /// uruchamia nagrywanie, kiedy gra wystartuje, i zatrzymuje je, kiedy jej nie ma.
    /// Wołane co sekundę przez strażnika.
    fn pilnuj_gry(&self) {
        let (wybrana, monitor, pauza, ma_sesje, czekal) = {
            let w = self.zamek();
            (w.cfg.wybrana_gra().to_string(), w.cfg.aktywny_monitor().0.to_string(), w.pauza, w.sesja.is_some(), w.czeka_na_gre)
        };
        if pauza {
            return;
        }
        // Nazwa gry na wierzchu — tylko do pokazania w oknie i w zasobniku
        let teraz = crate::gra::na_pierwszym_planie(&monitor);
        let zmiana_nazwy = {
            let mut w = self.zamek();
            let zmiana = w.gra != teraz;
            w.gra = teraz;
            zmiana
        };

        if wybrana.is_empty() {
            if czekal {
                // Wybór gry skasowany, a bufor czekał na grę — nagrywamy od razu
                self.uruchom();
            } else if zmiana_nazwy {
                self.wyslij_stan();
            }
            return;
        }

        let dziala = crate::gra::dziala(&wybrana);
        // Gra może na moment zniknąć (restart, zmiana trybu ekranu) — bufor wyrzucamy dopiero,
        // gdy nie ma jej dłużej niż GRA_ZNIKA_PO
        let zniknela_na_dobre = {
            let mut w = self.zamek();
            if dziala {
                w.gra_znikla_od = None;
                false
            } else {
                w.gra_znikla_od.get_or_insert_with(Instant::now).elapsed() >= GRA_ZNIKA_PO
            }
        };
        if dziala && czekal {
            crate::dziennik::info(&format!("Wybrana gra ({wybrana}) wystartowała — zaczynam nagrywać"));
            self.uruchom();
        } else if zniknela_na_dobre && ma_sesje {
            crate::dziennik::info(&format!("Wybrana gra ({wybrana}) się zamknęła — czekam na nią"));
            let sesja = {
                let mut w = self.zamek();
                w.czeka_na_gre = true;
                w.zachowane = None;
                w.wznawianie = None;
                // Błędy zatrzymanej sesji nie dotyczą czekania na grę (inaczej wisi czerwony baner)
                w.bledy.remove("nagrywanie");
                w.bledy.remove("wideo");
                w.sesja.take()
            };
            drop(sesja);
            self.wyslij_stan();
        } else if zmiana_nazwy {
            self.wyslij_stan();
        }
    }

    /// Co nie działa w nagrywaniu obrazu (None = działa albo jest wstrzymane).
    fn problem_nagrywania(&self, po_uspieniu: bool) -> Option<String> {
        let w = self.zamek();
        if w.pauza || w.czeka_na_gre {
            return None;
        }
        let Some(sesja) = &w.sesja else {
            // W serii chwilowych awarii brak sesji to dalej „chwilowa utrata” (cichy alarm dopiero po minucie)
            let domyslny = if w.seria_awarii.is_some() {
                "chwilowa utrata obrazu przy zmianie trybu ekranu"
            } else {
                "nagrywanie nie wystartowało"
            };
            return Some(w.bledy.get("wideo").cloned().unwrap_or_else(|| domyslny.into()));
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
        let ostatnia = sesja.bufor_wideo.zablokuj().ostatni_czas();
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

                // Jaka gra jest na wierzchu; przy wybranej grze: start, gdy rusza, stop, gdy znika
                r.pilnuj_gry();

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
                if obieg.is_multiple_of(5) {
                    r.pilnuj_filmu();
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
    /// następny start weźmie kolejną. Chwilowa utrata obrazu (gra zmienia tryb ekranu, okno UAC) zaczyna
    /// „serię”: ponawiamy tę samą drogę, a dopiero gdy seria trwa za długo, bierzemy następną — na jakiś czas.
    fn odrzuc_droge_po_problemie(&self, powod: &str, chwilowy: bool) {
        let mut w = self.zamek();
        let monitor = w.cfg.aktywny_monitor().0.to_string();
        let w_sesji = w.sesja.as_ref().map(|s| s._wideo.droga);
        let mlody = w.start_sesji.is_some_and(|t| t.elapsed() < ROZRUCH_DROGI);
        let mut decyzja = co_z_droga(w_sesji, w.seria_awarii, chwilowy, mlody, Instant::now());
        // Seria trwa długo, ale sesje na tej drodze startują (padają dopiero po chwili), a każda inna droga
        // rysowałaby żółtą ramkę (Windows 10): zostajemy bez ramki. Bufor przetrwa restart, więc kosztem są
        // tylko krótkie przeskoki obrazu w klipie (Fightcade u kolegi gubił obraz co 2–10 s).
        if let (Decyzja::OdrzucChwilowo(d), Some(teraz), Some(seria)) = (decyzja, w_sesji, w.seria_awarii)
            && teraz == d
            && !d.z_ramka()
            && wideo::drogi(&w.cfg).iter().filter(|x| **x != d).all(|x| x.z_ramka())
        {
            decyzja = Decyzja::Ponow(seria);
        }
        // Desktop Duplication w ffmpeg straciło obraz, a jest DDA w Clipeonie, które przeczeka taką chwilę bez
        // restartu (jak OBS): przechodzimy na nie od razu, zamiast restartować ffmpeg w kółko
        if let Decyzja::Ponow((_, Droga::Dda)) = decyzja
            && w.seria_awarii.is_none()
            && !w.odrzucone.contains(&(monitor.clone(), Droga::DdaRam))
            && wideo::drogi(&w.cfg).contains(&Droga::DdaRam)
        {
            crate::dziennik::info(&format!(
                "Droga przechwytywania „dda” chwilowo straciła obraz ({powod}) — przechodzę na „dda-ram”, która przeczekuje takie chwile bez restartu"
            ));
            w.seria_awarii = None;
            let wroc = Instant::now() + w.przerwa_powrotu;
            w.odrzucone_chwilowo.insert((monitor.clone(), Droga::Dda), wroc);
            drop(w);
            let mut p = crate::pamiec::pamiec();
            if p.drogi.get(&monitor).is_some_and(|d| d == Droga::Dda.nazwa()) {
                p.drogi.remove(&monitor);
                crate::pamiec::zapisz(&p);
            }
            return;
        }
        match decyzja {
            Decyzja::Nic => return,
            Decyzja::Ponow(seria) => {
                if w.seria_awarii.is_none() {
                    crate::dziennik::info(&format!(
                        "Droga przechwytywania „{}” chwilowo straciła obraz ({powod}) — ponawiam tę samą",
                        seria.1.nazwa()
                    ));
                }
                w.seria_awarii = Some(seria);
                return;
            }
            Decyzja::OdrzucChwilowo(droga) => {
                let przerwa = w.przerwa_powrotu;
                crate::dziennik::info(&format!(
                    "Droga przechwytywania „{}” traci obraz raz za razem ({powod}) — na {} s przechodzę na następną",
                    droga.nazwa(),
                    przerwa.as_secs()
                ));
                w.seria_awarii = None;
                w.odrzucone_chwilowo.insert((monitor.clone(), droga), Instant::now() + przerwa);
                w.przerwa_powrotu = (przerwa * 2).min(POWROT_NAJPOZNIEJ_PO);
            }
            Decyzja::Odrzuc(droga) => {
                crate::dziennik::info(&format!(
                    "Droga przechwytywania „{}” nie działa na tym komputerze ({powod}) — przechodzę na następną",
                    droga.nazwa()
                ));
                w.odrzucone.insert((monitor.clone(), droga));
                w.odrzucone_chwilowo.remove(&(monitor.clone(), droga));
            }
        }
        let (Decyzja::OdrzucChwilowo(droga) | Decyzja::Odrzuc(droga)) = decyzja else { return };
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
            Powrot(Droga),
        }
        let akcja = {
            let mut w = self.zamek();
            let dlugo = w.start_sesji.is_some_and(|t| t.elapsed() >= DROGA_DZIALA_PO);
            // Sesja trzyma się już chwilę: seria chwilowych awarii (zmiana trybu ekranu) minęła
            if w.start_sesji.is_some_and(|t| t.elapsed() >= SESJA_STABILNA_PO) {
                w.seria_awarii = None;
            }
            let lekki = w.lekki;
            let Some(s) = &w.sesja else { return };
            let (monitor, droga) = (s.monitor.clone(), s._wideo.droga);
            if dlugo && s.zapamietaj_droge {
                let mut p = crate::pamiec::pamiec();
                if p.drogi.get(&monitor).map(String::as_str) != Some(droga.nazwa()) {
                    p.drogi.insert(monitor.clone(), droga.nazwa().to_string());
                    crate::pamiec::zapisz(&p);
                }
            }
            // Pora wrócić na drogę odrzuconą chwilowo? Tylko gdy obecna rysuje żółtą ramkę — powrót
            // zaczyna bufor od nowa, więc bez potrzeby go nie robimy (wtedy wracamy przy następnym starcie)
            let teraz = Instant::now();
            let powrot = w
                .odrzucone_chwilowo
                .iter()
                .find(|((m, d), kiedy)| *m == monitor && *d != droga && **kiedy <= teraz)
                .map(|((_, d), _)| *d);
            let przeciazenie = s._wideo.przeciazenie.load(Ordering::SeqCst);
            let z_ramka = droga.z_ramka();
            let zastepczy_wrocil =
                obieg.is_multiple_of(5) && s._wideo.zastepczy.as_ref().is_some_and(|(n, id)| wideo::monitor_wrocil(n, id));
            if dlugo && droga.w_ffmpeg() && !z_ramka {
                // Droga bez ramki działa stabilnie: następny ewentualny powrót znów po krótkiej przerwie
                w.przerwa_powrotu = POWROT_NA_DROGE_PO;
                w.odrzucone_chwilowo.retain(|(m, d), _| !(*m == monitor && *d == droga));
            }
            if let Some(d) = powrot.filter(|_| z_ramka) {
                w.odrzucone_chwilowo.remove(&(monitor, d));
                Akcja::Powrot(d)
            } else if przeciazenie && !lekki {
                w.lekki = true;
                Akcja::Lekki
            } else if zastepczy_wrocil {
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
            Akcja::Powrot(droga) => {
                crate::dziennik::info(&format!(
                    "Strażnik: wracam na drogę „{}” (bez żółtej ramki) — uruchamiam nagrywanie od nowa",
                    droga.nazwa()
                ));
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

/// Dlaczego nie da się teraz zapisać klipu / nagrać filmu (nie ma sesji nagrywania).
fn brak_sesji(w: &Wnetrze, zeby: &str) -> String {
    if w.pauza {
        format!("Nagrywanie jest wstrzymane — wznów je, żeby {zeby}")
    } else if w.czeka_na_gre {
        format!("Clipeon czeka na {} — nagrywanie ruszy, gdy gra wystartuje", nazwa_wybranej(&w.cfg))
    } else {
        "Nagrywanie teraz nie działa — Clipeon próbuje je wznowić".to_string()
    }
}

/// Kolejność dróg do wypróbowania przy starcie sesji.
/// - `tylko`: trwa seria chwilowych awarii tej drogi — próbujemy wyłącznie jej (jeśli w ogóle jest możliwa),
/// - `zapamietana`: droga, która ostatnio działała na tym monitorze, idzie na początek — chyba że rysuje
///   żółtą ramkę, a możliwa jest droga bez ramki (wtedy ta bez ramki zawsze ma pierwszeństwo),
/// - `pomin`: drogi odrzucone (na stałe albo chwilowo); tryb zgodności zostaje zawsze jako ostatnia deska.
fn kolejnosc_prob(mut drogi: Vec<Droga>, zapamietana: Option<Droga>, pomin: impl Fn(Droga) -> bool, tylko: Option<Droga>) -> Vec<Droga> {
    if let Some(t) = tylko.filter(|t| drogi.contains(t)) {
        return vec![t];
    }
    let pierwsza_bez_ramki = drogi.first().is_some_and(|d| !d.z_ramka() && !pomin(*d));
    if let Some(z) = zapamietana.filter(|z| drogi.contains(z) && !(z.z_ramka() && pierwsza_bez_ramki)) {
        drogi.retain(|d| *d != z);
        drogi.insert(0, z);
    }
    drogi.retain(|d| !pomin(*d));
    drogi
}

/// Co zrobić z drogą przechwytywania po problemie zgłoszonym przez strażnika.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Decyzja {
    Nic,
    /// Chwilowa utrata obrazu: ponawiamy tę samą drogę (seria: od kiedy, która droga)
    Ponow((Instant, Droga)),
    /// Seria trwa za długo: następna droga, ale z powrotem po przerwie
    OdrzucChwilowo(Droga),
    /// Droga nie działa na tym komputerze
    Odrzuc(Droga),
}

/// `w_sesji` = droga działającej sesji (None = sesja nie wystartowała), `seria` = trwająca seria
/// chwilowych awarii, `mlody` = sesja wystartowała przed chwilą.
fn co_z_droga(w_sesji: Option<Droga>, seria: Option<(Instant, Droga)>, chwilowy: bool, mlody: bool, teraz: Instant) -> Decyzja {
    let za_dlugo = |od: Instant| teraz.duration_since(od) >= SERIA_AWARII_DO_ZMIANY;
    let Some(droga) = w_sesji else {
        // Sesja nie wystartowała: w serii ponawiamy do skutku albo do końca cierpliwości
        return match seria {
            Some((od, d)) if za_dlugo(od) => Decyzja::OdrzucChwilowo(d),
            Some(s) => Decyzja::Ponow(s),
            None => Decyzja::Nic,
        };
    };
    if !droga.w_ffmpeg() {
        return Decyzja::Nic;
    }
    match seria {
        // W serii każdy problem świeżej sesji (także „brak klatek”) to dalej ta sama zmiana trybu ekranu
        Some((od, d)) if chwilowy || mlody => {
            if za_dlugo(od) {
                Decyzja::OdrzucChwilowo(d)
            } else {
                Decyzja::Ponow((od, d))
            }
        }
        None if chwilowy => Decyzja::Ponow((teraz, droga)),
        None if mlody => Decyzja::Odrzuc(droga),
        _ => Decyzja::Nic,
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
    let mut od = od.zablokuj();
    let jest = r.zamek().bledy.keys().any(|k| *k != "nagrywanie");
    if !jest {
        *od = None;
        return false;
    }
    od.get_or_insert_with(Instant::now).elapsed() >= ALARM_PO
}

/// Nazwa wybranej gry do pokazania („Czekam na …”): ładna z ustawień albo sama nazwa pliku.
fn nazwa_wybranej(cfg: &Config) -> String {
    if cfg.gra_nazwa.is_empty() { cfg.gra.clone() } else { cfg.gra_nazwa.clone() }
}

#[cfg(test)]
mod testy {
    use super::*;

    const S: Duration = Duration::from_secs(1);

    #[test]
    fn zmiana_trybu_ekranu_nie_zmienia_drogi() {
        let t0 = Instant::now();
        // Pierwsza chwilowa awaria Desktop Duplication zaczyna serię; droga zostaje
        let d = co_z_droga(Some(Droga::Dda), None, true, false, t0);
        assert_eq!(d, Decyzja::Ponow((t0, Droga::Dda)));
        // Kolejne awarie w serii (Fightcade: pełny ekran -> okno -> pełny ekran) dalej tylko ponawiają
        for sekundy in [2, 6, 12, 29] {
            let d = co_z_droga(Some(Droga::Dda), Some((t0, Droga::Dda)), true, true, t0 + S * sekundy);
            assert_eq!(d, Decyzja::Ponow((t0, Droga::Dda)), "{sekundy} s");
        }
        // W serii „brak klatek” świeżej sesji też nie skreśla drogi na stałe
        assert_eq!(co_z_droga(Some(Droga::Dda), Some((t0, Droga::Dda)), false, true, t0 + S * 8), Decyzja::Ponow((t0, Droga::Dda)));
        // Sesja nie wystartowała w trakcie serii: ponawiamy
        assert_eq!(co_z_droga(None, Some((t0, Droga::Dda)), false, false, t0 + S * 5), Decyzja::Ponow((t0, Droga::Dda)));
    }

    #[test]
    fn dluga_seria_awarii_odrzuca_droge_tylko_chwilowo() {
        let t0 = Instant::now();
        let seria = Some((t0, Droga::Dda));
        assert_eq!(co_z_droga(Some(Droga::Dda), seria, true, true, t0 + S * 30), Decyzja::OdrzucChwilowo(Droga::Dda));
        assert_eq!(co_z_droga(None, seria, false, false, t0 + S * 31), Decyzja::OdrzucChwilowo(Droga::Dda));
    }

    #[test]
    fn droga_ktora_nie_dziala_od_startu_odpada_na_stale() {
        let t0 = Instant::now();
        assert_eq!(co_z_droga(Some(Droga::Dda), None, false, true, t0), Decyzja::Odrzuc(Droga::Dda));
        // Stara sesja z problemem innym niż chwilowy: zwykły restart, droga zostaje
        assert_eq!(co_z_droga(Some(Droga::Dda), None, false, false, t0), Decyzja::Nic);
        // Tryb zgodności nie ma dokąd przejść
        assert_eq!(co_z_droga(Some(Droga::Potok), None, true, true, t0), Decyzja::Nic);
        assert_eq!(co_z_droga(None, None, false, false, t0), Decyzja::Nic);
    }

    #[test]
    fn kolejnosc_prob_drog() {
        let wszystkie = || vec![Droga::Dda, Droga::Gfx, Droga::Potok];
        let nic = |_: Droga| false;
        assert_eq!(kolejnosc_prob(wszystkie(), None, nic, None), wszystkie());
        // Seria chwilowych awarii: tylko ta sama droga
        assert_eq!(kolejnosc_prob(wszystkie(), Some(Droga::Potok), nic, Some(Droga::Dda)), vec![Droga::Dda]);
        // …chyba że ustawienia już jej nie dopuszczają
        assert_eq!(kolejnosc_prob(vec![Droga::Potok], None, nic, Some(Droga::Dda)), vec![Droga::Potok]);
        // Odrzucone drogi wypadają, tryb zgodności zostaje
        assert_eq!(kolejnosc_prob(wszystkie(), None, |d| d == Droga::Dda, None), vec![Droga::Gfx, Droga::Potok]);
        // Zapamiętana droga bez ramki idzie na początek
        assert_eq!(kolejnosc_prob(vec![Droga::Gfx, Droga::Dda, Droga::Potok], Some(Droga::Dda), nic, None), wszystkie());
    }
}
