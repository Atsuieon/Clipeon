// Enkodery wideo: wykrywanie, które działają na tym komputerze (karta AMD, NVIDIA, Intel albo
// procesor), wybór najlepszego przy ustawieniu „auto” i argumenty ffmpeg dla każdego z nich.
// Wszystkie są dostępne w buildzie FFmpeg na licencji LGPL (bez GPL-owego x264).
use std::sync::OnceLock;

/// Rodzina kodeka — od niej zależy format surowego strumienia i dzielenie go na klatki.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kodek {
    H264,
    Hevc,
    Av1,
}

pub struct Enkoder {
    pub nazwa: &'static str,
    pub opis: &'static str,
    pub kodek: Kodek,
}

const fn e(nazwa: &'static str, opis: &'static str, kodek: Kodek) -> Enkoder {
    Enkoder { nazwa, opis, kodek }
}

/// Kandydaci w kolejności preferencji (sprzętowe przed procesorem).
pub const KANDYDACI: &[Enkoder] = &[
    e("h264_amf", "H.264 — karta AMD (działa wszędzie)", Kodek::H264),
    e("h264_nvenc", "H.264 — karta NVIDIA (działa wszędzie)", Kodek::H264),
    e("h264_qsv", "H.264 — grafika Intel (działa wszędzie)", Kodek::H264),
    e("hevc_amf", "HEVC — karta AMD (mniejsze pliki)", Kodek::Hevc),
    e("hevc_nvenc", "HEVC — karta NVIDIA (mniejsze pliki)", Kodek::Hevc),
    e("hevc_qsv", "HEVC — grafika Intel (mniejsze pliki)", Kodek::Hevc),
    e("av1_amf", "AV1 — karta AMD (najmniejsze pliki)", Kodek::Av1),
    e("av1_nvenc", "AV1 — karta NVIDIA (najmniejsze pliki)", Kodek::Av1),
    e("av1_qsv", "AV1 — grafika Intel (najmniejsze pliki)", Kodek::Av1),
    e("h264_mf", "H.264 — procesor (wolniejsze, gdy karta nie działa)", Kodek::H264),
    e("libx264", "H.264 — procesor, x264", Kodek::H264),
];

pub const AUTO: &str = "auto";

fn znajdz(nazwa: &str) -> Option<&'static Enkoder> {
    KANDYDACI.iter().find(|e| e.nazwa == nazwa)
}

