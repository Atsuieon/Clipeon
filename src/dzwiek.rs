// Krótkie dźwięki po zapisie klipu i po błędzie: syntezowane w pamięci (bez pliku WAV obok programu)
// i odtwarzane asynchronicznie przez PlaySound.
// „Kropla” (zapis): szybkie „plum” w górę, czysty ton z wybrzmieniem i cichy odprysk wyżej.
use std::sync::OnceLock;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::core::PCWSTR;

const CZESTOTLIWOSC: u32 = 44_100;

/// Jeden ton: częstotliwość płynie wykładniczo od `f0` do `f1` w czasie ~`tg`,
/// obwiednia ma miękki atak i wykładnicze wybrzmienie.
struct Ton {
    /// Początek w sekundach od startu dźwięku
    start: f32,
    f0: f32,
    f1: f32,
    /// Czas przejścia f0 -> f1 (0 = stała wysokość)
    tg: f32,
    /// Długość ataku w sekundach (miękki początek zamiast kliknięcia)
    atak: f32,
    /// Stała wybrzmienia w sekundach
    tau: f32,
    glosnosc: f32,
    /// Domieszka drugiej harmonicznej (0 = czysty sinus)
    druga: f32,
    /// Długość tonu w sekundach
    dlugosc: f32,
}

/// Miksuje tony do próbek (-1..1) o zadanej długości.
fn zmiksuj(tony: &[Ton], dlugosc: f32) -> Vec<f32> {
    let n = (dlugosc * CZESTOTLIWOSC as f32) as usize;
    let mut probki = vec![0.0f32; n];
    for ton in tony {
        let start = (ton.start * CZESTOTLIWOSC as f32) as usize;
        let ile = ((ton.dlugosc * CZESTOTLIWOSC as f32) as usize).min(n.saturating_sub(start));
        let mut faza = 0.0f32;
        for i in 0..ile {
            let t = i as f32 / CZESTOTLIWOSC as f32;
            // Wysokość: wykładnicze zejście z f0 do f1 (tg = 0 -> od razu f1)
            let hz = if ton.tg > 0.0 { ton.f1 + (ton.f0 - ton.f1) * (-t / ton.tg).exp() } else { ton.f1 };
            faza += 2.0 * std::f32::consts::PI * hz / CZESTOTLIWOSC as f32;
            // Atak: połowa okna Hanna (bez trzasku), potem wykładnicze wybrzmienie
            let narastanie = 0.5 - 0.5 * (std::f32::consts::PI * (t / ton.atak).min(1.0)).cos();
            let obwiednia = narastanie * (-(t - ton.atak).max(0.0) / ton.tau).exp();
            probki[start + i] += ton.glosnosc * obwiednia * (faza.sin() + ton.druga * (2.0 * faza).sin());
        }
    }
    probki
}

/// Lekkie echo: kilka krótkich, cichnących odbić — dźwięk brzmi miękko, a nie „w pudełku”.
fn echo(probki: &mut [f32], mokry: f32) {
    let odbicia = [(0.031, 0.50), (0.047, 0.38), (0.071, 0.28), (0.103, 0.20), (0.139, 0.13)];
    let suche = probki.to_vec();
    for (opoznienie, sila) in odbicia {
        let przesuniecie = (opoznienie * CZESTOTLIWOSC as f32) as usize;
        for i in przesuniecie..probki.len() {
            probki[i] += mokry * sila * suche[i - przesuniecie];
        }
    }
}

/// Wyrównuje głośność do zadanego szczytu i wycisza ostatnie 30 ms (żeby nic nie trzasnęło).
fn wykoncz(probki: &mut [f32], szczyt: f32) {
    let max = probki.iter().fold(0.0f32, |a, p| a.max(p.abs()));
    if max > 0.0 {
        let skala = szczyt / max;
        for p in probki.iter_mut() {
            *p *= skala;
        }
    }
    let zanik = (0.03 * CZESTOTLIWOSC as f32) as usize;
    let n = probki.len();
    for (i, p) in probki.iter_mut().enumerate().skip(n.saturating_sub(zanik)) {
        let x = (n - i) as f32 / zanik as f32;
        *p *= x * x;
    }
}

