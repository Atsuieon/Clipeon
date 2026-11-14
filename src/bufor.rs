// Bufor powtórki: ostatnie N sekund ZAKODOWANYCH pakietów w RAM (nie surowych klatek).
// Do bufora można podłączyć odbiornik (długie nagrywanie, film.rs): dostaje kopię każdego nowego pakietu.
use std::collections::VecDeque;
use std::sync::mpsc::Sender;
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
    /// Odbiornik długiego nagrywania: dostaje każdy nowy pakiet (tania kopia Arc)
    odbiornik: Option<Sender<Pakiet>>,
}

pub type WspolnyBufor = Arc<Mutex<Bufor>>;

impl Bufor {
    pub fn nowy(sekundy: u32) -> WspolnyBufor {
        Arc::new(Mutex::new(Bufor::pusty(sekundy)))
    }

    fn pusty(sekundy: u32) -> Bufor {
        Bufor {
            pakiety: VecDeque::new(),
            dlugosc: sekundy as i64 * 10_000_000,
            usuniete: 0,
            klucze: VecDeque::new(),
            odbiornik: None,
        }
    }

    /// Dodaje pakiet i usuwa najstarsze, ale zawsze tak, żeby bufor zaczynał się od klatki
    /// kluczowej i obejmował co najmniej `dlugosc` (czyli klip ma >= N sekund).
    pub fn dodaj(&mut self, pakiet: Pakiet) {
        // Zamknięty odbiornik (koniec filmu) odłączamy po cichu
        if self.odbiornik.as_ref().is_some_and(|o| o.send(pakiet.clone()).is_err()) {
            self.odbiornik = None;
        }
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

    /// Podłącza odbiornik obrazu: najpierw dostaje to, co już jest w buforze od ostatniej klatki kluczowej
    /// (film zaczyna się od razu, a nie od następnej klatki kluczowej), potem każdy nowy pakiet.
    /// Zwraca czas pierwszego wysłanego pakietu (None = w buforze nie ma jeszcze klatki kluczowej).
    pub fn podlacz_od_klucza(&mut self, odbiornik: Sender<Pakiet>) -> Option<i64> {
        // Klatka kluczowa z samego początku kolejki nie jest już w indeksie `klucze`
        let od = self
            .klucze
            .back()
            .map(|&k| (k - self.usuniete) as usize)
            .filter(|&i| i < self.pakiety.len())
            .or_else(|| self.pakiety.front().filter(|p| p.klucz).map(|_| 0));
        let czas = od.map(|i| self.pakiety[i].czas);
        if let Some(i) = od {
            for p in self.pakiety.iter().skip(i) {
                let _ = odbiornik.send(p.clone());
            }
        }
        self.odbiornik = Some(odbiornik);
        czas
    }

    /// Podłącza odbiornik dźwięku: dostaje pakiety z bufora nie starsze niż `od` i każdy nowy.
    pub fn podlacz_od_czasu(&mut self, odbiornik: Sender<Pakiet>, od: i64) {
        for p in self.pakiety.iter().filter(|p| p.czas >= od) {
            let _ = odbiornik.send(p.clone());
        }
        self.odbiornik = Some(odbiornik);
    }

    /// Odłącza odbiornik (jego wątek kończy plik, gdy kanał się zamknie).
    pub fn odlacz(&mut self) {
        self.odbiornik = None;
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
    fn odbiornik_dostaje_obraz_od_ostatniej_klatki_kluczowej() {
        let mut b = Bufor::pusty(10);
        for i in 0..25 {
            b.dodaj(p(i * 100, i % 10 == 0));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        // Ostatnia klatka kluczowa to nr 20: odbiornik dostaje 20..24, potem nowe pakiety
        assert_eq!(b.podlacz_od_klucza(tx), Some(20 * 1_000_000));
        b.dodaj(p(2500, false));
        let czasy: Vec<i64> = rx.try_iter().map(|p| p.czas / 1_000_000).collect();
        assert_eq!(czasy, vec![20, 21, 22, 23, 24, 25]);
        // Po odłączeniu nic więcej nie przychodzi, a bufor działa dalej
        b.odlacz();
        b.dodaj(p(2600, false));
        assert!(rx.try_recv().is_err());
        assert_eq!(b.ostatni_czas(), Some(26 * 1_000_000));
    }

    #[test]
    fn odbiornik_dzwieku_od_czasu_i_zamkniety_kanal() {
        let mut b = Bufor::pusty(10);
        for i in 0..10 {
            b.dodaj(p(i * 100, true));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        b.podlacz_od_czasu(tx, 7 * 1_000_000);
        assert_eq!(rx.try_iter().count(), 3);
        // Zamknięty kanał nie przeszkadza w nagrywaniu
        drop(rx);
        b.dodaj(p(1000, true));
        b.dodaj(p(1100, true));
        assert_eq!(b.migawka().len(), 12);
    }

    #[test]
    fn dlugie_nagrywanie_trzyma_staly_rozmiar() {
        // Godzina przy 60 FPS z klatką kluczową co 2 s: bufor 30 s nie rośnie i zawsze zaczyna się od klucza
        let mut b = Bufor::pusty(30);
        let okres = 10_000_000 / 60;
        let mut najwiecej = 0;
        for i in 0..60 * 3600i64 {
            b.dodaj(Pakiet { czas: i * okres, klucz: i % 120 == 0, dane: Arc::from(vec![0u8; 1]) });
            najwiecej = najwiecej.max(b.pakiety.len());
        }
        // 30 s + najwyżej jeden odstęp klatek kluczowych
        assert!(najwiecej <= 60 * 32 + 1, "{najwiecej}");
        let m = b.migawka();
        assert!(m[0].klucz);
        assert!(m.windows(2).all(|p| p[1].czas > p[0].czas));
        assert!(m.last().unwrap().czas - m[0].czas >= 30 * 10_000_000 - okres);
        // Wewnętrzna lista klatek kluczowych też się nie rozrasta
        assert!(b.klucze.len() <= 17, "{}", b.klucze.len());
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
