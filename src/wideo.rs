// Wideo: przechwytywanie ekranu -> enkoder (H.264 / HEVC / AV1) -> zakodowane klatki w buforze RAM.
// Cztery drogi przechwytywania (od najlżejszej):
// - Dda: ffmpeg sam przechwytuje ekran przez Desktop Duplication (`ddagrab`); obraz zostaje na karcie
//   aż do enkodera — prawie bez pracy procesora. Wymaga, żeby monitor i enkoder były na tej samej karcie.
// - Gfx: ffmpeg przechwytuje przez Windows.Graphics.Capture (`gfxcapture`) na karcie enkodera — działa
//   też, gdy monitor jest podłączony do innej karty (laptopy z dwiema kartami).
// - DdaRam: Desktop Duplication prowadzone przez Clipeona (dda.rs), obraz do ffmpeg przez RAM — bez żółtej
//   ramki i bez restartu, gdy Windows na chwilę odbierze obraz.
// - Potok: tryb zgodności, jak w wersjach do 1.2: Clipeon przechwytuje ekran (windows-capture), kopiuje
//   klatki do RAM i wysyła je potokiem do ffmpeg. Działa z każdym enkoderem, ale kosztuje procesor.
// Którą drogę wybrać, decyduje `kolejnosc_drog`; rdzeń przechodzi na następną, gdy poprzednia zawiedzie.
use crate::Zablokuj;
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use crate::config::Config;
use crate::enkodery::{self, Kodek};
use std::io::{Read, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::{GraphicsCaptureApi, InternalCaptureControl};
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};

type Blad = Box<dyn std::error::Error + Send + Sync>;

/// Opóźnienie przechwytywania i kodowania w ffmpeg (w klatkach): o tyle wcześniej niż przyjście
/// pierwszego pakietu zaczyna się obraz (ważne dla zgodności obrazu z dźwiękiem).
const OPOZNIENIE_KLATEK: i64 = 2;

// ---------- Drogi przechwytywania ----------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Droga {
    Dda,
    Gfx,
    /// Desktop Duplication prowadzone przez Clipeona (dda.rs), obraz do ffmpeg przez RAM. Bez żółtej
    /// ramki i bez restartu, gdy Windows na chwilę odbierze obraz (sam go odzyskuje).
    DdaRam,
    Potok,
}

impl Droga {
    pub fn nazwa(self) -> &'static str {
        match self {
            Droga::Dda => "dda",
            Droga::Gfx => "gfx",
            Droga::DdaRam => "dda-ram",
            Droga::Potok => "potok",
        }
    }

    pub fn z_nazwy(n: &str) -> Option<Droga> {
        [Droga::Dda, Droga::Gfx, Droga::DdaRam, Droga::Potok].into_iter().find(|d| d.nazwa() == n)
    }

    pub fn opis(self) -> &'static str {
        match self {
            Droga::Dda => "Desktop Duplication (obraz zostaje na karcie)",
            Droga::Gfx => "Windows Graphics Capture (obraz zostaje na karcie)",
            Droga::DdaRam => "Desktop Duplication w Clipeonie (obraz przez pamięć RAM)",
            Droga::Potok => "tryb zgodności (obraz przez pamięć RAM)",
        }
    }

    /// Czy przechwytuje ffmpeg (a nie Clipeon).
    pub fn w_ffmpeg(self) -> bool {
        matches!(self, Droga::Dda | Droga::Gfx)
    }

    /// Czy obraz idzie do ffmpeg przez stdin (przechwytuje Clipeon).
    fn przez_ram(self) -> bool {
        !self.w_ffmpeg()
    }

    /// Czy przy tej drodze Windows rysuje wokół ekranu żółtą ramkę. Desktop Duplication nie rysuje jej
    /// nigdy; pozostałe drogi korzystają z Windows Graphics Capture, w którym ramkę da się wyłączyć
    /// dopiero od Windows 11.
    pub fn z_ramka(self) -> bool {
        matches!(self, Droga::Gfx | Droga::Potok) && !mozliwosci_przechwytywania().bez_ramki
    }
}

/// Wszystko, od czego zależy kolejność dróg (wydzielone, żeby dało się to przetestować).
#[derive(Clone, Copy, Debug)]
pub struct Warunki<'a> {
    /// Ustawienie: "auto", "dda", "gfx" albo "potok"
    pub metoda: &'a str,
    /// Enkoder przyjmuje tekstury D3D11 (AMF, NVENC)
    pub enkoder_tekstury: bool,
    /// Monitor jest podłączony do tej samej karty, na której koduje enkoder
    pub ta_sama_karta: bool,
    /// Wiadomo, na której karcie koduje enkoder
    pub karta_enkodera: bool,
    /// Obraz trzeba zmniejszyć (ddagrab nie umie skalować)
    pub skalowanie: bool,
    pub filtr_dda: bool,
    pub filtr_gfx: bool,
    /// Windows umie wyłączyć żółtą ramkę Windows Graphics Capture (Windows 11)
    pub bez_ramki: bool,
    /// Wiadomo, do którego wyjścia karty podłączony jest monitor (warunek Desktop Duplication)
    pub wyjscie: bool,
}

/// Drogi do wypróbowania, od najlepszej. Potok jest zawsze na końcu — działa wszędzie.
/// DDA przez RAM (dowolny enkoder) idzie przed drogami z żółtą ramką, czyli na Windows 10 zaraz po DDA;
/// na Windows 11 Windows Graphics Capture nie ma ramki, więc tam nie jest potrzebna.
pub fn kolejnosc_drog(w: &Warunki) -> Vec<Droga> {
    let dda = w.enkoder_tekstury && w.filtr_dda && w.ta_sama_karta && !w.skalowanie;
    let gfx = w.enkoder_tekstury && w.filtr_gfx && w.karta_enkodera;
    let dda_ram = w.wyjscie;
    let mut l = Vec::new();
    match w.metoda {
        "potok" => {}
        "gfx" => {
            if gfx {
                l.push(Droga::Gfx);
            }
        }
        "dda" => {
            if dda {
                l.push(Droga::Dda);
            }
            if dda_ram {
                l.push(Droga::DdaRam);
            }
        }
        "dda-ram" => {
            if dda_ram {
                l.push(Droga::DdaRam);
            }
        }
        _ => {
            if dda {
                l.push(Droga::Dda);
            }
            if dda_ram && !w.bez_ramki {
                l.push(Droga::DdaRam);
            }
            if gfx {
                l.push(Droga::Gfx);
            }
        }
    }
    l.push(Droga::Potok);
    l
}

// ---------- Statystyki (do dziennika co minutę) ----------

#[derive(Default)]
pub struct Statystyki {
    /// Klatki z ekranu przyjęte do kodowania (Potok)
    pub przechwycone: AtomicU64,
    /// Klatki z ekranu odrzucone, bo przyszły szybciej niż nasze FPS (Potok, starszy Windows)
    pub pominiete: AtomicU64,
    /// Takty, w których ekran się nie zmienił i powtórzyliśmy klatkę (Potok)
    pub powtorzone: AtomicU64,
    /// Takty spóźnione o ponad klatkę (Potok) albo pakiety spóźnione względem zegara (ffmpeg)
    pub spoznione: AtomicU64,
    /// Zakodowane klatki odebrane z enkodera
    pub pakiety: AtomicU64,
    pub bajty: AtomicU64,
}

