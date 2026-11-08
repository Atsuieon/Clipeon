// Ikona w zasobniku rysowana w kodzie: biały znak nagrywania + kolorowa kropka stanu w rogu.
use tauri::image::Image;

pub const ZIELONY: [u8; 3] = [34, 197, 94];
pub const SZARY: [u8; 3] = [140, 140, 140];
pub const CZERWONY: [u8; 3] = [239, 68, 68];
pub const ZOLTY: [u8; 3] = [234, 179, 8];

/// Pokrycie piksela przez obszar odległy o `d` od krawędzi (wygładzanie krawędzi na 1 px).
fn pokrycie(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Rozmiar małej ikony systemu (16 px przy 100%, 20 przy 125%, 24 przy 150%, 32 przy 200%).
fn rozmiar() -> usize {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};
    (unsafe { GetSystemMetrics(SM_CXSMICON) } as usize).clamp(16, 64)
}

/// Ikona rysowana dokładnie w rozmiarze systemu (bez skalowania przez Windows — ostrzejsza).
pub fn ikona(kropka: [u8; 3]) -> Image<'static> {
    let n = rozmiar();
    // Rysunek jest zaprojektowany w siatce 32 px i skalowany
    let s = n as f32 / 32.0;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (px, py) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
            // Odległości w siatce 32 px przeliczamy na piksele, żeby krawędzie były wygładzone na 1 px
            let pokrycie = |d: f32| pokrycie(d * s);
            // Znak: pierścień i pełne kółko w środku
            let d = ((px - 14.0).powi(2) + (py - 14.0).powi(2)).sqrt();
            // Pierścień grubszy przy małych rozmiarach, żeby nie znikał na pasku zadań
            let grubosc = (1.4f32).max(0.75 / s);
            let znak = (pokrycie((d - 10.5).abs() - grubosc) + pokrycie(d - 4.5)).min(1.0);
            // Kropka stanu z przezroczystą obwódką, żeby odcinała się od znaku
            let dk = ((px - 25.0).powi(2) + (py - 25.0).powi(2)).sqrt();
            let kropka_a = pokrycie(dk - 5.5);
            let wyciecie = pokrycie(dk - 7.5);
            let znak = znak * (1.0 - wyciecie);

            let a = (znak + kropka_a).min(1.0);
            let kolor = |c: u8| {
                if a == 0.0 { 0 } else { ((255.0 * znak + c as f32 * kropka_a) / (znak + kropka_a)).round() as u8 }
            };
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[kolor(kropka[0]), kolor(kropka[1]), kolor(kropka[2]), (a * 255.0) as u8]);
        }
    }
    Image::new_owned(rgba, n as u32, n as u32)
}
