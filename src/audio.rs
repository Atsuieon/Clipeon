// Dźwięk: WASAPI (loopback = to, co słychać w głośnikach, albo mikrofon) -> ffmpeg AAC -> ramki ADTS w buforze RAM.
// Czas ramek liczony z zegara QPC, tak samo jak dla obrazu, więc synchronizacja wychodzi sama.
// Od 1.4.5 dźwięk komputera może iść dwiema ścieżkami: sama gra (z procesami potomnymi) i cała reszta
// (Discord, muzyka, przeglądarka) — przez „loopback procesu” z Windows 10 2004.
use crate::Zablokuj;
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_RESOURCES_INVALIDATED,
    AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, ActivateAudioInterfaceAsync, DEVICE_STATE_ACTIVE, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator, PROCESS_LOOPBACK_MODE, PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVE_FORMAT_PCM, WAVEFORMATEX, eCapture, eConsole,
    eRender,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{BLOB, CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree, IAgileObject, IAgileObject_Impl, STGM_READ};
use windows::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Variant::{VT_BLOB, VT_LPWSTR};
use windows::core::{HRESULT, IUnknown, Interface, implement};

/// Próbek na ramkę AAC
const RAMKA_AAC: i64 = 1024;
/// Tyle czasu spokojnie czekamy, aż urządzenie dźwięku wróci po zmianie w systemie (bez błędu, żółta
/// kropka); dopiero potem to jest błąd
const PRZELACZANIE_NAJDLUZEJ: Duration = Duration::from_secs(10);
/// Co tyle próbujemy otworzyć urządzenie w trakcie przełączania
const PROBA_PRZELACZENIA_CO: Duration = Duration::from_millis(250);
/// Format ścieżek „Gra” i „Discord i reszta”. Loopback procesu nie ma formatu miksera — podajemy własny,
/// a Windows sam przelicza (AUTOCONVERTPCM). Ten sam format w każdej sesji = enkoder przeżywa zmianę gry.
const FORMAT_PROCESU: Format = Format { czestotliwosc: 48_000, kanaly: 2, bity: 16 };
/// Tyle nieudanych prób loopbacku procesu z rzędu (nie licząc zmian urządzenia) i Clipeon przestaje
/// go używać do końca uruchomienia: „Gra” nagrywa wtedy cały dźwięk komputera, a „Discord i reszta” ciszę
const PROCES_PROB: u32 = 3;
/// Loopback procesu nie działa na tym komputerze (patrz PROCES_PROB)
static PROCES_ZEPSUTY: AtomicBool = AtomicBool::new(false);
/// Mikrofon (w przeciwieństwie do loopbacku) przysyła pakiety bez przerwy, nawet w ciszy. Gdy przez tyle
/// czasu nie przyśle nic, sterownik „zamarzł” (bywa z USB i Bluetooth, też po uśpieniu) — otwieramy go od nowa.
/// Dawniej Clipeon brał to za ciszę i do końca nagrywał w tej ścieżce same zera.
const MIKROFON_ZAMARZL_PO: Duration = Duration::from_secs(2);
/// Po tylu zamarznięciach z rzędu (bez żadnego pakietu pomiędzy) pokazujemy uwagę i dalej próbujemy po cichu
const ZAMARZNIEC_DO_UWAGI: u32 = 3;
/// Prawdziwy mikrofon zawsze trochę szumi, więc długa cisza cyfrowa (same zera) to zwykle wyciszenie
/// przyciskiem na słuchawkach albo zepsuty sterownik. Próg jest długi, bo mikrofony z bramką szumów
/// (np. NVIDIA Broadcast) dają same zera, gdy nikt nie mówi
const CISZA_CYFROWA_PO: Duration = Duration::from_secs(120);
/// Co tyle sprawdzamy, czy mikrofon nie jest wyciszony w Windows
const WYCISZENIE_CO: Duration = Duration::from_secs(2);
/// Co tyle wpis o mikrofonie w dzienniku (szczyt głośności, pakiety)
const WPIS_MIKROFONU_CO: Duration = Duration::from_secs(60);

/// Skąd nagrywamy dźwięk.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Zrodlo {
    /// Dźwięk systemowy (loopback domyślnego wyjścia)
    System,
    /// Domyślny mikrofon
    Mikrofon,
    /// Sama gra z procesami potomnymi; gdy gry nie widać — cały dźwięk komputera poza Clipeonem
    Gra,
    /// Wszystko poza grą (Discord, muzyka, przeglądarka); gdy gry nie widać — cisza (wszystko jest w „Gra”)
    Reszta,
}

impl Zrodlo {
    fn nazwa(self) -> &'static str {
        match self {
            Zrodlo::System => "dźwięk systemowy",
            Zrodlo::Mikrofon => "mikrofon",
            Zrodlo::Gra => "dźwięk gry",
            Zrodlo::Reszta => "Discord i reszta",
        }
    }

    fn proces(self) -> bool {
        matches!(self, Zrodlo::Gra | Zrodlo::Reszta)
    }
}

/// Numer procesu gry dla ścieżek „Gra” i „Discord i reszta” (0 = gry nie widać). Ustawia go rdzeń;
/// wątki dźwięku same zauważają zmianę i otwierają przechwytywanie od nowa.
pub type CelDzwieku = Arc<AtomicU32>;

