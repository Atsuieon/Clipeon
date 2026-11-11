// Enkodery wideo: wykrywanie, które działają na tym komputerze (karta AMD, NVIDIA, Intel albo
// procesor), wybór najlepszego przy ustawieniu „auto” i argumenty ffmpeg dla każdego z nich.
// Wszystkie są dostępne w buildzie FFmpeg na licencji LGPL (bez GPL-owego x264).
// Próby robimy w rozdzielczości nagrywanego ekranu (karta może nie obsłużyć np. ultrawide w H.264),
// a wyniki pamiętamy na dysku (pamiec.rs), żeby kolejny start nie uruchamiał ffmpeg kilkanaście razy.
use crate::Zablokuj;
use crate::karty::Producent;
use std::sync::OnceLock;

/// Rodzina kodeka — od niej zależy format surowego strumienia i dzielenie go na klatki.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kodek {
    H264,
    Hevc,
    Av1,
}

pub struct Enkoder {
    /// Nazwa w ustawieniach Clipeona
    pub nazwa: &'static str,
    pub opis: &'static str,
    pub kodek: Kodek,
    /// Nazwa enkodera w ffmpeg (zwykle ta sama co `nazwa`)
    pub ffmpeg: &'static str,
}

const fn e(nazwa: &'static str, opis: &'static str, kodek: Kodek) -> Enkoder {
    Enkoder { nazwa, opis, kodek, ffmpeg: nazwa }
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
    // Karta przez Windows Media Foundation: działa też ze starszym sterownikiem NVIDIA (poniżej 570),
    // którego NVENC w naszym ffmpeg już nie obsługuje
    Enkoder {
        nazwa: "h264_mf_sprzet",
        opis: "H.264 — karta przez Windows (starszy sterownik)",
        kodek: Kodek::H264,
        ffmpeg: "h264_mf",
    },
    e("h264_mf", "H.264 — procesor (wolniejsze, gdy karta nie działa)", Kodek::H264),
    // Gdy Windows nie ma Media Foundation (wydania „N” bez Media Feature Pack)
    e("libopenh264", "H.264 — procesor, OpenH264 (gdy Windows nie ma kodeków)", Kodek::H264),
    e("libx264", "H.264 — procesor, x264", Kodek::H264),
];

pub const AUTO: &str = "auto";

/// Rozdzielczość, w której domyślnie sprawdzamy enkodery (lista w ustawieniach, eksport).
const PROBA_DOMYSLNA: (u32, u32) = (1280, 720);

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
        KANDYDACI.iter().filter(|e| lista.lines().any(|l| l.split_whitespace().nth(1) == Some(e.ffmpeg))).collect()
    })
}

/// Jak sprawdzamy enkoder: rozdzielczość i czy klatki przychodzą jako tekstury D3D11 z danej karty
/// (tak je dostaje przy przechwytywaniu przez ffmpeg — obraz nie opuszcza karty).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Proba {
    pub w: u32,
    pub h: u32,
    pub tekstura: Option<u32>,
    /// Wyjście (ekran) tej karty: próba bierze wtedy prawdziwy obraz z ddagrab, a nie wgrany
    /// sztuczny. h264_amf na RX 7900 XTX przyjmował wgrany obraz, a na klatkach z przechwytywania padał.
    pub ekran: Option<u32>,
}

impl Proba {
    pub fn zwykla(w: u32, h: u32) -> Self {
        Proba { w, h, tekstura: None, ekran: None }
    }

    fn klucz(&self, e: &Enkoder) -> String {
        let t = self.tekstura.map(|k| format!("/d3d11:{k}")).unwrap_or_default();
        let ekran = self.ekran.map(|o| format!("/dda:{o}")).unwrap_or_default();
        format!("{}@{}x{}{t}{ekran}", e.nazwa, self.w, self.h)
    }
}