/// Enkodery wbudowane w ffmpeg (lista z `ffmpeg -encoders`), liczona raz.
fn wbudowane() -> &'static [&'static Enkoder] {
    static WYNIK: OnceLock<Vec<&'static Enkoder>> = OnceLock::new();
    WYNIK.get_or_init(|| {
        let lista = crate::ffmpeg::komenda()
            .args(["-hide_banner", "-encoders"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        KANDYDACI.iter().filter(|e| lista.lines().any(|l| l.split_whitespace().nth(1) == Some(e.nazwa))).collect()
    })
}

/// Czy enkoder działa tutaj — wynik próby zapamiętany (sondy idą po kolei: równoległe sesje
/// na jednej karcie potrafiły ciągnąć się kilkanaście sekund). Porażkę pamiętamy tylko kilka minut:
/// przy autostarcie karta bywa jeszcze zajęta, a sterownik mógł zostać w międzyczasie zaktualizowany.
static WYNIKI: OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, (bool, std::time::Instant)>>> =
    OnceLock::new();

fn dziala(e: &'static Enkoder) -> bool {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    let wyniki = WYNIKI.get_or_init(|| Mutex::new(HashMap::new()));
    // Blokada trzymana przez całą próbę = próby nigdy nie idą równolegle
    let mut w = wyniki.lock().unwrap_or_else(|x| x.into_inner());
    if let Some(&(ok, kiedy)) = w.get(e.nazwa) {
        if ok || kiedy.elapsed() < Duration::from_secs(300) {
            return ok;
        }
    }
    let ok = wbudowane().iter().any(|x| x.nazwa == e.nazwa) && proba_z_ponowieniem(e);
    w.insert(e.nazwa, (ok, Instant::now()));
    ok
}

/// Najstarszy sterownik NVIDIA obsługiwany przez NVENC w dołączonym FFmpeg 8.1 (SDK 13).
const MIN_NVIDIA: f64 = 570.0;

/// Ostrzeżenie dla użytkownika (np. za stary sterownik NVIDIA), pokazywane przy wyborze enkodera.
static OSTRZEZENIE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub fn ostrzezenie() -> Option<String> {
    // Za stary sterownik NVIDIA widać już po wersji (gdy karta jest najmocniejsza w komputerze)
    static SPRAWDZONO: OnceLock<()> = OnceLock::new();
    SPRAWDZONO.get_or_init(|| {
        let glowna = crate::karty::karty().first();
        if let Some(v) = glowna.and_then(|k| k.wersja_nvidia()).filter(|v| *v < MIN_NVIDIA) {
            let tekst = format!(
                "Sterownik karty NVIDIA ({v:.2}) jest za stary do nagrywania kartą (potrzebny {MIN_NVIDIA:.0} lub nowszy). \
                 Zaktualizuj go w aplikacji NVIDIA albo na nvidia.com — do tego czasu nagrywa procesor."
            );
            crate::dziennik::blad(&tekst);
            *OSTRZEZENIE.lock().unwrap_or_else(|x| x.into_inner()) = Some(tekst);
        }
    });
    OSTRZEZENIE.lock().unwrap_or_else(|x| x.into_inner()).clone()
}

/// Sprzętowa próba może się nie udać przy pierwszym podejściu (zimny start sterownika) — druga szansa.
fn proba_z_ponowieniem(e: &Enkoder) -> bool {
    let sprzetowy = !e.nazwa.ends_with("_mf") && e.nazwa != "libx264";
    let mut powod = match proba(e) {
        Ok(()) => return true,
        Err(p) => p,
    };
    if sprzetowy && !blad_sprzetu(&powod) {
        match proba(e) {
            Ok(()) => return true,
            Err(p) => powod = p,
        }
    }
    // Brak karty danego producenta to normalna sytuacja — logujemy tylko coś, co da się naprawić
    if !blad_sprzetu(&powod) {
        crate::dziennik::info(&format!("Enkoder {} nie działa: {powod}", e.nazwa));
    }
    if let Some(wersja) = wymagany_sterownik_nvidia(&powod) {
        let tekst = format!(
            "Sterownik karty NVIDIA jest za stary do nagrywania kartą (potrzebny {wersja} lub nowszy). \
             Zaktualizuj go w aplikacji NVIDIA albo na nvidia.com — do tego czasu nagrywa procesor."
        );
        crate::dziennik::blad(&tekst);
        *OSTRZEZENIE.lock().unwrap_or_else(|x| x.into_inner()) = Some(tekst);
    }
    false
}

/// Komunikaty ffmpeg oznaczające, że karty danego producenta po prostu nie ma.
fn blad_sprzetu(powod: &str) -> bool {
    let p = powod.to_lowercase();
    [
        "cannot load nvcuda.dll",
        "cannot load nvencodeapi",
        "no capable devices found",
        "dll amfrt64.dll failed to open",
        "amfrt64",
        "unsupported device",
        "no device available",
        "error creating a mfx session",
    ]
    .iter()
    .any(|x| p.contains(x))
}

/// „The minimum required Nvidia driver for nvenc is 570.0 or newer” -> Some("570.0").
fn wymagany_sterownik_nvidia(powod: &str) -> Option<String> {
    let reszta = powod.split("required Nvidia driver for nvenc is ").nth(1)?;
    Some(reszta.split_whitespace().next()?.to_string())
}

/// Próbne zakodowanie kilku klatek (karty może nie być albo sterownik jej nie obsługuje); limit 15 s
/// (przy autostarcie Windows karta potrafi odpowiadać wolno). Błąd = ostatnie linie komunikatu ffmpeg.
fn proba(e: &Enkoder) -> Result<(), String> {
    use std::io::Read;
    let mut cmd = crate::ffmpeg::komenda();
    // 1280x720: część sterowników odrzuca bardzo małe klatki
    cmd.args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=black:s=1280x720:r=30"])
        .args(["-frames:v", "3"]);
    if wymaga_nv12(e.nazwa) {
        cmd.args(["-pix_fmt", "nv12"]);
    }
    cmd.args(["-c:v", e.nazwa, "-f", "null", "-"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    let mut proces = cmd.spawn().map_err(|x| format!("nie udało się uruchomić ffmpeg: {x}"))?;
    // stderr czytamy w tle, żeby pełny potok nie zatrzymał ffmpeg
    let mut stderr = proces.stderr.take().unwrap();
    let czytanie = std::thread::spawn(move || {
        let mut t = String::new();
        let _ = stderr.read_to_string(&mut t);
        t
    });
    let start = std::time::Instant::now();
    let wynik = loop {
        match proces.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if start.elapsed() < std::time::Duration::from_secs(15) => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            _ => {
                let _ = proces.kill();
                let _ = proces.wait();
                return Err("brak odpowiedzi przez 15 s".into());
            }
        }
    };
    let tekst = czytanie.join().unwrap_or_default();
    if wynik {
        return Ok(());
    }
    let linie: Vec<&str> = tekst.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    Err(if linie.is_empty() { "nieznany błąd".into() } else { linie[linie.len().saturating_sub(3)..].join(" | ") })
}

/// Wszystkie działające enkodery (do listy w ustawieniach).
pub fn dostepne() -> Vec<&'static Enkoder> {
    let lista: Vec<_> = kandydaci_wg_kart().into_iter().filter(|e| dziala(e)).collect();
    static ZAPISANO: OnceLock<()> = OnceLock::new();
    ZAPISANO.get_or_init(|| {
        crate::dziennik::info(&format!(
            "Dostępne enkodery: {}",
            lista.iter().map(|e| e.nazwa).collect::<Vec<_>>().join(", ")
        ))
    });
    lista
}

/// Producent karty, której używa enkoder (None = procesor).
fn producent(nazwa: &str) -> Option<crate::karty::Producent> {
    use crate::karty::Producent;
    match nazwa.rsplit('_').next()? {
        "amf" => Some(Producent::Amd),
        "nvenc" => Some(Producent::Nvidia),
        "qsv" => Some(Producent::Intel),
        _ => None,
    }
}

/// Kandydaci w kolejności: najpierw karta z największą pamięcią (przy kilku kartach — np. grafika
/// w procesorze i osobna karta — kodujemy na mocniejszej), potem pozostałe, procesor na końcu.
fn kandydaci_wg_kart() -> Vec<&'static Enkoder> {
    let kolejnosc = crate::karty::kolejnosc_producentow();
    let mut l: Vec<&'static Enkoder> = KANDYDACI.iter().collect();
    // Sortowanie stabilne: w obrębie producenta zostaje kolejność kodeków z KANDYDACI
    l.sort_by_key(|e| producent(e.nazwa).and_then(|p| kolejnosc.iter().position(|x| *x == p)).unwrap_or(usize::MAX));
    l
}

/// Najlepszy działający enkoder danej rodziny (dalszych nie sprawdzamy).
fn najlepszy(kodek: Kodek) -> Option<&'static str> {
    kandydaci_wg_kart().into_iter().filter(|e| e.kodek == kodek).find(|e| dziala(e)).map(|e| e.nazwa)
}

