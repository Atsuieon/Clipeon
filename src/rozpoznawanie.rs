//! Rozpoznawanie jednej frazy (np. „record video”) po głosie użytkownika: bez modelu językowego,
//! bez sieci. Użytkownik mówi frazę 3 razy, a Clipeon zapamiętuje jej „odcisk” (MFCC — kształt widma
//! w czasie). Potem każdą wypowiedź z mikrofonu (odciętą ciszą) porównuje z odciskami przez DTW
//! (dopasowanie z rozciąganiem czasu, więc szybsze i wolniejsze powiedzenie też pasuje).
//! Koszt: jedna mała FFT co 10 ms, tylko gdy funkcja jest włączona.

/// Częstotliwość, na której liczymy cechy (mowa nie potrzebuje więcej)
pub const CZESTOTLIWOSC: u32 = 16_000;
/// Okno 25 ms, krok 10 ms
const OKNO: usize = 400;
const KROK: usize = 160;
const FFT: usize = 512;
const PASMA: usize = 26;
/// Liczba współczynników cepstralnych (c1..c12; c0 = głośność pomijamy)
pub const CECHY: usize = 12;
/// Wypowiedź: krótsza niż 0,3 s to stuknięcie, dłuższa niż 3 s — zostaje jej koniec
const MIN_RAMEK: usize = 30;
const MAX_RAMEK: usize = 300;
/// Ile ramek ciszy kończy wypowiedź (250 ms) i ile ramek zapasu przed jej początkiem
const CISZA_KONCA: usize = 25;
const ZAPAS: usize = 8;
/// Mowa = co najmniej tyle dB ponad poziom szumu (i nie ciszej niż MIN_DB)
const PONAD_SZUM_DB: f32 = 10.0;
const MIN_DB: f32 = -62.0;
/// Z ilu ostatnich ramek (1,5 s) bierzemy najcichszą jako poziom szumu
const HISTORIA_SZUMU: usize = 150;

pub type Ramka = [f32; CECHY];

/// Brzegi wypowiedzi cichsze o tyle od jej najgłośniejszej ramki są obcinane (oddech, stuknięcie, szum)
const BRZEG_DB: f32 = 30.0;

/// Wypowiedź odcięta ciszą: cechy ramek i jej dźwięk (mono 16 kHz, do zapisu próbek przy dostrajaniu)
pub struct Wypowiedz {
    pub ramki: Vec<Ramka>,
    pub probki: Vec<f32>,
    /// Najgłośniejsza ramka (dBFS)
    pub szczyt_db: f32,
    /// Najdłuższy ciąg ramek najwyżej 15 dB ciszej od szczytu (samogłoska trwa ≥ 120 ms bez przerwy,
    /// a stuknięcie klawisza 2–3 ramki)
    pub glosny_ciag: usize,
}

impl Wypowiedz {
    /// Czy to w ogóle może być powiedziana fraza: dość głośno (`najcichszy_szczyt_db`) i dość długo.
    /// Odsiewa ciche szumy, brum i klikanie klawiatury, które przy samym dopasowaniu potrafiły wyjść
    /// podobne do frazy.
    pub fn moze_byc_mowa(&self, najcichszy_szczyt_db: f32) -> bool {
        self.szczyt_db >= najcichszy_szczyt_db && self.glosny_ciag >= MIN_GLOSNEGO_CIAGU
    }
}

/// Fraza ma co najmniej tyle ramek (120 ms) z rzędu blisko swojego szczytu głośności
const MIN_GLOSNEGO_CIAGU: usize = 12;

/// Liczy cechy z ciągłego strumienia próbek 16 kHz i tnie go na wypowiedzi według głośności.
pub struct Sluchacz {
    probki: Vec<f32>,
    okno_hamminga: Vec<f32>,
    filtry: Vec<Vec<(usize, f32)>>,
    dct: Vec<[f32; PASMA]>,
    /// Głośność ostatnich ramek (dB) do oceny szumu tła
    historia_db: std::collections::VecDeque<f32>,
    /// Ostatnie ramki przed początkiem mowy (zapas): cechy, głośność i nowe próbki tej ramki
    przed: std::collections::VecDeque<(Ramka, f32, Vec<f32>)>,
    /// Trwająca wypowiedź
    mowa: Option<Vec<(Ramka, f32, Vec<f32>)>>,
    mowy_z_rzedu: usize,
    ciszy_z_rzedu: usize,
}