/// Dolicza do licznika statystyk (kolejność bez znaczenia — to tylko liczby do dziennika).
fn dolicz(licznik: &AtomicU64, ile: u64) {
    licznik.fetch_add(ile, Ordering::Relaxed);
}

impl Statystyki {
    /// Wartości liczników (do policzenia różnic co minutę).
    pub fn odczyt(&self) -> [u64; 6] {
        [&self.przechwycone, &self.pominiete, &self.powtorzone, &self.spoznione, &self.pakiety, &self.bajty]
            .map(|a| a.load(Ordering::Relaxed))
    }
}

// ---------- Monitory ----------

/// Monitor do wyboru w oknie.
#[derive(Clone, serde::Serialize)]
pub struct InfoMonitora {
    /// Nazwa urządzenia, np. \\.\DISPLAY2
    pub id: String,
    /// Numer z nazwy urządzenia (DISPLAY2 -> 2)
    pub numer: u32,
    pub nazwa: String,
    pub szerokosc: u32,
    pub wysokosc: u32,
    pub hz: u32,
    pub glowny: bool,
    pub hdr: bool,
    /// Pełny opis do list: „LC24RG50 — 1920×1080, 60 Hz (główny)”
    pub opis: String,
}

/// Numer z nazwy urządzenia: \\.\DISPLAY12 -> 12.
pub fn numer_monitora(nazwa_urzadzenia: &str) -> u32 {
    let cyfry: String = nazwa_urzadzenia.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    cyfry.chars().rev().collect::<String>().parse().unwrap_or(0)
}

/// Krótka nazwa ekranu do paska i zasobnika, np. „Ekran 2” (pusta nazwa = główny monitor).
pub fn etykieta_ekranu(nazwa_urzadzenia: &str) -> String {
    let nazwa = if nazwa_urzadzenia.is_empty() {
        Monitor::primary().ok().and_then(|m| m.device_name().ok()).unwrap_or_default()
    } else {
        nazwa_urzadzenia.to_string()
    };
    match numer_monitora(&nazwa) {
        0 => "Ekran".to_string(),
        n => format!("Ekran {n}"),
    }
}

/// Stały identyfikator monitora (ścieżka urządzenia z identyfikatorem producenta i portu); nie zmienia
/// się, gdy Windows przenumeruje DISPLAYn po aktualizacji sterownika.
pub fn trwaly_id(nazwa_urzadzenia: &str) -> Option<String> {
    use windows::Win32::Graphics::Gdi::{DISPLAY_DEVICEW, EnumDisplayDevicesW};
    const EDD_GET_DEVICE_INTERFACE_NAME: u32 = 1;
    let nazwa: Vec<u16> = nazwa_urzadzenia.encode_utf16().chain(std::iter::once(0)).collect();
    let mut dd = DISPLAY_DEVICEW { cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32, ..Default::default() };
    unsafe {
        EnumDisplayDevicesW(windows::core::PCWSTR(nazwa.as_ptr()), 0, &mut dd, EDD_GET_DEVICE_INTERFACE_NAME)
            .ok()
            .ok()?;
    }
    let id = crate::tekst_utf16(&dd.DeviceID);
    (!id.is_empty()).then_some(id)
}

/// Monitory do wyboru w oknie.
pub fn monitory() -> Vec<InfoMonitora> {
    let glowny = Monitor::primary().ok().and_then(|m| m.device_name().ok());
    Monitor::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let id = m.device_name().ok()?;
            let (w, h) = (m.width().ok()?, m.height().ok()?);
            let hz = m.refresh_rate().unwrap_or(0);
            let nazwa = m.name().unwrap_or_else(|_| id.trim_start_matches(r"\\.\").to_string());
            let jest_glowny = glowny.as_deref() == Some(id.as_str());
            let hdr = crate::karty::wyjscie_monitora(&id).is_some_and(|w| w.hdr);
            let opis = format!(
                "{nazwa} — {w}×{h}{}{}",
                if hz > 0 { format!(", {hz} Hz") } else { String::new() },
                if jest_glowny { " (główny)" } else { "" }
            );
            Some(InfoMonitora { numer: numer_monitora(&id), id, nazwa, szerokosc: w, wysokosc: h, hz, glowny: jest_glowny, hdr, opis })
        })
        .collect()
}

/// Monitor do nagrywania: najpierw po stałym identyfikatorze, potem po nazwie urządzenia, a gdy go
/// nie ma (odłączony) — główny. Zwraca też, czy to monitor zastępczy.
fn znajdz_monitor(nazwa: &str, id: &str) -> Result<(Monitor, String, bool), Blad> {
    let wszystkie = Monitor::enumerate().unwrap_or_default();
    if !id.is_empty()
        && let Some(m) = wszystkie.iter().find(|m| m.device_name().ok().and_then(|n| trwaly_id(&n)).as_deref() == Some(id)) {
            return Ok((*m, m.device_name()?, false));
        }
    if !nazwa.is_empty()
        && let Some(m) = wszystkie.iter().find(|m| m.device_name().is_ok_and(|n| n == nazwa)) {
            return Ok((*m, nazwa.to_string(), false));
        }
    let m = Monitor::primary()?;
    let n = m.device_name()?;
    Ok((m, n, !nazwa.is_empty() || !id.is_empty()))
}

/// Czy wybrany monitor jest znów podłączony (gdy nagrywamy zastępczy).
pub fn monitor_wrocil(nazwa: &str, id: &str) -> bool {
    znajdz_monitor(nazwa, id).is_ok_and(|(_, _, zastepczy)| !zastepczy)
}

/// Które opcjonalne ustawienia przechwytywania obsługuje ten Windows (starsze wersje Windows 10
/// nie mają części z nich — wtedy zostawiamy domyślne zamiast przerywać nagrywanie).
struct Mozliwosci {
    kursor: bool,
    bez_ramki: bool,
    odstep_klatek: bool,
}

/// Sprawdzane raz na uruchomienie (Windows się w trakcie nie zmieni), bo pytają o to m.in. `Droga::z_ramka`
/// przy każdym odświeżeniu stanu.
fn mozliwosci_przechwytywania() -> &'static Mozliwosci {
    static M: std::sync::OnceLock<Mozliwosci> = std::sync::OnceLock::new();
    M.get_or_init(|| {
        let m = Mozliwosci {
            kursor: GraphicsCaptureApi::is_cursor_settings_supported().unwrap_or(false),
            bez_ramki: GraphicsCaptureApi::is_border_settings_supported().unwrap_or(false),
            odstep_klatek: GraphicsCaptureApi::is_minimum_update_interval_supported().unwrap_or(false),
        };
        let mut braki = Vec::new();
        if !m.bez_ramki {
            braki.push("wyłączanie żółtej ramki (nagrywanie działa, ramka może być widoczna)");
        }
        if !m.odstep_klatek {
            braki.push("ograniczanie liczby klatek z ekranu (Clipeon odrzuca nadmiarowe sam)");
        }
        if !m.kursor {
            braki.push("wybór kursora");
        }
        if !braki.is_empty() {
            crate::dziennik::info(&format!("Ten Windows nie obsługuje: {} — używam ustawień domyślnych", braki.join(", ")));
        }
        m
    })
}