/// Enkoder przestał działać w trakcie nagrywania (np. po aktualizacji sterownika): przy następnym
/// starcie nagrywania sprawdzimy go od nowa, a jeśli nadal nie działa — weźmiemy inny.
pub fn zapomnij(nazwa: &str) {
    if let Some(w) = WYNIKI.get() {
        w.lock().unwrap_or_else(|x| x.into_inner()).remove(nazwa);
    }
}

/// Enkoder do nagrywania: wybrany w ustawieniach albo przy „auto” — najlepszy H.264.
/// Gdy wybrany przestał działać (np. zmiana karty), też bierzemy najlepszy H.264.
pub fn do_nagrywania(wybrany: &str) -> &'static str {
    if let Some(e) = znajdz(wybrany).filter(|e| dziala(e)) {
        return e.nazwa;
    }
    najlepszy(Kodek::H264).unwrap_or("h264_mf")
}

/// Enkoder do wersji przyciętych i zmniejszonych: zawsze H.264 (odtworzy go każdy komunikator).
pub fn do_eksportu() -> &'static str {
    najlepszy(Kodek::H264).unwrap_or("h264_mf")
}

pub fn kodek(nazwa: &str) -> Kodek {
    znajdz(nazwa).map(|e| e.kodek).unwrap_or(Kodek::H264)
}

/// Format surowego strumienia dla ffmpeg (wyjście enkodera i wejście przy zapisie klipu).
pub fn format_strumienia(nazwa: &str) -> &'static str {
    match kodek(nazwa) {
        Kodek::H264 => "h264",
        Kodek::Hevc => "hevc",
        Kodek::Av1 => "obu",
    }
}