impl Sluchacz {
    pub fn nowy() -> Sluchacz {
        let okno_hamminga =
            (0..OKNO).map(|n| 0.54 - 0.46 * (2.0 * std::f32::consts::PI * n as f32 / (OKNO - 1) as f32).cos()).collect();
        // Filtry trójkątne w skali mel, 80–7600 Hz
        let mel = |f: f32| 2595.0 * (1.0 + f / 700.0).log10();
        let hz = |m: f32| 700.0 * (10f32.powf(m / 2595.0) - 1.0);
        let (m0, m1) = (mel(80.0), mel(7600.0));
        let srodki: Vec<f32> = (0..PASMA + 2)
            .map(|i| hz(m0 + (m1 - m0) * i as f32 / (PASMA + 1) as f32) * FFT as f32 / CZESTOTLIWOSC as f32)
            .collect();
        let filtry = (0..PASMA)
            .map(|p| {
                let (a, b, c) = (srodki[p], srodki[p + 1], srodki[p + 2]);
                (a.floor() as usize..=c.ceil() as usize)
                    .filter_map(|k| {
                        let k_f = k as f32;
                        let w = if k_f < b { (k_f - a) / (b - a) } else { (c - k_f) / (c - b) };
                        (w > 0.0 && k <= FFT / 2).then_some((k, w))
                    })
                    .collect()
            })
            .collect();
        let dct = (1..=CECHY)
            .map(|i| {
                let mut w = [0f32; PASMA];
                for (j, x) in w.iter_mut().enumerate() {
                    *x = (std::f32::consts::PI * i as f32 * (j as f32 + 0.5) / PASMA as f32).cos();
                }
                w
            })
            .collect();
        Sluchacz {
            probki: Vec::with_capacity(OKNO * 4),
            okno_hamminga,
            filtry,
            dct,
            historia_db: std::collections::VecDeque::with_capacity(HISTORIA_SZUMU + 1),
            przed: std::collections::VecDeque::with_capacity(ZAPAS + 1),
            mowa: None,
            mowy_z_rzedu: 0,
            ciszy_z_rzedu: 0,
        }
    }

    /// Dokłada próbki (mono, 16 kHz, -1..1). Każdą wypowiedź, która właśnie się skończyła (surowe cechy),
    /// oddaje do `gotowa`.
    pub fn dodaj(&mut self, probki: &[f32], mut gotowa: impl FnMut(Wypowiedz)) {
        self.probki.extend_from_slice(probki);
        let mut start = 0;
        while self.probki.len() - start >= OKNO {
            let (ramka, db) = self.ramka(start);
            let nowe = self.probki[start + OKNO - KROK..start + OKNO].to_vec();
            start += KROK;
            if let Some(w) = self.krok(ramka, db, nowe) {
                gotowa(w);
            }
        }
        self.probki.drain(..start);
    }

    fn ramka(&self, od: usize) -> (Ramka, f32) {
        let mut re = [0f32; FFT];
        let mut im = [0f32; FFT];
        let mut energia = 0.0;
        let mut poprzednia = self.probki[od.saturating_sub(1)];
        for ((r, &x), h) in re.iter_mut().zip(&self.probki[od..od + OKNO]).zip(&self.okno_hamminga) {
            energia += x * x;
            // Preemfaza (podbicie wysokich tonów, jak zawsze przy MFCC); głośność liczona bez niej
            *r = (x - 0.97 * poprzednia) * h;
            poprzednia = x;
        }
        let db = 10.0 * (energia / OKNO as f32 + 1e-12).log10();
        fft(&mut re, &mut im);
        let mut pasma = [0f32; PASMA];
        for (p, filtr) in self.filtry.iter().enumerate() {
            pasma[p] = filtr.iter().map(|&(k, w)| w * (re[k] * re[k] + im[k] * im[k])).sum();
        }
        // Podłoga 30 dB pod średnią pasm: resztki szumu w „dolinach” widma nie zmieniają odcisku
        let podloga = pasma.iter().sum::<f32>() / PASMA as f32 * 1e-3 + 1e-10;
        for x in &mut pasma {
            *x = (*x + podloga).ln();
        }
        let mut r = [0f32; CECHY];
        for (i, w) in self.dct.iter().enumerate() {
            r[i] = w.iter().zip(&pasma).map(|(a, b)| a * b).sum();
        }
        (r, db)
    }

    fn szum_db(&self) -> f32 {
        self.historia_db.iter().copied().fold(f32::INFINITY, f32::min)
    }