// ---------- Parametry kodowania ----------

/// Bitrate dopasowany do rozdzielczości: ustawienie dotyczy 1080p, przy większym ekranie potrzeba
/// więcej danych na ten sam poziom ostrości (skala ~ pikseli^0,75, jak w zaleceniach YouTube/OBS).
pub fn bitrate_dla(mbps: u32, w: u32, h: u32) -> u32 {
    let skala = (w as f64 * h as f64 / (1920.0 * 1080.0)).powf(0.75).max(1.0);
    ((mbps as f64 * skala).round() as u32).clamp(1, 150)
}

/// Wspólne dla wszystkich enkoderów: bitrate (VBR ze szczytem 1,5×), klatka kluczowa co 1 s, bez B-klatek.
fn argumenty_strumienia(mbps: u32, fps: u32, w: u32, h: u32) -> Vec<String> {
    let (b, g) = (bitrate_dla(mbps, w, h), fps.to_string());
    [
        "-b:v", &format!("{b}M"), "-maxrate", &format!("{}M", b * 3 / 2), "-bufsize", &format!("{b}M"),
        "-g", &g, "-bf", "0", "-force_key_frames", "expr:gte(t,n_forced)",
    ]
    .map(String::from)
    .into()
}

/// Znaczniki kolorów BT.709 (tak samo na każdej karcie i w każdym odtwarzaczu). NVENC przy wejściu RGB
/// sam przelicza kolory po swojemu i sam zapisuje znaczniki, więc go pomijamy.
fn znaczniki_koloru(enkoder: &str) -> Vec<String> {
    if enkoder.ends_with("_nvenc") {
        return Vec::new();
    }
    ["-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709", "-color_range", "tv"].map(String::from).into()
}

// ---------- Uruchomienie ----------

/// Problem wykryty w trakcie nagrywania (np. zmiana rozdzielczości); strażnik w rdzeniu go odczytuje
/// i uruchamia nagrywanie od nowa.
type Problem = Arc<Mutex<Option<String>>>;

fn zglos_problem(p: &Problem, opis: String) {
    let mut g = p.zablokuj();
    if g.is_none() {
        crate::dziennik::blad(&format!("Wideo: {opis}"));
        *g = Some(opis);
    }
}

/// Uchwyt trzymający nagrywanie obrazu przy życiu; zniszczenie go zatrzymuje nagrywanie.
pub struct Wideo {
    sterowanie: Option<CaptureControl<Przechwyt, Blad>>,
    /// Wątek Desktop Duplication w Clipeonie (droga DdaRam); zatrzymuje się przy zniszczeniu
    _dda: Option<crate::dda::Przechwyt>,
    stop: Arc<AtomicBool>,
    proces: Arc<Mutex<std::process::Child>>,
    problem: Problem,
    /// Format surowego strumienia (h264 / hevc / obu) — potrzebny przy zapisie klipu
    pub format: &'static str,
    /// FPS strumienia (do zapisu klipu)
    pub fps: u32,
    /// Rozmiar zakodowanego obrazu i nazwa enkodera (film skleja tylko segmenty o tych samych)
    pub rozmiar: (u32, u32),
    pub enkoder: &'static str,
    pub droga: Droga,
    pub statystyki: Arc<Statystyki>,
    /// Enkoder nie nadąża (gra zabiera całą moc karty albo procesora)
    pub przeciazenie: Arc<AtomicBool>,
    /// Nagrywamy monitor główny zamiast odłączonego wybranego: (nazwa, stały identyfikator) wybranego
    pub zastepczy: Option<(String, String)>,
    /// Krótki opis do dziennika i diagnostyki: monitor, rozmiar, enkoder, droga
    pub opis: String,
}

impl Wideo {
    /// Problem zgłoszony przez przechwytywanie lub wątki (None = wszystko w porządku).
    pub fn problem(&self) -> Option<String> {
        self.problem.zablokuj().clone()
    }
}

impl Drop for Wideo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.sterowanie.take() {
            let _ = s.stop();
        }
        // Zabijamy ffmpeg od razu: zawieszony proces blokowałby wątki i restart
        let _ = self.proces.zablokuj().kill();
    }
}

/// Co trzeba wiedzieć o ekranie i enkoderze przed wyborem drogi.
struct Plan {
    monitor: Monitor,
    nazwa_monitora: String,
    zastepczy: bool,
    /// Rozmiar ekranu
    ekran: (u32, u32),
    wybor: enkodery::Wybor,
    wyjscie: Option<crate::karty::Wyjscie>,
    karta_enkodera: Option<u32>,
}

fn plan(cfg: &Config) -> Result<Plan, Blad> {
    let (nazwa, id) = cfg.aktywny_monitor();
    let (monitor, nazwa_monitora, zastepczy) = znajdz_monitor(nazwa, id)?;
    let ekran = (monitor.width()?, monitor.height()?);
    // Enkodery wymagają parzystych wymiarów; nieparzysty ekran przycinamy o piksel
    let (w, h) = (ekran.0 & !1, ekran.1 & !1);
    let wybor = enkodery::do_nagrywania(&cfg.enkoder, w, h);
    let wyjscie = crate::karty::wyjscie_monitora(&nazwa_monitora);
    let karta_enkodera = enkodery::producent(wybor.nazwa).and_then(crate::karty::indeks_karty);
    Ok(Plan { monitor, nazwa_monitora, zastepczy, ekran, wybor, wyjscie, karta_enkodera })
}

impl Plan {
    /// Ekran przycięty do parzystych wymiarów (tego wymagają enkodery).
    fn ekran_parzysty(&self) -> (u32, u32) {
        (self.ekran.0 & !1, self.ekran.1 & !1)
    }

    /// Enkoder koduje mniejszy obraz niż ekran (za duży ekran dla karty).
    fn skalowanie(&self) -> bool {
        (self.wybor.w, self.wybor.h) != self.ekran_parzysty()
    }

    fn warunki<'a>(&self, metoda: &'a str) -> Warunki<'a> {
        Warunki {
            metoda,
            enkoder_tekstury: enkodery::przyjmuje_tekstury(self.wybor.nazwa),
            ta_sama_karta: match (&self.wyjscie, self.karta_enkodera) {
                (Some(w), Some(k)) => w.karta == k,
                _ => false,
            },
            karta_enkodera: self.karta_enkodera.is_some(),
            skalowanie: self.skalowanie(),
            filtr_dda: crate::ffmpeg::ma_filtr("ddagrab"),
            filtr_gfx: crate::ffmpeg::ma_filtr("gfxcapture"),
            bez_ramki: mozliwosci_przechwytywania().bez_ramki,
            wyjscie: self.wyjscie.is_some(),
        }
    }
}

/// Drogi do wypróbowania dla bieżących ustawień, od najlepszej.
pub fn drogi(cfg: &Config) -> Vec<Droga> {
    match plan(cfg) {
        Ok(p) => kolejnosc_drog(&p.warunki(&cfg.metoda_przechwytywania)),
        Err(_) => vec![Droga::Potok],
    }
}