/// Wyniki prób w tym uruchomieniu: (działa, przyczyna, kiedy). Porażkę „do naprawienia” pamiętamy
/// tylko kilka minut: przy autostarcie karta bywa jeszcze zajęta, a sterownik mógł się zmienić.
type Wyniki = std::collections::HashMap<String, (bool, String, std::time::Instant)>;
static WYNIKI: OnceLock<std::sync::Mutex<Wyniki>> = OnceLock::new();

fn wyniki() -> std::sync::MutexGuard<'static, Wyniki> {
    WYNIKI.get_or_init(Default::default).zablokuj()
}

/// Czy enkoder działa tutaj przy danej próbie. Kolejność: wynik z tego uruchomienia, wynik z dysku
/// (sukces albo „nie ma takiej karty”), a dopiero potem prawdziwa próba. Próby idą po kolei —
/// równoległe sesje na jednej karcie potrafiły ciągnąć się kilkanaście sekund.
fn dziala_przy(e: &'static Enkoder, p: Proba) -> bool {
    use std::time::{Duration, Instant};
    let klucz = p.klucz(e);
    // Blokada trzymana przez całą próbę = próby nigdy nie idą równolegle
    let mut w = wyniki();
    if let Some((ok, _, kiedy)) = w.get(&klucz)
        && (*ok || kiedy.elapsed() < Duration::from_secs(300)) {
            return *ok;
        }
    if !w.contains_key(&klucz)
        && let Some(z) = crate::pamiec::pamiec().proby.get(&klucz).cloned()
            && (z.ok || blad_sprzetu(&z.powod) || blad_tekstur(&z.powod)) {
                w.insert(klucz, (z.ok, z.powod, Instant::now()));
                return z.ok;
            }
    let wynik = if wbudowane().iter().any(|x| x.nazwa == e.nazwa) {
        proba_z_ponowieniem(e, p)
    } else {
        Err("brak w tym ffmpeg".into())
    };
    let (ok, powod) = match wynik {
        Ok(()) => (true, String::new()),
        Err(p) => (false, p),
    };
    w.insert(klucz.clone(), (ok, powod.clone(), Instant::now()));
    drop(w);
    let mut pam = crate::pamiec::pamiec();
    pam.proby.insert(klucz, crate::pamiec::WynikProby { ok, powod });
    crate::pamiec::zapisz(&pam);
    ok
}

fn dziala(e: &'static Enkoder) -> bool {
    dziala_przy(e, Proba::zwykla(PROBA_DOMYSLNA.0, PROBA_DOMYSLNA.1))
}

/// Czy enkoder przyjmie tekstury D3D11 z karty `karta` w rozdzielczości w×h (warunek przechwytywania
/// przez ffmpeg bez kopiowania obrazu do pamięci RAM).
/// `ekran` = wyjście tej karty z monitorem (próba na prawdziwym obrazie z ekranu), gdy jest znane.
pub fn dziala_z_tekstura(nazwa: &str, w: u32, h: u32, karta: u32, ekran: Option<u32>) -> bool {
    znajdz(nazwa).is_some_and(|e| przyjmuje_tekstury(nazwa) && dziala_przy(e, Proba { w, h, tekstura: Some(karta), ekran }))
}

/// Najstarszy sterownik NVIDIA obsługiwany przez NVENC w dołączonym FFmpeg 8.1 (SDK 13).
const MIN_NVIDIA: f64 = 570.0;

/// Ostrzeżenia dla użytkownika (np. za stary sterownik NVIDIA, za duży ekran), pokazywane raz.
static OSTRZEZENIE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn ostrzez(tekst: String) {
    let mut o = OSTRZEZENIE.zablokuj();
    if o.is_none() {
        crate::dziennik::blad(&tekst);
        *o = Some(tekst);
    }
}