    fn krok(&mut self, ramka: Ramka, db: f32, nowe: Vec<f32>) -> Option<Wypowiedz> {
        // Poziom szumu = najcichsza ramka z ostatnich 1,5 s (w mowie zawsze trafiają się przerwy)
        self.historia_db.push_back(db);
        if self.historia_db.len() > HISTORIA_SZUMU {
            self.historia_db.pop_front();
        }
        let mowa = db > MIN_DB && db > self.szum_db() + PONAD_SZUM_DB;
        match &mut self.mowa {
            None => {
                self.mowy_z_rzedu = if mowa { self.mowy_z_rzedu + 1 } else { 0 };
                self.przed.push_back((ramka, db, nowe));
                if self.przed.len() > ZAPAS {
                    self.przed.pop_front();
                }
                if self.mowy_z_rzedu >= 3 {
                    self.mowa = Some(self.przed.drain(..).collect());
                    self.ciszy_z_rzedu = 0;
                }
                None
            }
            Some(w) => {
                w.push((ramka, db, nowe));
                if w.len() > MAX_RAMEK + CISZA_KONCA {
                    w.remove(0);
                }
                self.ciszy_z_rzedu = if mowa { 0 } else { self.ciszy_z_rzedu + 1 };
                if self.ciszy_z_rzedu < CISZA_KONCA {
                    return None;
                }
                let mut w = self.mowa.take().unwrap();
                self.mowy_z_rzedu = 0;
                // Z ciszy na końcu zostaje tyle, co zapasu na początku
                w.truncate(w.len() - CISZA_KONCA + ZAPAS);
                // Ciche brzegi (oddech przed, szum po) wydłużają wypowiedź i psują dopasowanie
                let szczyt = w.iter().map(|x| x.1).fold(f32::NEG_INFINITY, f32::max);
                let glosna = |x: &(Ramka, f32, Vec<f32>)| x.1 >= szczyt - BRZEG_DB;
                let od = w.iter().position(glosna).unwrap_or(0).saturating_sub(3);
                let do_ = (w.iter().rposition(glosna).unwrap_or(w.len()) + 4).min(w.len());
                let w = &w[od..do_];
                (w.len() >= MIN_RAMEK).then(|| Wypowiedz {
                    ramki: w.iter().map(|x| x.0).collect(),
                    probki: w.iter().flat_map(|x| x.2.iter().copied()).collect(),
                    szczyt_db: szczyt,
                    glosny_ciag: w
                        .iter()
                        .scan(0, |ciag, x| {
                            *ciag = if x.1 >= szczyt - 15.0 { *ciag + 1 } else { 0 };
                            Some(*ciag)
                        })
                        .max()
                        .unwrap_or(0),
                })
            }
        }
    }
}

/// Odejmuje średnią każdego współczynnika (inny mikrofon / pogłos zmienia widmo o stałą)
pub fn normalizuj(mut w: Vec<Ramka>) -> Vec<Ramka> {
    let mut srednia = [0f32; CECHY];
    for r in &w {
        for (s, x) in srednia.iter_mut().zip(r) {
            *s += x;
        }
    }
    for s in &mut srednia {
        *s /= w.len() as f32;
    }
    for r in &mut w {
        for (x, s) in r.iter_mut().zip(&srednia) {
            *x -= s;
        }
    }
    w
}

fn odleglosc(a: &Ramka, b: &Ramka) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt()
}

/// Jak bardzo wypowiedź przypomina wzorzec (mniej = bardziej). Wzorzec może leżeć w dowolnym miejscu
/// wypowiedzi („okej, record video”), ale musi pokryć się z nią w całości i trwać od połowy do
/// dwukrotności swojej długości. Wynik = średnia odległość ramek na ścieżce dopasowania.
pub fn dopasowanie(wzorzec: &[Ramka], wypowiedz: &[Ramka]) -> f32 {
    let (n, m) = (wzorzec.len(), wypowiedz.len());
    if n == 0 || m == 0 {
        return f32::INFINITY;
    }
    // Wiersz poprzedni / bieżący: (suma, długość ścieżki, początek w wypowiedzi)
    let mut pop: Vec<(f32, u32, usize)> = (0..m).map(|j| (odleglosc(&wzorzec[0], &wypowiedz[j]), 1, j)).collect();
    let mut biez = vec![(0f32, 0u32, 0usize); m];
    for wz in &wzorzec[1..] {
        for j in 0..m {
            let d = odleglosc(wz, &wypowiedz[j]);
            let mut najlepszy = pop[j]; // wzorzec idzie dalej, wypowiedź stoi
            let lepszy = |a: (f32, u32, usize), b: (f32, u32, usize)| {
                if (a.0 + d) / (a.1 + 1) as f32 <= (b.0 + d) / (b.1 + 1) as f32 { a } else { b }
            };
            if j > 0 {
                najlepszy = lepszy(pop[j - 1], najlepszy); // oba idą dalej
                najlepszy = lepszy(biez[j - 1], najlepszy); // wypowiedź idzie dalej
            }
            biez[j] = (najlepszy.0 + d, najlepszy.1 + 1, najlepszy.2);
        }
        std::mem::swap(&mut pop, &mut biez);
    }
    pop.iter()
        .enumerate()
        .filter(|(j, (_, _, od))| {
            let dl = j + 1 - od;
            dl * 2 >= n && dl <= n * 2
        })
        .map(|(_, (s, l, _))| s / *l as f32)
        .fold(f32::INFINITY, f32::min)
}