/// Startuje nagrywanie obrazu wybraną drogą. `zglos` dostaje opis błędu, gdy ffmpeg nieoczekiwanie
/// przestanie działać. `lekki` = lżejsze ustawienia enkodera (po wykryciu, że nie nadąża).
pub fn uruchom(
    cfg: &Config,
    droga: Droga,
    lekki: bool,
    bufor: impl FnOnce(&str) -> WspolnyBufor,
    zglos: impl Fn(String) + Send + 'static,
) -> Result<Wideo, Blad> {
    let p = plan(cfg)?;
    let fps = cfg.aktywne_fps();
    let enkoder = p.wybor.nazwa;
    let (w, h) = (p.wybor.w, p.wybor.h);
    let mbps = cfg.aktywny_bitrate();
    let format = enkodery::format_strumienia(enkoder);
    let opis = format!(
        "monitor {} {}x{}{}, {fps} FPS, {enkoder} {} Mb/s{}, droga {}",
        p.nazwa_monitora,
        w,
        h,
        if (w, h) != p.ekran { format!(" (ekran {}x{})", p.ekran.0, p.ekran.1) } else { String::new() },
        bitrate_dla(mbps, w, h),
        if lekki { " (lekkie ustawienia)" } else { "" },
        droga.opis()
    );
    if p.wyjscie.as_ref().is_some_and(|w| w.hdr) {
        static HDR: std::sync::Once = std::sync::Once::new();
        HDR.call_once(|| crate::dziennik::info("Monitor ma włączony HDR — nagrywam w 8 bitach (SDR)"));
    }

    let problem: Problem = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));
    let statystyki = Arc::new(Statystyki::default());
    let przeciazenie = Arc::new(AtomicBool::new(false));
    let mut sterowanie = None;
    let mut dda = None;

    let mut cmd = crate::ffmpeg::komenda_nagrywania();
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    let mut wejscie_potoku = None;
    match droga {
        Droga::Dda => {
            let wyj = p.wyjscie.clone().ok_or("nie wiadomo, do której karty podłączony jest monitor")?;
            enkodery::dziala_z_tekstura(enkoder, w, h, wyj.karta, Some(wyj.wyjscie))?;
            cmd.args(["-init_hw_device", &format!("d3d11va=gpu:{}", wyj.karta), "-filter_hw_device", "gpu"]);
            cmd.args([
                "-filter_complex",
                &format!("ddagrab=output_idx={}:framerate={fps}:draw_mouse=1:video_size={w}x{h}", wyj.wyjscie),
            ]);
        }
        Droga::Gfx => {
            let karta = p.karta_enkodera.ok_or("nie wiadomo, na której karcie koduje enkoder")?;
            // Próba na obrazie z ekranu, gdy monitor jest na karcie enkodera (klatki gfxcapture są takie same)
            let ekran = p.wyjscie.as_ref().filter(|wyj| wyj.karta == karta).map(|wyj| wyj.wyjscie);
            enkodery::dziala_z_tekstura(enkoder, w, h, karta, ekran)?;
            cmd.args(["-init_hw_device", &format!("d3d11va=gpu:{karta}"), "-filter_hw_device", "gpu"]);
            cmd.args([
                "-filter_complex",
                &format!(
                    "gfxcapture=hmonitor={}:max_framerate={fps}:capture_cursor=1:display_border=0:width={w}:height={h}:resize_mode={},fps={fps}",
                    p.monitor.as_raw_hmonitor() as usize,
                    if p.skalowanie() { "scale_aspect" } else { "crop" }
                ),
            ]);
        }
        Droga::Potok | Droga::DdaRam => {
            let (ew, eh) = p.ekran;
            let mut filtry = Vec::new();
            if p.skalowanie() {
                filtry.push(format!("scale={w}:{h}"));
            } else if (w, h) != (ew, eh) {
                filtry.push(format!("crop={w}:{h}:0:0"));
            }
            if let Some(f) = enkodery::format_wejscia(enkoder) {
                // Przeliczenie RGB -> YUV na procesorze, jawnie w BT.709 (zgodnie ze znacznikami)
                filtry.push(format!("scale=out_color_matrix=bt709:out_range=tv,format={f}"));
            }
            cmd.args(["-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{ew}x{eh}"), "-framerate", &fps.to_string(), "-i", "-"]);
            if !filtry.is_empty() {
                cmd.args(["-vf", &filtry.join(",")]);
            }
            if droga == Droga::DdaRam {
                let rozmiar_klatki = (ew * eh * 4) as usize;
                let wspolna = Arc::new(Mutex::new(OstatniaKlatka { piksele: vec![0; rozmiar_klatki], nowa: false }));
                let problem_dda = problem.clone();
                dda = Some(crate::dda::uruchom(&p.nazwa_monitora, p.ekran, fps, wspolna.clone(), statystyki.clone(), move |e| {
                    zglos_problem(&problem_dda, e)
                })?);
                wejscie_potoku = Some(wspolna);
            } else {
                let (s, wspolna) = uruchom_przechwyt(&p.monitor, p.ekran, fps, &problem, &statystyki)?;
                sterowanie = Some(s);
                wejscie_potoku = Some(wspolna);
            }
        }
    }
    cmd.args(enkodery::argumenty_nagrywania(enkoder, fps, lekki))
        .args(argumenty_strumienia(mbps, fps, w, h))
        .args(znaczniki_koloru(enkoder));
    if droga.w_ffmpeg() {
        cmd.args(enkodery::znaczniki_tekstur(enkoder));
    }
    cmd
        .args(["-f", format, "-"])
        .stdin(if droga.przez_ram() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut ffmpeg = match crate::ffmpeg::uruchom_nagrywanie(&mut cmd, "obraz") {
        Ok(f) => f,
        Err(e) => {
            if let Some(s) = sterowanie {
                let _ = s.stop();
            }
            return Err(format!("Nie udało się uruchomić ffmpeg: {e}").into());
        }
    };
    crate::dziennik::info(&format!("Wideo: {opis}"));

    // Komunikaty ffmpeg czytamy na bieżąco (pełny potok zatrzymałby enkoder); zostają ostatnie linie —
    // przy awarii ddagrab właściwa przyczyna (np. „AcquireNextFrame failed: 887a0026”) stoi linię wyżej
    // niż ogólne „Generic error in an external library”
    let ostatni_blad = Arc::new(Mutex::new(String::new()));
    {
        let ostatni = ostatni_blad.clone();
        let bledy = ffmpeg.stderr.take().unwrap();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let mut linie = std::collections::VecDeque::new();
            for linia in std::io::BufReader::new(bledy).lines().map_while(Result::ok) {
                if linia.trim().is_empty() {
                    continue;
                }
                if linie.len() == 3 {
                    linie.pop_front();
                }
                linie.push_back(linia.trim().to_string());
                *ostatni.zablokuj() = linie.iter().cloned().collect::<Vec<_>>().join(" | ");
            }
        });
    }
    let wyjscie = ffmpeg.stdout.take().unwrap();
    let wejscie = ffmpeg.stdin.take();
    let proces = Arc::new(Mutex::new(ffmpeg));
    // Potok: oś czasu zaczyna się teraz (klatki nadaje nasz wątek taktujący).
    // ffmpeg: początek ustalamy przy pierwszym pakiecie (start przechwytywania trwa różnie długo).
    let t0 = droga.przez_ram().then(teraz_100ns);
    if let (Some(wspolna), Some(wejscie)) = (wejscie_potoku, wejscie) {
        watek_taktujacy(wspolna, wejscie, fps, stop.clone(), problem.clone(), statystyki.clone(), przeciazenie.clone(), p.ekran);
    }

    // Wątek czytający: dzieli strumień na klatki i wrzuca je do bufora. Bufor wybiera wołający po opisie
    // strumienia: przy takim samym obrazie (restart po chwilowej utracie ekranu) dostajemy stary bufor
    let bufor = bufor(&opis);
    let odczyt = Odczyt {
        wyjscie,
        kodek: enkodery::kodek(enkoder),
        fps,
        t0,
        droga,
        bufor,
        statystyki: statystyki.clone(),
        przeciazenie: przeciazenie.clone(),
        problem: problem.clone(),
        stop: stop.clone(),
        proces: proces.clone(),
        ostatni_blad,
        enkoder,
        zglos: Box::new(zglos),
    };
    if let Err(e) = std::thread::Builder::new().name("wideo-odczyt".into()).spawn(move || czytaj_strumien(odczyt)) {
        // Bez wątku czytającego nikt nie zatrzymałby ffmpeg ani wątku taktującego (Wideo jeszcze nie istnieje)
        stop.store(true, Ordering::SeqCst);
        let _ = proces.zablokuj().kill();
        return Err(e.into());
    }

    let (nazwa, id) = cfg.aktywny_monitor();
    let zastepczy = p.zastepczy.then(|| (nazwa.to_string(), id.to_string()));
    Ok(Wideo { sterowanie, _dda: dda, stop, proces, problem, format, fps, rozmiar: (w, h), enkoder, droga, statystyki, przeciazenie, zastepczy, opis })
}

