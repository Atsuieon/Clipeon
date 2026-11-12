// Dźwięk: WASAPI (loopback = to, co słychać w głośnikach, albo mikrofon) -> ffmpeg AAC -> ramki ADTS w buforze RAM.
// Czas ramek liczony z zegara QPC, tak samo jak dla obrazu, więc synchronizacja wychodzi sama.
use crate::Zablokuj;
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_RESOURCES_INVALIDATED,
    AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE_ACTIVE, IAudioCaptureClient,
    IAudioClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::STGM_READ;
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};

/// Próbek na ramkę AAC
const RAMKA_AAC: i64 = 1024;
/// Tyle czasu spokojnie czekamy, aż urządzenie dźwięku wróci po zmianie w systemie (bez błędu, żółta
/// kropka); dopiero potem to jest błąd
const PRZELACZANIE_NAJDLUZEJ: Duration = Duration::from_secs(10);
/// Co tyle próbujemy otworzyć urządzenie w trakcie przełączania
const PROBA_PRZELACZENIA_CO: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq)]
pub enum Zrodlo {
    /// Dźwięk systemowy (loopback domyślnego wyjścia)
    System,
    /// Domyślny mikrofon
    Mikrofon,
}

impl Zrodlo {
    fn nazwa(self) -> &'static str {
        match self {
            Zrodlo::System => "dźwięk systemowy",
            Zrodlo::Mikrofon => "mikrofon",
        }
    }
}

/// Co nagrywanie dźwięku zgłasza rdzeniowi.
pub enum Zgloszenie {
    /// Nagrywa (po starcie albo po udanym przełączeniu urządzenia)
    Dziala,
    /// Windows zmienił albo przebudował urządzenie dźwięku (zamknięta gra, inne słuchawki, zmiana trybu
    /// ekranu przy dźwięku przez monitor) — otwieramy je od nowa. To nie błąd: obraz nagrywa się dalej
    Przelaczanie,
    /// Nie da się nagrywać dźwięku (opis dla użytkownika)
    Blad(String),
}

/// Dlaczego sesja przechwytywania się skończyła.
enum Blad {
    /// Błąd WASAPI: urządzenie albo usługa dźwięku Windows
    Urzadzenie(windows::core::Error),
    /// Reszta: enkoder (ffmpeg), nieobsługiwany format
    Inny(String),
}

impl From<windows::core::Error> for Blad {
    fn from(e: windows::core::Error) -> Self {
        Blad::Urzadzenie(e)
    }
}

impl From<std::io::Error> for Blad {
    fn from(e: std::io::Error) -> Self {
        Blad::Inny(format!("enkoder dźwięku (ffmpeg) przestał działać: {e}"))
    }
}

impl std::fmt::Display for Blad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Blad::Urzadzenie(e) if zmiana_urzadzenia(e) => {
                write!(f, "urządzenie dźwięku zostało odłączone albo zmienione ({e})")
            }
            Blad::Urzadzenie(e) => write!(f, "{e}"),
            Blad::Inny(e) => f.write_str(e),
        }
    }
}

/// Błędy, którymi WASAPI mówi „to urządzenie przestało istnieć w tej postaci”: odłączone, zmieniony
/// format, sterownik przebudował wyjście (np. gra przełączyła tryb ekranu, a dźwięk idzie przez monitor),
/// usługa dźwięku się restartuje. Wystarczy otworzyć urządzenie od nowa.
fn zmiana_urzadzenia(e: &windows::core::Error) -> bool {
    [AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_RESOURCES_INVALIDATED, AUDCLNT_E_SERVICE_NOT_RUNNING].contains(&e.code())
}