/// Wynik wypowiedzi (surowe cechy ze `Sluchacz`) względem wzorców (znormalizowanych): średnia z dwóch
/// najlepszych (jeden przypadkowo podobny wzorzec nie wystarczy). Fraza na końcu dłuższej wypowiedzi
/// („okej, record video”) ma inną średnią widma niż cała wypowiedź, więc sprawdzamy też same końcówki.
pub fn wynik(wzorce: &[Vec<Ramka>], wypowiedz: &[Ramka]) -> f32 {
    let srednia = wzorce.iter().map(|w| w.len()).sum::<usize>() / wzorce.len().max(1);
    let mut kawalki = vec![normalizuj(wypowiedz.to_vec())];
    for k in [0.9, 1.1, 1.3, 1.5] {
        let dl = (srednia as f32 * k) as usize + ZAPAS;
        if dl + 10 < wypowiedz.len() {
            kawalki.push(normalizuj(wypowiedz[wypowiedz.len() - dl..].to_vec()));
        }
    }
    kawalki.iter().map(|k| wynik_jednego(wzorce, k)).fold(f32::INFINITY, f32::min)
}

fn wynik_jednego(wzorce: &[Vec<Ramka>], wypowiedz: &[Ramka]) -> f32 {
    let mut w: Vec<f32> = wzorce.iter().map(|wz| dopasowanie(wz, wypowiedz)).collect();
    w.sort_by(|a, b| a.total_cmp(b));
    match w.len() {
        0 => f32::INFINITY,
        1 => w[0],
        _ => (w[0] + w[1]) / 2.0,
    }
}

/// Rozrzut próbek z nauki: jak bardzo różnią się między sobą powtórzenia tej samej frazy (najgorsze
/// dopasowanie jednej próbki do pozostałych). Z niego wychodzi próg rozpoznania.
#[cfg(test)]
pub fn rozrzut(wzorce: &[Vec<Ramka>]) -> f32 {
    wyniki_nauki(wzorce).into_iter().fold(0.0, f32::max)
}

/// Dopasowanie każdej próbki z nauki do pozostałych.
pub fn wyniki_nauki(wzorce: &[Vec<Ramka>]) -> Vec<f32> {
    (0..wzorce.len())
        .map(|i| {
            let inne: Vec<Vec<Ramka>> = wzorce.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, w)| w.clone()).collect();
            wynik_jednego(&inne, &wzorce[i])
        })
        .collect()
}

/// Próg rozpoznania: rozrzut z zapasem. Czułość 1 = mniej pomyłek, 2 = zwykła, 3 = łatwiej rozpoznaje.
/// Zapas sprawdzony na syntezie mowy: inne zdania tego samego głosu wychodzą zwykle ponad 2,5× rozrzutu,
/// a fraza w hałasie (ok. 25 dB ponad szum) do ok. 1,2×. Na prawdziwym głosie (1.5.2) fraza wyszła 1,0×,
/// a ciche szumy i klikanie od 1,2× — te odsiewa teraz `Wypowiedz::moze_byc_mowa`.
pub fn prog(rozrzut: f32, czulosc: u32) -> f32 {
    rozrzut
        * match czulosc {
            1 => 1.2,
            3 => 1.6,
            _ => 1.4,
        }
}