struct Odczyt {
    wyjscie: std::process::ChildStdout,
    kodek: Kodek,
    fps: u32,
    t0: Option<i64>,
    droga: Droga,
    bufor: WspolnyBufor,
    statystyki: Arc<Statystyki>,
    przeciazenie: Arc<AtomicBool>,
    problem: Problem,
    stop: Arc<AtomicBool>,
    proces: Arc<Mutex<std::process::Child>>,
    ostatni_blad: Arc<Mutex<String>>,
    enkoder: &'static str,
    zglos: Box<dyn Fn(String) + Send>,
}

fn czytaj_strumien(mut o: Odczyt) {
    let mut parser = Dzielnik::nowy(o.kodek);
    let mut kawalek = vec![0u8; 1 << 16];
    let mut nr: i64 = 0;
    let okres_100ns = 10_000_000 / o.fps as i64;
    let mut spoznione_od: Option<Instant> = None;
    loop {
        let n = match o.wyjscie.read(&mut kawalek) {
            Ok(0) | Err(_) => {
                let _ = o.proces.zablokuj().wait();
                if !o.stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                    let opis = o.ostatni_blad.zablokuj().clone();
                    crate::dziennik::blad(&format!("Wideo: ffmpeg przestał działać: {opis}"));
                    // Chwilowa utrata dostępu do ekranu (UAC, blokada) to nie wina enkodera. Tak samo
                    // przechwytywanie w ffmpeg, które nie dało ani jednej klatki („received no packets”):
                    // enkoder nie dostał obrazu, więc nie ma za co go zapominać (Desktop Duplication
                    // po zmianie trybu ekranu potrafi przez chwilę nic nie dawać)
                    if !chwilowy(&opis) && !bez_obrazu_z_przechwytywania(o.droga, nr, &opis) {
                        enkodery::zapomnij(o.enkoder);
                    }
                    (o.zglos)(if opis.is_empty() {
                        "enkoder wideo nieoczekiwanie się zamknął".to_string()
                    } else {
                        format!("enkoder wideo przestał działać ({opis})")
                    });
                }
                return;
            }
            Ok(n) => n,
        };
        let teraz = teraz_100ns();
        for (dane, klucz) in parser.karm(&kawalek[..n]) {
            // ffmpeg: początek osi czasu = przyjście pierwszego pakietu minus opóźnienie kodowania
            let t0 = *o.t0.get_or_insert(teraz - OPOZNIENIE_KLATEK * okres_100ns);
            let czas = t0 + nr * 10_000_000 / o.fps as i64;
            nr += 1;
            dolicz(&o.statystyki.pakiety, 1);
            dolicz(&o.statystyki.bajty, dane.len() as u64);
            // Przechwytywanie w ffmpeg: czy enkoder nadąża (pakiety nie przychodzą coraz później niż zegar).
            // Gfx pomijamy — przy nieruchomym ekranie stoi, a potem dogania powtórzonymi klatkami.
            if o.droga == Droga::Dda {
                let opoznienie = teraz - OPOZNIENIE_KLATEK * okres_100ns - czas;
                if opoznienie > 5_000_000 {
                    dolicz(&o.statystyki.spoznione, 1);
                    let od = *spoznione_od.get_or_insert_with(Instant::now);
                    if od.elapsed() > Duration::from_secs(5) {
                        o.przeciazenie.store(true, Ordering::SeqCst);
                    }
                    if opoznienie > 30_000_000 {
                        zglos_problem(&o.problem, "enkoder nie nadąża — obraz spóźnia się o ponad 3 s".into());
                    }
                } else {
                    spoznione_od = None;
                }
            }
            o.bufor.zablokuj().dodaj(Pakiet { czas, klucz, dane });
        }
    }
}

/// Czy błąd to chwilowa utrata dostępu do ekranu (okno UAC, ekran blokady, przełączenie użytkownika)
/// — wtedy wznawiamy po cichu i nie uznajemy drogi przechwytywania za zepsutą.
pub fn chwilowy(opis: &str) -> bool {
    let o = opis.to_lowercase();
    ["887a0026", "access_lost", "access lost", "80070005", "e_accessdenied", "chwilowa utrata"].iter().any(|x| o.contains(x))
        // Błąd „zewnętrznej biblioteki” w wątku filtrów (`fc#`), czyli w ddagrab / gfxcapture: Desktop
        // Duplication traci obraz, gdy gra przełącza pełny ekran albo rozdzielczość (enkoder jest w porządku)
        || (o.contains("[fc#") && (o.contains("-542398533") || o.contains("external library")))
}

/// ffmpeg zakończył się, zanim przechwytywanie (ddagrab / gfxcapture) dało choć jedną klatkę.
fn bez_obrazu_z_przechwytywania(droga: Droga, klatki: i64, opis: &str) -> bool {
    droga.w_ffmpeg() && klatki == 0 && opis.to_lowercase().contains("received no packets")
}

// ---------- Droga „Potok”: przechwytywanie w Clipeonie ----------

/// Ostatnia klatka z ekranu, wymieniana z wątkiem taktującym.
pub(crate) struct OstatniaKlatka {
    pub(crate) piksele: Vec<u8>,
    pub(crate) nowa: bool,
}

struct Przechwyt {
    wspolna: Arc<Mutex<OstatniaKlatka>>,
    rozmiar: (u32, u32),
    /// Własny bufor klatki: kopiujemy do niego poza blokadą, a z wątkiem taktującym tylko się zamieniamy
    wlasna: Vec<u8>,
    tymczasowy: Vec<u8>,
    problem: Problem,
    statystyki: Arc<Statystyki>,
    /// Gdy Windows nie umie ograniczać klatek (przed 11 24H2), odrzucamy nadmiarowe sami — przed
    /// kosztownym kopiowaniem z karty
    min_odstep: Option<Duration>,
    ostatnia: Option<Instant>,
}