/// Działające nagrywanie dźwięku; zniszczenie go kończy nagrywanie.
pub struct Audio {
    stop: Arc<AtomicBool>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// W jakim stanie jest wątek nagrywania między sesjami przechwytywania.
enum Faza {
    /// Nagrywa albo dopiero startuje
    Spokoj,
    /// Urządzenie się zmieniło — otwieramy je od nowa (od kiedy)
    Przelaczanie(Instant),
    /// Zgłoszony błąd; ponawiamy co sekundę
    Blad,
}

/// Uruchamia nagrywanie dźwięku w tle. Gdy Windows zmieni urządzenie (zamknięta gra, inne słuchawki),
/// wątek otwiera je od nowa i nagrywa dalej tym samym enkoderem — przerwa w dźwięku jest wypełniana ciszą,
/// więc dźwięk dalej zgadza się z obrazem. `zglos` dostaje `Przelaczanie`, potem `Dziala`; `Blad` dopiero
/// wtedy, gdy urządzenia nie da się otworzyć przez PRZELACZANIE_NAJDLUZEJ albo zawiodło coś innego.
/// `wybrane` to identyfikator urządzenia wybranego w ustawieniach (pusty = domyślne systemowe).
pub fn uruchom(
    zrodlo: Zrodlo,
    wybrane: String,
    bitrate_kbps: u32,
    bufor: WspolnyBufor,
    zglos: impl Fn(Zgloszenie) + Send + 'static,
) -> Audio {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_watku = stop.clone();
    std::thread::spawn(move || {
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
            zglos(Zgloszenie::Blad(format!("Dźwięk ({}): CoInitializeEx nie powiodło się: {e}", zrodlo.nazwa())));
            return;
        }
        // Enkoder żyje dłużej niż jedno otwarcie urządzenia (dopóki format dźwięku się nie zmieni)
        let mut koder: Option<Koder> = None;
        let mut faza = Faza::Spokoj;
        while !stop_watku.load(Ordering::SeqCst) {
            let wynik = sesja(zrodlo, &wybrane, bitrate_kbps, &bufor, &stop_watku, &mut koder, &mut || {
                if matches!(faza, Faza::Przelaczanie(_)) {
                    crate::dziennik::info(&format!("Dźwięk ({}): przełączone, nagrywam dalej", zrodlo.nazwa()));
                }
                faza = Faza::Spokoj;
                zglos(Zgloszenie::Dziala);
            });
            if stop_watku.load(Ordering::SeqCst) {
                break;
            }
            let przerwa = match wynik {
                // Sesja sama zauważyła zmianę (inne domyślne urządzenie, wybrane wróciło): otwieramy od razu
                Ok(()) => {
                    faza = Faza::Przelaczanie(Instant::now());
                    zglos(Zgloszenie::Przelaczanie);
                    Duration::ZERO
                }
                Err(e) => {
                    let spokojnie = match (&faza, &e) {
                        (Faza::Spokoj, Blad::Urzadzenie(w)) => zmiana_urzadzenia(w),
                        // W trakcie przełączania urządzenie może jeszcze chwilę nie dać się otworzyć
                        (Faza::Przelaczanie(od), Blad::Urzadzenie(_)) => od.elapsed() < PRZELACZANIE_NAJDLUZEJ,
                        _ => false,
                    };
                    if spokojnie {
                        if matches!(faza, Faza::Spokoj) {
                            crate::dziennik::info(&format!("Dźwięk ({}): {e} — przełączam", zrodlo.nazwa()));
                            faza = Faza::Przelaczanie(Instant::now());
                            zglos(Zgloszenie::Przelaczanie);
                        }
                        // Oś czasu dźwięku idzie dalej (cisza), żeby po przełączeniu zgadzała się z obrazem
                        if koder.as_mut().is_some_and(|k| k.cisza().is_err()) {
                            koder = None;
                        }
                        PROBA_PRZELACZENIA_CO
                    } else {
                        // Rdzeń zapisuje błąd w dzienniku (raz, dopóki opis się nie zmieni)
                        koder = None;
                        faza = Faza::Blad;
                        zglos(Zgloszenie::Blad(format!("Nie można nagrywać ({}): {e}", zrodlo.nazwa())));
                        Duration::from_secs(1)
                    }
                }
            };
            std::thread::sleep(przerwa);
        }
    });
    Audio { stop }
}

/// Format próbek z miksera Windows.
#[derive(Clone, Copy, PartialEq)]
struct Format {
    czestotliwosc: u32,
    kanaly: u32,
    bity: u16,
}

