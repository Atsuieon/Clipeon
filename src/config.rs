// Konfiguracja z %APPDATA%\Clipeon\config.toml.
// Jeśli pliku nie ma, tworzymy go z wartościami domyślnymi.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Ile ostatnich sekund trzymać w pamięci
    pub bufor_sekundy: u32,
    /// Stała liczba klatek na sekundę nagrania
    pub fps: u32,
    /// Bitrate wideo w Mb/s (VBR)
    pub bitrate_mbps: u32,
    /// Enkoder wideo („auto” albo nazwa z enkodery::KANDYDACI)
    pub enkoder: String,
    /// Nagrywanie dźwięku systemowego (to, co słychać w głośnikach)
    pub dzwiek_systemowy: bool,
    /// Nagrywanie mikrofonu (domieszanego do dźwięku systemowego przy zapisie)
    pub mikrofon: bool,
    /// Folder, do którego trafiają klipy
    pub folder_klipow: PathBuf,
    /// Skrót zapisu klipu, np. "Alt+F10", "Ctrl+Shift+S"
    pub skrot: String,
    /// Ikona w zasobniku zawsze widoczna przy zegarze (a nie w ukrytych ikonach pod ^)
    pub ikona_przy_zegarze: bool,
    /// Czy Clipeon już raz ustawił widoczność ikony (później decyduje użytkownik w Windows)
    pub ikona_ustawiona: bool,
    /// Dźwięk po zapisie klipu
    pub dzwiek_zapisu: bool,
    /// Dymek w prawym górnym rogu ekranu po zapisie klipu
    pub powiadomienie_zapisu: bool,
    /// Sprzątanie: klipy starsze niż tyle dni idą do Kosza (0 = wyłączone)
    pub sprzatanie_dni: u32,
    /// Sprzątanie: folder klipów nie większy niż tyle GB (0 = wyłączone)
    pub sprzatanie_gb: u32,
    /// Czy zapisywane wersje (przycięte, zmniejszone) mają domyślnie zawierać głos z mikrofonu
    pub glos_w_edytorze: bool,
    /// Sprawdzanie przy starcie i raz na dobę, czy na GitHubie jest nowsza wersja
    pub sprawdzaj_aktualizacje: bool,
    /// Nagrywany monitor w trybie gry (nazwa urządzenia, np. \\.\DISPLAY2); pusty = główny
    pub monitor: String,
    /// Stały identyfikator tego monitora (nazwa DISPLAYn potrafi się zmienić po aktualizacji
    /// sterownika albo przełożeniu kabla); uzupełniany przez Clipeona przy zapisie ustawień
    pub monitor_id: String,
    /// Tryb nagrywania: "gra" albo "pulpit"
    pub tryb: String,
    /// Monitor w trybie pulpitu (jak `monitor`)
    pub monitor_pulpitu: String,
    pub monitor_pulpitu_id: String,
    /// Klatki na sekundę w trybie pulpitu
    pub fps_pulpitu: u32,
    /// Bitrate w trybie pulpitu (Mb/s)
    pub bitrate_pulpitu_mbps: u32,
    /// Metoda przechwytywania: "auto", "dda" (Desktop Duplication), "gfx" (Windows Graphics Capture),
    /// "potok" (tryb zgodności: obraz przez pamięć RAM, jak w wersjach do 1.2)
    pub metoda_przechwytywania: String,
    /// Mikrofon do nagrywania: identyfikator urządzenia (pusty = domyślny systemowy)
    pub mikrofon_urzadzenie: String,
    /// Gra do nagrywania: nazwa pliku programu, np. „Cyberpunk2077.exe”
    /// (pusta = nagrywaj zawsze, bez względu na to, co jest na wierzchu)
    pub gra: String,
    /// Ładna nazwa wybranej gry (żeby pokazać „Czekam na …”, zanim gra wystartuje)
    pub gra_nazwa: String,
}

