// Rysunek dymka liczony piksel po pikselu: półprzezroczysta karta z obwódką, kadr z zapisanego klipu
// z plakietką długości, kropka stanu i pasek czasu. Bez Windows API, żeby dało się go sprawdzić
// testami; napisy dopisuje GDI w powiadomienie.rs.

/// Kolor karty (prawie czarny, jak w oknie Clipeona) i jej krycie: gra lekko przebija przez dymek.
const TLO: [f32; 3] = [14.0, 14.0, 16.0];
const KRYCIE: f32 = 0.9;
/// Rozmiar kadru z klipu w dymku (16:9), w pikselach przy skali 100%.
const KADR: (f32, f32) = (112.0, 63.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prostokat {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Prostokat {
    /// Odległość punktu od krawędzi prostokąta o zaokrąglonych rogach (ujemna w środku).
    fn odleglosc(&self, px: f32, py: f32, r: f32) -> f32 {
        let qx = (px - (self.x + self.w / 2.0)).abs() - (self.w / 2.0 - r);
        let qy = (py - (self.y + self.h / 2.0)).abs() - (self.h / 2.0 - r);
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
    }
}

/// Kadr z zapisanego klipu: piksele 0x00RRGGBB wierszami od góry i długość klipu do plakietki („0:30”).
pub struct Kadr {
    pub piksele: Vec<u32>,
    pub szer: i32,
    pub wys: i32,
    pub dlugosc: String,
}

/// Rozmiar kadru w pikselach ekranu przy danej skali — o taki obrazek trzeba poprosić ffmpeg.
pub fn rozmiar_kadru(s: f32) -> (i32, i32) {
    ((KADR.0 * s).round() as i32, (KADR.1 * s).round() as i32)
}

/// Długość do plakietki na kadrze: „0:30”, „12:05”, „1:02:05”.
pub fn czas(sekundy: f64) -> String {
    let s = sekundy.max(0.0).round() as u64;
    match s / 3600 {
        0 => format!("{}:{:02}", s / 60, s % 60),
        g => format!("{g}:{:02}:{:02}", s % 3600 / 60, s % 60),
    }
}

/// Rozmieszczenie części dymka w pikselach ekranu.
pub struct Uklad {
    pub szer: i32,
    pub wys: i32,
    pub s: f32,
    /// Środek i promień kropki stanu
    pub kropka: (f32, f32, f32),
    pub kadr: Option<Prostokat>,
    /// Ciemne tło pod długością klipu, w prawym dolnym rogu kadru (ustawiane przez `plakietka`)
    pub plakietka: Option<Prostokat>,
    pub tytul: Prostokat,
    pub opis: Prostokat,
}

impl Uklad {
    /// Dymek z kadrem (300×80) albo sam tekst z kropką po lewej (360×68). Bez opisu tytuł stoi na środku.
    pub fn nowy(s: f32, z_kadrem: bool, z_opisem: bool) -> Uklad {
        let px = |v: f32| (v * s).round();
        if !z_kadrem {
            let (szer, wys) = (px(360.0), px(68.0));
            let lewa = px(40.0);
            let wiersz = |gora: f32, dol: f32| Prostokat { x: lewa, y: px(gora), w: szer - px(16.0) - lewa, h: px(dol) - px(gora) };
            return Uklad {
                szer: szer as i32,
                wys: wys as i32,
                s,
                kropka: (22.0 * s, wys / 2.0, 4.5 * s),
                kadr: None,
                plakietka: None,
                tytul: if z_opisem { wiersz(10.0, 36.0) } else { wiersz(0.0, 68.0) },
                opis: wiersz(34.0, 58.0),
            };
        }
        let (szer, wys) = (px(300.0), px(80.0));
        let (kw, kh) = rozmiar_kadru(s);
        let kadr = Prostokat { x: px(8.0), y: ((wys - kh as f32) / 2.0).floor(), w: kw as f32, h: kh as f32 };
        let lewa = kadr.x + kadr.w + px(12.0);
        let prawa = szer - px(14.0);
        let (gora, dol) = if z_opisem { (px(17.0), px(41.0)) } else { (0.0, wys) };
        Uklad {
            szer: szer as i32,
            wys: wys as i32,
            s,
            kropka: (lewa + 4.0 * s, (gora + dol) / 2.0 + 0.5 * s, 3.5 * s),
            kadr: Some(kadr),
            plakietka: None,
            tytul: Prostokat { x: lewa + px(15.0), y: gora, w: prawa - lewa - px(15.0), h: dol - gora },
            opis: Prostokat { x: lewa, y: px(40.0), w: prawa - lewa, h: px(22.0) },
        }
    }

    /// Ustawia plakietkę pod napis o podanej szerokości (w pikselach ekranu).
    pub fn plakietka(&mut self, szer_napisu: f32) {
        let Some(k) = self.kadr else { return };
        let s = self.s;
        let (w, h) = (szer_napisu + (12.0 * s).round(), (17.0 * s).round());
        let odstep = (5.0 * s).round();
        self.plakietka = Some(Prostokat { x: k.x + k.w - odstep - w, y: k.y + k.h - odstep - h, w, h });
    }

    /// Miejsce paska czasu przy dolnej krawędzi: (lewa, prawa, góra, dół) w pikselach.
    pub fn pasek(&self) -> (i32, i32, i32, i32) {
        let brzeg = (20.0 * self.s).round() as i32;
        let grubosc = ((2.0 * self.s).round() as i32).max(1);
        (brzeg, self.szer - brzeg, self.wys - grubosc, self.wys)
    }
}

/// Cienka obwódka tuż przy krawędzi kształtu (0..1); `d` = odległość od krawędzi.
fn obwodka(d: f32, s: f32) -> f32 {
    (0.5 - (d + s).abs() + 0.5 * s).clamp(0.0, 1.0)
}

fn mieszaj(a: [f32; 3], b: [f32; 3], ile: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * ile, a[1] + (b[1] - a[1]) * ile, a[2] + (b[2] - a[2]) * ile]
}

/// Rysuje dymek bez napisów do `piksele` (0x00RRGGBB) i zwraca krycie każdego piksela (0..1):
/// zero poza zaokrągloną kartą, pełne na kadrze, a na reszcie karty lekko przepuszcza grę.
pub fn rysuj(piksele: &mut [u32], u: &Uklad, akcent: [u8; 3], kadr: Option<&Kadr>) -> Vec<f32> {
    let s = u.s;
    let karta = Prostokat { x: 0.0, y: 0.0, w: u.szer as f32, h: u.wys as f32 };
    let akcent = akcent.map(f32::from);
    let (kx, ky, kr) = u.kropka;
    // Kadr w złym rozmiarze pomijamy, zamiast czytać poza obrazkiem
    let kadr = u.kadr.zip(kadr.filter(|k| k.szer > 0 && k.wys > 0 && k.piksele.len() == (k.szer * k.wys) as usize));
    let mut krycie = vec![0.0f32; piksele.len()];
    for y in 0..u.wys {
        for x in 0..u.szer {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = karta.odleglosc(px, py, 18.0 * s);
            let mut kol = TLO.map(|v| v + obwodka(d, s) * 30.0);
            let mut pelne = 0.0;

            if let Some((p, k)) = kadr {
                let dk = p.odleglosc(px, py, 11.0 * s);
                pelne = (0.5 - dk).clamp(0.0, 1.0);
                if pelne > 0.0 {
                    let ix = (x - p.x as i32).clamp(0, k.szer - 1);
                    let iy = (y - p.y as i32).clamp(0, k.wys - 1);
                    let v = k.piksele[(iy * k.szer + ix) as usize];
                    let mut obraz = [((v >> 16) & 0xff) as f32, ((v >> 8) & 0xff) as f32, (v & 0xff) as f32];
                    // Jasna kreska na brzegu kadru odcina ciemne kadry od karty
                    obraz = mieszaj(obraz, [255.0; 3], obwodka(dk, s) * 0.12);
                    if let Some(pl) = u.plakietka {
                        let cien = (0.5 - pl.odleglosc(px, py, 6.0 * s)).clamp(0.0, 1.0) * 0.72;
                        obraz = mieszaj(obraz, [0.0; 3], cien);
                    }
                    kol = mieszaj(kol, obraz, pelne);
                }
            }

            // Kropka stanu z delikatną poświatą
            let dkr = (px - kx).hypot(py - ky) - kr;
            let poswiata = (1.0 - dkr / (5.0 * s)).clamp(0.0, 1.0).powi(2) * 0.22;
            kol = mieszaj(kol, akcent, (0.5 - dkr).clamp(0.0, 1.0).max(poswiata));

            let i = (y * u.szer + x) as usize;
            piksele[i] = ((kol[0] as u32) << 16) | ((kol[1] as u32) << 8) | kol[2] as u32;
            krycie[i] = (0.5 - d).clamp(0.0, 1.0) * (KRYCIE + (1.0 - KRYCIE) * pelne);
        }
    }
    krycie
}

/// Pasek czasu przy dolnej krawędzi: `ile` (1 → 0) to część czasu, jaka została do zniknięcia dymka.
/// `piksele` są już pomnożone przez alfę, a `podstawa` to wiersze paska bez paska (od `pasek().2`).
pub fn rysuj_pasek(piksele: &mut [u32], podstawa: &[u32], u: &Uklad, akcent: [u8; 3], ile: f32) {
    let (x0, x1, y0, y1) = u.pasek();
    let koniec = x0 as f32 + (x1 - x0) as f32 * ile.clamp(0.0, 1.0);
    for y in y0..y1 {
        for x in x0..x1 {
            let tlo = podstawa[((y - y0) * u.szer + x) as usize];
            let a = tlo >> 24;
            let k = (koniec - x as f32).clamp(0.0, 1.0) * 0.8;
            let kanal = |przesuniecie: u32, barwa: u8| {
                let t = ((tlo >> przesuniecie) & 0xff) as f32;
                let cel = f32::from(barwa) * a as f32 / 255.0;
                ((t + (cel - t) * k) as u32) << przesuniecie
            };
            piksele[(y * u.szer + x) as usize] = (a << 24) | kanal(16, akcent[0]) | kanal(8, akcent[1]) | kanal(0, akcent[2]);
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    const ZIELONY: [u8; 3] = [52, 211, 153];

    fn kadr(s: f32, kolor: u32) -> Kadr {
        let (szer, wys) = rozmiar_kadru(s);
        Kadr { piksele: vec![kolor; (szer * wys) as usize], szer, wys, dlugosc: "0:30".into() }
    }

    #[test]
    fn czas_na_plakietce() {
        assert_eq!(czas(30.2), "0:30");
        assert_eq!(czas(725.0), "12:05");
        assert_eq!(czas(3725.0), "1:02:05");
        assert_eq!(czas(-1.0), "0:00");
    }

    #[test]
    fn kadr_miesci_sie_w_dymku_przy_kazdej_skali() {
        for s in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5] {
            let mut u = Uklad::nowy(s, true, true);
            u.plakietka(30.0 * s);
            let (k, p) = (u.kadr.unwrap(), u.plakietka.unwrap());
            assert_eq!((k.w as i32, k.h as i32), rozmiar_kadru(s));
            assert!(k.x >= 0.0 && k.y >= 0.0 && k.y + k.h <= u.wys as f32, "skala {s}");
            assert!(p.x > k.x && p.y > k.y && p.x + p.w < k.x + k.w && p.y + p.h < k.y + k.h, "skala {s}");
            assert!(u.tytul.x > k.x + k.w && u.tytul.x + u.tytul.w <= u.szer as f32, "skala {s}");
            let (x0, x1, y0, y1) = u.pasek();
            assert!(0 < x0 && x0 < x1 && x1 < u.szer && y0 < y1 && y1 == u.wys, "skala {s}");
        }
    }

    #[test]
    fn kadr_jest_pelny_a_karta_przepuszcza_gre() {
        let u = Uklad::nowy(1.0, true, true);
        let mut piksele = vec![0u32; (u.szer * u.wys) as usize];
        let krycie = rysuj(&mut piksele, &u, ZIELONY, Some(&kadr(1.0, 0x0010_80F0)));
        let i = |x: i32, y: i32| (y * u.szer + x) as usize;
        // Środek kadru: kolor z klipu, pełne krycie
        assert_eq!(piksele[i(60, 40)], 0x0010_80F0);
        assert_eq!(krycie[i(60, 40)], 1.0);
        // Karta obok kadru: ciemna i lekko przezroczysta
        assert_eq!(piksele[i(250, 70)], 0x000E_0E10);
        assert!((krycie[i(250, 70)] - KRYCIE).abs() < 1e-6);
        // Rogi okna są poza zaokrągloną kartą
        assert_eq!(krycie[i(0, 0)], 0.0);
        assert_eq!(krycie[i(u.szer - 1, u.wys - 1)], 0.0);
        // Kropka stanu ma kolor akcentu
        let (kx, ky, _) = u.kropka;
        assert_eq!(piksele[i(kx as i32, ky as i32)], 0x0034_D399);
    }

    #[test]
    fn plakietka_przyciemnia_kadr() {
        let mut u = Uklad::nowy(1.0, true, true);
        u.plakietka(26.0);
        let p = u.plakietka.unwrap();
        let mut piksele = vec![0u32; (u.szer * u.wys) as usize];
        rysuj(&mut piksele, &u, ZIELONY, Some(&kadr(1.0, 0x00FF_FFFF)));
        let srodek = piksele[((p.y + p.h / 2.0) as i32 * u.szer + (p.x + p.w / 2.0) as i32) as usize];
        assert!(srodek & 0xff < 90, "plakietka za jasna: {srodek:#x}");
    }

    #[test]
    fn zly_kadr_nie_psuje_dymka() {
        let u = Uklad::nowy(1.0, true, false);
        let mut piksele = vec![0u32; (u.szer * u.wys) as usize];
        let zly = Kadr { piksele: vec![0; 10], szer: 112, wys: 63, dlugosc: String::new() };
        let krycie = rysuj(&mut piksele, &u, ZIELONY, Some(&zly));
        assert_eq!(piksele[(40 * u.szer + 60) as usize], 0x000E_0E10);
        assert!(krycie[(40 * u.szer + 60) as usize] < 1.0);
    }

    #[test]
    fn dymek_bez_kadru_ma_stary_rozmiar() {
        let u = Uklad::nowy(1.5, false, true);
        assert_eq!((u.szer, u.wys), (540, 102));
        assert!(u.kadr.is_none());
    }

    #[test]
    fn pasek_skraca_sie_z_czasem() {
        let u = Uklad::nowy(1.0, true, true);
        let (x0, x1, y0, y1) = u.pasek();
        let podstawa = vec![0xE60C_0C0Eu32; ((y1 - y0) * u.szer) as usize];
        let mut piksele = vec![0xE60C_0C0Eu32; (u.szer * u.wys) as usize];
        let zielone = |p: &[u32]| (x0..x1).filter(|x| (p[(y0 * u.szer + x) as usize] >> 8) & 0xff > 100).count() as i32;
        rysuj_pasek(&mut piksele, &podstawa, &u, ZIELONY, 1.0);
        assert_eq!(zielone(&piksele), x1 - x0);
        rysuj_pasek(&mut piksele, &podstawa, &u, ZIELONY, 0.5);
        assert!((zielone(&piksele) - (x1 - x0) / 2).abs() <= 1);
        rysuj_pasek(&mut piksele, &podstawa, &u, ZIELONY, 0.0);
        assert_eq!(zielone(&piksele), 0);
        // Alfa i piksele poza paskiem zostają nietknięte
        assert!(piksele.iter().all(|p| p >> 24 == 0xE6));
        assert_eq!(piksele[0], 0xE60C_0C0E);
    }
}