/// Enkoder AAC (ffmpeg) z zegarem próbek i wątkiem, który odkłada gotowe ramki do bufora.
/// Zniszczenie go zamyka ffmpeg.
struct Koder {
    format: Format,
    ffmpeg: Child,
    wejscie: ChildStdin,
    czytnik: Option<std::thread::JoinHandle<()>>,
    zegar: Zegar,
}

impl Koder {
    fn uruchom(zrodlo: Zrodlo, format: Format, bitrate_kbps: u32, bufor: &WspolnyBufor) -> Result<Koder, Blad> {
        let Format { czestotliwosc, kanaly, bity } = format;
        // Format miksera to prawie zawsze float32; 16/24-bit PCM też obsługujemy
        let format_ff = match bity {
            32 => "f32le",
            16 => "s16le",
            24 => "s24le",
            b => return Err(Blad::Inny(format!("nieobsługiwany format miksera: {b} bitów"))),
        };
        println!("Dźwięk ({}): {czestotliwosc} Hz, {kanaly} kan., {bity} bit -> AAC {bitrate_kbps} kb/s", zrodlo.nazwa());

        let mut ffmpeg = crate::ffmpeg::uruchom_nagrywanie(
            crate::ffmpeg::komenda_nagrywania()
                .args(["-hide_banner", "-loglevel", "error"])
                .args(["-f", format_ff, "-ar", &czestotliwosc.to_string(), "-ac", &kanaly.to_string(), "-i", "-"])
                .args(["-c:a", "aac", "-b:a", &format!("{bitrate_kbps}k"), "-ac", "2", "-f", "adts", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped()),
            if zrodlo == Zrodlo::System { "dźwięk gry" } else { "mikrofon" },
        )
        .map_err(|e| Blad::Inny(format!("nie udało się uruchomić ffmpeg: {e}")))?;
        let wejscie = ffmpeg.stdin.take().unwrap();
        let mut wyjscie = ffmpeg.stdout.take().unwrap();
        let t0 = teraz_100ns();

        // Wątek czytający ramki ADTS. Ramka k zaczyna się w próbce k*1024, ale enkoder AAC
        // dokleja na początku 1024 próbki rozbiegu, więc odejmujemy jedną ramkę.
        let bufor_watku = bufor.clone();
        let czytnik = std::thread::spawn(move || {
            let mut dane = Vec::new();
            let mut kawalek = vec![0u8; 1 << 14];
            let mut k: i64 = 0;
            while let Ok(n @ 1..) = wyjscie.read(&mut kawalek) {
                dane.extend_from_slice(&kawalek[..n]);
                let mut poz = 0;
                loop {
                    // Zgubiona synchronizacja: przeskakujemy do następnego znacznika ramki ADTS
                    // (inaczej dźwięk po cichu by stanął, a bufor rósł bez końca)
                    while dane.len() - poz >= 2 && !(dane[poz] == 0xFF && dane[poz + 1] & 0xF0 == 0xF0) {
                        poz += 1;
                    }
                    let Some(dl) = dlugosc_adts(&dane[poz..]) else {
                        // Znacznik bez sensownej długości ramki: pomijamy bajt i szukamy dalej
                        if dane.len() - poz >= 7 && adts_dlugosc_pola(&dane[poz..]) < 7 {
                            poz += 1;
                            continue;
                        }
                        break;
                    };
                    let czas = t0 + (k - 1) * RAMKA_AAC * 10_000_000 / czestotliwosc as i64;
                    k += 1;
                    let ramka = Arc::from(&dane[poz..poz + dl]);
                    bufor_watku.zablokuj().dodaj(Pakiet { czas, klucz: true, dane: ramka });
                    poz += dl;
                }
                dane.drain(..poz);
            }
        });

        let bajtow_na_probke = (bity as usize / 8) * kanaly as usize;
        let zegar = Zegar { t0, czestotliwosc: czestotliwosc as i64, wyslane: 0, bajtow_na_probke };
        Ok(Koder { format, ffmpeg, wejscie, czytnik: Some(czytnik), zegar })
    }

    /// Gdy nic nie przychodzi (cisza w głośnikach albo przełączanie urządzenia), dopycha ciszę do
    /// „teraz − 50 ms” (zapas na spóźnione pakiety), żeby oś czasu dźwięku nie stanęła.
    fn cisza(&mut self) -> std::io::Result<()> {
        self.zegar.cisza_do(&mut self.wejscie, teraz_100ns() - 500_000)
    }
}

impl Drop for Koder {
    fn drop(&mut self) {
        let _ = self.ffmpeg.kill();
        let _ = self.ffmpeg.wait();
        if let Some(c) = self.czytnik.take() {
            let _ = c.join();
        }
    }
}

/// Uchwyt zdarzenia Windows zamykany razem ze zmienną.
struct Zdarzenie(HANDLE);

impl Drop for Zdarzenie {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Jedna sesja przechwytywania: na wybranym urządzeniu albo na bieżącym domyślnym. Próbki idą do
/// `koder` — zostaje ten z poprzedniej sesji, jeśli nowe urządzenie ma ten sam format (wtedy przerwę
/// między sesjami zegar wypełnia ciszą). `ruszylo` jest wołane, gdy przechwytywanie wystartowało.
/// Ok = trzeba otworzyć urządzenie od nowa (zmieniło się domyślne, wybrane wróciło) albo koniec nagrywania.
fn sesja(
    zrodlo: Zrodlo,
    wybrane: &str,
    bitrate_kbps: u32,
    bufor: &WspolnyBufor,
    stop: &AtomicBool,
    koder: &mut Option<Koder>,
    ruszylo: &mut dyn FnMut(),
) -> Result<(), Blad> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let (kierunek, flagi) = match zrodlo {
            Zrodlo::System => (eRender, AUDCLNT_STREAMFLAGS_LOOPBACK),
            Zrodlo::Mikrofon => (eCapture, 0),
        };
        // Wybrane urządzenie z ustawień; gdy jest odłączone, bierzemy domyślne i pilnujemy,
        // czy wybrane nie wróciło (wtedy sesja startuje od nowa na nim)
        let wybrane_urzadzenie = (!wybrane.is_empty())
            .then(|| urzadzenie_po_id(&enumerator, wybrane))
            .flatten();
        let zapasowe = !wybrane.is_empty() && wybrane_urzadzenie.is_none();
        if zapasowe {
            crate::dziennik::info(&format!(
                "Dźwięk ({}): wybrane urządzenie jest niedostępne — nagrywam z domyślnego",
                zrodlo.nazwa()
            ));
        }
        let urzadzenie = match wybrane_urzadzenie {
            Some(u) => u,
            None => enumerator.GetDefaultAudioEndpoint(kierunek, eConsole)?,
        };
        // Identyfikator urządzenia: gdy użytkownik zmieni domyślne (np. na słuchawki), zaczynamy od nowa
        let id_start = id_urzadzenia(&urzadzenie);
        let mut klient: IAudioClient = urzadzenie.Activate(CLSCTX_ALL, None)?;