pub const TRYB_GRA: &str = "gra";
pub const TRYB_PULPIT: &str = "pulpit";
pub const METODY: &[&str] = &["auto", "dda", "gfx", "potok"];

impl Default for Config {
    fn default() -> Self {
        let profil = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Self {
            bufor_sekundy: 30,
            fps: 60,
            bitrate_mbps: 20,
            enkoder: crate::enkodery::AUTO.into(),
            dzwiek_systemowy: true,
            mikrofon: true,
            folder_klipow: profil.join("Videos").join("Clipeon"),
            skrot: "Alt+F10".into(),
            ikona_przy_zegarze: true,
            ikona_ustawiona: false,
            dzwiek_zapisu: true,
            powiadomienie_zapisu: true,
            sprzatanie_dni: 0,
            sprzatanie_gb: 0,
            monitor: String::new(),
            monitor_id: String::new(),
            tryb: TRYB_GRA.into(),
            monitor_pulpitu: String::new(),
            monitor_pulpitu_id: String::new(),
            fps_pulpitu: 30,
            bitrate_pulpitu_mbps: 12,
            metoda_przechwytywania: "auto".into(),
            mikrofon_urzadzenie: String::new(),
            gra: String::new(),
            gra_nazwa: String::new(),
            glos_w_edytorze: true,
            sprawdzaj_aktualizacje: true,
        }
    }
}

