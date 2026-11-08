// Wideo: przechwytywanie ekranu (Windows.Graphics.Capture) -> stałe FPS -> ffmpeg h264_amf
// -> pakiety H.264 w buforze RAM.
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use crate::config::Config;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
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

/// Uchwyt trzymający przechwytywanie przy życiu.
pub struct Wideo {
    _sterowanie: CaptureControl<Przechwyt, Blad>,
}

pub fn uruchom(cfg: &Config, bufor: WspolnyBufor) -> Result<Wideo, Blad> {
    let monitor = Monitor::primary()?;
    let (w, h) = (monitor.width()?, monitor.height()?);
    let fps = cfg.fps;
    println!("Wideo: główny monitor {w}x{h}, {fps} FPS, h264_amf {} Mb/s", cfg.bitrate_mbps);

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

    // Enkoder sprzętowy AMD: GOP = 1 s (każda klatka kluczowa to IDR z SPS/PPS), bez B-klatek,
    // znacznik AUD przed każdą klatką, żeby łatwo dzielić strumień na pakiety.
    let b = cfg.bitrate_mbps;
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error"])
        .args(["-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{h}"), "-framerate", &fps.to_string(), "-i", "-"])
        .args(["-c:v", "h264_amf", "-rc", "vbr_peak", "-b:v", &format!("{b}M"), "-maxrate", &format!("{}M", b * 3 / 2)])
        .args(["-bufsize", &format!("{b}M"), "-g", &fps.to_string(), "-bf", "0", "-forced_idr", "1"])
        .args(["-force_key_frames", "expr:gte(t,n_forced)", "-aud", "1", "-header_spacing", &fps.to_string()])
        // Bez -pix_fmt: AMF przyjmuje BGRA i sam konwertuje kolory na GPU (mniej CPU)
        .args(["-f", "h264", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Nie udało się uruchomić ffmpeg (czy jest w PATH?): {e}"))?;
    let mut wejscie = ffmpeg.stdin.take().unwrap();
    let mut wyjscie = ffmpeg.stdout.take().unwrap();

    // Początek osi czasu: klatka n ma czas t0 + n/fps
    let start = Instant::now();
    let t0 = teraz_100ns();
    let okres = Duration::from_secs_f64(1.0 / fps as f64);

    // Wątek taktujący: co 1/fps wysyła najnowszą klatkę (albo powtarza poprzednią)
    std::thread::spawn(move || {
        let mut moja = vec![0u8; rozmiar_klatki];
        let mut n: u32 = 0;
        loop {
            {
                let mut w = wspolna.lock().unwrap();
                if w.nowa {
                    std::mem::swap(&mut w.piksele, &mut moja);
                    w.nowa = false;
                }
            }
            if let Err(e) = wejscie.write_all(&moja) {
                eprintln!("Wideo: ffmpeg przestał przyjmować klatki: {e}");
                return;
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
    std::thread::spawn(move || {
        let mut parser = ParserH264::default();
        let mut kawalek = vec![0u8; 1 << 16];
        let mut nr: i64 = 0;
        loop {
            let n = match wyjscie.read(&mut kawalek) {
                Ok(0) | Err(_) => {
                    eprintln!("Wideo: ffmpeg zakończył pracę");
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

    Ok(Wideo { _sterowanie: sterowanie })
}

/// Dzieli strumień Annex B na jednostki dostępu (klatki) po znacznikach AUD (NAL typu 9).
#[derive(Default)]
struct ParserH264 {
    bufor: Vec<u8>,
    skan: usize,
}

impl ParserH264 {
    fn karm(&mut self, dane: &[u8]) -> Vec<(Vec<u8>, bool)> {
        self.bufor.extend_from_slice(dane);
        let mut klatki = Vec::new();
        // Pomijamy AUD na samym początku bufora (należy do bieżącej klatki)
        while let Some(p) = znajdz_nal(&self.bufor, self.skan.max(4), 9) {
            let klatka: Vec<u8> = self.bufor.drain(..p).collect();
            let klucz = znajdz_nal(&klatka, 0, 5).is_some();
            klatki.push((klatka, klucz));
            self.skan = 0;
        }
        self.skan = self.bufor.len().saturating_sub(3);
        klatki
    }
}

/// Pozycja kodu startowego (00 00 01 lub 00 00 00 01) NAL danego typu, szukając od `od`.
fn znajdz_nal(dane: &[u8], od: usize, typ: u8) -> Option<usize> {
    let mut i = od;
    while i + 3 < dane.len() {
        if dane[i] == 0 && dane[i + 1] == 0 && dane[i + 2] == 1 {
            if dane[i + 3] & 0x1f == typ {
                return Some(if i > 0 && dane[i - 1] == 0 { i - 1 } else { i });
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    None
}