/// Czy Windows umie nagrywać dźwięk jednego procesu (Windows 10 2004, kompilacja 19041, i nowsze).
pub fn dzwiek_procesu_dostepny() -> bool {
    static WYNIK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WYNIK.get_or_init(|| {
        windows_registry::LOCAL_MACHINE
            .open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
            .and_then(|k| k.get_string("CurrentBuild"))
            .ok()
            .and_then(|b| b.parse::<u32>().ok())
            .is_some_and(|b| b >= 19041)
    })
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
    /// Mikrofon nagrywa, ale chyba nie to, co trzeba (wyciszony, cisza, zamarzł, zapasowe urządzenie,
    /// blokada w prywatności Windows) — uwaga przy statusie, dymek i dźwięk ostrzeżenia, nie błąd.
    /// `opis` None = problem minął; `krotko` = jedna linijka do dymka
    Uwaga { opis: Option<String>, krotko: String },
}

/// Dlaczego sesja przechwytywania się skończyła bez błędu.
#[derive(Debug, PartialEq)]
enum Koniec {
    /// Nagrywanie się kończy albo trzeba otworzyć urządzenie od nowa (inne domyślne, wybrane wróciło) —
    /// z żółtą kropką
    Zmiana,
    /// Zmieniła się gra: otwieramy przechwytywanie od nowa po cichu (to nie zmiana urządzenia)
    NowaGra,
    /// Mikrofon przestał przysyłać pakiety (patrz MIKROFON_ZAMARZL_PO) — otwieramy go od nowa
    Zamarzl,
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
/// `cel` to proces gry dla źródeł `Gra` i `Reszta` (dla pozostałych bez znaczenia).
pub fn uruchom(
    zrodlo: Zrodlo,
    wybrane: String,
    bitrate_kbps: u32,
    bufor: WspolnyBufor,
    cel: CelDzwieku,
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
        let mut nieudane_procesu = 0;
        // Pilnowanie mikrofonu (cisza, zamarznięcie, zapasowe urządzenie) przeżywa ponowne otwarcia
        let mut czujnik = (zrodlo == Zrodlo::Mikrofon).then(Czujnik::nowy);
        while !stop_watku.load(Ordering::SeqCst) {
            let mut ruszylo = || {
                if matches!(faza, Faza::Przelaczanie(_)) {
                    crate::dziennik::info(&format!("Dźwięk ({}): przełączone, nagrywam dalej", zrodlo.nazwa()));
                }
                faza = Faza::Spokoj;
                zglos(Zgloszenie::Dziala);
            };
            let wynik = match zrodlo {
                // Loopback procesu nie działa na tym komputerze: „Gra” = cały dźwięk, „Reszta” = cisza
                Zrodlo::Gra if PROCES_ZEPSUTY.load(Ordering::SeqCst) => {
                    sesja(Zrodlo::System, "", bitrate_kbps, &bufor, &stop_watku, &mut koder, &mut ruszylo, None)
                }
                Zrodlo::Reszta if PROCES_ZEPSUTY.load(Ordering::SeqCst) => {
                    sesja_ciszy(zrodlo, &cel, None, bitrate_kbps, &bufor, &stop_watku, &mut koder, &mut ruszylo)
                }
                z if z.proces() => sesja_procesu(z, &cel, bitrate_kbps, &bufor, &stop_watku, &mut koder, &mut ruszylo),
                z => {
                    let czujnik = czujnik.as_mut().map(|c| (c, &zglos as &dyn Fn(Zgloszenie)));
                    sesja(z, &wybrane, bitrate_kbps, &bufor, &stop_watku, &mut koder, &mut ruszylo, czujnik)
                }
            };
            if stop_watku.load(Ordering::SeqCst) {
                break;
            }
            let przerwa = match wynik {
                Ok(Koniec::NowaGra) => {
                    nieudane_procesu = 0;
                    Duration::ZERO
                }
                // Sesja sama zauważyła zmianę (inne domyślne urządzenie, wybrane wróciło): otwieramy od razu
                Ok(Koniec::Zmiana) => {
                    faza = Faza::Przelaczanie(Instant::now());
                    zglos(Zgloszenie::Przelaczanie);
                    Duration::ZERO
                }
                // Mikrofon zamarzł: pierwsze razy jak zmiana urządzenia (żółta kropka), potem uwaga przy
                // statusie i dalsze próby po cichu (kropka nie miga co 2 s, dziennik się nie zapycha)
                Ok(Koniec::Zamarzl) => {
                    if let Some(c) = czujnik.as_mut() {
                        c.zamarzniecia += 1;
                        if c.zamarzniecia == 1 {
                            crate::dziennik::info(&format!(
                                "Dźwięk ({}): urządzenie od {} s nic nie przysyła — otwieram je od nowa",
                                zrodlo.nazwa(),
                                MIKROFON_ZAMARZL_PO.as_secs()
                            ));
                        }
                        if c.zamarzniecia < ZAMARZNIEC_DO_UWAGI {
                            faza = Faza::Przelaczanie(Instant::now());
                            zglos(Zgloszenie::Przelaczanie);
                        } else if c.zamarzniecia == ZAMARZNIEC_DO_UWAGI {
                            crate::dziennik::info(&format!(
                                "Dźwięk ({}): po ponownym otwarciu dalej nic — próbuję dalej po cichu",
                                zrodlo.nazwa()
                            ));
                            c.odswiez(&zglos);
                        }
                    }
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
                    } else if zrodlo.proces() && matches!(e, Blad::Urzadzenie(_)) && !PROCES_ZEPSUTY.load(Ordering::SeqCst) {
                        // Loopback procesu nie ruszył: po kilku próbach przechodzimy na stary sposób, bez błędu
                        nieudane_procesu += 1;
                        if nieudane_procesu >= PROCES_PROB {
                            crate::dziennik::info(&format!(
                                "Dźwięk ({}): nagrywanie dźwięku samej gry nie działa na tym komputerze ({e}) — \
                                 cały dźwięk komputera idzie do ścieżki „Gra”",
                                zrodlo.nazwa()
                            ));
                            PROCES_ZEPSUTY.store(true, Ordering::SeqCst);
                        }
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
#[derive(Clone, Copy, PartialEq, Debug)]
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
            match zrodlo {
                Zrodlo::Mikrofon => "mikrofon",
                Zrodlo::Reszta => "Discord i reszta",
                _ => "dźwięk gry",
            },
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

    /// Enkoder z poprzedniej sesji, jeśli ma ten sam format; inaczej nowy.
    fn dla<'a>(koder: &'a mut Option<Koder>, zrodlo: Zrodlo, format: Format, bitrate_kbps: u32, bufor: &WspolnyBufor) -> Result<&'a mut Koder, Blad> {
        Ok(match koder.take().filter(|k| k.format == format) {
            Some(k) => koder.insert(k),
            None => koder.insert(Koder::uruchom(zrodlo, format, bitrate_kbps, bufor)?),
        })
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
/// `czujnik` (tylko mikrofon) pilnuje, czy urządzenie naprawdę nagrywa.
#[allow(clippy::too_many_arguments)]
fn sesja(
    zrodlo: Zrodlo,
    wybrane: &str,
    bitrate_kbps: u32,
    bufor: &WspolnyBufor,
    stop: &AtomicBool,
    koder: &mut Option<Koder>,
    ruszylo: &mut dyn FnMut(),
    mut czujnik: Option<(&mut Czujnik, &dyn Fn(Zgloszenie))>,
) -> Result<Koniec, Blad> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let (kierunek, flagi) = match zrodlo {
            Zrodlo::Mikrofon => (eCapture, 0),
            _ => (eRender, AUDCLNT_STREAMFLAGS_LOOPBACK),
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
        // Mikrofon: z którego urządzenia nagrywamy (do dziennika i do uwagi o zapasowym) i czy Windows
        // nie blokuje programom mikrofonu (wtedy zamiast błędu przychodzi sama cisza)
        if let Some((c, zglos)) = czujnik.as_mut() {
            c.urzadzenie(zrodlo, &urzadzenie, zapasowe);
            c.odswiez(*zglos);
        }
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
        let k = Koder::dla(koder, zrodlo, format, bitrate_kbps, bufor)?;

        // Co ~2 s: czy urządzenie, z którego nagrywamy, dalej jest tym właściwym
        let mut sprawdz = || -> Option<Koniec> {
            if zapasowe {
                // Nagrywamy z domyślnego, bo wybrane było odłączone — wróciło?
                if urzadzenie_po_id(&enumerator, wybrane).is_some() {
                    crate::dziennik::info(&format!("Dźwięk ({}): wybrane urządzenie wróciło, przełączam", zrodlo.nazwa()));
                    return Some(Koniec::Zmiana);
                }
            } else if wybrane.is_empty() {
                let teraz = enumerator.GetDefaultAudioEndpoint(kierunek, eConsole).ok().and_then(|u| id_urzadzenia(&u));
                if teraz.is_some() && teraz != id_start {
                    crate::dziennik::info(&format!("Dźwięk ({}): zmiana domyślnego urządzenia, przełączam", zrodlo.nazwa()));
                    return Some(Koniec::Zmiana);
                }
            }
            None
        };
        let przechwyt: IAudioCaptureClient = klient.GetService()?;
        klient.Start()?;
        ruszylo();
        let wynik =
            przechwytuj(&przechwyt, k, zdarzenie.as_ref().filter(|_| ze_zdarzeniem), stop, Duration::from_secs(2), &mut sprawdz, czujnik);
        let _ = klient.Stop();
        wynik
    }
}

/// Pętla odbioru próbek, wspólna dla wszystkich sesji: pakiety z WASAPI do enkodera, a gdy nic nie
/// przychodzi — cisza. Co `co` woła `sprawdz`, który może zakończyć sesję (zmiana urządzenia albo gry).
/// Przy mikrofonie `czujnik` pilnuje, czy pakiety przychodzą i czy jest w nich coś poza zerami.
fn przechwytuj(
    przechwyt: &IAudioCaptureClient,
    k: &mut Koder,
    zdarzenie: Option<&Zdarzenie>,
    stop: &AtomicBool,
    co: Duration,
    sprawdz: &mut dyn FnMut() -> Option<Koniec>,
    mut czujnik: Option<(&mut Czujnik, &dyn Fn(Zgloszenie))>,
) -> Result<Koniec, Blad> {
    let mut sprawdzone = Instant::now();
    if let Some((c, _)) = czujnik.as_mut() {
        c.ostatni_pakiet = Instant::now();
    }
    while !stop.load(Ordering::SeqCst) {
        match zdarzenie {
            // Czekamy na dane najwyżej 100 ms (przy ciszy loopback nie budzi nas wcale)
            Some(z) => unsafe {
                WaitForSingleObject(z.0, 100);
            },
            None => std::thread::sleep(Duration::from_millis(10)),
        }
        if sprawdzone.elapsed() >= co {
            sprawdzone = Instant::now();
            if let Some(koniec) = sprawdz() {
                return Ok(koniec);
            }
        }
        let mut byly_dane = false;
        while unsafe { przechwyt.GetNextPacketSize()? } > 0 {
            let (mut wsk_danych, mut ramek, mut flagi, mut qpc) = (std::ptr::null_mut(), 0u32, 0u32, 0u64);
            unsafe { przechwyt.GetBuffer(&mut wsk_danych, &mut ramek, &mut flagi, None, Some(&mut qpc))? };
            let rozmiar = ramek as usize * k.zegar.bajtow_na_probke;
            let probki = if flagi & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || wsk_danych.is_null() {
                None
            } else {
                Some(unsafe { std::slice::from_raw_parts(wsk_danych, rozmiar) })
            };
            // Bez wiarygodnego znacznika czasu (loopback procesu bywa bez niego) próbki idą zaraz za poprzednimi
            let czas = (qpc != 0 && flagi & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0).then_some(qpc as i64);
            if let Some((c, _)) = czujnik.as_mut() {
                c.pakiet(k.format.bity, probki);
            }
            let w = k.zegar.pakiet(&mut k.wejscie, czas, ramek as i64, probki);
            unsafe { przechwyt.ReleaseBuffer(ramek)? };
            w?;
            byly_dane = true;
        }
        // Gdy nic nie gra, loopback nie daje pakietów — dopychamy ciszę; przy zdarzeniach co ~100 ms
        if !byly_dane {
            k.cisza()?;
        }
        if let Some((c, zglos)) = czujnik.as_mut() {
            if c.ostatni_pakiet.elapsed() >= MIKROFON_ZAMARZL_PO {
                return Ok(Koniec::Zamarzl);
            }
            c.wpis_do_dziennika();
            c.sprawdz_wyciszenie(false);
            c.odswiez(*zglos);
        }
    }
    Ok(Koniec::Zmiana)
}

/// Pilnuje, czy mikrofon naprawdę nagrywa głos: czy pakiety przychodzą, czy jest w nich coś poza cyfrowymi
/// zerami, z którego urządzenia nagrywamy i czy Windows nie blokuje mikrofonu. Problemy zgłasza jako uwagę
/// przy statusie (`Zgloszenie::Uwaga`), a co minutę zapisuje w dzienniku szczyt głośności. Żyje dłużej niż
/// jedno otwarcie urządzenia, więc ponowne otwarcie nie zeruje liczenia ciszy.
struct Czujnik {
    /// Kiedy przyszedł ostatni pakiet (licznik zaczyna się od nowa z każdym otwarciem urządzenia)
    ostatni_pakiet: Instant,
    /// Kiedy ostatnio w próbkach było coś poza cyfrowymi zerami
    ostatni_dzwiek: Instant,
    /// Zamarznięcia z rzędu (zeruje je pierwszy pakiet)
    zamarzniecia: u32,
    /// Wybrany mikrofon jest odłączony i nagrywamy z domyślnego: jego nazwa
    zapas: Option<String>,
    /// Windows nie pozwala programom na mikrofon (Prywatność → Mikrofon)
    prywatnosc: bool,
    /// Głośność urządzenia w Windows (do sprawdzania wyciszenia) i wynik ostatniego sprawdzenia
    glosnosc: Option<IAudioEndpointVolume>,
    wyciszony: bool,
    sprawdzone_wyciszenie: Instant,
    /// Ostatnio zgłoszona uwaga (zgłaszamy tylko zmiany)
    zgloszona: Option<String>,
    /// Z którego urządzenia nagrywaliśmy ostatnio (wpis w dzienniku tylko przy zmianie)
    ostatnie_urzadzenie: Option<String>,
    /// Do wpisu w dzienniku: od kiedy liczymy, szczyt (0–1), pakiety, pakiety z samymi zerami
    od_wpisu: Instant,
    szczyt: f32,
    pakiety: u64,
    zerowe: u64,
}

impl Czujnik {
    fn nowy() -> Czujnik {
        let teraz = Instant::now();
        Czujnik {
            ostatni_pakiet: teraz,
            ostatni_dzwiek: teraz,
            zamarzniecia: 0,
            zapas: None,
            prywatnosc: false,
            glosnosc: None,
            wyciszony: false,
            sprawdzone_wyciszenie: teraz,
            zgloszona: None,
            ostatnie_urzadzenie: None,
            od_wpisu: teraz,
            szczyt: 0.0,
            pakiety: 0,
            zerowe: 0,
        }
    }

    /// Sesja otworzyła urządzenie (`zapasowe` = zamiast odłączonego wybranego).
    fn urzadzenie(&mut self, zrodlo: Zrodlo, urzadzenie: &IMMDevice, zapasowe: bool) {
        let nazwa = nazwa_urzadzenia(urzadzenie).unwrap_or_else(|| "urządzenie bez nazwy".into());
        if self.ostatnie_urzadzenie.as_deref() != Some(nazwa.as_str()) {
            crate::dziennik::info(&format!("Dźwięk ({}): nagrywam z „{nazwa}”", zrodlo.nazwa()));
            self.ostatnie_urzadzenie = Some(nazwa.clone());
        }
        self.zapas = zapasowe.then_some(nazwa);
        self.glosnosc = unsafe { urzadzenie.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }.ok();
        self.prywatnosc = mikrofon_zablokowany();
        self.sprawdz_wyciszenie(true);
    }

    /// Czy mikrofon jest wyciszony w Windows (albo ma głośność 0) — co WYCISZENIE_CO, `teraz` = od razu.
    /// Część słuchawek wycisza tak mikrofon przyciskiem.
    fn sprawdz_wyciszenie(&mut self, teraz: bool) {
        if !teraz && self.sprawdzone_wyciszenie.elapsed() < WYCISZENIE_CO {
            return;
        }
        self.sprawdzone_wyciszenie = Instant::now();
        self.wyciszony = self.glosnosc.as_ref().is_some_and(|g| unsafe {
            g.GetMute().is_ok_and(|m| m.as_bool()) || g.GetMasterVolumeLevelScalar().is_ok_and(|v| v <= 0.0)
        });
    }

    /// Przyszedł pakiet z mikrofonu (`probki` = None: pakiet oznaczony przez Windows jako cisza).
    fn pakiet(&mut self, bity: u16, probki: Option<&[u8]>) {
        let teraz = Instant::now();
        self.ostatni_pakiet = teraz;
        self.zamarzniecia = 0;
        self.pakiety += 1;
        let szczyt = probki.map_or(0.0, |p| szczyt_probek(bity, p));
        if szczyt > 0.0 {
            self.ostatni_dzwiek = teraz;
            self.szczyt = self.szczyt.max(szczyt);
        } else {
            self.zerowe += 1;
        }
    }

    /// Co pokazać przy statusie (None = mikrofon nagrywa normalnie): pełne wyjaśnienie i krótki napis
    /// do dymka (dymek ma jedną linijkę).
    fn uwaga(&self) -> Option<(String, String)> {
        let mut u: Vec<(String, String)> = Vec::new();
        let mut dodaj = |pelny: &str, krotki: &str| u.push((pelny.to_string(), krotki.to_string()));
        if self.prywatnosc {
            dodaj(
                "Windows nie pozwala programom używać mikrofonu, więc w klipach jest cisza. Włącz: Ustawienia Windows → \
                 Prywatność i zabezpieczenia → Mikrofon → dostęp do mikrofonu oraz „Zezwalaj aplikacjom klasycznym na \
                 dostęp do mikrofonu”.",
                "Windows blokuje mikrofon (Prywatność → Mikrofon)",
            );
        } else if self.zamarzniecia >= ZAMARZNIEC_DO_UWAGI {
            dodaj(
                "Mikrofon przestał przysyłać dźwięk. Clipeon próbuje go otworzyć od nowa — sprawdź, czy jest podłączony \
                 (przy Bluetooth: czy słuchawki są połączone).",
                "Mikrofon przestał przysyłać dźwięk",
            );
        } else if self.wyciszony {
            dodaj(
                "Mikrofon jest wyciszony w Windows (albo ma głośność 0), więc w klipach jest cisza. Włącz go w ustawieniach \
                 dźwięku Windows albo przyciskiem na słuchawkach.",
                "Mikrofon jest wyciszony w Windows",
            );
        } else if self.ostatni_dzwiek.elapsed() >= CISZA_CYFROWA_PO {
            dodaj(
                "Mikrofon od dłuższej chwili nagrywa idealną ciszę. Sprawdź, czy nie jest wyciszony przyciskiem na \
                 słuchawkach i czy w Ustawieniach → Dźwięk wybrany jest właściwy mikrofon. (Mikrofony z bramką szumów, \
                 np. NVIDIA Broadcast, tak mają, gdy nikt nie mówi.)",
                "Mikrofon nagrywa samą ciszę",
            );
        }
        if let Some(n) = &self.zapas {
            u.push((
                format!("Wybrany mikrofon jest odłączony — Clipeon nagrywa z „{n}”. Wróci na wybrany, gdy tylko się pojawi."),
                format!("Wybrany odłączony, nagrywam z „{n}”"),
            ));
        }
        let krotki = u.first()?.1.clone();
        Some((u.into_iter().map(|(p, _)| p).collect::<Vec<_>>().join("\n"), krotki))
    }

    /// Zgłasza uwagę rdzeniowi, gdy się zmieniła (i zapisuje ją w dzienniku).
    fn odswiez(&mut self, zglos: &dyn Fn(Zgloszenie)) {
        let u = self.uwaga();
        let opis = u.as_ref().map(|(p, _)| p.clone());
        if opis != self.zgloszona {
            match &opis {
                Some(t) => crate::dziennik::info(&format!("Mikrofon — uwaga: {t}")),
                None => crate::dziennik::info("Mikrofon: znowu nagrywa normalnie"),
            }
            self.zgloszona = opis.clone();
            zglos(Zgloszenie::Uwaga { opis, krotko: u.map(|(_, k)| k).unwrap_or_default() });
        }
    }

    /// Co WPIS_MIKROFONU_CO: szczyt głośności i liczba pakietów w dzienniku — po tym widać od razu,
    /// czy mikrofon w danej chwili nagrywał głos, ciszę cyfrową, czy nic nie przysyłał.
    fn wpis_do_dziennika(&mut self) {
        let minelo = self.od_wpisu.elapsed();
        if minelo < WPIS_MIKROFONU_CO {
            return;
        }
        let glosnosc = if self.szczyt > 0.0 {
            format!("szczyt {:.0} dB", 20.0 * self.szczyt.log10())
        } else {
            "same zera (cisza cyfrowa)".to_string()
        };
        crate::dziennik::info(&format!(
            "Mikrofon ({} s): {glosnosc}, pakiety {}, w tym z samymi zerami {}",
            minelo.as_secs(),
            self.pakiety,
            self.zerowe
        ));
        self.od_wpisu = Instant::now();
        self.szczyt = 0.0;
        self.pakiety = 0;
        self.zerowe = 0;
    }
}

/// Największa wartość bezwzględna próbki w pakiecie (0–1); 0 = same cyfrowe zera.
fn szczyt_probek(bity: u16, dane: &[u8]) -> f32 {
    match bity {
        32 => dane
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs())
            .filter(|v| v.is_finite())
            .fold(0.0, f32::max),
        16 => dane.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]).unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0,
        24 => {
            dane.chunks_exact(3).map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8).unsigned_abs()).max().unwrap_or(0) as f32
                / 8_388_608.0
        }
        _ => 0.0,
    }
}

