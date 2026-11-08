// Wideo: przechwytywanie ekranu (Windows.Graphics.Capture) -> stałe FPS -> ffmpeg (H.264 / HEVC / AV1)
// -> zakodowane klatki w buforze RAM.
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use crate::config::Config;
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

struct Przechwyt {
    wspolna: Arc<Mutex<OstatniaKlatka>>,
    rozmiar: (u32, u32),
    tymczasowy: Vec<u8>,
    ostrzezono: bool,
}

impl GraphicsCaptureApiHandler for Przechwyt {
    type Flags = (Arc<Mutex<OstatniaKlatka>>, (u32, u32));
    type Error = Blad;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        let (wspolna, rozmiar) = ctx.flags;
        Ok(Self { wspolna, rozmiar, tymczasowy: Vec::new(), ostrzezono: false })
    }

    fn on_frame_arrived(&mut self, frame: &mut Frame, _: InternalCaptureControl) -> Result<(), Self::Error> {
        let bufor = frame.buffer()?;
        // Zmiana rozdzielczości w trakcie: encoder ma stały rozmiar, więc pomijamy takie klatki
        if (bufor.width(), bufor.height()) != self.rozmiar {
            if !self.ostrzezono {
                eprintln!("Uwaga: rozdzielczość ekranu zmieniła się na {}x{}, klatki pomijane", bufor.width(), bufor.height());
                self.ostrzezono = true;
            }
            return Ok(());
        }
        let piksele = bufor.as_nopadding_buffer(&mut self.tymczasowy);
        let mut w = self.wspolna.lock().unwrap();
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
}

impl Drop for Wideo {
    fn drop(&mut self) {
        // Wątek taktujący kończy pracę i zamyka wejście ffmpeg, a ten kończy się sam
        self.stop.store(true, Ordering::SeqCst);
        if let Some(s) = self.sterowanie.take() {
            let _ = s.stop();
        }
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

/// Rodzina kodeka nagrania — od niej zależy format surowego strumienia i dzielenie go na klatki.
#[derive(Clone, Copy, PartialEq)]
enum Kodek {
    H264,
    Hevc,
    Av1,
}

fn kodek(enkoder: &str) -> Kodek {
    match enkoder {
        "hevc_amf" => Kodek::Hevc,
        "av1_amf" => Kodek::Av1,
        _ => Kodek::H264,
    }
}

/// Format surowego strumienia dla ffmpeg (wyjście enkodera i wejście przy zapisie klipu).
pub fn format_strumienia(enkoder: &str) -> &'static str {
    match kodek(enkoder) {
        Kodek::H264 => "h264",
        Kodek::Hevc => "hevc",
        Kodek::Av1 => "obu",
    }
}

/// Argumenty enkodera: GOP = 1 s (każda klatka kluczowa to IDR z SPS/PPS), bez B-klatek,
/// znacznik AUD przed każdą klatką, żeby łatwo dzielić strumień na pakiety.
fn argumenty_enkodera(cfg: &Config) -> Vec<String> {
    let (b, g) = (cfg.bitrate_mbps, cfg.fps.to_string());
    let mut a: Vec<String> = match cfg.enkoder.as_str() {
        // Bez -pix_fmt: AMF przyjmuje BGRA i sam konwertuje kolory na GPU (mniej CPU)
        "h264_amf" => vec!["-c:v", "h264_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1", "-header_spacing", &g],
        // Nagłówki (VPS/SPS/PPS) przy każdej klatce IDR, znacznik AUD przed każdą klatką
        "hevc_amf" => vec![
            "-c:v", "hevc_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1",
            "-header_insertion_mode", "idr", "-gops_per_idr", "1",
        ],
        // Nagłówek sekwencji przy każdej klatce kluczowej (po nim rozpoznajemy klatki kluczowe)
        "av1_amf" => vec!["-c:v", "av1_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-header_insertion_mode", "gop"],
        _ => vec!["-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-sc_threshold", "0", "-keyint_min", &g]
            .into_iter()
            .chain(["-x264-params", "aud=1:repeat-headers=1"])
            .collect(),
    }
    .into_iter()
    .map(String::from)
    .collect();
    let bitrate = [format!("{b}M"), format!("{}M", b * 3 / 2)];
    a.extend(["-b:v".into(), bitrate[0].clone(), "-maxrate".into(), bitrate[1].clone(), "-bufsize".into(), bitrate[0].clone()]);
    a.extend(["-g".into(), g, "-bf".into(), "0".into(), "-force_key_frames".into(), "expr:gte(t,n_forced)".into()]);
    a
}

/// Startuje przechwytywanie. `zglos` dostaje opis błędu, gdy ffmpeg nieoczekiwanie przestanie działać.
pub fn uruchom(cfg: &Config, bufor: WspolnyBufor, zglos: impl Fn(String) + Send + 'static) -> Result<Wideo, Blad> {
    let monitor = wybierz_monitor(&cfg.monitor)?;
    let (w, h) = (monitor.width()?, monitor.height()?);
    let fps = cfg.fps;
    let nazwa = if cfg.monitor.is_empty() { "główny".to_string() } else { cfg.monitor.clone() };
    println!("Wideo: monitor {nazwa} {w}x{h}, {fps} FPS, {} {} Mb/s", cfg.enkoder, cfg.bitrate_mbps);

    let rozmiar_klatki = (w * h * 4) as usize;
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
        (wspolna.clone(), (w, h)),
    );
    let sterowanie = Przechwyt::start_free_threaded(ustawienia)?;

