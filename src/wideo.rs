// Wideo: przechwytywanie ekranu (Windows.Graphics.Capture) -> stałe FPS -> ffmpeg (H.264 / HEVC / AV1)
// -> zakodowane klatki w buforze RAM.
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use crate::config::Config;
use crate::enkodery::{self, Kodek};
use std::io::{Read, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings, MinimumUpdateIntervalSettings,
    SecondaryWindowSettings, Settings,
};

type Blad = Box<dyn std::error::Error + Send + Sync>;

/// Ostatnia klatka z ekranu, wymieniana z wątkiem taktującym.
struct OstatniaKlatka {
    piksele: Vec<u8>,
    nowa: bool,
}

/// Problem wykryty w trakcie nagrywania (np. zmiana rozdzielczości); strażnik w rdzeniu go odczytuje
/// i uruchamia nagrywanie od nowa.
type Problem = Arc<Mutex<Option<String>>>;

fn zglos_problem(p: &Problem, opis: String) {
    let mut g = p.lock().unwrap_or_else(|e| e.into_inner());
    if g.is_none() {
        crate::dziennik::blad(&format!("Wideo: {opis}"));
        *g = Some(opis);
    }
}

struct Przechwyt {
    wspolna: Arc<Mutex<OstatniaKlatka>>,
    rozmiar: (u32, u32),
    tymczasowy: Vec<u8>,
    problem: Problem,
}

impl GraphicsCaptureApiHandler for Przechwyt {
    type Flags = (Arc<Mutex<OstatniaKlatka>>, (u32, u32), Problem);
    type Error = Blad;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (wspolna, rozmiar, problem) = ctx.flags;
        Ok(Self { wspolna, rozmiar, tymczasowy: Vec::new(), problem })
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        zglos_problem(&self.problem, "przechwytywanie ekranu zostało przerwane przez system".into());
        Ok(())
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<(), Self::Error> {
        let bufor = frame.buffer()?;
        // Zmiana rozdzielczości: enkoder ma stały rozmiar — zgłaszamy, strażnik uruchomi nagrywanie od nowa
        if (bufor.width(), bufor.height()) != self.rozmiar {
            zglos_problem(&self.problem, format!("rozdzielczość ekranu zmieniła się na {}x{}", bufor.width(), bufor.height()));
            return Ok(());
        }
        let piksele = bufor.as_nopadding_buffer(&mut self.tymczasowy);
        let mut w = self.wspolna.lock().unwrap_or_else(|e| e.into_inner());
        w.piksele.clear();
        w.piksele.extend_from_slice(piksele);
        w.nowa = true;
        Ok(())
    }
}

/// Uchwyt trzymający przechwytywanie przy życiu; zniszczenie go zatrzymuje nagrywanie obrazu.
pub struct Wideo {
    sterowanie: Option<CaptureControl<Przechwyt, Blad>>,
    stop: Arc<AtomicBool>,
    proces: Arc<Mutex<std::process::Child>>,
    problem: Problem,
    /// Format surowego strumienia (h264 / hevc / obu) — potrzebny przy zapisie klipu
    pub format: &'static str,
}

impl Wideo {
    /// Problem zgłoszony przez przechwytywanie lub wątek taktujący (None = wszystko w porządku).
    pub fn problem(&self) -> Option<String> {
        self.problem.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for Wideo {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.sterowanie.take() {
            let _ = s.stop();
        }
        // Zabijamy ffmpeg od razu: zawieszony proces blokowałby wątek taktujący i restart
        let _ = self.proces.lock().unwrap_or_else(|e| e.into_inner()).kill();
    }
}

/// Monitor o danej nazwie urządzenia (pusta = główny).
fn wybierz_monitor(nazwa: &str) -> Result<Monitor, Blad> {
    if nazwa.is_empty() {
        return Ok(Monitor::primary()?);
    }
    Monitor::enumerate()?
        .into_iter()
        .find(|m| m.device_name().is_ok_and(|n| n == nazwa))
        .ok_or_else(|| format!("Nie znaleziono monitora {nazwa} — wybierz inny w Ustawieniach").into())
}

/// Monitory do wyboru w ustawieniach: (nazwa urządzenia, opis).
pub fn monitory() -> Vec<(String, String)> {
    let glowny = Monitor::primary().ok().and_then(|m| m.device_name().ok());
    Monitor::enumerate()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let id = m.device_name().ok()?;
            let (w, h) = (m.width().ok()?, m.height().ok()?);
            let hz = m.refresh_rate().map(|r| format!(", {r} Hz")).unwrap_or_default();
            let nazwa = m.name().unwrap_or_else(|_| id.trim_start_matches(r"\.").to_string());
            let dopisek = if glowny.as_deref() == Some(id.as_str()) { " (główny)" } else { "" };
            Some((id, format!("{nazwa} — {w}×{h}{hz}{dopisek}")))
        })
        .collect()
}

