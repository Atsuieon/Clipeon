// Karty graficzne w komputerze (DXGI): producent, pamięć i wersja sterownika.
// Przy kilku kartach (np. grafika w procesorze + karta NVIDIA) enkoder wybieramy z najmocniejszej.
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Producent {
    Amd,
    Nvidia,
    Intel,
    Inny,
}

pub struct Karta {
    pub nazwa: String,
    pub producent: Producent,
    /// Pamięć karty (bajty) — przy kilku kartach mocniejsza ma zwykle więcej
    pub pamiec: u64,
    /// Wersja sterownika z Windows, np. 32.0.15.6094
    pub sterownik: String,
}

impl Karta {
    /// Numer sterownika NVIDIA w znanej postaci (np. 560.94) — z dwóch ostatnich członów wersji Windows.
    pub fn wersja_nvidia(&self) -> Option<f64> {
        if self.producent != Producent::Nvidia {
            return None;
        }
        let czesci: Vec<u32> = self.sterownik.split('.').filter_map(|c| c.parse().ok()).collect();
        let [.., a, b] = czesci[..] else { return None };
        let liczba = (a % 10) * 10000 + b; // 15.6094 -> 56094
        Some(liczba as f64 / 100.0)
    }
}

/// Karty (bez programowego renderera Microsoftu), od najmocniejszej. Liczone raz.
pub fn karty() -> &'static [Karta] {
    static KARTY: OnceLock<Vec<Karta>> = OnceLock::new();
    KARTY.get_or_init(|| {
        let mut l = wykryj();
        l.sort_by(|a, b| b.pamiec.cmp(&a.pamiec));
        for k in &l {
            crate::dziennik::info(&format!(
                "Karta graficzna: {} ({:.1} GB), sterownik {}{}",
                k.nazwa,
                k.pamiec as f64 / (1u64 << 30) as f64,
                k.sterownik,
                k.wersja_nvidia().map(|v| format!(" (NVIDIA {v:.2})")).unwrap_or_default()
            ));
        }
        zapamietaj_sterowniki(&l);
        l
    })
}

fn wykryj() -> Vec<Karta> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIDevice, IDXGIFactory1};
    use windows::core::Interface;
    let mut wynik = Vec::new();
    unsafe {
        let Ok(fabryka) = CreateDXGIFactory1::<IDXGIFactory1>() else { return wynik };
        let mut i = 0;
        while let Ok(adapter) = fabryka.EnumAdapters1(i) {
            i += 1;
            let Ok(opis) = adapter.GetDesc1() else { continue };
            if opis.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 || opis.VendorId == 0x1414 {
                continue;
            }
            let dl = opis.Description.iter().position(|&c| c == 0).unwrap_or(opis.Description.len());
            let sterownik = adapter
                .CheckInterfaceSupport(&IDXGIDevice::IID)
                .map(|v| {
                    let v = v as u64;
                    format!("{}.{}.{}.{}", v >> 48, (v >> 32) & 0xffff, (v >> 16) & 0xffff, v & 0xffff)
                })
                .unwrap_or_else(|_| "nieznany".into());
            wynik.push(Karta {
                nazwa: String::from_utf16_lossy(&opis.Description[..dl]).trim().to_string(),
                producent: match opis.VendorId {
                    0x1002 | 0x1022 => Producent::Amd,
                    0x10de => Producent::Nvidia,
                    0x8086 => Producent::Intel,
                    _ => Producent::Inny,
                },
                pamiec: opis.DedicatedVideoMemory as u64,
                sterownik,
            });
        }
    }
    wynik
}

/// Producenci w kolejności „od najmocniejszej karty”; ci bez karty na końcu (ich enkoderów nie ma
/// sensu próbować pierwszych).
pub fn kolejnosc_producentow() -> Vec<Producent> {
    let mut l: Vec<Producent> = Vec::new();
    for k in karty() {
        if !l.contains(&k.producent) {
            l.push(k.producent);
        }
    }
    for p in [Producent::Amd, Producent::Nvidia, Producent::Intel] {
        if !l.contains(&p) {
            l.push(p);
        }
    }
    l
}

/// Zapisuje wersje sterowników i loguje, gdy zmieniły się od poprzedniego uruchomienia
/// (po aktualizacji sterownika enkodery są i tak sprawdzane od nowa przy każdym starcie).
fn zapamietaj_sterowniki(karty: &[Karta]) {
    let Some(appdata) = std::env::var_os("APPDATA") else { return };
    let plik = std::path::PathBuf::from(appdata).join("Clipeon").join("sterowniki.txt");
    let teraz: String = karty.iter().map(|k| format!("{}={}\n", k.nazwa, k.sterownik)).collect();
    let wczesniej = std::fs::read_to_string(&plik).unwrap_or_default();
    if !wczesniej.is_empty() && wczesniej != teraz {
        crate::dziennik::info(&format!(
            "Sterownik karty graficznej zmienił się od ostatniego uruchomienia (było: {})",
            wczesniej.trim().replace('\n', "; ")
        ));
    }
    if wczesniej != teraz {
        let _ = std::fs::write(&plik, teraz);
    }
}