/// Plik WAV (16 bit, mono, 44,1 kHz) z gotowych próbek.
fn wav(probki: &[f32]) -> Vec<u8> {
    let dane_bajty = (probki.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + dane_bajty as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + dane_bajty).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&CZESTOTLIWOSC.to_le_bytes());
    w.extend_from_slice(&(CZESTOTLIWOSC * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&dane_bajty.to_le_bytes());
    for p in probki {
        let v = (p.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        w.extend_from_slice(&v.to_le_bytes());
    }
    w
}

fn zbuduj(tony: &[Ton], dlugosc: f32, mokry: f32, szczyt: f32) -> Vec<u8> {
    let mut probki = zmiksuj(tony, dlugosc);
    echo(&mut probki, mokry);
    wykoncz(&mut probki, szczyt);
    wav(&probki)
}

/// Dźwięk musi żyć, dopóki gra (odtwarzanie asynchroniczne), więc jest statyczny.
fn odtworz(wav: &'static [u8]) {
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

/// Dźwięk zapisu klipu: „Kropla”.
pub fn zagraj() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    odtworz(WAV.get_or_init(|| {
        zbuduj(
            &[
                // Szybkie „plum” w górę
                Ton { start: 0.0, f0: 620.0, f1: 1245.0, tg: 0.030, atak: 0.004, tau: 0.040, glosnosc: 1.0, druga: 0.0, dlugosc: 0.20 },
                // Czysty ton z wybrzmieniem
                Ton { start: 0.030, f0: 0.0, f1: 1244.5, tg: 0.0, atak: 0.020, tau: 0.110, glosnosc: 0.42, druga: 0.10, dlugosc: 0.45 },
                // Cichy odprysk oktawę wyżej
                Ton { start: 0.150, f0: 830.0, f1: 1661.0, tg: 0.024, atak: 0.004, tau: 0.050, glosnosc: 0.22, druga: 0.0, dlugosc: 0.30 },
            ],
            0.50,
            0.16,
            0.40,
        )
    }));
}

/// Dźwięk problemu: ta sama barwa, ale opadająca i niższa.
pub fn zagraj_blad() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    odtworz(WAV.get_or_init(|| {
        zbuduj(
            &[
                Ton { start: 0.0, f0: 740.0, f1: 494.0, tg: 0.040, atak: 0.006, tau: 0.070, glosnosc: 1.0, druga: 0.08, dlugosc: 0.30 },
                Ton { start: 0.130, f0: 494.0, f1: 370.0, tg: 0.040, atak: 0.006, tau: 0.130, glosnosc: 0.70, druga: 0.10, dlugosc: 0.40 },
            ],
            0.55,
            0.14,
            0.40,
        )
    }));
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn wav_ma_naglowek_i_dane() {
        let probki = zmiksuj(
            &[Ton { start: 0.0, f0: 0.0, f1: 440.0, tg: 0.0, atak: 0.005, tau: 0.05, glosnosc: 1.0, druga: 0.0, dlugosc: 0.1 }],
            0.1,
        );
        let plik = wav(&probki);
        assert_eq!(&plik[..4], b"RIFF");
        assert_eq!(&plik[8..12], b"WAVE");
        assert_eq!(plik.len(), 44 + probki.len() * 2);
    }

    #[test]
    fn dzwiek_nie_przestereowuje_i_cichnie_na_koncu() {
        let mut probki = zmiksuj(
            &[Ton { start: 0.0, f0: 620.0, f1: 1245.0, tg: 0.03, atak: 0.004, tau: 0.04, glosnosc: 1.0, druga: 0.0, dlugosc: 0.2 }],
            0.5,
        );
        echo(&mut probki, 0.16);
        wykoncz(&mut probki, 0.40);
        let max = probki.iter().fold(0.0f32, |a, p| a.max(p.abs()));
        assert!((max - 0.40).abs() < 0.001, "szczyt = {max}");
        assert!(probki[probki.len() - 1].abs() < 0.001);
    }
}