type FlagiPrzechwytu = (Arc<Mutex<OstatniaKlatka>>, (u32, u32), Problem, Arc<Statystyki>, Option<Duration>);

impl GraphicsCaptureApiHandler for Przechwyt {
    type Flags = FlagiPrzechwytu;
    type Error = Blad;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (wspolna, rozmiar, problem, statystyki, min_odstep) = ctx.flags;
        Ok(Self { wspolna, rozmiar, wlasna: Vec::new(), tymczasowy: Vec::new(), problem, statystyki, min_odstep, ostatnia: None })
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        zglos_problem(&self.problem, "przechwytywanie ekranu zostało przerwane przez system".into());
        Ok(())
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<(), Self::Error> {
        if let (Some(min), Some(t)) = (self.min_odstep, self.ostatnia)
            && t.elapsed() < min {
                dolicz(&self.statystyki.pominiete, 1);
                return Ok(());
            }
        self.ostatnia = Some(Instant::now());
        let bufor = frame.buffer()?;
        // Zmiana rozdzielczości: enkoder ma stały rozmiar — zgłaszamy, strażnik uruchomi nagrywanie od nowa
        if (bufor.width(), bufor.height()) != self.rozmiar {
            zglos_problem(&self.problem, format!("rozdzielczość ekranu zmieniła się na {}x{}", bufor.width(), bufor.height()));
            return Ok(());
        }
        let piksele = bufor.as_nopadding_buffer(&mut self.tymczasowy);
        self.wlasna.clear();
        self.wlasna.extend_from_slice(piksele);
        {
            let mut w = self.wspolna.zablokuj();
            std::mem::swap(&mut w.piksele, &mut self.wlasna);
            w.nowa = true;
        }
        dolicz(&self.statystyki.przechwycone, 1);
        Ok(())
    }
}

/// Uchwyt do zatrzymania wątku przechwytywania Windows Graphics Capture.
type Sterowanie = CaptureControl<Przechwyt, Blad>;

fn uruchom_przechwyt(
    monitor: &Monitor,
    rozmiar: (u32, u32),
    fps: u32,
    problem: &Problem,
    statystyki: &Arc<Statystyki>,
) -> Result<(Sterowanie, Arc<Mutex<OstatniaKlatka>>), Blad> {
    let rozmiar_klatki = (rozmiar.0 * rozmiar.1 * 4) as usize;
    let wspolna = Arc::new(Mutex::new(OstatniaKlatka { piksele: vec![0; rozmiar_klatki], nowa: false }));
    let mozliwosci = mozliwosci_przechwytywania();
    let ustawienia = Settings::new(
        *monitor,
        if mozliwosci.kursor { CursorCaptureSettings::WithCursor } else { CursorCaptureSettings::Default },
        // Starszy Windows 10 nie pozwala wyłączyć ramki — wtedy nagrywamy z ramką zamiast przerywać
        if mozliwosci.bez_ramki { DrawBorderSettings::WithoutBorder } else { DrawBorderSettings::Default },
        SecondaryWindowSettings::Default,
        // Nie potrzebujemy klatek częściej niż nasze FPS (Windows 11 24H2+; wcześniej odrzucamy je sami)
        if mozliwosci.odstep_klatek {
            MinimumUpdateIntervalSettings::Custom(najkrotszy_odstep(fps))
        } else {
            MinimumUpdateIntervalSettings::Default
        },
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        (
            wspolna.clone(),
            rozmiar,
            problem.clone(),
            statystyki.clone(),
            (!mozliwosci.odstep_klatek).then(|| najkrotszy_odstep(fps)),
        ),
    );
    let sterowanie = Przechwyt::start_free_threaded(ustawienia)?;
    Ok((sterowanie, wspolna))
}

/// Najkrótszy odstęp między klatkami przyjmowanymi z ekranu: pół okresu nagrania. Ekran nie podaje klatek
/// idealnie równo, więc próg równy całemu okresowi (tak było do 1.4.3) odrzucał klatkę, która przyszła
/// odrobinę za wcześnie po spóźnionej poprzedniej — w nagraniu w jej miejscu była powtórka (ok. 1% klatek
/// przy 60 FPS). Przy pół okresie przechodzi każda klatka ekranu 60 Hz, a przy ekranach 120 Hz i szybszych
/// kopiujemy najwyżej dwa razy więcej klatek, niż nagrywamy.
pub(crate) fn najkrotszy_odstep(fps: u32) -> Duration {
    Duration::from_secs_f64(0.5 / fps.max(1) as f64)
}

/// Gdy w chwili taktu nie ma nowej klatki, wątek taktujący czeka na nią najwyżej tę część okresu
/// (zamiast od razu powtarzać poprzednią). Zegar ekranu i nasz chodzą osobno: klatka, która przyszła
/// milisekundę po takcie, wcześniej przepadała (następna ją nadpisywała), a w nagraniu była powtórka.
const CZEKANIE_NA_KLATKE: f64 = 0.25;