pub fn ostrzezenie() -> Option<String> {
    // Za stary sterownik NVIDIA widać już po wersji (gdy karta jest najmocniejsza w komputerze)
    static SPRAWDZONO: OnceLock<()> = OnceLock::new();
    SPRAWDZONO.get_or_init(|| {
        let glowna = crate::karty::karty().first();
        if let Some(v) = glowna.and_then(|k| k.wersja_nvidia()).filter(|v| *v < MIN_NVIDIA) {
            ostrzez(format!(
                "Sterownik karty NVIDIA ({v:.2}) jest za stary do nagrywania przez NVENC (potrzebny {MIN_NVIDIA:.0} lub nowszy). \
                 Zaktualizuj go w aplikacji NVIDIA albo na nvidia.com — do tego czasu Clipeon nagrywa przez Windows albo procesor."
            ));
        }
    });
    OSTRZEZENIE.zablokuj().clone()
}

/// Enkoder sprzętowy (karta), a nie procesor.
fn sprzetowy(nazwa: &str) -> bool {
    producent(nazwa).is_some() || nazwa == "h264_mf_sprzet"
}

/// Sprzętowa próba może się nie udać przy pierwszym podejściu (zimny start sterownika) — druga szansa.
fn proba_z_ponowieniem(e: &Enkoder, p: Proba) -> Result<(), String> {
    let mut powod = match proba(e, p) {
        Ok(()) => return Ok(()),
        Err(x) => x,
    };
    if sprzetowy(e.nazwa) && !blad_sprzetu(&powod) {
        match proba(e, p) {
            Ok(()) => return Ok(()),
            Err(x) => powod = x,
        }
    }
    // Brak karty danego producenta to normalna sytuacja — logujemy tylko coś, co da się naprawić
    if !blad_sprzetu(&powod) {
        crate::dziennik::info(&format!("Enkoder {} ({}x{}{}) nie działa: {powod}", e.nazwa, p.w, p.h,
            if p.tekstura.is_some() { ", tekstury z karty" } else { "" }));
    }
    if let Some(wersja) = wymagany_sterownik_nvidia(&powod) {
        ostrzez(format!(
            "Sterownik karty NVIDIA jest za stary do nagrywania przez NVENC (potrzebny {wersja} lub nowszy). \
             Zaktualizuj go w aplikacji NVIDIA albo na nvidia.com — do tego czasu Clipeon nagrywa przez Windows albo procesor."
        ));
    }
    Err(powod)
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
        "brak w tym ffmpeg",
    ]
    .iter()
    .any(|x| p.contains(x))
}

/// Enkoder odrzuca klatki prosto z karty (np. h264_amf: „SubmitInput() failed with error 18”).
/// To cecha sterownika i ffmpeg, a nie chwilowy stan — pamiętamy ją do ich zmiany, więc kolejne
/// starty od razu nagrywają drogą przez RAM zamiast tracić kilka sekund na próby.
fn blad_tekstur(powod: &str) -> bool {
    powod.to_lowercase().contains("submitinput")
}

/// „The minimum required Nvidia driver for nvenc is 570.0 or newer” -> Some("570.0").
fn wymagany_sterownik_nvidia(powod: &str) -> Option<String> {
    let reszta = powod.split("required Nvidia driver for nvenc is ").nth(1)?;
    Some(reszta.split_whitespace().next()?.to_string())
}

/// Argumenty wybierające enkoder w ffmpeg (bez bitrate, GOP i opcji strumienia).
fn wybor_enkodera(e: &Enkoder) -> Vec<String> {
    let mut a = s(&["-c:v", e.ffmpeg]);
    if e.nazwa == "h264_mf_sprzet" {
        a.extend(s(&["-hw_encoding", "1"]));
    }
    a
}