/// Czy Windows blokuje programom mikrofon (Ustawienia → Prywatność i zabezpieczenia → Mikrofon). Wtedy
/// nagrywanie startuje bez błędu, ale mikrofon przysyła same zera — bez tej uwagi nie da się tego zauważyć.
fn mikrofon_zablokowany() -> bool {
    const KLUCZ: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
    [
        // Cały komputer, wszyscy użytkownicy („Dostęp do mikrofonu”)
        (HKEY_LOCAL_MACHINE, KLUCZ.to_string()),
        // Ten użytkownik („Zezwalaj aplikacjom na dostęp do mikrofonu”)
        (HKEY_CURRENT_USER, KLUCZ.to_string()),
        // Programy spoza Sklepu, jak Clipeon („Zezwalaj aplikacjom klasycznym…”)
        (HKEY_CURRENT_USER, format!(r"{KLUCZ}\NonPackaged")),
    ]
    .iter()
    .any(|(klucz, sciezka)| tekst_z_rejestru(*klucz, sciezka, "Value").is_some_and(|w| odmowa(&w)))
}

/// Windows zapisuje zgodę jako „Allow” albo „Deny”.
fn odmowa(wartosc: &str) -> bool {
    wartosc.trim().eq_ignore_ascii_case("Deny")
}