/// Wątek taktujący: co 1/fps wysyła do ffmpeg najnowszą klatkę (albo powtarza poprzednią).
#[allow(clippy::too_many_arguments)]
fn watek_taktujacy(
    wspolna: Arc<Mutex<OstatniaKlatka>>,
    mut wejscie: std::process::ChildStdin,
    fps: u32,
    stop: Arc<AtomicBool>,
    problem: Problem,
    statystyki: Arc<Statystyki>,
    przeciazenie: Arc<AtomicBool>,
    rozmiar: (u32, u32),
) {
    let okres = Duration::from_secs_f64(1.0 / fps as f64);
    let start = Instant::now();
    std::thread::Builder::new()
        .name("wideo-takt".into())
        .spawn(move || {
            let mut moja = vec![0u8; (rozmiar.0 * rozmiar.1 * 4) as usize];
            let mut n: u32 = 0;
            // Okno 1 s: ile taktów się spóźniło; 5 s z rzędu ponad 10% = enkoder nie nadąża
            let (mut spoznione_w_oknie, mut zle_sekundy) = (0u32, 0u32);
            let czekanie = okres.mul_f64(CZEKANIE_NA_KLATKE);
            while !stop.load(Ordering::SeqCst) {
                let czekaj_do = Instant::now() + czekanie;
                loop {
                    let mut w = wspolna.zablokuj();
                    if w.nowa {
                        std::mem::swap(&mut w.piksele, &mut moja);
                        w.nowa = false;
                        break;
                    }
                    drop(w);
                    if Instant::now() >= czekaj_do || stop.load(Ordering::SeqCst) {
                        dolicz(&statystyki.powtorzone, 1);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                if wejscie.write_all(&moja).is_err() {
                    return; // ffmpeg nie żyje; wątek czytający zgłosi błąd
                }
                n += 1;
                // Czekamy do czasu następnej klatki; małe spóźnienie nadrabiamy bez spania
                let cel = start + okres * n;
                let teraz = Instant::now();
                if cel > teraz {
                    std::thread::sleep(cel - teraz);
                } else {
                    if teraz - cel > okres {
                        dolicz(&statystyki.spoznione, 1);
                        spoznione_w_oknie += 1;
                    }
                    if teraz - cel > Duration::from_secs(2) {
                        // Duża przerwa (uśpienie komputera, zawieszony enkoder): nadrabianie tysięcy klatek
                        // zalałoby enkoder, a znaczniki czasu by się rozjechały — lepiej zacząć od nowa
                        zglos_problem(&problem, "przerwa w nagrywaniu (uśpienie komputera lub zawieszony enkoder)".into());
                        return;
                    }
                }
                if n.is_multiple_of(fps) {
                    zle_sekundy = if spoznione_w_oknie * 10 > fps { zle_sekundy + 1 } else { 0 };
                    spoznione_w_oknie = 0;
                    if zle_sekundy >= 5 {
                        przeciazenie.store(true, Ordering::SeqCst);
                    }
                }
            }
        })
        .expect("wątek wideo-takt");
}

// ---------- Dzielenie strumienia na klatki ----------

/// Zakodowana klatka i czy jest kluczowa. Od razu `Arc` — tak trafia do bufora, bez dodatkowej kopii.
type Klatka = (Arc<[u8]>, bool);

/// Dzieli surowy strumień enkodera na klatki i rozpoznaje klatki kluczowe:
/// H.264 / HEVC (Annex B) po znacznikach AUD, AV1 (OBU) po ogranicznikach czasowych.
struct Dzielnik {
    kodek: Kodek,
    bufor: Vec<u8>,
    skan: usize,
}

impl Dzielnik {
    fn nowy(kodek: Kodek) -> Self {
        Dzielnik { kodek, bufor: Vec::new(), skan: 0 }
    }

    fn karm(&mut self, dane: &[u8]) -> Vec<Klatka> {
        self.bufor.extend_from_slice(dane);
        match self.kodek {
            Kodek::Av1 => self.karm_av1(),
            k => self.karm_annex_b(k),
        }
    }

    fn karm_annex_b(&mut self, k: Kodek) -> Vec<Klatka> {
        // Typ jednostki NAL z pierwszego bajtu nagłówka
        let typ = |b: u8| if k == Kodek::Hevc { (b >> 1) & 0x3f } else { b & 0x1f };
        let aud = |b: u8| typ(b) == if k == Kodek::Hevc { 35 } else { 9 };
        // Klatka kluczowa: IDR (H.264: 5; HEVC: 19/20, a także CRA 21)
        let kluczowa = |b: u8| if k == Kodek::Hevc { (19..=21).contains(&typ(b)) } else { typ(b) == 5 };
        let mut klatki = Vec::new();
        // Pomijamy AUD na samym początku bufora (należy do bieżącej klatki)
        while let Some(p) = znajdz_nal(&self.bufor, self.skan.max(4), aud) {
            let klatka: Arc<[u8]> = Arc::from(&self.bufor[..p]);
            self.bufor.drain(..p);
            let klucz = znajdz_nal(&klatka, 0, kluczowa).is_some();
            klatki.push((klatka, klucz));
            self.skan = 0;
        }
        self.skan = self.bufor.len().saturating_sub(3);
        klatki
    }

    /// AV1: każda klatka (temporal unit) zaczyna się OBU typu 2; klatka kluczowa ma nagłówek sekwencji (typ 1).
    fn karm_av1(&mut self) -> Vec<Klatka> {
        let mut klatki = Vec::new();
        let mut poz = self.skan; // początek pierwszego nieprzeczytanego OBU
        let mut poczatek_klatki = 0;
        let mut klucz = false;
        while let Some((typ, dl)) = obu(&self.bufor[poz..]) {
            if typ == 2 && poz > poczatek_klatki {
                klatki.push((Arc::from(&self.bufor[poczatek_klatki..poz]), klucz));
                poczatek_klatki = poz;
                klucz = false;
            }
            klucz |= typ == 1;
            poz += dl;
        }
        // Zostawiamy niedokończoną klatkę; flaga klucza zostanie wyliczona ponownie
        self.bufor.drain(..poczatek_klatki);
        self.skan = 0;
        klatki
    }
}

/// Typ i całkowita długość pierwszego OBU (None, jeśli jeszcze niekompletne).
fn obu(d: &[u8]) -> Option<(u8, usize)> {
    let naglowek = *d.first()?;
    let typ = (naglowek >> 3) & 0x0f;
    let rozszerzenie = ((naglowek >> 2) & 1) as usize;
    // ffmpeg zapisuje OBU zawsze z polem rozmiaru (obu_has_size_field = 1)
    let mut i = 1 + rozszerzenie;
    let (mut rozmiar, mut przesuniecie) = (0usize, 0);
    loop {
        let b = *d.get(i)?;
        rozmiar |= ((b & 0x7f) as usize) << przesuniecie;
        i += 1;
        if b & 0x80 == 0 {
            break;
        }
        przesuniecie += 7;
        if przesuniecie > 56 {
            return None;
        }
    }
    let calosc = i + rozmiar;
    (d.len() >= calosc).then_some((typ, calosc))
}

/// Pozycja kodu startowego (00 00 01 lub 00 00 00 01) NAL, którego bajt nagłówka spełnia warunek.
fn znajdz_nal(dane: &[u8], od: usize, warunek: impl Fn(u8) -> bool) -> Option<usize> {
    let mut i = od;
    while i + 3 < dane.len() {
        if dane[i] == 0 && dane[i + 1] == 0 && dane[i + 2] == 1 {
            if warunek(dane[i + 3]) {
                return Some(if i > 0 && dane[i - 1] == 0 { i - 1 } else { i });
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod testy {
    use super::*;

    fn warunki() -> Warunki<'static> {
        Warunki {
            metoda: "auto",
            enkoder_tekstury: true,
            ta_sama_karta: true,
            karta_enkodera: true,
            skalowanie: false,
            filtr_dda: true,
            filtr_gfx: true,
            bez_ramki: true,
            wyjscie: true,
        }
    }

    #[test]
    fn drogi_auto() {
        assert_eq!(kolejnosc_drog(&warunki()), vec![Droga::Dda, Droga::Gfx, Droga::Potok]);
        // Monitor na innej karcie niż enkoder (laptop): Desktop Duplication odpada
        assert_eq!(kolejnosc_drog(&Warunki { ta_sama_karta: false, ..warunki() }), vec![Droga::Gfx, Droga::Potok]);
        // Zmniejszanie obrazu: ddagrab nie skaluje
        assert_eq!(kolejnosc_drog(&Warunki { skalowanie: true, ..warunki() }), vec![Droga::Gfx, Droga::Potok]);
        // Quick Sync / procesor: tylko potok
        assert_eq!(kolejnosc_drog(&Warunki { enkoder_tekstury: false, ..warunki() }), vec![Droga::Potok]);
        // Stary ffmpeg bez filtrów
        assert_eq!(kolejnosc_drog(&Warunki { filtr_dda: false, filtr_gfx: false, ..warunki() }), vec![Droga::Potok]);
    }

    #[test]
    fn drogi_na_windows_10_omijaja_zolta_ramke() {
        let win10 = Warunki { bez_ramki: false, ..warunki() };
        // DDA przez RAM przed drogami, które rysują żółtą ramkę
        assert_eq!(kolejnosc_drog(&win10), vec![Droga::Dda, Droga::DdaRam, Droga::Gfx, Droga::Potok]);
        // Także dla enkoderów bez tekstur (Quick Sync, procesor)
        assert_eq!(kolejnosc_drog(&Warunki { enkoder_tekstury: false, ..win10 }), vec![Droga::DdaRam, Droga::Potok]);
        // Nieznane wyjście karty: zostaje dawna kolejność
        assert_eq!(kolejnosc_drog(&Warunki { wyjscie: false, ..win10 }), vec![Droga::Dda, Droga::Gfx, Droga::Potok]);
        assert!(!Droga::DdaRam.w_ffmpeg());
    }

    #[test]
    fn drogi_wymuszone() {
        assert_eq!(kolejnosc_drog(&Warunki { metoda: "potok", ..warunki() }), vec![Droga::Potok]);
        assert_eq!(kolejnosc_drog(&Warunki { metoda: "gfx", ..warunki() }), vec![Droga::Gfx, Droga::Potok]);
        assert_eq!(kolejnosc_drog(&Warunki { metoda: "dda", ..warunki() }), vec![Droga::Dda, Droga::DdaRam, Droga::Potok]);
        assert_eq!(kolejnosc_drog(&Warunki { metoda: "dda", ta_sama_karta: false, ..warunki() }), vec![Droga::DdaRam, Droga::Potok]);
        assert_eq!(kolejnosc_drog(&Warunki { metoda: "dda-ram", ..warunki() }), vec![Droga::DdaRam, Droga::Potok]);
    }

    #[test]
    fn nazwy_drog() {
        for d in [Droga::Dda, Droga::Gfx, Droga::DdaRam, Droga::Potok] {
            assert_eq!(Droga::z_nazwy(d.nazwa()), Some(d));
        }
        assert_eq!(Droga::z_nazwy("inna"), None);
    }

    #[test]
    fn numery_monitorow() {
        assert_eq!(numer_monitora(r"\\.\DISPLAY2"), 2);
        assert_eq!(numer_monitora(r"\\.\DISPLAY12"), 12);
        assert_eq!(numer_monitora(""), 0);
    }

    #[test]
    fn brak_obrazu_z_przechwytywania_nie_obciaza_enkodera() {
        let opis = "[out#0/h264 @ 0x1] Nothing was written into output file, because at least one of its streams received no packets.";
        assert!(bez_obrazu_z_przechwytywania(Droga::Dda, 0, opis));
        assert!(bez_obrazu_z_przechwytywania(Droga::Gfx, 0, opis));
        assert!(!bez_obrazu_z_przechwytywania(Droga::Dda, 5, opis));
        assert!(!bez_obrazu_z_przechwytywania(Droga::Potok, 0, opis));
        assert!(!bez_obrazu_z_przechwytywania(Droga::Dda, 0, "Error while opening encoder"));
        // Nieudana próba na obrazie z ekranu po wcześniejszym sukcesie: strażnik ponawia, nie odrzuca drogi
        assert!(chwilowy("chwilowa utrata obrazu z ekranu (próba h264_nvenc z karty 0 się nie udała)"));
    }

    #[test]
    fn chwilowe_bledy() {
        assert!(chwilowy("enkoder wideo przestał działać ([Parsed_ddagrab_0 @ 0x1] AcquireNextFrame failed: 887a0026)"));
        assert!(!chwilowy("enkoder wideo przestał działać (Error while opening encoder)"));
        // Fightcade przy zmianie trybu ekranu (Windows 10, NVENC, Desktop Duplication)
        assert!(chwilowy(
            "enkoder wideo przestał działać ([fc#0 @ 00000282feb192c0] Terminating thread with return code -542398533 (Generic error in an external library))"
        ));
        // Ten sam kod z enkodera (nie z filtra) to prawdziwy problem enkodera
        assert!(!chwilowy("[vost#0:0/h264_nvenc @ 0x1] Terminating thread with return code -542398533 (Generic error in an external library)"));
    }

    /// Klatki z dzielnika jako zwykłe wektory (do porównań)
    fn wektory(k: Vec<Klatka>) -> Vec<(Vec<u8>, bool)> {
        k.into_iter().map(|(d, klucz)| (d.to_vec(), klucz)).collect()
    }

    fn nal(typ: &[u8], dane: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 1];
        v.extend_from_slice(typ);
        v.extend_from_slice(dane);
        v
    }

    #[test]
    fn dzielnik_h264() {
        // Klatka kluczowa: AUD, SPS, PPS, IDR; potem zwykła: AUD, P
        let k1 = [nal(&[0x09], &[0xF0]), nal(&[0x67], &[1, 2, 3]), nal(&[0x68], &[4]), nal(&[0x65], &[9, 9, 9, 9])].concat();
        let k2 = [nal(&[0x09], &[0xF0]), nal(&[0x41], &[7, 7, 7])].concat();
        let k3 = nal(&[0x09], &[0xF0]);
        let strumien = [k1.clone(), k2.clone(), k3].concat();
        // Podajemy po kawałku (jak z potoku), także przecinając kody startowe
        let mut d = Dzielnik::nowy(Kodek::H264);
        let mut klatki = Vec::new();
        for kawalek in strumien.chunks(3) {
            klatki.extend(wektory(d.karm(kawalek)));
        }
        assert_eq!(klatki.len(), 2);
        assert_eq!(klatki[0], (k1, true));
        assert_eq!(klatki[1], (k2, false));
    }

    #[test]
    fn dzielnik_hevc() {
        // Typ NAL w HEVC: (bajt >> 1) & 0x3f; AUD = 35, IDR_W_RADL = 19, TRAIL_R = 1
        let k1 = [nal(&[35 << 1, 1], &[0x50]), nal(&[19 << 1, 1], &[5, 5, 5])].concat();
        let k2 = [nal(&[35 << 1, 1], &[0x50]), nal(&[1 << 1, 1], &[6, 6])].concat();
        let mut d = Dzielnik::nowy(Kodek::Hevc);
        let mut klatki = wektory(d.karm(&[k1.clone(), k2.clone()].concat()));
        klatki.extend(wektory(d.karm(&nal(&[35 << 1, 1], &[0x50]))));
        assert_eq!(klatki, vec![(k1, true), (k2, false)]);
    }

    #[test]
    fn dzielnik_av1() {
        // OBU: nagłówek (typ << 3 | pole rozmiaru), rozmiar (leb128), dane.
        // Ogranicznik czasowy = 2, nagłówek sekwencji = 1, klatka = 6
        let obu = |typ: u8, dane: &[u8]| [vec![(typ << 3) | 0x02, dane.len() as u8], dane.to_vec()].concat();
        let k1 = [obu(2, &[]), obu(1, &[1, 2]), obu(6, &[3, 3, 3])].concat();
        let k2 = [obu(2, &[]), obu(6, &[4, 4])].concat();
        let mut d = Dzielnik::nowy(Kodek::Av1);
        let mut klatki = Vec::new();
        for kawalek in [k1.clone(), k2.clone(), obu(2, &[])].concat().chunks(2) {
            klatki.extend(wektory(d.karm(kawalek)));
        }
        assert_eq!(klatki, vec![(k1, true), (k2, false)]);
    }

    #[test]
    fn bitrate_rosnie_z_rozdzielczoscia() {
        assert_eq!(bitrate_dla(20, 1920, 1080), 20);
        assert_eq!(bitrate_dla(20, 1280, 720), 20);
        assert!(bitrate_dla(20, 3840, 2160) > 40);
    }
}