        let wsk = klient.GetMixFormat()?;
        let f = *wsk;
        let format = Format { czestotliwosc: f.nSamplesPerSec, kanaly: f.nChannels as u32, bity: f.wBitsPerSample };
        // Bufor 200 ms po stronie systemu. Najpierw z powiadamianiem zdarzeniem (wątek śpi, aż przyjdą dane,
        // zamiast budzić się co 10 ms); gdy system tego nie przyjmie — zwykłe odpytywanie
        let zdarzenie = CreateEventW(None, false, false, None).ok().map(Zdarzenie);
        let mut wynik = Err(windows::core::Error::empty());
        if zdarzenie.is_some() {
            wynik = klient.Initialize(AUDCLNT_SHAREMODE_SHARED, flagi | AUDCLNT_STREAMFLAGS_EVENTCALLBACK, 2_000_000, 0, wsk, None);
        }
        let mut ze_zdarzeniem = wynik.is_ok();
        if !ze_zdarzeniem {
            // Nieudany Initialize zostawia klienta w nieokreślonym stanie — bierzemy nowego
            match urzadzenie.Activate(CLSCTX_ALL, None) {
                Ok(k) => {
                    klient = k;
                    wynik = klient.Initialize(AUDCLNT_SHAREMODE_SHARED, flagi, 2_000_000, 0, wsk, None);
                }
                Err(e) => wynik = Err(e),
            }
        }
        CoTaskMemFree(Some(wsk as *const _));
        wynik?;
        if let (true, Some(z)) = (ze_zdarzeniem, &zdarzenie) {
            ze_zdarzeniem = klient.SetEventHandle(z.0).is_ok();
        }