/// Czy przy tym starcie przeniesiono ustawienia z Clippera 1.0 (wtedy też autostart trzeba przenieść).
pub static PRZENIESIONO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Config {
    pub fn pulpit(&self) -> bool {
        self.tryb == TRYB_PULPIT
    }

    /// Gra, na którą czeka nagrywanie (nazwa pliku programu). W trybie Pulpit zawsze pusta:
    /// pulpit nagrywa się bez względu na grę, a wybór gry zostaje zapamiętany na powrót do trybu Gra.
    pub fn wybrana_gra(&self) -> &str {
        if self.pulpit() { "" } else { &self.gra }
    }

    /// Monitor nagrywany w bieżącym trybie: (nazwa urządzenia, stały identyfikator).
    pub fn aktywny_monitor(&self) -> (&str, &str) {
        if self.pulpit() {
            (&self.monitor_pulpitu, &self.monitor_pulpitu_id)
        } else {
            (&self.monitor, &self.monitor_id)
        }
    }

    pub fn aktywne_fps(&self) -> u32 {
        if self.pulpit() { self.fps_pulpitu } else { self.fps }
    }

    pub fn aktywny_bitrate(&self) -> u32 {
        if self.pulpit() { self.bitrate_pulpitu_mbps } else { self.bitrate_mbps }
    }

    fn sciezka() -> Result<PathBuf, String> {
        let appdata = std::env::var_os("APPDATA").ok_or("Brak zmiennej APPDATA")?;
        Ok(PathBuf::from(appdata).join("Clipeon").join("config.toml"))
    }

    /// Wczytuje config (albo tworzy domyślny). Błąd składni pliku jest zgłaszany, a nie ignorowany.
    pub fn wczytaj() -> Result<Self, String> {
        let sciezka = Self::sciezka()?;
        Self::przenies_z_clippera(&sciezka);
        if !sciezka.exists() {
            let domyslny = Config::default();
            domyslny.zapisz()?;
            println!("Utworzono domyślny config: {}", sciezka.display());
            return Ok(domyslny);
        }
        let tekst = std::fs::read_to_string(&sciezka).map_err(|e| format!("Nie mogę odczytać {}: {e}", sciezka.display()))?;
        let cfg: Config = toml::from_str(&tekst).map_err(|e| format!("Błąd w {}: {e}", sciezka.display()))?;
        cfg.sprawdz().map_err(|e| format!("Błąd w {}: {e}", sciezka.display()))?;
        println!("Config: {}", sciezka.display());
        Ok(cfg)
    }

    /// Migracja z wersji 1.0 (nazwa „Clipper”): ustawienia z %APPDATA%\Clipper trafiają do
    /// %APPDATA%\Clipeon. Folder klipów zostaje taki, jaki był (klipów nie ruszamy).
    /// Ikonę przy zegarze trzeba wyciągnąć jeszcze raz, bo Windows pamięta ją po ścieżce programu.
    fn przenies_z_clippera(nowa: &std::path::Path) {
        if nowa.exists() {
            return;
        }
        let Some(appdata) = std::env::var_os("APPDATA") else { return };
        let stara = PathBuf::from(appdata).join("Clipper").join("config.toml");
        let Ok(tekst) = std::fs::read_to_string(&stara) else { return };
        let Ok(mut cfg) = toml::from_str::<Config>(&tekst) else { return };
        cfg.ikona_ustawiona = false;
        match cfg.zapisz() {
            Ok(()) => {
                PRZENIESIONO.store(true, std::sync::atomic::Ordering::SeqCst);
                crate::dziennik::info("Przeniesiono ustawienia z Clippera 1.0 do Clipeona")
            }
            Err(e) => crate::dziennik::blad(&format!("Nie udało się przenieść ustawień z Clippera: {e}")),
        }
    }

    pub fn zapisz(&self) -> Result<(), String> {
        let sciezka = Self::sciezka()?;
        std::fs::create_dir_all(sciezka.parent().unwrap()).map_err(|e| format!("Nie mogę utworzyć folderu configu: {e}"))?;
        let tekst = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&sciezka, tekst).map_err(|e| format!("Nie mogę zapisać {}: {e}", sciezka.display()))
    }

    /// Sprawdza zakresy wartości (dane mogą przyjść z pliku albo z okna ustawień).
    pub fn sprawdz(&self) -> Result<(), String> {
        if !(5..=600).contains(&self.bufor_sekundy) {
            return Err("Długość bufora musi wynosić od 5 do 600 s".into());
        }
        if !(10..=240).contains(&self.fps) || !(10..=240).contains(&self.fps_pulpitu) {
            return Err("FPS musi wynosić od 10 do 240".into());
        }
        if !(1..=200).contains(&self.bitrate_mbps) || !(1..=200).contains(&self.bitrate_pulpitu_mbps) {
            return Err("Bitrate musi wynosić od 1 do 200 Mb/s".into());
        }
        if self.tryb != TRYB_GRA && self.tryb != TRYB_PULPIT {
            return Err(format!("Nieznany tryb \"{}\"", self.tryb));
        }
        if !METODY.contains(&self.metoda_przechwytywania.as_str()) {
            return Err(format!("Nieznana metoda przechwytywania \"{}\"", self.metoda_przechwytywania));
        }
        if !crate::enkodery::poprawna_nazwa(&self.enkoder) {
            return Err(format!("Nieznany enkoder \"{}\"", self.enkoder));
        }
        if !self.folder_klipow.is_absolute() {
            return Err("Folder klipów musi być pełną ścieżką".into());
        }
        if self.sprzatanie_dni > 3650 || self.sprzatanie_gb > 100_000 {
            return Err("Nieprawidłowe ustawienia sprzątania".into());
        }
        crate::skrot::parsuj(&self.skrot).map(|_| ())
    }
}

#[cfg(test)]
mod testy {
    use super::{Config, TRYB_PULPIT};

    #[test]
    fn pulpit_nie_czeka_na_gre() {
        let mut c = Config { gra: "fcadefbneo.exe".into(), ..Config::default() };
        assert_eq!(c.wybrana_gra(), "fcadefbneo.exe");
        c.tryb = TRYB_PULPIT.into();
        assert_eq!(c.wybrana_gra(), "");
        // Wybór gry zostaje na powrót do trybu Gra
        assert_eq!(c.gra, "fcadefbneo.exe");
    }
}
