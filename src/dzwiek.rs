// Krótki dźwięk po zapisie klipu: dwa wysokie tony z wybrzmieniem, syntezowane w pamięci
// (bez pliku WAV obok programu) i odtwarzane asynchronicznie przez PlaySound.
use std::sync::OnceLock;
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::core::PCWSTR;

/// Plik WAV (16 bit, mono, 44,1 kHz) z tonami (początek w s, wysokość w Hz).
fn wav(tony: &[(f32, f32)]) -> Vec<u8> {
    {
        const CZESTOTLIWOSC: u32 = 44_100;
        let tony = tony.to_vec();
        let dlugosc = 0.42;
        let n = (dlugosc * CZESTOTLIWOSC as f32) as usize;
        let mut probki = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / CZESTOTLIWOSC as f32;
            let mut v = 0.0;
            for &(start, hz) in &tony {
                if t >= start {
                    let tt = t - start;
                    // Szybki atak (5 ms), wykładnicze wybrzmienie
                    let obwiednia = (tt / 0.005).min(1.0) * (-tt * 11.0).exp();
                    v += (2.0 * std::f32::consts::PI * hz * tt).sin() * obwiednia;
                }
            }
            probki.push((v * 0.22 * i16::MAX as f32) as i16);
        }
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
            w.extend_from_slice(&p.to_le_bytes());
        }
        w
    }
}

/// Dźwięk musi żyć, dopóki gra (odtwarzanie asynchroniczne), więc jest statyczny.
fn odtworz(wav: &'static [u8]) {
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

/// Dźwięk zapisu: dwa wznoszące się tony.
pub fn zagraj() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    odtworz(WAV.get_or_init(|| wav(&[(0.0, 1046.5), (0.075, 1568.0)])));
}

/// Dźwięk problemu: dwa opadające, niższe tony.
pub fn zagraj_blad() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    odtworz(WAV.get_or_init(|| wav(&[(0.0, 659.3), (0.11, 440.0)])));
}
