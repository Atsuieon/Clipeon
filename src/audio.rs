// Dźwięk: WASAPI (loopback = to, co słychać w głośnikach, albo mikrofon) -> ffmpeg AAC -> ramki ADTS w buforze RAM.
// Czas ramek liczony z zegara QPC, tak samo jak dla obrazu, więc synchronizacja wychodzi sama.
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use std::io::{Read, Write};
use std::process::{ChildStdin, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    DEVICE_STATE_ACTIVE, IAudioCaptureClient,
    IAudioClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::STGM_READ;
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};

type Blad = Box<dyn std::error::Error + Send + Sync>;

/// Próbek na ramkę AAC
const RAMKA_AAC: i64 = 1024;

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

/// Działające nagrywanie dźwięku; zniszczenie go kończy nagrywanie.
pub struct Audio {
    stop: Arc<AtomicBool>,
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Uruchamia nagrywanie dźwięku w tle. Po błędzie (np. zmiana urządzenia) startuje od nowa;
/// `zglos` dostaje opis błędu albo None, gdy wszystko znów działa.
/// `wybrane` to identyfikator urządzenia wybranego w ustawieniach (pusty = domyślne systemowe).
pub fn uruchom(
    zrodlo: Zrodlo,
    wybrane: String,
    bitrate_kbps: u32,
    bufor: WspolnyBufor,
    zglos: impl Fn(Option<String>) + Send + 'static,
) -> Audio {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_watku = stop.clone();
    std::thread::spawn(move || {
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
            zglos(Some(format!("Dźwięk ({}): CoInitializeEx nie powiodło się: {e}", zrodlo.nazwa())));
            return;
        }
        while !stop_watku.load(Ordering::SeqCst) {
            if let Err(e) = sesja(zrodlo, &wybrane, bitrate_kbps, &bufor, &stop_watku, &zglos)
                && !stop_watku.load(Ordering::SeqCst) {
                    crate::dziennik::blad(&format!("Dźwięk ({}): {e} — ponawiam za 1 s", zrodlo.nazwa()));
                    zglos(Some(format!("Nie można nagrywać ({}): {e}", zrodlo.nazwa())));
                }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    Audio { stop }
}

/// Jedna sesja przechwytywania: na wybranym urządzeniu albo na bieżącym domyślnym.
fn sesja(
    zrodlo: Zrodlo,
    wybrane: &str,
    bitrate_kbps: u32,
    bufor: &WspolnyBufor,
    stop: &AtomicBool,
    zglos: &impl Fn(Option<String>),
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
        let id_urzadzenia = |u: &windows::Win32::Media::Audio::IMMDevice| -> Option<String> {
            let p = u.GetId().ok()?;
            let s = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            s
        };
        let id_start = id_urzadzenia(&urzadzenie);
        let mut klient: IAudioClient = urzadzenie.Activate(CLSCTX_ALL, None)?;

        let wsk = klient.GetMixFormat()?;
        let f = *wsk;
        let (czestotliwosc, kanaly, bity) = (f.nSamplesPerSec, f.nChannels as u32, f.wBitsPerSample);
        // Bufor 200 ms po stronie systemu. Najpierw z powiadamianiem zdarzeniem (wątek śpi, aż przyjdą dane,
        // zamiast budzić się co 10 ms); gdy system tego nie przyjmie — zwykłe odpytywanie
        let zdarzenie = CreateEventW(None, false, false, None).ok();
        let mut wynik = Err(windows::core::Error::empty());
        if zdarzenie.is_some() {
            wynik = klient.Initialize(AUDCLNT_SHAREMODE_SHARED, flagi | AUDCLNT_STREAMFLAGS_EVENTCALLBACK, 2_000_000, 0, wsk, None);
        }
        let mut ze_zdarzeniem = wynik.is_ok();
        if !ze_zdarzeniem {
            // Nieudany Initialize zostawia klienta w nieokreślonym stanie — bierzemy nowego
            klient = urzadzenie.Activate(CLSCTX_ALL, None)?;
            wynik = klient.Initialize(AUDCLNT_SHAREMODE_SHARED, flagi, 2_000_000, 0, wsk, None);
        }
        CoTaskMemFree(Some(wsk as *const _));
        wynik?;
        if let (true, Some(z)) = (ze_zdarzeniem, zdarzenie) {
            ze_zdarzeniem = klient.SetEventHandle(z).is_ok();
        }

        // Format miksera to prawie zawsze float32; 16/24-bit PCM też obsługujemy
        let format_ff = match bity {
            32 => "f32le",
            16 => "s16le",
            24 => "s24le",
            b => return Err(format!("nieobsługiwany format miksera: {b} bitów").into()),
        };
        let bajtow_na_probke = (bity as usize / 8) * kanaly as usize;
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
        .map_err(|e| format!("nie udało się uruchomić ffmpeg: {e}"))?;
        let mut wejscie = ffmpeg.stdin.take().unwrap();
        let mut wyjscie = ffmpeg.stdout.take().unwrap();

        let przechwyt: IAudioCaptureClient = klient.GetService()?;
        let t0 = teraz_100ns();
        klient.Start()?;
        zglos(None);

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
                    let ramka: Box<[u8]> = dane[poz..poz + dl].into();
                    bufor_watku.lock().unwrap_or_else(|e| e.into_inner()).dodaj(Pakiet { czas, klucz: true, dane: ramka.into() });
                    poz += dl;
                }
                dane.drain(..poz);
            }
        });

        let mut zegar = Zegar { t0, czestotliwosc: czestotliwosc as i64, wyslane: 0, bajtow_na_probke };
        let wynik = (|| -> Result<(), Blad> {
            let mut sprawdzone = std::time::Instant::now();
            while !stop.load(Ordering::SeqCst) {
                match (ze_zdarzeniem, zdarzenie) {
                    // Czekamy na dane najwyżej 100 ms (przy ciszy loopback nie budzi nas wcale)
                    (true, Some(z)) => {
                        WaitForSingleObject(z, 100);
                    }
                    _ => std::thread::sleep(Duration::from_millis(10)),
                }
                // Co ~2 s: czy urządzenie, z którego nagrywamy, dalej jest tym właściwym
                if sprawdzone.elapsed() >= Duration::from_secs(2) {
                    sprawdzone = std::time::Instant::now();
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
                    let rozmiar = ramek as usize * bajtow_na_probke;
                    let probki = if flagi & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                        None
                    } else {
                        Some(std::slice::from_raw_parts(wsk_danych, rozmiar))
                    };
                    let w = zegar.pakiet(&mut wejscie, qpc as i64, ramek as i64, probki);
                    przechwyt.ReleaseBuffer(ramek)?;
                    w?;
                    byly_dane = true;
                }
                // Gdy nic nie gra, loopback nie daje pakietów — dopychamy ciszę (z zapasem 50 ms
                // na spóźnione pakiety), żeby oś czasu dźwięku nie stanęła; przy zdarzeniach co ~100 ms
                if !byly_dane {
                    zegar.cisza_do(&mut wejscie, teraz_100ns() - 500_000)?;
                }
            }
            Ok(())
        })();
        let _ = klient.Stop();
        if let Some(z) = zdarzenie {
            let _ = windows::Win32::Foundation::CloseHandle(z);
        }
        drop(wejscie);
        let _ = ffmpeg.kill();
        let _ = czytnik.join();
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
        if ile > 0 {
            wejscie.write_all(&vec![0u8; ile as usize * self.bajtow_na_probke])?;
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
    let dl = ((d[3] as usize & 0x03) << 11) | ((d[4] as usize) << 3) | (d[5] as usize >> 5);
    (dl >= 7 && d.len() >= dl).then_some(dl)
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

/// Podłączone mikrofony. Pusta lista = nic nie znaleziono (albo COM nie wystartował).
pub fn mikrofony() -> Vec<Mikrofon> {
    unsafe {
        // Polecenie z okna leci innym wątkiem niż nagrywanie, więc COM inicjujemy tutaj
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) else {
            return Vec::new();
        };
        let domyslny = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok()
            .and_then(|u| u.GetId().ok())
            .map(|p| {
                let s = p.to_string().unwrap_or_default();
                CoTaskMemFree(Some(p.0 as *const _));
                s
            })
            .unwrap_or_default();
        let Ok(lista) = enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) else { return Vec::new() };
        let ile = lista.GetCount().unwrap_or(0);
        let mut mikrofony = Vec::new();
        for i in 0..ile {
            let Ok(urzadzenie) = lista.Item(i) else { continue };
            let Ok(wsk) = urzadzenie.GetId() else { continue };
            let id = wsk.to_string().unwrap_or_default();
            CoTaskMemFree(Some(wsk.0 as *const _));
            if id.is_empty() {
                continue;
            }
            let nazwa = nazwa_urzadzenia(&urzadzenie).unwrap_or_else(|| "Mikrofon".into());
            mikrofony.push(Mikrofon { domyslny: id == domyslny, id, nazwa });
        }
        mikrofony.sort_by(|a, b| b.domyslny.cmp(&a.domyslny).then_with(|| a.nazwa.cmp(&b.nazwa)));
        mikrofony
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