/// Próbne zakodowanie kilku klatek (karty może nie być albo sterownik jej nie obsługuje); limit 15 s
/// (przy autostarcie Windows karta potrafi odpowiadać wolno). Błąd = ostatnie linie komunikatu ffmpeg.
fn proba(e: &Enkoder, p: Proba) -> Result<(), String> {
    use std::io::Read;
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    if let Some(karta) = p.tekstura {
        cmd.args(["-init_hw_device", &format!("d3d11va=gpu:{karta}"), "-filter_hw_device", "gpu"]);
    }
    match (p.tekstura, p.ekran) {
        // Prawdziwy obraz z ekranu, tak jak przy nagrywaniu (ddagrab daje klatki także przy nieruchomym ekranie)
        (Some(_), Some(o)) => {
            cmd.args(["-filter_complex", &format!("ddagrab=output_idx={o}:framerate=30:video_size={}x{}", p.w, p.h)]);
        }
        (Some(_), None) => {
            cmd.args(["-f", "lavfi", "-i", &format!("color=black:s={}x{}:r=30", p.w, p.h), "-vf", "format=bgra,hwupload"]);
        }
        (None, _) => {
            cmd.args(["-f", "lavfi", "-i", &format!("color=black:s={}x{}:r=30", p.w, p.h)]);
            if let Some(f) = format_wejscia(e.nazwa) {
                cmd.args(["-pix_fmt", f]);
            }
        }
    }
    cmd.args(["-frames:v", "3"]);
    cmd.args(wybor_enkodera(e))
        .args(["-f", "null", "-"])
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

/// Producent karty, której używa enkoder (None = procesor albo Media Foundation).
pub fn producent(nazwa: &str) -> Option<Producent> {
    match nazwa.rsplit('_').next()? {
        "amf" => Some(Producent::Amd),
        "nvenc" => Some(Producent::Nvidia),
        "qsv" => Some(Producent::Intel),
        _ => None,
    }
}

/// Czy enkoder przyjmuje klatki jako tekstury D3D11 prosto z przechwytywania (AMF i NVENC tak;
/// Quick Sync i enkodery przez procesor potrzebują klatek w pamięci RAM).
pub fn przyjmuje_tekstury(nazwa: &str) -> bool {
    nazwa.ends_with("_amf") || nazwa.ends_with("_nvenc")
}

/// Kandydaci w kolejności producentów (od najmocniejszej karty), procesor na końcu.
/// Sortowanie stabilne: w obrębie producenta zostaje kolejność kodeków z KANDYDACI.
pub fn posortuj(kolejnosc: &[Producent]) -> Vec<&'static Enkoder> {
    let mut l: Vec<&'static Enkoder> = KANDYDACI.iter().collect();
    l.sort_by_key(|e| producent(e.nazwa).and_then(|p| kolejnosc.iter().position(|x| *x == p)).unwrap_or(usize::MAX));
    l
}

/// Kandydaci w kolejności: najpierw karta z największą pamięcią (przy kilku kartach — np. grafika
/// w procesorze i osobna karta — kodujemy na mocniejszej), potem pozostałe, procesor na końcu.
fn kandydaci_wg_kart() -> Vec<&'static Enkoder> {
    posortuj(&crate::karty::kolejnosc_producentow())
}

/// Najlepszy działający enkoder danej rodziny przy danej próbie (dalszych nie sprawdzamy).
fn najlepszy_przy(kodek: Kodek, p: Proba) -> Option<&'static str> {
    kandydaci_wg_kart().into_iter().filter(|e| e.kodek == kodek).find(|e| dziala_przy(e, p)).map(|e| e.nazwa)
}

fn najlepszy(kodek: Kodek) -> Option<&'static str> {
    najlepszy_przy(kodek, Proba::zwykla(PROBA_DOMYSLNA.0, PROBA_DOMYSLNA.1))
}

/// Enkoder przestał działać w trakcie nagrywania (np. po aktualizacji sterownika): przy następnym
/// starcie nagrywania sprawdzimy go od nowa, a jeśli nadal nie działa — weźmiemy inny.
pub fn zapomnij(nazwa: &str) {
    let przedrostek = format!("{nazwa}@");
    wyniki().retain(|k, _| !k.starts_with(&przedrostek));
    let mut p = crate::pamiec::pamiec();
    p.proby.retain(|k, _| !k.starts_with(&przedrostek));
    crate::pamiec::zapisz(&p);
}

