// Dźwięk: WASAPI loopback (to, co słychać w głośnikach) -> ffmpeg AAC -> ramki ADTS w buforze RAM.
// Czas ramek liczony z zegara QPC, tak samo jak dla obrazu, więc synchronizacja wychodzi sama.
use crate::bufor::{Pakiet, WspolnyBufor, teraz_100ns};
use std::io::{Read, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::time::Duration;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient,
    IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};

type Blad = Box<dyn std::error::Error + Send + Sync>;

/// Próbek na ramkę AAC
const RAMKA_AAC: i64 = 1024;

/// Uruchamia nagrywanie dźwięku w tle. Po błędzie (np. zmiana urządzenia wyjściowego) startuje od nowa.
pub fn uruchom(bitrate_kbps: u32, bufor: WspolnyBufor) {
    std::thread::spawn(move || {
        if let Err(e) = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok() {
            eprintln!("Dźwięk: CoInitializeEx nie powiodło się: {e}");
            return;
        }
        loop {
            if let Err(e) = sesja(bitrate_kbps, &bufor) {
                eprintln!("Dźwięk: {e} — ponawiam za 1 s");
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

/// Jedna sesja przechwytywania na bieżącym domyślnym urządzeniu wyjściowym.
fn sesja(bitrate_kbps: u32, bufor: &WspolnyBufor) -> Result<(), Blad> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let urzadzenie = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let klient: IAudioClient = urzadzenie.Activate(CLSCTX_ALL, None)?;

        let wsk = klient.GetMixFormat()?;
        let f = *wsk;
        let (czestotliwosc, kanaly, bity) = (f.nSamplesPerSec, f.nChannels as u32, f.wBitsPerSample);
        // Bufor 200 ms po stronie systemu
        let wynik = klient.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 2_000_000, 0, wsk, None);
        CoTaskMemFree(Some(wsk as *const _));
        wynik?;

        // Format miksera to prawie zawsze float32; 16/24-bit PCM też obsługujemy
        let format_ff = match bity {
            32 => "f32le",
            16 => "s16le",
            24 => "s24le",
            b => return Err(format!("nieobsługiwany format miksera: {b} bitów").into()),
        };
        let bajtow_na_probke = (bity as usize / 8) * kanaly as usize;
        println!("Dźwięk: loopback {czestotliwosc} Hz, {kanaly} kan., {bity} bit -> AAC {bitrate_kbps} kb/s");

        let mut ffmpeg = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-f", format_ff, "-ar", &czestotliwosc.to_string(), "-ac", &kanaly.to_string(), "-i", "-"])
            .args(["-c:a", "aac", "-b:a", &format!("{bitrate_kbps}k"), "-ac", "2", "-f", "adts", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("nie udało się uruchomić ffmpeg: {e}"))?;
        let mut wejscie = ffmpeg.stdin.take().unwrap();
        let mut wyjscie = ffmpeg.stdout.take().unwrap();

        let przechwyt: IAudioCaptureClient = klient.GetService()?;
        let t0 = teraz_100ns();
        klient.Start()?;

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
                while let Some(dl) = dlugosc_adts(&dane[poz..]) {
                    let czas = t0 + (k - 1) * RAMKA_AAC * 10_000_000 / czestotliwosc as i64;
                    k += 1;
                    let ramka: Box<[u8]> = dane[poz..poz + dl].into();
                    bufor_watku.lock().unwrap().dodaj(Pakiet { czas, klucz: true, dane: ramka.into() });
                    poz += dl;
                }
                dane.drain(..poz);
            }
        });

        let mut zegar = Zegar { t0, czestotliwosc: czestotliwosc as i64, wyslane: 0, bajtow_na_probke };
        let wynik = (|| -> Result<(), Blad> {
            loop {
                std::thread::sleep(Duration::from_millis(10));
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
                // na spóźnione pakiety), żeby oś czasu dźwięku nie stanęła
                if !byly_dane {
                    zegar.cisza_do(&mut wejscie, teraz_100ns() - 500_000)?;
                }
            }
        })();
        let _ = klient.Stop();
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

/// Długość pełnej ramki ADTS na początku danych (None, jeśli jeszcze niekompletna).
fn dlugosc_adts(d: &[u8]) -> Option<usize> {
    if d.len() < 7 {
        return None;
    }
    if d[0] != 0xFF || d[1] & 0xF0 != 0xF0 {
        // Nie powinno się zdarzyć przy wyjściu z ffmpeg; nie wiemy, jak się zsynchronizować
        return None;
    }
    let dl = ((d[3] as usize & 0x03) << 11) | ((d[4] as usize) << 3) | (d[5] as usize >> 5);
    (dl >= 7 && d.len() >= dl).then_some(dl)
}
