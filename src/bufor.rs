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
    /// Ile pakietów usunięto od początku (numer bezwzględny pierwszego pakietu w kolejce)
    usuniete: u64,
    /// Numery bezwzględne klatek kluczowych — żeby nie przeszukiwać całej kolejki przy każdym pakiecie
    klucze: VecDeque<u64>,
}

pub type WspolnyBufor = Arc<Mutex<Bufor>>;

impl Bufor {
    pub fn nowy(sekundy: u32) -> WspolnyBufor {
        Arc::new(Mutex::new(Bufor::pusty(sekundy)))
    }

    fn pusty(sekundy: u32) -> Bufor {
        Bufor { pakiety: VecDeque::new(), dlugosc: sekundy as i64 * 10_000_000, usuniete: 0, klucze: VecDeque::new() }
    }

    /// Dodaje pakiet i usuwa najstarsze, ale zawsze tak, żeby bufor zaczynał się od klatki
    /// kluczowej i obejmował co najmniej `dlugosc` (czyli klip ma >= N sekund).
    pub fn dodaj(&mut self, pakiet: Pakiet) {
        let granica = pakiet.czas - self.dlugosc;
        if pakiet.klucz {
            self.klucze.push_back(self.usuniete + self.pakiety.len() as u64);
        }
        self.pakiety.push_back(pakiet);
        loop {
            // Klatki kluczowe na początku kolejki i przed nim nie są już potrzebne
            while self.klucze.front().is_some_and(|&k| k <= self.usuniete) {
                self.klucze.pop_front();
            }
            // Następna klatka kluczowa po pierwszym pakiecie
            let Some(&k) = self.klucze.front() else { break };
            let ile = (k - self.usuniete) as usize;
            if self.pakiety[ile].czas <= granica {
                self.pakiety.drain(..ile);
                self.usuniete = k;
            } else {
                break;
            }
        }
    }

    /// Czas ostatniego pakietu (strażnik sprawdza nim, czy nagrywanie nie stoi).
    pub fn ostatni_czas(&self) -> Option<i64> {
        self.pakiety.back().map(|p| p.czas)
    }

    /// Kopia zawartości (tanie klony Arc) do zapisu klipu.
    pub fn migawka(&self) -> Vec<Pakiet> {
        self.pakiety.iter().cloned().collect()
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    /// Pakiet o czasie w milisekundach
    fn p(ms: i64, klucz: bool) -> Pakiet {
        Pakiet { czas: ms * 10_000, klucz, dane: Arc::from(vec![0u8; 1]) }
    }

    #[test]
    fn bufor_zaczyna_sie_od_klatki_kluczowej_i_trzyma_n_sekund() {
        let mut b = Bufor::pusty(2);
        // Klatka kluczowa co 1 s, 10 klatek na sekundę, 6 sekund
        for i in 0..60 {
            b.dodaj(p(i * 100, i % 10 == 0));
        }
        let m = b.migawka();
        assert!(m[0].klucz);
        let dlugosc = m.last().unwrap().czas - m[0].czas;
        // Co najmniej 2 s, ale nie więcej niż 2 s + odstęp klatek kluczowych
        assert!(dlugosc >= 2 * 10_000_000 - 1_000_000, "{dlugosc}");
        assert!(dlugosc <= 3 * 10_000_000);
        assert_eq!(m[0].czas, 30_000_000);
    }

    #[test]
    fn bez_klatek_kluczowych_niczego_nie_usuwa() {
        let mut b = Bufor::pusty(1);
        b.dodaj(p(0, true));
        for i in 1..50 {
            b.dodaj(p(i * 100, false));
        }
        // Jedyna klatka kluczowa jest na początku — bez niej klipu nie da się odtworzyć
        assert_eq!(b.migawka().len(), 50);
        assert_eq!(b.ostatni_czas(), Some(49 * 1_000_000));
    }

    #[test]
    fn dzwiek_kazda_ramka_kluczowa() {
        let mut b = Bufor::pusty(1);
        for i in 0..100 {
            b.dodaj(p(i * 50, true));
        }
        let m = b.migawka();
        assert_eq!(m.last().unwrap().czas - m[0].czas, 10_000_000);
    }
}