/// Wybrany enkoder i rozdzielczość, w której koduje (mniejsza od ekranu, gdy karta nie daje rady).
#[derive(Clone, Debug, PartialEq)]
pub struct Wybor {
    pub nazwa: &'static str,
    pub w: u32,
    pub h: u32,
}

/// Największy rozmiar, który obsłuży H.264 na każdej karcie (poziom 5.1/5.2), z zachowaniem proporcji
/// i parzystymi wymiarami. None = ekran już się mieści.
pub fn zmniejszony(w: u32, h: u32) -> Option<(u32, u32)> {
    const MAX_W: f64 = 4096.0;
    const MAX_H: f64 = 2304.0;
    let skala = (MAX_W / w as f64).min(MAX_H / h as f64);
    if skala >= 1.0 {
        return None;
    }
    let parzyste = |v: f64| ((v.floor() as u32) & !1).max(2);
    Some((parzyste(w as f64 * skala), parzyste(h as f64 * skala)))
}

/// Enkoder do nagrywania ekranu w×h: wybrany w ustawieniach albo przy „auto” — najlepszy H.264.
/// Gdy wybrany nie działa w tej rozdzielczości: najlepszy H.264, potem HEVC (obsługuje większe ekrany),
/// a na końcu H.264 w zmniejszonym obrazie.
pub fn do_nagrywania(wybrany: &str, w: u32, h: u32) -> Wybor {
    let p = Proba::zwykla(w, h);
    if let Some(e) = znajdz(wybrany).filter(|e| dziala_przy(e, p)) {
        return Wybor { nazwa: e.nazwa, w, h };
    }
    if let Some(n) = najlepszy_przy(Kodek::H264, p) {
        return Wybor { nazwa: n, w, h };
    }
    if let Some(n) = najlepszy_przy(Kodek::Hevc, p) {
        ostrzez(format!("Ekran {w}×{h} jest za duży dla H.264 na tej karcie — Clipeon nagrywa go w HEVC."));
        return Wybor { nazwa: n, w, h };
    }
    if let Some((mw, mh)) = zmniejszony(w, h)
        && let Some(n) = najlepszy_przy(Kodek::H264, Proba::zwykla(mw, mh)) {
            ostrzez(format!("Ekran {w}×{h} jest za duży dla enkodera — Clipeon nagrywa go zmniejszony do {mw}×{mh}."));
            return Wybor { nazwa: n, w: mw, h: mh };
        }
    Wybor { nazwa: "h264_mf", w, h }
}

/// Enkoder do wersji przyciętych i zmniejszonych: zawsze H.264 (odtworzy go każdy komunikator).
pub fn do_eksportu() -> &'static str {
    najlepszy(Kodek::H264).unwrap_or("h264_mf")
}

pub fn kodek(nazwa: &str) -> Kodek {
    znajdz(nazwa).map(|e| e.kodek).unwrap_or(Kodek::H264)
}

/// Nazwa enkodera w ffmpeg (h264_mf_sprzet -> h264_mf).
#[cfg(test)]
pub fn nazwa_ffmpeg(nazwa: &str) -> &str {
    znajdz(nazwa).map(|e| e.ffmpeg).unwrap_or(nazwa)
}

/// Format surowego strumienia dla ffmpeg (wyjście enkodera i wejście przy zapisie klipu).
pub fn format_strumienia(nazwa: &str) -> &'static str {
    match kodek(nazwa) {
        Kodek::H264 => "h264",
        Kodek::Hevc => "hevc",
        Kodek::Av1 => "obu",
    }
}

