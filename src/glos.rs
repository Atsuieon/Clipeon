// Komenda głosowa: powiedziana fraza (np. „record video”) zapisuje klip jak skrót Alt+F10.
// Rozpoznawanie jest w `rozpoznawanie.rs` (odcisk frazy z 3 próbek głosu użytkownika, bez sieci).
// Próbki przychodzą ze ścieżki „Mikrofon”, która i tak się nagrywa (`audio::przechwytuj`) — Clipeon nie
// otwiera mikrofonu drugi raz. Wątek dźwięku tylko kopiuje pakiet do kanału (gdy funkcja jest włączona),
// a liczenie odbywa się w osobnym wątku „glos”.
use crate::rozpoznawanie::{self as rozp, Przelicznik, Ramka, Sluchacz};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

/// Ile razy użytkownik mówi frazę przy nauce
pub const PROBEK_NAUKI: usize = 3;
/// Po rozpoznaniu tyle czasu przerwy (powtórzona fraza nie zapisze dwóch klipów naraz)
const PRZERWA_PO_ROZPOZNANIU: Duration = Duration::from_secs(3);
/// Najdłużej czekamy na jedno powiedzenie frazy przy nauce
const CZEKANIE_NA_PROBKE: Duration = Duration::from_secs(8);
/// Gdy przez tyle czasu przy nauce nie przyjdzie nic z mikrofonu, mikrofon się nie nagrywa
const CZEKANIE_NA_MIKROFON: Duration = Duration::from_millis(1500);
/// Przerwa w dopływie próbek dłuższa niż to (pauza, restart nagrywania) = słuchamy od nowa
const PRZERWA_STRUMIENIA: Duration = Duration::from_secs(1);
/// Próbki, które za bardzo różnią się od siebie (kaszel, ktoś wszedł w słowo), nie nadają się na wzorzec
const NAJWIEKSZY_ROZRZUT: f32 = 12.0;

/// Czy wątek dźwięku ma przysyłać próbki mikrofonu (włączona funkcja z nauczoną frazą albo trwa nauka)
static SLUCHA: AtomicBool = AtomicBool::new(false);
/// Czy jest zapisana fraza (do okna ustawień)
static NAUCZONA: AtomicBool = AtomicBool::new(false);
/// Czy od początku nauki przyszły jakiekolwiek próbki
static PROBKI_PRZYSZLY: AtomicBool = AtomicBool::new(false);
static NADAJNIK: OnceLock<mpsc::Sender<Wiadomosc>> = OnceLock::new();
/// Próbki zebrane w trwającej nauce
static NAUKA: Mutex<Vec<Vec<Ramka>>> = Mutex::new(Vec::new());

enum Wiadomosc {
    Probki { czestotliwosc: u32, kanaly: u16, bity: u16, dane: Vec<u8> },
    /// Pakiet oznaczony przez Windows jako cisza
    Cisza { czestotliwosc: u32, ramek: u32 },
    Ustaw { wlaczona: bool, czulosc: u32 },
    Wzorce(Option<Wzorce>),
    /// Następna wypowiedź idzie do nauki zamiast do rozpoznawania (None = koniec czekania)
    Nauka(Option<mpsc::Sender<Vec<Ramka>>>),
}

/// Nauczona fraza: znormalizowane cechy każdej próbki i ich rozrzut (podstawa progu).
#[derive(Clone, Serialize, Deserialize)]
struct Wzorce {
    wzorce: Vec<Vec<Ramka>>,
    rozrzut: f32,
}

fn sciezka() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Clipeon").join("glos.json"))
}

fn wczytaj() -> Option<Wzorce> {
    let tekst = std::fs::read_to_string(sciezka()?).ok()?;
    match serde_json::from_str::<Wzorce>(&tekst) {
        Ok(w) if w.wzorce.len() >= 2 && w.rozrzut.is_finite() => Some(w),
        _ => {
            crate::dziennik::blad("Komenda głosowa: plik z nauczoną frazą jest uszkodzony — naucz frazy od nowa");
            None
        }
    }
}

fn wyslij(w: Wiadomosc) {
    if let Some(n) = NADAJNIK.get() {
        let _ = n.send(w);
    }
}

/// Uruchamia wątek rozpoznawania (raz, przy starcie programu). `przy_frazie` jest wołane z wątku „glos”
/// po rozpoznaniu frazy.
pub fn uruchom(przy_frazie: impl Fn() + Send + 'static) {
    let (tx, rx) = mpsc::channel();
    if NADAJNIK.set(tx).is_err() {
        return;
    }
    let wzorce = wczytaj();
    NAUCZONA.store(wzorce.is_some(), Ordering::SeqCst);
    let _ = std::thread::Builder::new().name("glos".into()).spawn(move || petla(rx, wzorce, przy_frazie));
}

/// Włącza / wyłącza rozpoznawanie (z configu: funkcja i mikrofon włączone) i ustawia czułość 1–3.
pub fn ustaw(wlaczona: bool, czulosc: u32) {
    wyslij(Wiadomosc::Ustaw { wlaczona, czulosc });
}