/// QSV i Media Foundation nie przyjmują BGRA — potrzebują NV12.
fn wymaga_nv12(nazwa: &str) -> bool {
    nazwa.ends_with("_qsv") || nazwa.ends_with("_mf") || nazwa == "libx264"
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Argumenty enkodera do nagrywania strumienia (bez bitrate i GOP — te dokłada wywołujący).
/// Każda klatka kluczowa musi nieść nagłówki (SPS/PPS/VPS albo nagłówek sekwencji AV1),
/// a każda klatka H.264/HEVC znacznik AUD — po nich dzielimy strumień i od nich zaczynamy klip.
pub fn argumenty_nagrywania(nazwa: &str, fps: u32) -> Vec<String> {
    let g = fps.to_string();
    let mut a = match nazwa {
        "h264_amf" => s(&["-c:v", "h264_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1", "-header_spacing", &g]),
        "hevc_amf" => s(&[
            "-c:v", "hevc_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1", "-header_insertion_mode", "idr",
            "-gops_per_idr", "1",
        ]),
        "av1_amf" => s(&["-c:v", "av1_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-header_insertion_mode", "gop"]),
        n if n.ends_with("_nvenc") => s(&["-c:v", n, "-preset", "p4", "-tune", "ll", "-rc", "vbr", "-forced-idr", "1"]),
        n if n.ends_with("_qsv") => s(&["-c:v", n, "-preset", "faster", "-forced_idr", "1"]),
        "libx264" => s(&["-c:v", "libx264", "-preset", "veryfast", "-sc_threshold", "0", "-keyint_min", &g]),
        n => s(&["-c:v", n]),
    };
    if wymaga_nv12(nazwa) {
        a.extend(s(&["-pix_fmt", "nv12"]));
    }
    // Dla enkoderów innych niż AMF nagłówki i AUD dokładają filtry strumienia — niezależnie od sterownika
    if !nazwa.ends_with("_amf") {
        let filtr = match kodek(nazwa) {
            Kodek::H264 => "h264_metadata=aud=insert,dump_extra=freq=keyframe",
            Kodek::Hevc => "hevc_metadata=aud=insert,dump_extra=freq=keyframe",
            Kodek::Av1 => "dump_extra=freq=keyframe",
        };
        a.extend(s(&["-bsf:v", filtr]));
    }
    a
}

/// Argumenty enkodera do eksportu pliku (H.264) z podanym bitrate. Przy celu rozmiaru (`sztywny_limit`)
/// szczyt = średnia, żeby plik na pewno się zmieścił.
pub fn argumenty_eksportu(nazwa: &str, bps: u64, sztywny_limit: bool) -> Vec<String> {
    let b = bps.to_string();
    let max = if sztywny_limit { b.clone() } else { (bps * 3 / 2).to_string() };
    let mut a = match nazwa {
        n if n.ends_with("_amf") => s(&["-c:v", n, "-rc", "vbr_peak", "-quality", "quality"]),
        n if n.ends_with("_nvenc") => s(&["-c:v", n, "-preset", "p5", "-rc", "vbr"]),
        n if n.ends_with("_qsv") => s(&["-c:v", n, "-preset", "medium"]),
        "libx264" => s(&["-c:v", "libx264", "-preset", "medium"]),
        n => s(&["-c:v", n]),
    };
    a.extend(s(&["-b:v", &b, "-maxrate", &max, "-bufsize", &b]));
    a.extend(s(&["-pix_fmt", if nazwa.ends_with("_amf") || nazwa.ends_with("_nvenc") { "yuv420p" } else { "nv12" }]));
    a
}

/// Lista do ustawień: (nazwa, opis); pierwsza pozycja to „auto” z informacją, co zostanie użyte.
pub fn do_ustawien() -> Vec<(String, String)> {
    let auto = znajdz(do_nagrywania(AUTO)).map(|e| e.opis).unwrap_or("brak");
    let uwaga = if ostrzezenie().is_some() { " (zaktualizuj sterownik NVIDIA)" } else { "" };
    let mut l = vec![(AUTO.to_string(), format!("Automatycznie — teraz: {auto}{uwaga}"))];
    l.extend(dostepne().iter().map(|e| (e.nazwa.to_string(), e.opis.to_string())));
    l
}

/// Czy nazwa z ustawień jest znana (sama dostępność sprawdzana jest przy nagrywaniu).
pub fn poprawna_nazwa(nazwa: &str) -> bool {
    nazwa == AUTO || znajdz(nazwa).is_some()
}