/// Format pikseli, którego enkoder potrzebuje na wejściu z pamięci RAM (None = przyjmie BGRA).
/// QSV i Media Foundation chcą NV12, OpenH264 — YUV 4:2:0.
pub fn format_wejscia(nazwa: &str) -> Option<&'static str> {
    if nazwa.ends_with("_qsv") || nazwa.starts_with("h264_mf") || nazwa == "libx264" {
        Some("nv12")
    } else if nazwa == "libopenh264" {
        Some("yuv420p")
    } else {
        None
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Argumenty enkodera do nagrywania strumienia (bez bitrate, GOP i formatu pikseli — te dokłada
/// wywołujący). Każda klatka kluczowa musi nieść nagłówki (SPS/PPS/VPS albo nagłówek sekwencji AV1),
/// a każda klatka H.264/HEVC znacznik AUD — po nich dzielimy strumień i od nich zaczynamy klip.
/// `lekki` = lżejsze ustawienia, gdy karta lub procesor nie nadąża (gra zabiera całą moc).
pub fn argumenty_nagrywania(nazwa: &str, fps: u32, lekki: bool) -> Vec<String> {
    let g = fps.to_string();
    let mut a = match nazwa {
        "h264_amf" => s(&["-c:v", "h264_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1", "-header_spacing", &g]),
        "hevc_amf" => s(&[
            "-c:v", "hevc_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-aud", "1", "-header_insertion_mode", "idr",
            "-gops_per_idr", "1",
        ]),
        "av1_amf" => s(&["-c:v", "av1_amf", "-rc", "vbr_peak", "-forced_idr", "1", "-header_insertion_mode", "gop"]),
        n if n.ends_with("_nvenc") => {
            s(&["-c:v", n, "-preset", if lekki { "p1" } else { "p4" }, "-tune", "ll", "-rc", "vbr", "-forced-idr", "1"])
        }
        n if n.ends_with("_qsv") => s(&["-c:v", n, "-preset", if lekki { "veryfast" } else { "faster" }, "-forced_idr", "1"]),
        "libx264" => s(&["-c:v", "libx264", "-preset", if lekki { "superfast" } else { "veryfast" }, "-sc_threshold", "0", "-keyint_min", &g]),
        n => znajdz(n).map(wybor_enkodera).unwrap_or_else(|| s(&["-c:v", n])),
    };
    if lekki && nazwa.ends_with("_amf") {
        a.extend(s(&["-quality", "speed"]));
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
        n => znajdz(n).map(wybor_enkodera).unwrap_or_else(|| s(&["-c:v", n])),
    };
    a.extend(s(&["-b:v", &b, "-maxrate", &max, "-bufsize", &b]));
    let pix = match nazwa {
        n if n.ends_with("_amf") || n.ends_with("_nvenc") || n == "libopenh264" => "yuv420p",
        _ => "nv12",
    };
    a.extend(s(&["-pix_fmt", pix]));
    a
}

/// Lista do ustawień: (nazwa, opis); pierwsza pozycja to „auto” z informacją, co zostanie użyte.
pub fn do_ustawien() -> Vec<(String, String)> {
    let auto = najlepszy(Kodek::H264).and_then(znajdz).map(|e| e.opis).unwrap_or("brak");
    let uwaga = if ostrzezenie().is_some_and(|o| o.contains("NVIDIA")) { " (zaktualizuj sterownik NVIDIA)" } else { "" };
    let mut l = vec![(AUTO.to_string(), format!("Automatycznie — teraz: {auto}{uwaga}"))];
    l.extend(dostepne().iter().map(|e| (e.nazwa.to_string(), e.opis.to_string())));
    l
}

/// Czy nazwa z ustawień jest znana (sama dostępność sprawdzana jest przy nagrywaniu).
pub fn poprawna_nazwa(nazwa: &str) -> bool {
    nazwa == AUTO || znajdz(nazwa).is_some()
}

/// Wyniki prób z tego uruchomienia (do diagnostyki): „enkoder@rozmiar: działa / nie działa (przyczyna)”.
pub fn raport() -> Vec<String> {
    let mut l: Vec<String> = wyniki()
        .iter()
        .map(|(k, (ok, powod, _))| if *ok { format!("{k}: działa") } else { format!("{k}: nie działa ({powod})") })
        .collect();
    l.sort();
    l
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn kolejnosc_producentow_ustala_kolejnosc_kandydatow() {
        let l = posortuj(&[Producent::Nvidia, Producent::Amd, Producent::Intel]);
        let nazwy: Vec<_> = l.iter().map(|e| e.nazwa).collect();
        // Najpierw wszystkie NVIDIA (w kolejności kodeków), potem AMD, Intel, a procesor na końcu
        assert_eq!(&nazwy[..3], &["h264_nvenc", "hevc_nvenc", "av1_nvenc"]);
        assert_eq!(&nazwy[3..6], &["h264_amf", "hevc_amf", "av1_amf"]);
        assert_eq!(&nazwy[nazwy.len() - 4..], &["h264_mf_sprzet", "h264_mf", "libopenh264", "libx264"]);
    }

    #[test]
    fn zmniejszanie_zachowuje_proporcje_i_parzystosc() {
        assert_eq!(zmniejszony(1920, 1080), None);
        assert_eq!(zmniejszony(3840, 2160), None);
        let (w, h) = zmniejszony(5120, 1440).unwrap();
        assert!(w <= 4096 && h <= 2304 && w % 2 == 0 && h % 2 == 0);
        assert!(((w as f64 / h as f64) - 5120.0 / 1440.0).abs() < 0.01);
        let (w, h) = zmniejszony(7680, 4320).unwrap();
        assert!(w <= 4096 && h <= 2304 && w % 2 == 0 && h % 2 == 0);
    }

    #[test]
    fn proba_na_obrazie_z_ekranu_ma_wlasny_klucz() {
        let e = znajdz("h264_amf").unwrap();
        let wgrany = Proba { w: 1920, h: 1080, tekstura: Some(0), ekran: None };
        let z_ekranu = Proba { ekran: Some(1), ..wgrany };
        assert_eq!(wgrany.klucz(e), "h264_amf@1920x1080/d3d11:0");
        assert_eq!(z_ekranu.klucz(e), "h264_amf@1920x1080/d3d11:0/dda:1");
    }

    #[test]
    fn odrzucone_tekstury_to_trwaly_blad() {
        assert!(blad_tekstur("[h264_amf @ 0x1] SubmitInput() failed with error 18"));
        assert!(!blad_tekstur("[Parsed_ddagrab_0 @ 0x1] AcquireNextFrame failed: 887a0026"));
    }

    #[test]
    fn tekstury_tylko_dla_amf_i_nvenc() {
        assert!(przyjmuje_tekstury("h264_amf"));
        assert!(przyjmuje_tekstury("av1_nvenc"));
        assert!(!przyjmuje_tekstury("h264_qsv"));
        assert!(!przyjmuje_tekstury("h264_mf_sprzet"));
        assert!(!przyjmuje_tekstury("libopenh264"));
    }

    #[test]
    fn enkoder_przez_windows_ma_wlasna_nazwe_w_ffmpeg() {
        assert_eq!(nazwa_ffmpeg("h264_mf_sprzet"), "h264_mf");
        assert_eq!(format_wejscia("h264_mf_sprzet"), Some("nv12"));
        let a = argumenty_nagrywania("h264_mf_sprzet", 60, false);
        assert!(a.windows(2).any(|w| w == ["-c:v", "h264_mf"]));
        assert!(a.windows(2).any(|w| w == ["-hw_encoding", "1"]));
    }

    #[test]
    fn lekkie_ustawienia() {
        assert!(argumenty_nagrywania("h264_nvenc", 60, true).contains(&"p1".to_string()));
        assert!(argumenty_nagrywania("h264_amf", 60, true).windows(2).any(|w| w == ["-quality", "speed"]));
        assert!(!argumenty_nagrywania("h264_amf", 60, false).contains(&"-quality".to_string()));
    }
}