        // Enkoder z poprzedniej sesji zostaje, jeśli format jest ten sam; inaczej startuje nowy
        let k = match koder.take().filter(|k| k.format == format) {
            Some(k) => koder.insert(k),
            None => koder.insert(Koder::uruchom(zrodlo, format, bitrate_kbps, bufor)?),
        };

        let przechwyt: IAudioCaptureClient = klient.GetService()?;
        klient.Start()?;
        ruszylo();

        let wynik = (|| -> Result<(), Blad> {
            let mut sprawdzone = Instant::now();
            while !stop.load(Ordering::SeqCst) {
                match (ze_zdarzeniem, &zdarzenie) {
                    // Czekamy na dane najwyżej 100 ms (przy ciszy loopback nie budzi nas wcale)
                    (true, Some(z)) => {
                        WaitForSingleObject(z.0, 100);
                    }
                    _ => std::thread::sleep(Duration::from_millis(10)),
                }
                // Co ~2 s: czy urządzenie, z którego nagrywamy, dalej jest tym właściwym
                if sprawdzone.elapsed() >= Duration::from_secs(2) {
                    sprawdzone = Instant::now();
                    if zapasowe {
                        // Nagrywamy z domyślnego, bo wybrane było odłączone — wróciło?
                        if urzadzenie_po_id(&enumerator, wybrane).is_some() {
                            crate::dziennik::info(&format!(
                                "Dźwięk ({}): wybrane urządzenie wróciło, przełączam",
                                zrodlo.nazwa()
                            ));
                            return Ok(());
                        }
                    } else if wybrane.is_empty() {
                        let teraz = enumerator.GetDefaultAudioEndpoint(kierunek, eConsole).ok().and_then(|u| id_urzadzenia(&u));
                        if teraz.is_some() && teraz != id_start {
                            crate::dziennik::info(&format!("Dźwięk ({}): zmiana domyślnego urządzenia, przełączam", zrodlo.nazwa()));
                            return Ok(());
                        }
                    }
                }
                let mut byly_dane = false;
                while przechwyt.GetNextPacketSize()? > 0 {
                    let (mut wsk_danych, mut ramek, mut flagi, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
                    przechwyt.GetBuffer(&mut wsk_danych, &mut ramek, &mut flagi, None, Some(&mut qpc))?;
                    let rozmiar = ramek as usize * k.zegar.bajtow_na_probke;
                    let probki = if flagi & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                        None
                    } else {
                        Some(std::slice::from_raw_parts(wsk_danych, rozmiar))
                    };
                    let w = k.zegar.pakiet(&mut k.wejscie, qpc as i64, ramek as i64, probki);
                    przechwyt.ReleaseBuffer(ramek)?;
                    w?;
                    byly_dane = true;
                }
                // Gdy nic nie gra, loopback nie daje pakietów — dopychamy ciszę; przy zdarzeniach co ~100 ms
                if !byly_dane {
                    k.cisza()?;
                }
            }
            Ok(())
        })();
        let _ = klient.Stop();
        wynik
    }
}

/// Pilnuje, żeby liczba próbek wysłanych do enkodera zgadzała się z upływem czasu (QPC).
struct Zegar {
    t0: i64,
    czestotliwosc: i64,
    /// Ile próbek (na kanał) już poszło do ffmpeg
    wyslane: i64,
    bajtow_na_probke: usize,
}

