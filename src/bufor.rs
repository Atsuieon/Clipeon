// Bufor powtórki: ostatnie N sekund ZAKODOWANYCH pakietów w RAM (nie surowych klatek).
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Czas w jednostkach 100 ns liczony z zegara QPC (ta sama skala co znaczniki WASAPI).
pub fn teraz_100ns() -> i64 {
    use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut licznik, mut czestotliwosc) = (0i64, 0i64);
    unsafe {
        let _ = QueryPerformanceCounter(&mut licznik);
        let _ = QueryPerformanceFrequency(&mut czestotliwosc);
    }
    (licznik as i128 * 10_000_000 / czestotliwosc as i128) as i64
}

#[derive(Clone)]
pub struct Pakiet {
    /// Czas prezentacji (100 ns, QPC)
    pub czas: i64,
    /// Czy od tego pakietu można zacząć dekodowanie (IDR / każda ramka AAC)
    pub klucz: bool,
    /// Dane pakietu (Arc, żeby kopia bufora przy zapisie była tania)
    pub dane: Arc<[u8]>,
}

pub struct Bufor {
    pakiety: VecDeque<Pakiet>,
    /// Długość bufora w 100 ns
    dlugosc: i64,
}

pub type WspolnyBufor = Arc<Mutex<Bufor>>;

impl Bufor {
    pub fn nowy(sekundy: u32) -> WspolnyBufor {
        Arc::new(Mutex::new(Bufor { pakiety: VecDeque::new(), dlugosc: sekundy as i64 * 10_000_000 }))
    }

    /// Dodaje pakiet i usuwa najstarsze, ale zawsze tak, żeby bufor zaczynał się od klatki
    /// kluczowej i obejmował co najmniej `dlugosc` (czyli klip ma >= N sekund).
    pub fn dodaj(&mut self, pakiet: Pakiet) {
        let granica = pakiet.czas - self.dlugosc;
        self.pakiety.push_back(pakiet);
        loop {
            // Następna klatka kluczowa po pierwszym pakiecie
            let Some(k) = self.pakiety.iter().skip(1).position(|p| p.klucz).map(|i| i + 1) else { break };
            if self.pakiety[k].czas <= granica {
                self.pakiety.drain(..k);
            } else {
                break;
            }
        }
    }

    /// Kopia zawartości (tanie klony Arc) do zapisu klipu.
    pub fn migawka(&self) -> Vec<Pakiet> {
        self.pakiety.iter().cloned().collect()
    }
}