/// Krótka wartość tekstowa z rejestru (None, gdy jej nie ma).
fn tekst_z_rejestru(klucz: HKEY, sciezka: &str, nazwa: &str) -> Option<String> {
    let sciezka: Vec<u16> = sciezka.encode_utf16().chain(std::iter::once(0)).collect();
    let nazwa: Vec<u16> = nazwa.encode_utf16().chain(std::iter::once(0)).collect();
    let mut bufor = [0u16; 64];
    let mut rozmiar = std::mem::size_of_val(&bufor) as u32;
    let wynik = unsafe {
        RegGetValueW(
            klucz,
            windows::core::PCWSTR(sciezka.as_ptr()),
            windows::core::PCWSTR(nazwa.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(bufor.as_mut_ptr().cast()),
            Some(&mut rozmiar),
        )
    };
    if wynik != ERROR_SUCCESS {
        return None;
    }
    let dl = (rozmiar as usize / 2).min(bufor.len());
    Some(String::from_utf16_lossy(&bufor[..dl]).trim_end_matches('\0').to_string())
}

/// Sesja ścieżki „Gra” albo „Discord i reszta”: loopback procesu gry (z procesami potomnymi).
/// „Gra” bierze proces gry, a gdy gry nie widać — wszystko poza Clipeonem; „Reszta” bierze wszystko poza
/// grą, a bez gry nagrywa ciszę. Zmiana gry kończy sesję (po cichu) i otwiera nową z nowym procesem.
fn sesja_procesu(
    zrodlo: Zrodlo,
    cel: &CelDzwieku,
    bitrate_kbps: u32,
    bufor: &WspolnyBufor,
    stop: &AtomicBool,
    koder: &mut Option<Koder>,
    ruszylo: &mut dyn FnMut(),
) -> Result<Koniec, Blad> {
    let gra = cel.load(Ordering::SeqCst);
    let (pid, tryb) = match (zrodlo, gra) {
        (Zrodlo::Reszta, 0) => return sesja_ciszy(zrodlo, cel, Some(gra), bitrate_kbps, bufor, stop, koder, ruszylo),
        (Zrodlo::Reszta, p) => (p, PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE),
        (_, 0) => (std::process::id(), PROCESS_LOOPBACK_MODE_EXCLUDE_TARGET_PROCESS_TREE),
        (_, p) => (p, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE),
    };
    unsafe {
        let klient = petla_procesu(pid, tryb)?;
        let wf = FORMAT_PROCESU.waveformat();
        let zdarzenie = CreateEventW(None, false, false, None).ok().map(Zdarzenie);
        let mut flagi = AUDCLNT_STREAMFLAGS_LOOPBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;
        if zdarzenie.is_some() {
            flagi |= AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
        }
        klient.Initialize(AUDCLNT_SHAREMODE_SHARED, flagi, 2_000_000, 0, &wf, None)?;
        let ze_zdarzeniem = matches!(&zdarzenie, Some(z) if klient.SetEventHandle(z.0).is_ok());
        let k = Koder::dla(koder, zrodlo, FORMAT_PROCESU, bitrate_kbps, bufor)?;
        let przechwyt: IAudioCaptureClient = klient.GetService()?;
        klient.Start()?;
        ruszylo();
        let mut sprawdz = || (cel.load(Ordering::SeqCst) != gra).then_some(Koniec::NowaGra);
        let wynik =
            przechwytuj(&przechwyt, k, zdarzenie.as_ref().filter(|_| ze_zdarzeniem), stop, Duration::from_millis(250), &mut sprawdz, None);
        let _ = klient.Stop();
        wynik
    }
}

/// Ścieżka bez źródła (np. „Discord i reszta”, gdy gry nie widać): sama cisza, żeby oś czasu szła równo
/// z obrazem. `gra` = przy jakim procesie gry zaczęliśmy (zmiana kończy sesję); None = do końca.
#[allow(clippy::too_many_arguments)]
fn sesja_ciszy(
    zrodlo: Zrodlo,
    cel: &CelDzwieku,
    gra: Option<u32>,
    bitrate_kbps: u32,
    bufor: &WspolnyBufor,
    stop: &AtomicBool,
    koder: &mut Option<Koder>,
    ruszylo: &mut dyn FnMut(),
) -> Result<Koniec, Blad> {
    let k = Koder::dla(koder, zrodlo, FORMAT_PROCESU, bitrate_kbps, bufor)?;
    ruszylo();
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
        k.cisza()?;
        if gra.is_some_and(|g| cel.load(Ordering::SeqCst) != g) {
            return Ok(Koniec::NowaGra);
        }
    }
    Ok(Koniec::Zmiana)
}