impl Zegar {
    fn probka_dla(&self, czas: i64) -> i64 {
        (czas - self.t0) * self.czestotliwosc / 10_000_000
    }

    fn zera(&mut self, wejscie: &mut ChildStdin, ile: i64) -> std::io::Result<()> {
        // Cisza idzie co ~100 ms — kawałkami ze stałego bufora zamiast nowej tablicy za każdym razem
        static ZERA: [u8; 16 * 1024] = [0; 16 * 1024];
        if ile > 0 {
            let mut bajty = ile as usize * self.bajtow_na_probke;
            while bajty > 0 {
                let n = bajty.min(ZERA.len());
                wejscie.write_all(&ZERA[..n])?;
                bajty -= n;
            }
            self.wyslane += ile;
        }
        Ok(())
    }

    fn cisza_do(&mut self, wejscie: &mut ChildStdin, czas: i64) -> std::io::Result<()> {
        let ile = self.probka_dla(czas) - self.wyslane;
        self.zera(wejscie, ile)
    }

    /// Pakiet z WASAPI: luka przed nim -> cisza, nakładka na już wysłane próbki -> obcinamy początek.
    fn pakiet(&mut self, wejscie: &mut ChildStdin, qpc: i64, ramek: i64, probki: Option<&[u8]>) -> std::io::Result<()> {
        let poczatek = self.probka_dla(qpc);
        // Drobne drgania znaczników (< 5 ms) ignorujemy, żeby nie wstawiać trzasków
        let tolerancja = self.czestotliwosc / 200;
        let mut pomin = 0;
        if poczatek > self.wyslane + tolerancja {
            self.zera(wejscie, poczatek - self.wyslane)?;
        } else if poczatek + tolerancja < self.wyslane {
            pomin = (self.wyslane - poczatek).min(ramek);
        }
        let reszta = ramek - pomin;
        match probki {
            Some(p) => {
                wejscie.write_all(&p[pomin as usize * self.bajtow_na_probke..])?;
                self.wyslane += reszta;
            }
            None => self.zera(wejscie, reszta)?,
        }
        Ok(())
    }
}

/// Pole długości z nagłówka ADTS (bez sprawdzania znacznika).
fn adts_dlugosc_pola(d: &[u8]) -> usize {
    ((d[3] as usize & 0x03) << 11) | ((d[4] as usize) << 3) | (d[5] as usize >> 5)
}

/// Długość pełnej ramki ADTS na początku danych (None, jeśli jeszcze niekompletna).
fn dlugosc_adts(d: &[u8]) -> Option<usize> {
    if d.len() < 7 {
        return None;
    }
    if d[0] != 0xFF || d[1] & 0xF0 != 0xF0 {
        return None;
    }
    let dl = adts_dlugosc_pola(d);
    (dl >= 7 && d.len() >= dl).then_some(dl)
}

/// Stały identyfikator urządzenia audio (zapisywany w ustawieniach, porównywany przy zmianie domyślnego).
fn id_urzadzenia(urzadzenie: &IMMDevice) -> Option<String> {
    unsafe {
        let p = urzadzenie.GetId().ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s
    }
}

/// Urządzenie o danym identyfikatorze, jeśli jest podłączone i włączone.
fn urzadzenie_po_id(enumerator: &IMMDeviceEnumerator, id: &str) -> Option<IMMDevice> {
    let szeroki: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let urzadzenie = unsafe { enumerator.GetDevice(windows::core::PCWSTR(szeroki.as_ptr())) }.ok()?;
    let stan = unsafe { urzadzenie.GetState() }.ok()?;
    (stan == DEVICE_STATE_ACTIVE).then_some(urzadzenie)
}

/// Mikrofon do pokazania w ustawieniach.
#[derive(serde::Serialize)]
pub struct Mikrofon {
    /// Stały identyfikator urządzenia (zapisywany w ustawieniach)
    pub id: String,
    /// Nazwa widoczna w Windows, np. „Mikrofon (HyperX QuadCast)”
    pub nazwa: String,
    /// Czy to urządzenie jest teraz domyślne w systemie
    pub domyslny: bool,
}