pub fn nauczona() -> bool {
    NAUCZONA.load(Ordering::SeqCst)
}

/// Pakiet z mikrofonu (z wątku dźwięku). `dane` = None: pakiet ciszy. Gdy nikt nie słucha — nic nie robi.
pub fn probki(czestotliwosc: u32, kanaly: u16, bity: u16, dane: Option<&[u8]>, ramek: u32) {
    if !SLUCHA.load(Ordering::Relaxed) {
        return;
    }
    wyslij(match dane {
        Some(d) => Wiadomosc::Probki { czestotliwosc, kanaly, bity, dane: d.to_vec() },
        None => Wiadomosc::Cisza { czestotliwosc, ramek },
    });
}

/// Postęp nauki dla okna
#[derive(Serialize)]
pub struct Nauka {
    /// Ile próbek już jest (po zapisie frazy = PROBEK_NAUKI)
    pub zebrane: usize,
    /// Fraza zapisana — od teraz działa
    pub gotowe: bool,
}

/// Czeka (blokująco, do 8 s), aż użytkownik powie frazę, i dokłada ją do nauki. `od_nowa` zaczyna
/// naukę od pierwszej próbki. Po trzeciej próbce fraza jest zapisywana i od razu działa.
pub fn nagraj_probke(od_nowa: bool) -> Result<Nauka, String> {
    if od_nowa {
        NAUKA.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
    let (tx, rx) = mpsc::channel();
    PROBKI_PRZYSZLY.store(false, Ordering::SeqCst);
    SLUCHA.store(true, Ordering::SeqCst);
    wyslij(Wiadomosc::Nauka(Some(tx)));
    let start = Instant::now();
    let wynik = loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(w) => break Ok(w),
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("Rozpoznawanie mowy nie działa".to_string()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if start.elapsed() >= CZEKANIE_NA_MIKROFON && !PROBKI_PRZYSZLY.load(Ordering::SeqCst) {
            break Err("Mikrofon się nie nagrywa. Włącz „Mój mikrofon” i zapisz ustawienia, a potem spróbuj jeszcze raz.".into());
        }
        if start.elapsed() >= CZEKANIE_NA_PROBKE {
            break Err("Nic nie usłyszałem. Powiedz frazę wyraźnie, z krótką przerwą przed i po.".into());
        }
    };
    wyslij(Wiadomosc::Nauka(None));
    let ramki = wynik?;
    let mut nauka = NAUKA.lock().unwrap_or_else(|e| e.into_inner());
    nauka.push(rozp::normalizuj(ramki));
    if nauka.len() < PROBEK_NAUKI {
        return Ok(Nauka { zebrane: nauka.len(), gotowe: false });
    }
    let wzorce = std::mem::take(&mut *nauka);
    drop(nauka);
    let dlugosci: Vec<usize> = wzorce.iter().map(|w| w.len()).collect();
    let rozrzut = rozp::rozrzut(&wzorce);
    crate::dziennik::info(&format!(
        "Komenda głosowa: nauka — długości próbek {:?} (×10 ms), rozrzut {rozrzut:.2}",
        dlugosci
    ));
    let (min, max) = (dlugosci.iter().min().copied().unwrap_or(0), dlugosci.iter().max().copied().unwrap_or(0));
    if max > min * 2 || rozrzut > NAJWIEKSZY_ROZRZUT {
        return Err("Te trzy powtórzenia za bardzo się różnią (może coś przerwało). Spróbuj jeszcze raz, w ciszy.".into());
    }
    let w = Wzorce { wzorce, rozrzut };
    let sciezka = sciezka().ok_or("Brak zmiennej APPDATA")?;
    let tekst = serde_json::to_string(&w).map_err(|e| e.to_string())?;
    std::fs::write(&sciezka, tekst).map_err(|e| format!("Nie mogę zapisać {}: {e}", sciezka.display()))?;
    NAUCZONA.store(true, Ordering::SeqCst);
    wyslij(Wiadomosc::Wzorce(Some(w)));
    Ok(Nauka { zebrane: PROBEK_NAUKI, gotowe: true })
}

/// Zapomina nauczoną frazę.
pub fn usun() {
    if let Some(s) = sciezka() {
        let _ = std::fs::remove_file(s);
    }
    NAUCZONA.store(false, Ordering::SeqCst);
    wyslij(Wiadomosc::Wzorce(None));
}

/// Próbki z miksera Windows → mono -1..1 (średnia kanałów); 16/24/32-bitowe całkowite albo 32-bitowe float.
fn na_mono(kanaly: u16, bity: u16, dane: &[u8]) -> impl Iterator<Item = f32> + '_ {
    let bajty = (bity / 8).max(1) as usize;
    let kanaly = kanaly.max(1) as usize;
    let probka = move |b: &[u8]| -> f32 {
        match bity {
            16 => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
            24 => (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0,
            32 => {
                let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if v.is_finite() { v } else { 0.0 }
            }
            _ => 0.0,
        }
    };
    dane.chunks_exact(bajty * kanaly)
        .map(move |ramka| ramka.chunks_exact(bajty).map(probka).sum::<f32>() / kanaly as f32)
}

fn petla(rx: mpsc::Receiver<Wiadomosc>, mut wzorce: Option<Wzorce>, przy_frazie: impl Fn()) {
    let mut sluchacz = Sluchacz::nowy();
    let mut przelicznik: Option<(u32, Przelicznik)> = None;
    let mut wlaczona = false;
    let mut czulosc = 2;
    let mut nauka: Option<mpsc::Sender<Vec<Ramka>>> = None;
    let mut ostatnie_probki = Instant::now();
    let mut ostatnio_rozpoznana: Option<Instant> = None;
    let mut mono16 = Vec::with_capacity(2048);
    while let Ok(w) = rx.recv() {
        let (czestotliwosc, ile) = match &w {
            Wiadomosc::Probki { czestotliwosc, .. } | Wiadomosc::Cisza { czestotliwosc, .. } => (*czestotliwosc, true),
            _ => (0, false),
        };
        if ile {
            PROBKI_PRZYSZLY.store(true, Ordering::SeqCst);
            // Po przerwie (pauza, restart nagrywania) albo przy innej częstotliwości słuchamy od nowa
            if ostatnie_probki.elapsed() > PRZERWA_STRUMIENIA || przelicznik.as_ref().is_none_or(|(c, _)| *c != czestotliwosc) {
                sluchacz = Sluchacz::nowy();
                przelicznik = Some((czestotliwosc, Przelicznik::nowy(czestotliwosc.max(1))));
            }
            ostatnie_probki = Instant::now();
        }
        mono16.clear();
        match w {
            Wiadomosc::Probki { kanaly, bity, dane, .. } => {
                if let Some((_, p)) = przelicznik.as_mut() {
                    p.dodaj(na_mono(kanaly, bity, &dane), &mut mono16);
                }
            }
            Wiadomosc::Cisza { ramek, .. } => {
                if let Some((_, p)) = przelicznik.as_mut() {
                    p.dodaj(std::iter::repeat_n(0.0, ramek as usize), &mut mono16);
                }
            }
            Wiadomosc::Ustaw { wlaczona: wl, czulosc: c } => {
                wlaczona = wl;
                czulosc = c;
            }
            Wiadomosc::Wzorce(w) => wzorce = w,
            Wiadomosc::Nauka(n) => nauka = n,
        }
        SLUCHA.store((wlaczona && wzorce.is_some()) || nauka.is_some(), Ordering::SeqCst);
        if mono16.is_empty() {
            continue;
        }
        let mut gotowe = Vec::new();
        sluchacz.dodaj(&mono16, |r| gotowe.push(r));
        for ramki in gotowe {
            // Nauka: wypowiedź idzie do okna, nie do rozpoznawania
            if let Some(n) = nauka.take() {
                let _ = n.send(ramki);
                continue;
            }
            let Some(wz) = wzorce.as_ref().filter(|_| wlaczona) else { continue };
            let wynik = rozp::wynik(&wz.wzorce, &ramki);
            let prog = rozp::prog(wz.rozrzut, czulosc);
            if wynik <= prog {
                if ostatnio_rozpoznana.is_some_and(|t| t.elapsed() < PRZERWA_PO_ROZPOZNANIU) {
                    continue;
                }
                ostatnio_rozpoznana = Some(Instant::now());
                crate::dziennik::info(&format!("Komenda głosowa: rozpoznana (wynik {wynik:.2}, próg {prog:.2}) — zapisuję klip"));
                przy_frazie();
            } else if wynik <= prog * 1.4 {
                // Prawie: do dziennika, żeby dało się dobrać czułość
                crate::dziennik::info(&format!("Komenda głosowa: podobne, ale nie ta fraza (wynik {wynik:.2}, próg {prog:.2})"));
            }
        }
    }
}

#[cfg(test)]
mod testy {
    use super::na_mono;

    #[test]
    fn mono_z_roznych_formatow() {
        // 16 bit stereo: (0,5 + 0) / 2
        let d: Vec<u8> = [16_384i16, 0, -32_768, -32_768].iter().flat_map(|x| x.to_le_bytes()).collect();
        let m: Vec<f32> = na_mono(2, 16, &d).collect();
        assert_eq!(m, vec![0.25, -1.0]);
        // 32 bit float mono, NaN = 0
        let d: Vec<u8> = [0.5f32, f32::NAN].iter().flat_map(|x| x.to_le_bytes()).collect();
        assert_eq!(na_mono(1, 32, &d).collect::<Vec<_>>(), vec![0.5, 0.0]);
        // 24 bit: -1
        assert_eq!(na_mono(1, 24, &[0, 0, 0x80]).collect::<Vec<_>>(), vec![-1.0]);
    }
}