impl Format {
    fn waveformat(self) -> WAVEFORMATEX {
        let blok = (self.kanaly * self.bity as u32 / 8) as u16;
        WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: self.kanaly as u16,
            nSamplesPerSec: self.czestotliwosc,
            nAvgBytesPerSec: self.czestotliwosc * blok as u32,
            nBlockAlign: blok,
            wBitsPerSample: self.bity,
            cbSize: 0,
        }
    }
}

/// Odbiera wynik asynchronicznej aktywacji (ActivateAudioInterfaceAsync woła go z innego wątku).
#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct Aktywacja {
    gotowe: Mutex<Option<std::sync::mpsc::Sender<()>>>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Aktywacja_Impl {
    fn ActivateCompleted(&self, _operacja: windows::core::Ref<IActivateAudioInterfaceAsyncOperation>) -> windows::core::Result<()> {
        if let Some(tx) = self.gotowe.zablokuj().take() {
            let _ = tx.send(());
        }
        Ok(())
    }
}

impl IAgileObject_Impl for Aktywacja_Impl {}

/// Klient WASAPI nagrywający dźwięk procesu `pid` z procesami potomnymi (`tryb` INCLUDE) albo wszystko
/// poza nimi (EXCLUDE). Windows 10 2004 i nowsze.
unsafe fn petla_procesu(pid: u32, tryb: PROCESS_LOOPBACK_MODE) -> windows::core::Result<IAudioClient> {
    unsafe {
        let mut parametry = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS { TargetProcessId: pid, ProcessLoopbackMode: tryb },
            },
        };
        // PROPVARIANT typu BLOB wskazuje na parametry na stosie — nie wolno go „czyścić” (stąd ManuallyDrop)
        let mut wariant = std::mem::ManuallyDrop::new(PROPVARIANT::default());
        {
            let w = &mut *wariant.Anonymous.Anonymous;
            w.vt = VT_BLOB;
            w.Anonymous.blob = BLOB {
                cbSize: std::mem::size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                pBlobData: &mut parametry as *mut _ as *mut u8,
            };
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let obsluga: IActivateAudioInterfaceCompletionHandler = Aktywacja { gotowe: Mutex::new(Some(tx)) }.into();
        let operacja = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&*wariant as *const PROPVARIANT),
            &obsluga,
        )?;
        if rx.recv_timeout(Duration::from_secs(5)).is_err() {
            return Err(windows::core::Error::new(windows::Win32::Foundation::E_FAIL, "Windows nie odpowiedział na włączenie nagrywania dźwięku gry"));
        }
        let mut wynik = HRESULT(0);
        let mut interfejs: Option<IUnknown> = None;
        operacja.GetActivateResult(&mut wynik, &mut interfejs)?;
        wynik.ok()?;
        interfejs.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_NOINTERFACE))?.cast()
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

    fn zera(&mut self, wejscie: &mut impl Write, ile: i64) -> std::io::Result<()> {
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

    fn cisza_do(&mut self, wejscie: &mut impl Write, czas: i64) -> std::io::Result<()> {
        let ile = self.probka_dla(czas) - self.wyslane;
        self.zera(wejscie, ile)
    }

    /// Pakiet z WASAPI: luka przed nim -> cisza, nakładka na już wysłane próbki -> obcinamy początek.
    /// Bez znacznika czasu (`qpc` = None) pakiet idzie zaraz za poprzednimi.
    fn pakiet(&mut self, wejscie: &mut impl Write, qpc: Option<i64>, ramek: i64, probki: Option<&[u8]>) -> std::io::Result<()> {
        let poczatek = qpc.map_or(self.wyslane, |q| self.probka_dla(q));
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
    use super::{Blad, Czujnik, FORMAT_PROCESU, Zegar, dlugosc_adts, odmowa, szczyt_probek, zmiana_urzadzenia};
    use windows::core::{Error, HRESULT};

    /// Zegar 48 kHz, stereo 16 bit (4 bajty na próbkę), start w zerze
    fn zegar() -> Zegar {
        Zegar { t0: 0, czestotliwosc: 48_000, wyslane: 0, bajtow_na_probke: 4 }
    }

    /// Czas QPC (100 ns) dla milisekundy
    const fn ms(ms: i64) -> Option<i64> {
        Some(ms * 10_000)
    }

    #[test]
    fn zegar_wypelnia_luki_cisza_i_obcina_nakladki() {
        let (mut z, mut wyj) = (zegar(), Vec::new());
        // 10 ms dźwięku od zera
        z.pakiet(&mut wyj, ms(0), 480, Some(&[1; 480 * 4])).unwrap();
        assert_eq!((z.wyslane, wyj.len()), (480, 480 * 4));
        // Następny pakiet dopiero w 20 ms: 10 ms ciszy, potem próbki
        z.pakiet(&mut wyj, ms(20), 480, Some(&[2; 480 * 4])).unwrap();
        assert_eq!(z.wyslane, 1440);
        assert!(wyj[480 * 4..960 * 4].iter().all(|b| *b == 0));
        assert!(wyj[960 * 4..].iter().all(|b| *b == 2));
        // Pakiet zachodzący na już wysłane próbki (6 ms wstecz): wysyłamy tylko nową końcówkę
        z.pakiet(&mut wyj, ms(24), 480, Some(&[3; 480 * 4])).unwrap();
        assert_eq!(z.wyslane, 1152 + 480);
        assert_eq!(wyj.len(), 1632 * 4);
        assert!(wyj[1440 * 4..].iter().all(|b| *b == 3));
        // Drganie znacznika poniżej 5 ms nie wstawia ciszy (trzaski) ani nie obcina próbek
        z.pakiet(&mut wyj, ms(35), 48, Some(&[4; 48 * 4])).unwrap();
        assert_eq!(z.wyslane, 1632 + 48);
        // Pakiet ciszy (flaga SILENT) to zera, a cisza_do dopycha oś czasu do zadanej chwili
        z.pakiet(&mut wyj, None, 100, None).unwrap();
        assert_eq!(z.wyslane, 1780);
        z.cisza_do(&mut wyj, 100 * 10_000).unwrap();
        assert_eq!(z.wyslane, 4800);
        assert_eq!(wyj.len(), 4800 * 4);
    }

    #[test]
    fn zegar_bez_znacznika_czasu_dokleja_za_poprzednimi() {
        // Loopback procesu potrafi oddać pakiet bez znacznika czasu — nie może zrobić luki ani nakładki
        let (mut z, mut wyj) = (zegar(), Vec::new());
        for _ in 0..3 {
            z.pakiet(&mut wyj, None, 480, Some(&[7; 480 * 4])).unwrap();
        }
        assert_eq!(z.wyslane, 1440);
        assert!(wyj.iter().all(|b| *b == 7));
    }

    #[test]
    fn format_dzwieku_gry() {
        let f = FORMAT_PROCESU.waveformat();
        assert_eq!((f.nSamplesPerSec, f.nChannels, f.wBitsPerSample), (48_000, 2, 16));
        assert_eq!((f.nBlockAlign, f.nAvgBytesPerSec), (4, 192_000));
    }

    #[test]
    fn ramka_adts() {
        // Nagłówek ADTS z długością ramki 10 bajtów (pole 13-bitowe w bajtach 3–5)
        let mut ramka = vec![0xFF, 0xF1, 0x4C, 0x80, 0x01, 0x40, 0xFC, 9, 9, 9];
        assert_eq!(dlugosc_adts(&ramka), Some(10));
        // Niepełna ramka i brak znacznika synchronizacji
        assert_eq!(dlugosc_adts(&ramka[..8]), None);
        ramka[0] = 0;
        assert_eq!(dlugosc_adts(&ramka), None);
    }

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

    #[test]
    fn szczyt_probek_odroznia_szum_od_cyfrowej_ciszy() {
        // float32: same zera (też -0.0) = cisza cyfrowa, cichy szum mikrofonu już nie
        let zera: Vec<u8> = [0.0f32, -0.0, 0.0, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(szczyt_probek(32, &zera), 0.0);
        let szum: Vec<u8> = [0.0f32, -0.001, 0.0005, 0.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert!((szczyt_probek(32, &szum) - 0.001).abs() < 1e-6);
        // 16 bit: i16::MIN nie może się przepełnić
        let p16: Vec<u8> = [0i16, i16::MIN, 3].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(szczyt_probek(16, &p16), 1.0);
        assert_eq!(szczyt_probek(16, &[0, 0, 0, 0]), 0.0);
        // 24 bit: -1 to FF FF FF
        assert!(szczyt_probek(24, &[0xFF, 0xFF, 0xFF, 0, 0, 0]) > 0.0);
        assert_eq!(szczyt_probek(24, &[0, 0, 0, 0, 0, 0]), 0.0);
    }

    #[test]
    fn zgoda_na_mikrofon_z_rejestru() {
        assert!(odmowa("Deny"));
        assert!(odmowa("deny "));
        assert!(!odmowa("Allow"));
        assert!(!odmowa(""));
    }

    #[test]
    fn uwaga_o_mikrofonie() {
        let mut c = Czujnik::nowy();
        assert_eq!(c.uwaga(), None);
        // Zapasowy mikrofon zamiast odłączonego wybranego
        c.zapas = Some("Mikrofon (Kamera)".into());
        let (opis, krotko) = c.uwaga().unwrap();
        assert!(opis.contains("„Mikrofon (Kamera)”") && krotko.contains("Kamera"));
        // Zamarznięty mikrofon: uwaga dopiero po kilku nieudanych otwarciach, pierwszy pakiet ją zdejmuje.
        // Przy dwóch problemach dymek mówi o ważniejszym, a podpowiedź o obu
        c.zamarzniecia = super::ZAMARZNIEC_DO_UWAGI;
        let (opis, krotko) = c.uwaga().unwrap();
        assert!(opis.contains("przestał przysyłać") && opis.contains("Kamera") && krotko.contains("przestał"));
        c.zapas = None;
        c.pakiet(32, Some(&0.01f32.to_le_bytes()));
        assert_eq!(c.uwaga(), None);
        // Wyciszony w Windows
        c.wyciszony = true;
        assert!(c.uwaga().unwrap().1.contains("wyciszony"));
        // Blokada w prywatności Windows jest ważniejsza niż reszta i mówi, gdzie ją wyłączyć
        c.prywatnosc = true;
        assert!(c.uwaga().unwrap().0.contains("Prywatność"));
    }
}