/// Podłączone mikrofony (domyślny pierwszy). Liczbę i błędy zapisujemy w dzienniku, gdy się zmienią.
pub fn mikrofony() -> Vec<Mikrofon> {
    static OSTATNIO: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    let (lista, opis) = match lista_mikrofonow() {
        Ok(l) => {
            let opis = format!("Mikrofony w systemie: {}", l.len());
            (l, opis)
        }
        Err(e) => (Vec::new(), format!("Nie udało się odczytać listy mikrofonów: {e}")),
    };
    let mut ostatnio = OSTATNIO.zablokuj();
    if ostatnio.as_deref() != Some(opis.as_str()) {
        crate::dziennik::info(&opis);
        *ostatnio = Some(opis);
    }
    lista
}

fn lista_mikrofonow() -> windows::core::Result<Vec<Mikrofon>> {
    unsafe {
        // Polecenie z okna leci innym wątkiem niż nagrywanie, więc COM inicjujemy tutaj
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let domyslny = enumerator.GetDefaultAudioEndpoint(eCapture, eConsole).ok().and_then(|u| id_urzadzenia(&u)).unwrap_or_default();
        let lista = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)?;
        let ile = lista.GetCount()?;
        let mut mikrofony = Vec::new();
        for i in 0..ile {
            let Some((urzadzenie, id)) = lista.Item(i).ok().and_then(|u| id_urzadzenia(&u).map(|id| (u, id))) else { continue };
            if id.is_empty() {
                continue;
            }
            let nazwa = nazwa_urzadzenia(&urzadzenie).unwrap_or_else(|| "Mikrofon".into());
            mikrofony.push(Mikrofon { domyslny: id == domyslny, id, nazwa });
        }
        mikrofony.sort_by(|a, b| b.domyslny.cmp(&a.domyslny).then_with(|| a.nazwa.cmp(&b.nazwa)));
        Ok(mikrofony)
    }
}

/// Nazwa urządzenia widoczna w Windows (PKEY_Device_FriendlyName).
fn nazwa_urzadzenia(urzadzenie: &IMMDevice) -> Option<String> {
    unsafe {
        let magazyn = urzadzenie.OpenPropertyStore(STGM_READ).ok()?;
        let wartosc = magazyn.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let pole = &wartosc.Anonymous.Anonymous;
        (pole.vt == VT_LPWSTR).then(|| pole.Anonymous.pwszVal.to_string().unwrap_or_default()).filter(|n| !n.is_empty())
    }
}

#[cfg(test)]
mod testy {
    use super::{Blad, zmiana_urzadzenia};
    use windows::core::{Error, HRESULT};

    fn blad(kod: u32) -> Error {
        Error::from_hresult(HRESULT(kod as i32))
    }

    #[test]
    fn zamkniecie_gry_to_zmiana_urzadzenia_a_nie_blad() {
        // AUDCLNT_E_DEVICE_INVALIDATED — to zgłasza WASAPI, gdy Windows przebuduje wyjście dźwięku
        assert!(zmiana_urzadzenia(&blad(0x8889_0004)));
        // AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_E_RESOURCES_INVALIDATED
        assert!(zmiana_urzadzenia(&blad(0x8889_0010)));
        assert!(zmiana_urzadzenia(&blad(0x8889_0026)));
        // Brak dostępu (E_ACCESSDENIED) i zajęte urządzenie (AUDCLNT_E_DEVICE_IN_USE) to prawdziwe błędy
        assert!(!zmiana_urzadzenia(&blad(0x8007_0005)));
        assert!(!zmiana_urzadzenia(&blad(0x8889_000A)));
    }

    #[test]
    fn opis_zmiany_urzadzenia_jest_po_ludzku() {
        let opis = Blad::Urzadzenie(blad(0x8889_0004)).to_string();
        assert!(opis.starts_with("urządzenie dźwięku zostało odłączone albo zmienione"), "{opis}");
        assert!(opis.contains("0x88890004"), "{opis}");
    }
}