/// Zwykła FFT radix-2 w miejscu (rozmiar = potęga dwójki).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut dl = 2;
    while dl <= n {
        let kat = -2.0 * std::f32::consts::PI / dl as f32;
        let (wr, wi) = (kat.cos(), kat.sin());
        for start in (0..n).step_by(dl) {
            let (mut cr, mut ci) = (1f32, 0f32);
            for k in 0..dl / 2 {
                let (a, b) = (start + k, start + k + dl / 2);
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        dl <<= 1;
    }
}

/// Zamienia próbki z miksera Windows (dowolna częstotliwość, kanały, 16/24/32 bit int albo 32 bit float)
/// na mono 16 kHz. Trzyma resztę między pakietami.
pub struct Przelicznik {
    krok: f64,
    pozycja: f64,
    suma: f32,
    ile: u32,
}

impl Przelicznik {
    pub fn nowy(czestotliwosc: u32) -> Przelicznik {
        Przelicznik { krok: CZESTOTLIWOSC as f64 / czestotliwosc as f64, pozycja: 0.0, suma: 0.0, ile: 0 }
    }

    /// Dokłada próbki mono (już zmiksowane); co 1/16000 s oddaje średnią (prosty filtr przed decymacją).
    pub fn dodaj(&mut self, mono: impl Iterator<Item = f32>, wyjscie: &mut Vec<f32>) {
        for x in mono {
            self.suma += x;
            self.ile += 1;
            self.pozycja += self.krok;
            if self.pozycja >= 1.0 {
                self.pozycja -= 1.0;
                wyjscie.push(self.suma / self.ile as f32);
                self.suma = 0.0;
                self.ile = 0;
            }
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    /// „Mowa” do testów: dźwięk z kilkoma tonami, których wysokość zmienia się w czasie, i obwiednią
    fn sylaby(sekundy: f32, ton: f32, glosnosc: f32) -> Vec<f32> {
        let n = (sekundy * CZESTOTLIWOSC as f32) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / CZESTOTLIWOSC as f32;
                let f = ton * (1.0 + 0.3 * (t * 7.0).sin());
                let obwiednia = (0.6 + 0.4 * (t * 25.0).sin()).max(0.0);
                glosnosc * obwiednia * ((2.0 * std::f32::consts::PI * f * t).sin() + 0.5 * (4.0 * std::f32::consts::PI * f * t).sin())
            })
            .collect()
    }

    fn szum(n: usize, poziom: f32, ziarno: &mut u32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *ziarno = ziarno.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                poziom * (((*ziarno >> 8) as f32 / 16_777_216.0) - 0.5)
            })
            .collect()
    }

    fn wypowiedzi(probki: &[f32]) -> Vec<Vec<Ramka>> {
        let mut s = Sluchacz::nowy();
        let mut w = Vec::new();
        for kawalek in probki.chunks(480) {
            s.dodaj(kawalek, |x| {
                assert_eq!(x.probki.len(), x.ramki.len() * KROK);
                w.push(x.ramki)
            });
        }
        w
    }

    fn nagranie(srodek: &[f32], ziarno: &mut u32) -> Vec<f32> {
        let mut p = szum(16_000, 0.002, ziarno);
        let mowa = szum(srodek.len(), 0.002, ziarno);
        p.extend(srodek.iter().zip(&mowa).map(|(a, b)| a + b));
        p.extend(szum(8_000, 0.002, ziarno));
        p
    }

    #[test]
    fn fft_impulsu_to_same_jedynki() {
        let mut re = [0f32; 16];
        let mut im = [0f32; 16];
        re[0] = 1.0;
        fft(&mut re, &mut im);
        assert!(re.iter().all(|x| (x - 1.0).abs() < 1e-6) && im.iter().all(|x| x.abs() < 1e-6));
    }

    #[test]
    fn fft_sinusa_trafia_w_jego_prazek() {
        let mut re: Vec<f32> = (0..64).map(|i| (2.0 * std::f32::consts::PI * 5.0 * i as f32 / 64.0).cos()).collect();
        let mut im = vec![0f32; 64];
        fft(&mut re, &mut im);
        let moc: Vec<f32> = re.iter().zip(&im).map(|(a, b)| a * a + b * b).collect();
        let max = (0..32).max_by(|&a, &b| moc[a].total_cmp(&moc[b])).unwrap();
        assert_eq!(max, 5);
    }

    #[test]
    fn przelicznik_48_na_16_khz() {
        let mut p = Przelicznik::nowy(48_000);
        let mut w = Vec::new();
        p.dodaj((0..4800).map(|_| 0.5), &mut w);
        assert_eq!(w.len(), 1600);
        assert!(w.iter().all(|x| (x - 0.5).abs() < 1e-6));
        let mut p = Przelicznik::nowy(44_100);
        let mut w = Vec::new();
        p.dodaj((0..44_100).map(|_| 0.0), &mut w);
        assert!((15_999..=16_001).contains(&w.len()));
    }

    #[test]
    fn wypowiedz_odcieta_cisza() {
        let mut z = 1;
        let w = wypowiedzi(&nagranie(&sylaby(0.8, 300.0, 0.3), &mut z));
        assert_eq!(w.len(), 1);
        // 0,8 s = 80 ramek (+ zapas na brzegach)
        assert!((75..=100).contains(&w[0].len()), "{}", w[0].len());
        // Sama cisza i szum to żadna wypowiedź, stuknięcie (50 ms) też nie
        assert!(wypowiedzi(&szum(48_000, 0.01, &mut z)).is_empty());
        assert!(wypowiedzi(&nagranie(&sylaby(0.05, 300.0, 0.5), &mut z)).is_empty());
    }

    #[test]
    fn rozpoznaje_fraze_a_nie_inna() {
        let mut z = 7;
        let wzorce: Vec<Vec<Ramka>> = [0.9, 1.0, 1.1]
            .iter()
            .map(|&tempo| {
                let w = wypowiedzi(&nagranie(&sylaby(0.9 * tempo, 300.0, 0.3), &mut z));
                assert_eq!(w.len(), 1);
                normalizuj(w.into_iter().next().unwrap())
            })
            .collect();
        let r = rozrzut(&wzorce);
        let prog = prog(r, 2);
        // Ta sama fraza ciszej i wolniej
        let ta = wypowiedzi(&nagranie(&sylaby(1.05, 300.0, 0.1), &mut z));
        assert!(wynik(&wzorce, &ta[0]) <= prog, "{} > {prog}", wynik(&wzorce, &ta[0]));
        // Fraza po innych słowach w jednym ciągu
        let mut dluga = sylaby(0.8, 700.0, 0.3);
        dluga.extend(sylaby(0.9, 300.0, 0.3));
        let po = wypowiedzi(&nagranie(&dluga, &mut z));
        assert!(wynik(&wzorce, &po[0]) <= prog, "{} > {prog}", wynik(&wzorce, &po[0]));
        // Coś zupełnie innego
        let inna = wypowiedzi(&nagranie(&sylaby(0.9, 900.0, 0.3), &mut z));
        assert!(wynik(&wzorce, &inna[0]) > prog, "{} <= {prog}", wynik(&wzorce, &inna[0]));
    }

    #[test]
    fn ciche_szumy_i_klikanie_to_nie_mowa() {
        let mut z = 3;
        let mowa = wypowiedzi_pelne(&nagranie(&sylaby(0.9, 300.0, 0.3), &mut z));
        assert!(mowa[0].moze_byc_mowa(-30.0), "szczyt {} ciąg {}", mowa[0].szczyt_db, mowa[0].glosny_ciag);
        let cicha = wypowiedzi_pelne(&nagranie(&sylaby(0.9, 300.0, 0.01), &mut z));
        assert!(cicha.iter().all(|w| !w.moze_byc_mowa(-30.0)));
        // Klawiatura: krótkie trzaski co 150 ms
        let klik: Vec<f32> = (0..16_000).map(|i| if i % 2_400 < 24 { 0.5 * (((i * 7_919) % 13) as f32 / 6.0 - 1.0) } else { 0.0 }).collect();
        let kliki = wypowiedzi_pelne(&nagranie(&klik, &mut z));
        assert!(kliki.iter().all(|w| !w.moze_byc_mowa(-30.0)));
    }

    fn wypowiedzi_pelne(probki: &[f32]) -> Vec<Wypowiedz> {
        let mut s = Sluchacz::nowy();
        let mut w = Vec::new();
        for kawalek in probki.chunks(480) {
            s.dodaj(kawalek, |x| w.push(x));
        }
        w
    }

    #[test]
    fn dopasowanie_samego_siebie_to_zero() {
        let w: Vec<Ramka> = (0..50).map(|i| [i as f32; CECHY]).collect();
        assert_eq!(dopasowanie(&w, &w), 0.0);
        assert_eq!(dopasowanie(&w, &[]), f32::INFINITY);
        // Za krótki fragment (mniej niż połowa wzorca) nie pasuje
        assert_eq!(dopasowanie(&w, &w[..20]), f32::INFINITY);
    }
}