/// Wspólne dla wszystkich enkoderów: bitrate (VBR ze szczytem 1,5×), klatka kluczowa co 1 s, bez B-klatek.
fn argumenty_strumienia(cfg: &Config) -> Vec<String> {
    let (b, g) = (cfg.bitrate_mbps, cfg.fps.to_string());
    [
        "-b:v", &format!("{b}M"), "-maxrate", &format!("{}M", b * 3 / 2), "-bufsize", &format!("{b}M"),
        "-g", &g, "-bf", "0", "-force_key_frames", "expr:gte(t,n_forced)",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect()
}

/// Startuje przechwytywanie. `zglos` dostaje opis błędu, gdy ffmpeg nieoczekiwanie przestanie działać.
pub fn uruchom(cfg: &Config, bufor: WspolnyBufor, zglos: impl Fn(String) + Send + 'static) -> Result<Wideo, Blad> {
    let monitor = wybierz_monitor(&cfg.monitor)?;
    let (w, h) = (monitor.width()?, monitor.height()?);
    let fps = cfg.fps;
    let nazwa = if cfg.monitor.is_empty() { "główny".to_string() } else { cfg.monitor.clone() };
    let enkoder = enkodery::do_nagrywania(&cfg.enkoder);
    let format = enkodery::format_strumienia(enkoder);
    crate::dziennik::info(&format!("Wideo: monitor {nazwa} {w}x{h}, {fps} FPS, {enkoder} {} Mb/s", cfg.bitrate_mbps));

    let rozmiar_klatki = (w * h * 4) as usize;
    let problem: Problem = Arc::new(Mutex::new(None));
    let wspolna = Arc::new(Mutex::new(OstatniaKlatka { piksele: vec![0; rozmiar_klatki], nowa: false }));

    let ustawienia = Settings::new(
        monitor,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        // Nie potrzebujemy klatek częściej niż nasze FPS
        MinimumUpdateIntervalSettings::Custom(Duration::from_secs_f64(1.0 / fps as f64)),
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        (wspolna.clone(), (w, h), problem.clone()),
    );
    let sterowanie = Przechwyt::start_free_threaded(ustawienia)?;

    let ffmpeg = crate::ffmpeg::komenda()
        .args(["-hide_banner", "-loglevel", "error"])
        .args(["-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{h}"), "-framerate", &fps.to_string(), "-i", "-"])
        .args(enkodery::argumenty_nagrywania(enkoder, fps))
        .args(argumenty_strumienia(cfg))
        .args(["-f", format, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut ffmpeg = match ffmpeg {
        Ok(f) => f,
        Err(e) => {
            let _ = sterowanie.stop();
            return Err(format!("Nie udało się uruchomić ffmpeg (czy jest w PATH?): {e}").into());
        }
    };
    // Komunikaty ffmpeg czytamy na bieżąco (pełny potok zatrzymałby enkoder); zostaje ostatnia linia
    let ostatni_blad = Arc::new(Mutex::new(String::new()));
    {
        let ostatni = ostatni_blad.clone();
        let bledy = ffmpeg.stderr.take().unwrap();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for linia in std::io::BufReader::new(bledy).lines().map_while(Result::ok) {
                *ostatni.lock().unwrap_or_else(|e| e.into_inner()) = linia;
            }
        });
    }
    let mut wejscie = ffmpeg.stdin.take().unwrap();
    let mut wyjscie = ffmpeg.stdout.take().unwrap();
    let proces = Arc::new(Mutex::new(ffmpeg));

    // Początek osi czasu: klatka n ma czas t0 + n/fps
    let start = Instant::now();
    let t0 = teraz_100ns();
    let okres = Duration::from_secs_f64(1.0 / fps as f64);

    // Wątek taktujący: co 1/fps wysyła najnowszą klatkę (albo powtarza poprzednią)
    let stop = Arc::new(AtomicBool::new(false));
    let stop_taktu = stop.clone();
    let problem_taktu = problem.clone();
    std::thread::Builder::new().name("wideo-takt".into()).spawn(move || {
        let mut moja = vec![0u8; rozmiar_klatki];
        let mut n: u32 = 0;
        while !stop_taktu.load(Ordering::SeqCst) {
            {
                let mut w = wspolna.lock().unwrap_or_else(|e| e.into_inner());
                if w.nowa {
                    std::mem::swap(&mut w.piksele, &mut moja);
                    w.nowa = false;
                }
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
            } else if teraz - cel > Duration::from_secs(2) {
                // Duża przerwa (uśpienie komputera, zawieszony enkoder): nadrabianie tysięcy klatek
                // zalałoby enkoder, a znaczniki czasu by się rozjechały — lepiej zacząć od nowa
                zglos_problem(&problem_taktu, "przerwa w nagrywaniu (uśpienie komputera lub zawieszony enkoder)".into());
                return;
            }
        }
    }).expect("wątek wideo-takt");

    // Wątek czytający: dzieli strumień H.264 na klatki i wrzuca je do bufora
    let stop_czytania = stop.clone();
    let kodek_strumienia = enkodery::kodek(enkoder);
    let proces_czytania = proces.clone();
    std::thread::Builder::new().name("wideo-odczyt".into()).spawn(move || {
        let mut parser = Dzielnik::nowy(kodek_strumienia);
        let mut kawalek = vec![0u8; 1 << 16];
        let mut nr: i64 = 0;
        loop {
            let n = match wyjscie.read(&mut kawalek) {
                Ok(0) | Err(_) => {
                    let _ = proces_czytania.lock().unwrap_or_else(|e| e.into_inner()).wait();
                    if !stop_czytania.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(100));
                        let opis = ostatni_blad.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        crate::dziennik::blad(&format!("Wideo: ffmpeg przestał działać: {opis}"));
                        zglos(if opis.is_empty() {
                            "enkoder wideo nieoczekiwanie się zamknął".to_string()
                        } else {
                            format!("enkoder wideo przestał działać ({opis})")
                        });
                    }
                    return;
                }
                Ok(n) => n,
            };
            for (dane, klucz) in parser.karm(&kawalek[..n]) {
                let czas = t0 + nr * 10_000_000 / fps as i64;
                nr += 1;
                bufor.lock().unwrap_or_else(|e| e.into_inner()).dodaj(Pakiet { czas, klucz, dane: dane.into() });
            }
        }
    }).expect("wątek wideo-odczyt");

    Ok(Wideo { sterowanie: Some(sterowanie), stop, proces, problem, format })
}

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

    fn karm(&mut self, dane: &[u8]) -> Vec<(Vec<u8>, bool)> {
        self.bufor.extend_from_slice(dane);
        match self.kodek {
            Kodek::Av1 => self.karm_av1(),
            k => self.karm_annex_b(k),
        }
    }

    fn karm_annex_b(&mut self, k: Kodek) -> Vec<(Vec<u8>, bool)> {
        // Typ jednostki NAL z pierwszego bajtu nagłówka
        let typ = |b: u8| if k == Kodek::Hevc { (b >> 1) & 0x3f } else { b & 0x1f };
        let aud = |b: u8| typ(b) == if k == Kodek::Hevc { 35 } else { 9 };
        // Klatka kluczowa: IDR (H.264: 5; HEVC: 19/20, a także CRA 21)
        let kluczowa = |b: u8| if k == Kodek::Hevc { (19..=21).contains(&typ(b)) } else { typ(b) == 5 };
        let mut klatki = Vec::new();
        // Pomijamy AUD na samym początku bufora (należy do bieżącej klatki)
        while let Some(p) = znajdz_nal(&self.bufor, self.skan.max(4), aud) {
            let klatka: Vec<u8> = self.bufor.drain(..p).collect();
            let klucz = znajdz_nal(&klatka, 0, kluczowa).is_some();
            klatki.push((klatka, klucz));
            self.skan = 0;
        }
        self.skan = self.bufor.len().saturating_sub(3);
        klatki
    }

    /// AV1: każda klatka (temporal unit) zaczyna się OBU typu 2; klatka kluczowa ma nagłówek sekwencji (typ 1).
    fn karm_av1(&mut self) -> Vec<(Vec<u8>, bool)> {
        let mut klatki = Vec::new();
        let mut poz = self.skan; // początek pierwszego nieprzeczytanego OBU
        let mut poczatek_klatki = 0;
        let mut klucz = false;
        while let Some((typ, dl)) = obu(&self.bufor[poz..]) {
            if typ == 2 && poz > poczatek_klatki {
                klatki.push((self.bufor[poczatek_klatki..poz].to_vec(), klucz));
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