    let mut ffmpeg = crate::ffmpeg::komenda()
        .args(["-hide_banner", "-loglevel", "error"])
        .args(["-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{h}"), "-framerate", &fps.to_string(), "-i", "-"])
        .args(argumenty_enkodera(cfg))
        .args(["-f", format_strumienia(&cfg.enkoder), "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Nie udało się uruchomić ffmpeg (czy jest w PATH?): {e}"))?;
    let mut bledy_ffmpeg = ffmpeg.stderr.take().unwrap();
    let mut wejscie = ffmpeg.stdin.take().unwrap();
    let mut wyjscie = ffmpeg.stdout.take().unwrap();

    // Początek osi czasu: klatka n ma czas t0 + n/fps
    let start = Instant::now();
    let t0 = teraz_100ns();
    let okres = Duration::from_secs_f64(1.0 / fps as f64);

    // Wątek taktujący: co 1/fps wysyła najnowszą klatkę (albo powtarza poprzednią)
    let stop = Arc::new(AtomicBool::new(false));
    let stop_taktu = stop.clone();
    std::thread::spawn(move || {
        let mut moja = vec![0u8; rozmiar_klatki];
        let mut n: u32 = 0;
        while !stop_taktu.load(Ordering::SeqCst) {
            {
                let mut w = wspolna.lock().unwrap();
                if w.nowa {
                    std::mem::swap(&mut w.piksele, &mut moja);
                    w.nowa = false;
                }
            }
            if wejscie.write_all(&moja).is_err() {
                return; // ffmpeg nie żyje; wątek czytający zgłosi błąd
            }
            n += 1;
            // Czekamy do czasu następnej klatki; jeśli jesteśmy spóźnieni, nadrabiamy bez spania
            let cel = start + okres * n;
            let teraz = Instant::now();
            if cel > teraz {
                std::thread::sleep(cel - teraz);
            }
        }
    });

    // Wątek czytający: dzieli strumień H.264 na klatki i wrzuca je do bufora
    let stop_czytania = stop.clone();
    let kodek_strumienia = kodek(&cfg.enkoder);
    std::thread::spawn(move || {
        let mut parser = Dzielnik::nowy(kodek_strumienia);
        let mut kawalek = vec![0u8; 1 << 16];
        let mut nr: i64 = 0;
        loop {
            let n = match wyjscie.read(&mut kawalek) {
                Ok(0) | Err(_) => {
                    let _ = ffmpeg.wait();
                    if !stop_czytania.load(Ordering::SeqCst) {
                        let mut opis = String::new();
                        let _ = bledy_ffmpeg.read_to_string(&mut opis);
                        let opis = opis.lines().last().unwrap_or("brak szczegółów").to_string();
                        eprintln!("Wideo: ffmpeg przestał działać: {opis}");
                        zglos(format!("Enkoder wideo przestał działać: {opis}"));
                    }
                    return;
                }
                Ok(n) => n,
            };
            for (dane, klucz) in parser.karm(&kawalek[..n]) {
                let czas = t0 + nr * 10_000_000 / fps as i64;
                nr += 1;
                bufor.lock().unwrap().dodaj(Pakiet { czas, klucz, dane: dane.into() });
            }
        }
    });

    Ok(Wideo { sterowanie: Some(sterowanie), stop })
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
