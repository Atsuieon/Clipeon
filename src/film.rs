// Długie nagrywanie („film”): ten sam zakodowany obraz i dźwięk, które idą do bufora powtórki,
// są dodatkowo dopisywane do plików tymczasowych na dysku. Po zatrzymaniu ffmpeg składa z nich MP4
// bez ponownego kodowania obrazu. Nie ma drugiego enkodera — film kosztuje tylko miejsce na dysku.
// Każdy restart sesji nagrywania (gra zmieniła tryb ekranu, pauza) zaczyna nowy segment; segmenty
// o tych samych parametrach są na końcu sklejane w jeden plik, a inne dają kolejne pliki.
// Znaczniki (skrót w trakcie filmu) trafiają do pliku jako rozdziały MP4.
// Obok plików tymczasowych leżą opisy (film.json, s0.json, postęp zapisu): gdy program padnie w trakcie
// filmu, przy następnym starcie składamy film z tego, co zdążyło się zapisać.
use crate::Zablokuj;
use crate::bufor::{Bufor, Pakiet, WspolnyBufor, teraz_100ns};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use crate::enkodery::Kodek;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Początek nazwy pliku filmu — po nim biblioteka odróżnia filmy od klipów.
pub const PRZEDROSTEK: &str = "Clipeon_Film";
/// Podfolder folderu klipów na pliki tymczasowe trwającego nagrania
const FOLDER_TMP: &str = ".nagrywanie";
/// Tyle wolnego miejsca trzeba mieć, żeby zacząć film
const MIEJSCE_NA_START: u64 = 2_000_000_000;
/// Poniżej tylu wolnych bajtów film kończy się sam (zostaje miejsce na złożenie pliku)
const MIEJSCE_NA_KONIEC: u64 = 500_000_000;
/// Opis filmu w folderze plików tymczasowych (do odzyskania po awarii)
const OPIS_FILMU: &str = "film.json";
/// Czasy znaczników (po jednym w wierszu)
const ZNACZNIKI: &str = "znaczniki.txt";
/// Ślad próby odzyskania: gdy odzyskiwanie samo się wywróci, następny start już go nie powtarza
const PROBA_ODZYSKANIA: &str = "odzyskiwanie";
/// Co tyle klatek wątek obrazu dopisuje na dysk, ile już bezpiecznie zapisał (przy 60 FPS co 2 s)
const POSTEP_CO: u64 = 120;
/// Znacznik tuż po końcu segmentu (spóźniony o ułamek sekundy skrót) wciąż należy do tego segmentu
const ZAPAS_ZNACZNIKA: i64 = 10_000_000;
/// Do tego rozmiaru film dostaje „szybki start” (nagłówek na początku pliku — wygodne do wysyłania);
/// większe pliki musiałyby być przepisywane drugi raz
const SZYBKI_START_DO: u64 = 2_000_000_000;

/// Ustawienie `format_filmu`: film zostaje taki, jak się nagrał
pub const BEZ_ZMIAN: &str = "bez_zmian";
/// Mniejsze formaty: (wartość ustawienia, rodzina enkodera, opis, jaka część bitrate obrazu zostaje).
/// HEVC przy mniej więcej połowie bitrate wygląda jak H.264, AV1 już przy jednej trzeciej.
const MNIEJSZE: &[(&str, Kodek, &str, f64)] = &[
    ("hevc", Kodek::Hevc, "Mniejszy plik — HEVC, ok. połowa rozmiaru", 0.45),
    ("av1", Kodek::Av1, "Najmniejszy plik — AV1, ok. 1/3 rozmiaru", 0.33),
];
/// Zmniejszony film nie schodzi poniżej tylu bitów obrazu na sekundę (niżej widać już kwadraty)
const NAJMNIEJSZY_BITRATE: u64 = 1_500_000;
/// Zamykanie programu albo instalacja aktualizacji: trwające zmniejszanie ma się przerwać
static PRZERWIJ_ZMNIEJSZANIE: AtomicBool = AtomicBool::new(false);
/// Naraz zmniejsza się jeden film (kolejny czeka)
static ZMNIEJSZANIE: Mutex<()> = Mutex::new(());

/// Parametry obrazu jednej sesji nagrywania (segmenty o tych samych da się skleić bez kodowania).
#[derive(Clone, PartialEq, Debug)]
pub struct Obraz {
    /// Format surowego strumienia dla ffmpeg (h264 / hevc / obu)
    pub format: &'static str,
    pub fps: u32,
    pub rozmiar: (u32, u32),
    pub enkoder: &'static str,
}

/// Opis filmu na dysku (film.json).
#[derive(serde::Serialize, serde::Deserialize)]
struct OpisFilmu {
    znacznik: String,
    gra: Option<String>,
}

/// Opis segmentu na dysku (s{nr}.json): parametry obrazu i nazwy ścieżek dźwięku.
#[derive(serde::Serialize, serde::Deserialize)]
struct OpisSegmentu {
    format: String,
    fps: u32,
    szerokosc: u32,
    wysokosc: u32,
    enkoder: String,
    dzwiek: Vec<String>,
}

/// Wspólne liczniki wątków zapisujących.
#[derive(Default)]
struct Wspolne {
    bajty: AtomicU64,
    blad: Mutex<Option<String>>,
}

struct WynikToru {
    /// Czas pierwszego zapisanego pakietu (None = nic nie zapisano)
    pierwszy: Option<i64>,
    /// Czas ostatniego zapisanego pakietu
    ostatni: i64,
    pakiety: u64,
}

/// Jeden plik tymczasowy (obraz albo jedna ścieżka dźwięku) i wątek, który go pisze.
struct Tor {
    nazwa: &'static str,
    plik: PathBuf,
    watek: JoinHandle<WynikToru>,
}

struct Segment {
    obraz: Obraz,
    wideo: Tor,
    audio: Vec<Tor>,
    /// Bufory, do których podłączone są odbiorniki (żeby je odłączyć na końcu)
    bufory: Vec<Weak<Mutex<Bufor>>>,
}

/// Segment po zakończeniu zapisu: pliki i czasy potrzebne do złożenia MP4.
#[derive(Debug)]
struct Gotowy {
    obraz: Obraz,
    wideo: PathBuf,
    poczatek: i64,
    /// Czas ostatniej klatki
    koniec: i64,
    klatki: u64,
    /// (nazwa ścieżki, plik, czas pierwszej ramki)
    audio: Vec<(&'static str, PathBuf, i64)>,
}

impl Gotowy {
    fn nazwy(&self) -> Vec<&'static str> {
        self.audio.iter().map(|(n, _, _)| *n).collect()
    }
}

/// Zapisany film.
pub struct Wynik {
    pub pliki: Vec<PathBuf>,
    pub sekundy: f64,
}

/// Trwające długie nagrywanie.
pub struct Film {
    folder: PathBuf,
    tmp: PathBuf,
    znacznik: String,
    start: Instant,
    gra: Option<String>,
    segmenty: Vec<Segment>,
    wspolne: Arc<Wspolne>,
    /// Czasy znaczników (zegar QPC, jak pakiety)
    znaczniki: Vec<i64>,
}

impl Film {
    /// Zaczyna film: sprawdza miejsce na dysku i zakłada folder plików tymczasowych.
    pub fn nowy(folder: &Path, gra: Option<String>) -> Result<Film, String> {
        std::fs::create_dir_all(folder).map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder.display()))?;
        if crate::zapis::wolne_miejsce(folder).is_some_and(|w| w < MIEJSCE_NA_START) {
            return Err(format!(
                "Za mało miejsca na dysku z folderem {} — do nagrania filmu potrzeba co najmniej 2 GB",
                folder.display()
            ));
        }
        let znacznik = crate::zapis::znacznik_czasu();
        let tmp = folder.join(FOLDER_TMP).join(&znacznik);
        std::fs::create_dir_all(&tmp).map_err(|e| format!("Nie mogę utworzyć {}: {e}", tmp.display()))?;
        zapisz_json(&tmp.join(OPIS_FILMU), &OpisFilmu { znacznik: znacznik.clone(), gra: gra.clone() })?;
        Ok(Film {
            folder: folder.to_path_buf(),
            tmp,
            znacznik,
            start: Instant::now(),
            gra,
            segmenty: Vec::new(),
            wspolne: Arc::default(),
            znaczniki: Vec::new(),
        })
    }

    /// Podłącza film do buforów nowej sesji nagrywania (nowy segment).
    pub fn dodaj_segment(&mut self, obraz: Obraz, wideo: &WspolnyBufor, audio: &[(&'static str, WspolnyBufor)]) -> Result<(), String> {
        let nr = self.segmenty.len();
        let poczatek = Arc::new(AtomicI64::new(0));
        let blad_pliku = |e: std::io::Error| format!("Nie mogę zapisać pliku filmu: {e}");
        let opis = OpisSegmentu {
            format: obraz.format.to_string(),
            fps: obraz.fps,
            szerokosc: obraz.rozmiar.0,
            wysokosc: obraz.rozmiar.1,
            enkoder: obraz.enkoder.to_string(),
            dzwiek: audio.iter().map(|(n, _)| n.to_string()).collect(),
        };
        zapisz_json(&self.tmp.join(format!("s{nr}.json")), &opis)?;

        let plik_wideo = self.tmp.join(format!("s{nr}-obraz.tmp"));
        let (tx, rx) = channel();
        let postep = self.tmp.join(format!("s{nr}-postep.txt"));
        let watek = pisz_obraz(std::fs::File::create(&plik_wideo).map_err(blad_pliku)?, rx, poczatek.clone(), self.wspolne.clone(), postep)?;
        // Film zaczyna się od ostatniej klatki kluczowej w buforze (najwyżej sekundę wstecz)
        let od = wideo.zablokuj().podlacz_od_klucza(tx).unwrap_or_else(teraz_100ns);
        let tor_wideo = Tor { nazwa: "", plik: plik_wideo, watek };

        let mut tory = Vec::new();
        for (i, (nazwa, bufor)) in audio.iter().enumerate() {
            let plik = self.tmp.join(format!("s{nr}-dzwiek{i}.aac"));
            let (tx, rx) = channel();
            let watek = pisz_dzwiek(
                std::fs::File::create(&plik).map_err(blad_pliku)?,
                rx,
                poczatek.clone(),
                self.wspolne.clone(),
                plik.with_extension("poczatek"),
            )?;
            bufor.zablokuj().podlacz_od_czasu(tx, od);
            tory.push(Tor { nazwa, plik, watek });
        }
        let bufory = std::iter::once(wideo).chain(audio.iter().map(|(_, b)| b)).map(Arc::downgrade).collect();
        self.segmenty.push(Segment { obraz, wideo: tor_wideo, audio: tory, bufory });
        Ok(())
    }

    /// Dodaje znacznik w tej chwili (zapisany też na dysku, na wypadek awarii). Zwraca jego numer.
    pub fn dodaj_znacznik(&mut self) -> usize {
        let czas = teraz_100ns();
        self.znaczniki.push(czas);
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.tmp.join(ZNACZNIKI))
            .and_then(|mut f| writeln!(f, "{czas}"));
        self.znaczniki.len()
    }

    /// Ile sekund trwa nagrywanie (razem z przerwami).
    pub fn sekundy(&self) -> u64 {
        self.start.elapsed().as_secs()
    }

    /// Powód, dla którego film trzeba zakończyć teraz (błąd zapisu albo kończy się miejsce).
    pub fn problem(&self) -> Option<String> {
        if let Some(b) = self.wspolne.blad.zablokuj().clone() {
            return Some(b);
        }
        crate::zapis::wolne_miejsce(&self.folder)
            .is_some_and(|w| w < MIEJSCE_NA_KONIEC)
            .then(|| "kończy się miejsce na dysku".to_string())
    }

    /// Odłącza odbiorniki od buforów (wątki dopisują to, co już dostały, i zamykają pliki).
    fn odlacz(&mut self) {
        for s in &mut self.segmenty {
            for b in s.bufory.drain(..).filter_map(|b| b.upgrade()) {
                b.zablokuj().odlacz();
            }
        }
    }

    /// Czeka na wątki zapisujące i zbiera segmenty, w których jest obraz.
    fn zbierz(&mut self) -> Vec<Gotowy> {
        self.odlacz();
        let mut gotowe = Vec::new();
        for s in self.segmenty.drain(..) {
            let wideo = s.wideo.watek.join().unwrap_or(WynikToru { pierwszy: None, ostatni: 0, pakiety: 0 });
            let audio: Vec<_> = s
                .audio
                .into_iter()
                .filter_map(|t| {
                    let w = t.watek.join().ok()?;
                    Some((t.nazwa, t.plik, w.pierwszy?))
                })
                .collect();
            if let Some(poczatek) = wideo.pierwszy.filter(|_| wideo.pakiety > 0) {
                gotowe.push(Gotowy { obraz: s.obraz, wideo: s.wideo.plik, poczatek, koniec: wideo.ostatni, klatki: wideo.pakiety, audio });
            }
        }
        gotowe
    }

    /// Kończy film: zamyka pliki tymczasowe i składa z nich MP4 (może potrwać przy długim nagraniu).
    pub fn zakoncz(mut self) -> Result<Wynik, String> {
        let start = Instant::now();
        let gotowe = self.zbierz();
        if gotowe.is_empty() {
            self.posprzataj();
            return Err("Film jest pusty — nagrywanie nie zdążyło ruszyć".into());
        }
        let sekundy: f64 = gotowe.iter().map(|g| g.klatki as f64 / tempo(g)).sum();
        let szybki_start = self.wspolne.bajty.load(Ordering::Relaxed) < SZYBKI_START_DO;
        let gra = self.gra.as_deref().filter(|g| !g.is_empty());
        let pliki = zloz(&self.folder, &self.tmp, &self.znacznik, gra, &gotowe, &self.znaczniki, szybki_start)?;
        self.posprzataj();
        crate::dziennik::info(&format!(
            "Zapisano film: {} ({:.0} s, części: {}, znaczniki: {}) w {:.1} s",
            pliki[0].display(),
            sekundy,
            pliki.len(),
            self.znaczniki.len(),
            start.elapsed().as_secs_f64()
        ));
        Ok(Wynik { pliki, sekundy })
    }

    /// Usuwa pliki tymczasowe tego filmu.
    fn posprzataj(&self) {
        posprzataj(&self.tmp);
    }
}

/// Usuwa folder plików tymczasowych filmu (i pusty folder .nagrywanie).
fn posprzataj(tmp: &Path) {
    usun_folder(tmp);
    if let Some(rodzic) = tmp.parent() {
        let _ = std::fs::remove_dir(rodzic);
    }
}

fn zapisz_json(plik: &Path, wartosc: &impl serde::Serialize) -> Result<(), String> {
    let tekst = serde_json::to_string(wartosc).map_err(|e| e.to_string())?;
    std::fs::write(plik, tekst).map_err(|e| format!("Nie mogę zapisać pliku filmu: {e}"))
}

/// Składa gotowe segmenty w pliki MP4 w folderze miesiąca: grupa zgodnych segmentów = jeden plik.
/// Wspólne dla zakończonego filmu i filmu odzyskanego po awarii.
fn zloz(folder: &Path, tmp: &Path, znacznik: &str, gra: Option<&str>, gotowe: &[Gotowy], znaczniki: &[i64], szybki_start: bool) -> Result<Vec<PathBuf>, String> {
    let folder_miesiaca = folder.join(crate::zapis::miesiac(znacznik));
    std::fs::create_dir_all(&folder_miesiaca).map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder_miesiaca.display()))?;
    let grupy = grupuj(gotowe);
    let pozycje = pozycje_znacznikow(gotowe, &grupy, znaczniki);
    let mut pliki = Vec::new();
    for (nr, (grupa, znaczniki)) in grupy.into_iter().zip(&pozycje).enumerate() {
        let segmenty = &gotowe[grupa];
        let plik = crate::zapis::nowa_nazwa(&folder_miesiaca, PRZEDROSTEK, znacznik);
        // Jak przy klipach: najpierw plik tymczasowy, nazwa docelowa dopiero po udanym zapisie
        let tymczasowy = plik.with_extension("mp4.tmp");
        let wynik = zloz_grupe(tmp, nr, segmenty, gra, znaczniki, &tymczasowy, szybki_start)
            .and_then(|()| std::fs::rename(&tymczasowy, &plik).map_err(|e| format!("Nie udało się nadać nazwy filmowi: {e}")));
        if let Err(e) = wynik {
            let _ = std::fs::remove_file(&tymczasowy);
            // Pliki tymczasowe zostają (to jedyna kopia nagrania); następny start spróbuje jeszcze raz,
            // a sprzątanie usunie je po kilku dniach
            crate::dziennik::blad(&format!("Nie udało się złożyć filmu z {}: {e}", tmp.display()));
            return Err(if crate::zapis::wolne_miejsce(folder).is_some_and(|w| w < 64 * 1024 * 1024) {
                format!("Za mało miejsca na dysku z folderem {} — zwolnij miejsce", folder.display())
            } else {
                e
            });
        }
        pliki.push(plik);
    }
    Ok(pliki)
}

/// Jeden plik wynikowy z grupy zgodnych segmentów (kilka segmentów: każdy do MP4, potem sklejenie).
/// `znaczniki` to sekundy od początku tego pliku (trafiają do niego jako rozdziały).
fn zloz_grupe(tmp: &Path, nr: usize, segmenty: &[Gotowy], gra: Option<&str>, znaczniki: &[f64], wyjscie: &Path, szybki_start: bool) -> Result<(), String> {
    let rozdzialy = if znaczniki.is_empty() {
        None
    } else {
        let plik = tmp.join(format!("g{nr}-rozdzialy.txt"));
        let dlugosc = segmenty.iter().map(|s| s.klatki as f64 / tempo(s)).sum();
        std::fs::write(&plik, opis_rozdzialow(znaczniki, dlugosc)).map_err(|e| format!("Nie mogę zapisać znaczników filmu: {e}"))?;
        Some(plik)
    };
    if let [jeden] = segmenty {
        return zloz_segment(jeden, gra, rozdzialy.as_deref(), wyjscie, szybki_start);
    }
    let mut lista = String::new();
    for (i, s) in segmenty.iter().enumerate() {
        let czesc = tmp.join(format!("g{nr}-czesc{i}.mp4"));
        zloz_segment(s, None, None, &czesc, false)?;
        lista.push_str(&wpis_listy(&czesc));
    }
    let plik_listy = tmp.join(format!("g{nr}-lista.txt"));
    std::fs::write(&plik_listy, lista).map_err(|e| format!("Nie mogę zapisać listy części filmu: {e}"))?;
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"]).arg(&plik_listy);
    if let Some(r) = &rozdzialy {
        cmd.args(["-f", "ffmetadata", "-i"]).arg(r);
    }
    cmd.args(["-map", "0", "-c", "copy"]);
    if rozdzialy.is_some() {
        cmd.args(["-map_chapters", "1"]);
    }
    opisz_sciezki(&mut cmd, &segmenty[0].nazwy());
    wykonaj(dokoncz(cmd, gra, wyjscie, szybki_start))
}

/// Znaczniki (czasy QPC) jako sekundy w każdym pliku wynikowym (plik = grupa kolejnych segmentów).
/// Znacznik należy do pierwszej grupy, która kończy się po nim, a w grupie do pierwszego segmentu, który
/// kończy się po nim; w przerwie między segmentami (pauza, restart) trafia na początek następnego.
fn pozycje_znacznikow(gotowe: &[Gotowy], grupy: &[std::ops::Range<usize>], znaczniki: &[i64]) -> Vec<Vec<f64>> {
    let mut wynik = vec![Vec::new(); grupy.len()];
    for &t in znaczniki {
        let przed_koncem = |s: &Gotowy| t <= s.koniec + ZAPAS_ZNACZNIKA;
        // Znacznik po końcu ostatniej grupy (np. tuż przed zatrzymaniem) — na koniec ostatniego pliku
        let Some(g) = grupy.iter().position(|g| przed_koncem(&gotowe[g.end - 1])).or(grupy.len().checked_sub(1)) else { continue };
        let segmenty = &gotowe[grupy[g].clone()];
        let mut przesuniecie = 0.0;
        for (i, s) in segmenty.iter().enumerate() {
            let dlugosc = s.klatki as f64 / tempo(s);
            if przed_koncem(s) || i + 1 == segmenty.len() {
                wynik[g].push(przesuniecie + ((t - s.poczatek) as f64 / 1e7).clamp(0.0, dlugosc));
                break;
            }
            przesuniecie += dlugosc;
        }
    }
    for w in &mut wynik {
        w.sort_by(f64::total_cmp);
    }
    wynik
}

/// Rozdziały dla ffmpeg (format FFMETADATA): „Znacznik 1”, „Znacznik 2”… od każdego znacznika do następnego.
/// MP4 trzyma rozdziały jako ścieżkę, która zaczyna się od zera — bez rozdziału „Początek” pierwszy
/// znacznik po odczycie wypadałby na początku filmu.
fn opis_rozdzialow(znaczniki: &[f64], dlugosc: f64) -> String {
    let ms = |s: f64| (s * 1000.0).round() as i64;
    let mut opis = String::from(";FFMETADATA1\n");
    if let Some(&pierwszy) = znaczniki.first().filter(|p| ms(**p) > 0) {
        opis.push_str(&format!("[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND={}\ntitle=Początek\n", ms(pierwszy)));
    }
    for (i, &od) in znaczniki.iter().enumerate() {
        let do_ = znaczniki.get(i + 1).copied().unwrap_or(dlugosc).max(od);
        opis.push_str(&format!("[CHAPTER]\nTIMEBASE=1/1000\nSTART={}\nEND={}\ntitle=Znacznik {}\n", ms(od), ms(do_), i + 1));
    }
    opis
}

/// Wątek zapisujący obraz: zaczyna od klatki kluczowej i ogłasza jej czas (dźwięk zaczyna się od niego).
/// Co POSTEP_CO klatek zapisuje do pliku `postep`, ile klatek i bajtów jest już na pewno na dysku
/// (po awarii programu film składamy dokładnie z tej części).
fn pisz_obraz(
    plik: std::fs::File,
    rx: Receiver<Pakiet>,
    poczatek: Arc<AtomicI64>,
    wspolne: Arc<Wspolne>,
    postep: PathBuf,
) -> Result<JoinHandle<WynikToru>, String> {
    std::thread::Builder::new()
        .name("film-obraz".into())
        .spawn(move || {
            let mut plik = std::io::BufWriter::with_capacity(1 << 20, plik);
            let mut w = WynikToru { pierwszy: None, ostatni: 0, pakiety: 0 };
            let mut bajty = 0u64;
            for p in rx {
                if w.pierwszy.is_none() {
                    if !p.klucz {
                        continue;
                    }
                    w.pierwszy = Some(p.czas);
                    poczatek.store(p.czas, Ordering::SeqCst);
                }
                if !zapisz_pakiet(&mut plik, &p, &wspolne) {
                    return w;
                }
                w.pakiety += 1;
                w.ostatni = p.czas;
                bajty += p.dane.len() as u64;
                if w.pakiety % POSTEP_CO == 1 && plik.flush().is_ok() {
                    zapisz_postep(&postep, &Postep { pierwszy: w.pierwszy.unwrap_or(p.czas), ostatni: w.ostatni, klatki: w.pakiety, bajty });
                }
            }
            zamknij(plik, &wspolne);
            w
        })
        .map_err(|e| format!("Nie udało się uruchomić zapisu filmu: {e}"))
}

/// Wątek zapisujący jedną ścieżkę dźwięku: pomija ramki sprzed pierwszej klatki obrazu. Czas pierwszej
/// zapisanej ramki idzie też do pliku `plik_poczatku` (do odzyskania filmu po awarii).
fn pisz_dzwiek(
    plik: std::fs::File,
    rx: Receiver<Pakiet>,
    poczatek: Arc<AtomicI64>,
    wspolne: Arc<Wspolne>,
    plik_poczatku: PathBuf,
) -> Result<JoinHandle<WynikToru>, String> {
    std::thread::Builder::new()
        .name("film-dzwiek".into())
        .spawn(move || {
            let mut plik = std::io::BufWriter::with_capacity(1 << 16, plik);
            let mut w = WynikToru { pierwszy: None, ostatni: 0, pakiety: 0 };
            // Ramki, które przyszły, zanim obraz ogłosił swój początek (najwyżej kilkanaście sekund)
            let mut czekajace: VecDeque<Pakiet> = VecDeque::new();
            for p in rx {
                let od = poczatek.load(Ordering::SeqCst);
                czekajace.push_back(p);
                if od == 0 {
                    if czekajace.len() > 1000 {
                        czekajace.pop_front();
                    }
                    continue;
                }
                for p in czekajace.drain(..).filter(|p| p.czas >= od) {
                    if !zapisz_pakiet(&mut plik, &p, &wspolne) {
                        return w;
                    }
                    if w.pierwszy.is_none() {
                        w.pierwszy = Some(p.czas);
                        let _ = std::fs::write(&plik_poczatku, p.czas.to_string());
                    }
                    w.pakiety += 1;
                }
            }
            zamknij(plik, &wspolne);
            w
        })
        .map_err(|e| format!("Nie udało się uruchomić zapisu filmu: {e}"))
}

/// Ile obrazu segmentu jest na pewno na dysku (plik s{nr}-postep.txt).
#[derive(Debug, PartialEq)]
struct Postep {
    pierwszy: i64,
    ostatni: i64,
    klatki: u64,
    bajty: u64,
}

impl Postep {
    fn tekst(&self) -> String {
        format!("{} {} {} {}", self.pierwszy, self.ostatni, self.klatki, self.bajty)
    }

    fn z_tekstu(t: &str) -> Option<Postep> {
        let mut l = t.split_whitespace();
        let p = Postep { pierwszy: l.next()?.parse().ok()?, ostatni: l.next()?.parse().ok()?, klatki: l.next()?.parse().ok()?, bajty: l.next()?.parse().ok()? };
        (p.klatki > 0 && p.bajty > 0 && p.ostatni >= p.pierwszy).then_some(p)
    }
}

/// Zapis postępu przez plik pośredni: awaria w trakcie zapisu nie zostawi połowy wiersza.
fn zapisz_postep(plik: &Path, p: &Postep) {
    let nowy = plik.with_extension("nowy");
    if std::fs::write(&nowy, p.tekst()).is_ok() {
        let _ = std::fs::rename(&nowy, plik);
    }
}

/// Dopisuje pakiet; błąd (np. pełny dysk) zapamiętuje dla strażnika i kończy wątek (false).
fn zapisz_pakiet(plik: &mut impl Write, p: &Pakiet, wspolne: &Wspolne) -> bool {
    match plik.write_all(&p.dane) {
        Ok(()) => {
            wspolne.bajty.fetch_add(p.dane.len() as u64, Ordering::Relaxed);
            true
        }
        Err(e) => {
            wspolne.blad.zablokuj().get_or_insert(format!("nie udało się zapisać filmu na dysk ({e})"));
            false
        }
    }
}

fn zamknij(mut plik: impl Write, wspolne: &Wspolne) {
    if let Err(e) = plik.flush() {
        wspolne.blad.zablokuj().get_or_insert(format!("nie udało się zapisać filmu na dysk ({e})"));
    }
}

/// Zakresy kolejnych segmentów, które da się skleić bez kodowania (ten sam obraz i te same ścieżki dźwięku).
fn grupuj(segmenty: &[Gotowy]) -> Vec<std::ops::Range<usize>> {
    let mut grupy: Vec<std::ops::Range<usize>> = Vec::new();
    for (i, s) in segmenty.iter().enumerate() {
        match grupy.last_mut() {
            Some(g) if segmenty[g.start].obraz == s.obraz && segmenty[g.start].nazwy() == s.nazwy() => g.end = i + 1,
            _ => grupy.push(i..i + 1),
        }
    }
    grupy
}

/// Wiersz listy dla `ffmpeg -f concat`: ścieżka w apostrofach, z ukośnikami w przód.
fn wpis_listy(plik: &Path) -> String {
    format!("file '{}'\n", plik.to_string_lossy().replace('\\', "/").replace('\'', r"'\''"))
}

/// Nazwy ścieżek dźwięku w pliku sklejanym z części (sklejanie gubi nazwy nadane częściom).
fn opisz_sciezki(cmd: &mut std::process::Command, nazwy: &[&str]) {
    let tytuly: Vec<&str> = match nazwy.len() {
        0 | 1 => return,
        _ => std::iter::once(crate::zapis::tytul_miksu(nazwy)).chain(nazwy.iter().copied()).collect(),
    };
    for (i, t) in tytuly.iter().enumerate() {
        cmd.args([format!("-metadata:s:a:{i}"), format!("title={t}")])
            .args([format!("-metadata:s:a:{i}"), format!("handler_name={t}")])
            .args([format!("-disposition:a:{i}"), if i == 0 { "default" } else { "0" }.to_string()]);
    }
}

/// Jeden segment -> MP4: obraz bez ponownego kodowania, dźwięk przesunięty względem pierwszej klatki.
/// `rozdzialy` = plik FFMETADATA ze znacznikami (None = bez znaczników).
fn zloz_segment(s: &Gotowy, gra: Option<&str>, rozdzialy: Option<&Path>, wyjscie: &Path, szybki_start: bool) -> Result<(), String> {
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-fflags", "+genpts", "-f", s.obraz.format, "-framerate", &format!("{:.6}", tempo(s)), "-i"])
        .arg(&s.wideo);
    for (_, plik, pierwszy) in &s.audio {
        let przesuniecie = (pierwszy - s.poczatek).max(0) as f64 / 1e7;
        cmd.args(["-itsoffset", &format!("{przesuniecie:.6}"), "-f", "aac", "-i"]).arg(plik);
    }
    if let Some(r) = rozdzialy {
        cmd.args(["-f", "ffmetadata", "-i"]).arg(r);
    }
    cmd.args(["-map", "0:v", "-c:v", "copy"]);
    crate::zapis::mapuj_dzwiek(&mut cmd, &s.nazwy());
    if rozdzialy.is_some() {
        // Plik ze znacznikami jest ostatnim wejściem: za obrazem i ścieżkami dźwięku
        cmd.args(["-map_chapters", &(1 + s.audio.len()).to_string()]);
    }
    wykonaj(dokoncz(cmd, gra, wyjscie, szybki_start))
}

/// Ile klatek na sekundę ma obraz segmentu. Surowy strumień nie niesie czasu klatek, więc ffmpeg rozkłada je
/// równo według tej liczby. Przy klipie (kilkadziesiąt sekund) wystarcza ustawione FPS; przy filmie nawet
/// drobna różnica między zegarem obrazu a zegarem karty dźwiękowej (albo zgubione klatki) po godzinie daje
/// słyszalne rozjechanie obrazu z dźwiękiem. Dlatego długi segment dostaje tempo zmierzone: liczba klatek
/// przez czas, jaki naprawdę minął między pierwszą a ostatnią.
fn tempo(s: &Gotowy) -> f64 {
    let ustawione = s.obraz.fps as f64;
    let sekundy = (s.koniec - s.poczatek) as f64 / 1e7;
    if s.klatki < 2 || sekundy < 60.0 {
        return ustawione;
    }
    let zmierzone = (s.klatki - 1) as f64 / sekundy;
    // Odchyłka o połowę to już nie dryf, tylko coś dziwnego w pomiarze — wtedy ufamy ustawieniu
    if (zmierzone / ustawione - 1.0).abs() > 0.5 { ustawione } else { zmierzone }
}

/// Wspólna końcówka polecenia: nazwa gry w metadanych, szybki start, plik wyjściowy.
fn dokoncz(mut cmd: std::process::Command, gra: Option<&str>, wyjscie: &Path, szybki_start: bool) -> std::process::Command {
    if let Some(g) = gra {
        cmd.args(["-metadata", &format!("title={g}")]);
    }
    if szybki_start {
        cmd.args(["-movflags", "+faststart"]);
    }
    cmd.args(["-f", "mp4"]).arg(wyjscie);
    cmd
}

fn wykonaj(mut cmd: std::process::Command) -> Result<(), String> {
    let wynik = cmd.output().map_err(|e| format!("Nie udało się uruchomić ffmpeg: {e}"))?;
    if wynik.status.success() {
        return Ok(());
    }
    let opis = String::from_utf8_lossy(&wynik.stderr);
    Err(format!("ffmpeg nie złożył filmu: {}", opis.lines().last().unwrap_or("nieznany błąd").trim()))
}

fn usun_folder(folder: &Path) {
    if let Ok(wpisy) = std::fs::read_dir(folder) {
        for w in wpisy.flatten() {
            let _ = std::fs::remove_file(w.path());
        }
    }
    let _ = std::fs::remove_dir(folder);
}

/// Przy starcie programu: pliki tymczasowe po filmach przerwanych awarią albo nieudanym składaniem,
/// starsze niż 3 dni, są usuwane (inaczej zajmowałyby dysk bez końca).
pub fn usun_resztki(folder_klipow: &Path) {
    let tmp = folder_klipow.join(FOLDER_TMP);
    let Ok(wpisy) = std::fs::read_dir(&tmp) else { return };
    for w in wpisy.flatten() {
        let stary = w
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|wiek| wiek > Duration::from_secs(3 * 86_400));
        if stary && w.file_type().is_ok_and(|t| t.is_dir()) {
            crate::dziennik::info(&format!("Usuwam pliki tymczasowe przerwanego filmu: {}", w.path().display()));
            usun_folder(&w.path());
        }
    }
    let _ = std::fs::remove_dir(tmp);
}

// ---------- Odzyskiwanie filmu po awarii ----------

/// Foldery filmów przerwanych awarią programu (albo komputera), które da się złożyć. Woła się przy
/// starcie programu, zanim użytkownik zdąży zacząć nowy film.
pub fn przerwane(folder_klipow: &Path) -> Vec<PathBuf> {
    let Ok(wpisy) = std::fs::read_dir(folder_klipow.join(FOLDER_TMP)) else { return Vec::new() };
    let mut foldery: Vec<PathBuf> = wpisy
        .flatten()
        .map(|w| w.path())
        .filter(|f| f.join(OPIS_FILMU).is_file() && !f.join(PROBA_ODZYSKANIA).exists())
        .collect();
    foldery.sort();
    foldery
}

/// Składa film z plików tymczasowych przerwanego nagrania (to, co zdążyło trafić na dysk) i sprząta je.
/// Gdy się nie uda, pliki zostają (sprzątanie usunie je po kilku dniach), a następny start już nie próbuje.
pub fn odzyskaj(tmp: &Path) -> Result<Wynik, String> {
    let folder = tmp.parent().and_then(Path::parent).ok_or("zła ścieżka plików tymczasowych")?;
    std::fs::write(tmp.join(PROBA_ODZYSKANIA), "").map_err(|e| format!("Nie mogę zapisać w {}: {e}", tmp.display()))?;
    let opis: OpisFilmu = std::fs::read_to_string(tmp.join(OPIS_FILMU))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("brak opisu filmu")?;
    let gotowe = wczytaj_segmenty(tmp);
    if gotowe.is_empty() {
        posprzataj(tmp);
        return Err("w przerwanym filmie nie zdążył się zapisać żaden obraz".into());
    }
    let znaczniki: Vec<i64> =
        std::fs::read_to_string(tmp.join(ZNACZNIKI)).unwrap_or_default().lines().filter_map(|l| l.trim().parse().ok()).collect();
    let rozmiar: u64 = std::fs::read_dir(tmp).map(|w| w.flatten().filter_map(|w| w.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
    let sekundy = gotowe.iter().map(|g| g.klatki as f64 / tempo(g)).sum();
    let gra = opis.gra.as_deref().filter(|g| !g.is_empty());
    let pliki = zloz(folder, tmp, &opis.znacznik, gra, &gotowe, &znaczniki, rozmiar < SZYBKI_START_DO)?;
    posprzataj(tmp);
    crate::dziennik::info(&format!("Odzyskano przerwany film: {} ({sekundy:.0} s, części: {})", pliki[0].display(), pliki.len()));
    Ok(Wynik { pliki, sekundy })
}

/// Segmenty przerwanego filmu z opisów na dysku. Obraz ucinamy do ostatniego zapisanego postępu
/// (za nim może być niepełna klatka); ścieżka dźwięku bez zapisanego początku jest pomijana.
fn wczytaj_segmenty(tmp: &Path) -> Vec<Gotowy> {
    let mut gotowe = Vec::new();
    for nr in 0.. {
        let Ok(tekst) = std::fs::read_to_string(tmp.join(format!("s{nr}.json"))) else { break };
        let Ok(opis) = serde_json::from_str::<OpisSegmentu>(&tekst) else { continue };
        let Some(postep) = std::fs::read_to_string(tmp.join(format!("s{nr}-postep.txt"))).ok().and_then(|t| Postep::z_tekstu(&t)) else {
            continue;
        };
        let wideo = tmp.join(format!("s{nr}-obraz.tmp"));
        let uciete = std::fs::OpenOptions::new()
            .write(true)
            .open(&wideo)
            .and_then(|f| if f.metadata()?.len() >= postep.bajty { f.set_len(postep.bajty) } else { Err(std::io::ErrorKind::UnexpectedEof.into()) });
        if uciete.is_err() {
            continue;
        }
        let Some(format) = ["h264", "hevc", "obu"].into_iter().find(|f| *f == opis.format) else { continue };
        let audio = opis
            .dzwiek
            .iter()
            .enumerate()
            .filter_map(|(i, nazwa)| {
                let plik = tmp.join(format!("s{nr}-dzwiek{i}.aac"));
                let pierwszy = std::fs::read_to_string(plik.with_extension("poczatek")).ok()?.trim().parse().ok()?;
                Some((nazwa_sciezki(nazwa), plik, pierwszy))
            })
            .collect();
        gotowe.push(Gotowy {
            obraz: Obraz {
                format,
                fps: opis.fps,
                rozmiar: (opis.szerokosc, opis.wysokosc),
                // Nazwa enkodera służy tylko do porównania segmentów; odzyskiwanie zdarza się rzadko
                enkoder: Box::leak(opis.enkoder.into_boxed_str()),
            },
            wideo,
            poczatek: postep.pierwszy,
            koniec: postep.ostatni,
            klatki: postep.klatki,
            audio,
        });
    }
    gotowe
}

/// Nazwa ścieżki dźwięku z opisu na dysku (nazwy w filmie są stałe).
fn nazwa_sciezki(nazwa: &str) -> &'static str {
    ["Gra", "Discord i reszta", "Mikrofon"].into_iter().find(|n| *n == nazwa).unwrap_or_else(|| Box::leak(nazwa.into()))
}

// ---------- Zmniejszanie gotowego filmu ----------

pub fn poprawny_format(format: &str) -> bool {
    format == BEZ_ZMIAN || MNIEJSZE.iter().any(|m| m.0 == format)
}

/// Formaty filmu do wyboru w ustawieniach: (wartość, opis). Mniejsze tylko wtedy, gdy karta graficzna
/// umie je zapisać — kodowanie godzin obrazu procesorem trwałoby za długo i przeszkadzało w grze.
pub fn formaty() -> Vec<(String, String)> {
    let mut l = vec![(BEZ_ZMIAN.to_string(), "Pełna jakość — bez zmian (największy plik)".to_string())];
    for (wartosc, kodek, opis, _) in MNIEJSZE {
        if crate::enkodery::sprzetowy_dla(*kodek).is_some() {
            l.push((wartosc.to_string(), opis.to_string()));
        }
    }
    l
}

/// Przerywa trwające zmniejszanie filmu (zamknięcie programu, instalacja aktualizacji).
/// Film zostaje wtedy w pełnej jakości.
pub fn przerwij_zmniejszanie() {
    PRZERWIJ_ZMNIEJSZANIE.store(true, Ordering::SeqCst);
}

pub fn zmniejszanie_przerwane() -> bool {
    PRZERWIJ_ZMNIEJSZANIE.load(Ordering::SeqCst)
}

/// Bitrate obrazu zmniejszonego filmu: część tego, co ma teraz (rozmiar pliku minus dźwięk).
fn bitrate_zmniejszonego(rozmiar: u64, sekundy: f64, sciezki_audio: u32, udzial: f64) -> u64 {
    let caly = rozmiar as f64 * 8.0 / sekundy.max(1.0);
    let obraz = (caly - sciezki_audio as f64 * 160_000.0).max(0.0);
    ((obraz * udzial) as u64).max(NAJMNIEJSZY_BITRATE)
}

/// Rozmiar filmu przed zmniejszeniem i po nim (bajty).
pub struct Zmniejszony {
    pub przed: u64,
    pub po: u64,
}

/// Zapisuje film od nowa w mniejszym formacie (kartą graficzną, w tle) i zastępuje nim oryginał.
/// Dźwięk i nazwy ścieżek zostają bez zmian. `postep` dostaje 0..1; gdy cokolwiek się nie uda,
/// film zostaje w pełnej jakości. Ok(None) = nie było czego zmniejszać.
pub fn zmniejsz(plik: &Path, format: &str, postep: &dyn Fn(f64)) -> Result<Option<Zmniejszony>, String> {
    let Some((_, kodek, _, udzial)) = MNIEJSZE.iter().find(|m| m.0 == format) else { return Ok(None) };
    let enkoder = crate::enkodery::sprzetowy_dla(*kodek)
        .ok_or("Karta graficzna nie umie zapisać tego formatu — film został w pełnej jakości")?;
    let _kolejka = ZMNIEJSZANIE.zablokuj();
    let info = crate::klipy::info(plik)?;
    let przed = std::fs::metadata(plik).map_err(|e| e.to_string())?.len();
    let bps = bitrate_zmniejszonego(przed, info.dlugosc, info.sciezki_audio, *udzial);
    let spodziewany = (bps + info.sciezki_audio as u64 * 160_000) as f64 / 8.0 * info.dlugosc;
    // Nie ma sensu, gdy film już jest tak mały (bardzo niski bitrate w ustawieniach)
    if spodziewany > przed as f64 * 0.85 {
        return Ok(None);
    }
    if let Some(folder) = plik.parent() {
        crate::zapis::sprawdz_miejsce(folder, spodziewany as u64 * 5 / 4 + 64 * 1024 * 1024)?;
    }
    // Do końca pracy porządki i usuwanie nie ruszą filmu
    let _uzywany = crate::klipy::uzywaj(plik);
    let tymczasowy = plik.with_extension("mp4.tmp");
    let tekst = |p: &Path| p.to_string_lossy().into_owned();
    // Dekodowanie też kartą (gdy się nie da, ffmpeg sam przechodzi na procesor)
    let mut a: Vec<String> = ["-hwaccel", "d3d11va", "-i"].map(String::from).to_vec();
    a.push(tekst(plik));
    // Obraz i dźwięk; znaczniki (rozdziały) ffmpeg przenosi sam — `-map 0` dokleiłby ich ścieżkę jako dane
    a.extend(["-map", "0:v", "-map", "0:a", "-c:a", "copy"].map(String::from));
    a.extend(crate::enkodery::argumenty_eksportu(enkoder, bps, false));
    if *kodek == Kodek::Hevc {
        // Z tym znacznikiem HEVC w MP4 odtwarzają też programy Apple
        a.extend(["-tag:v", "hvc1"].map(String::from));
    }
    if (spodziewany as u64) < SZYBKI_START_DO {
        a.extend(["-movflags", "+faststart"].map(String::from));
    }
    a.extend(["-f".to_string(), "mp4".to_string(), tekst(&tymczasowy)]);
    let wynik = crate::edycja::uruchom_z_postepem(crate::ffmpeg::komenda_w_tle(), &a, info.dlugosc, &|p| {
        postep(p);
        !PRZERWIJ_ZMNIEJSZANIE.load(Ordering::SeqCst)
    });
    let po = std::fs::metadata(&tymczasowy).map(|m| m.len()).unwrap_or(0);
    let wynik = wynik.and_then(|()| match po {
        0 => Err("ffmpeg nie zapisał zmniejszonego filmu".to_string()),
        po if po >= przed => Err("zmniejszony film nie wyszedł mniejszy".to_string()),
        _ => Ok(()),
    });
    if let Err(e) = wynik {
        let _ = std::fs::remove_file(&tymczasowy);
        return Err(e);
    }
    // Jak przy „Zastąp oryginał”: oryginał znika dopiero, gdy nowy plik ma już jego nazwę
    let odsuniety = plik.with_extension("mp4.oryginal");
    if let Err(e) = std::fs::rename(plik, &odsuniety) {
        let _ = std::fs::remove_file(&tymczasowy);
        return Err(format!("film jest otwarty w innym programie ({e})"));
    }
    if let Err(e) = std::fs::rename(&tymczasowy, plik) {
        let _ = std::fs::rename(&odsuniety, plik);
        let _ = std::fs::remove_file(&tymczasowy);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_file(&odsuniety);
    Ok(Some(Zmniejszony { przed, po }))
}

#[cfg(test)]
mod testy {
    use super::*;

    fn pakiet(ms: i64, klucz: bool, bajt: u8) -> Pakiet {
        Pakiet { czas: ms * 10_000, klucz, dane: Arc::from(vec![bajt; 2]) }
    }

    fn obraz(format: &'static str) -> Obraz {
        Obraz { format, fps: 60, rozmiar: (1920, 1080), enkoder: "h264_nvenc" }
    }

    fn gotowy(format: &'static str, sciezki: &[&'static str]) -> Gotowy {
        Gotowy {
            obraz: obraz(format),
            wideo: PathBuf::new(),
            poczatek: 0,
            koniec: 0,
            klatki: 1,
            audio: sciezki.iter().map(|n| (*n, PathBuf::new(), 0)).collect(),
        }
    }

    #[test]
    fn zgodne_segmenty_trafiaja_do_jednego_pliku() {
        let s = [
            gotowy("h264", &["Gra", "Mikrofon"]),
            gotowy("h264", &["Gra", "Mikrofon"]),
            // Inny format obrazu albo inne ścieżki dźwięku = osobny plik
            gotowy("hevc", &["Gra", "Mikrofon"]),
            gotowy("hevc", &["Gra"]),
            gotowy("hevc", &["Gra"]),
        ];
        assert_eq!(grupuj(&s), vec![0..2, 2..3, 3..5]);
        assert!(grupuj(&[]).is_empty());
    }

    #[test]
    fn dlugi_segment_dostaje_zmierzone_tempo() {
        let segment = |sekundy: f64, klatki: u64| Gotowy { koniec: (sekundy * 1e7) as i64, klatki, ..gotowy("h264", &["Gra"]) };
        // Krótki segment: ustawione FPS, nawet gdy pomiar wyszedł trochę inaczej
        assert_eq!(tempo(&segment(30.0, 1790)), 60.0);
        // Godzina, w której zgubiło się 0,1% klatek: bez poprawki dźwięk spóźniłby się na końcu o 3,6 s
        let t = tempo(&segment(3600.0, 215_785));
        assert!((t - 59.94).abs() < 0.001, "{t}");
        // Równo 60 klatek na sekundę zostaje przy 60
        assert!((tempo(&segment(600.0, 36_001)) - 60.0).abs() < 1e-9);
        // Pomiar bez sensu (prawie brak klatek): ustawione FPS
        assert_eq!(tempo(&segment(600.0, 100)), 60.0);
    }

    #[test]
    fn zmniejszony_film_dostaje_czesc_bitrate_obrazu() {
        // Godzina przy 20 Mb/s obrazu i trzech ścieżkach dźwięku
        let sekundy = 3600.0;
        let rozmiar = ((20_000_000.0 + 3.0 * 160_000.0) / 8.0 * sekundy) as u64;
        let hevc = bitrate_zmniejszonego(rozmiar, sekundy, 3, 0.45);
        assert!((hevc as i64 - 9_000_000).abs() < 10_000, "{hevc}");
        // Bardzo mały film nie schodzi poniżej dolnej granicy
        assert_eq!(bitrate_zmniejszonego(1_000_000, sekundy, 3, 0.33), NAJMNIEJSZY_BITRATE);
        assert!(poprawny_format(BEZ_ZMIAN) && poprawny_format("hevc") && poprawny_format("av1"));
        assert!(!poprawny_format("mkv"));
    }

    #[test]
    fn lista_czesci_dla_ffmpeg() {
        assert_eq!(wpis_listy(Path::new(r"C:\Wideo\Clipeon\.nagrywanie\a\g0-czesc0.mp4")), "file 'C:/Wideo/Clipeon/.nagrywanie/a/g0-czesc0.mp4'\n");
        assert_eq!(wpis_listy(Path::new(r"C:\Filmy Ani'ego\x.mp4")), "file 'C:/Filmy Ani'\\''ego/x.mp4'\n");
    }

    #[test]
    fn segment_zaczyna_sie_od_klatki_kluczowej_a_dzwiek_od_obrazu() {
        let folder = std::env::temp_dir().join(format!("clipeon-test-film-{}", std::process::id()));
        let mut film = Film::nowy(&folder, None).unwrap();
        let (wideo, gra, mik) = (Bufor::nowy(30), Bufor::nowy(30), Bufor::nowy(30));
        // W buforze jest już obraz: klatka kluczowa w 1000 ms, potem dwie zwykłe; dźwięk od 900 ms co 20 ms
        for (ms, klucz) in [(0, true), (500, false), (1000, true), (1017, false), (1033, false)] {
            wideo.zablokuj().dodaj(pakiet(ms, klucz, 1));
        }
        for i in 0..10 {
            gra.zablokuj().dodaj(pakiet(900 + i * 20, true, 2));
        }
        film.dodaj_segment(obraz("h264"), &wideo, &[("Gra", gra.clone()), ("Mikrofon", mik.clone())]).unwrap();
        // Dalszy ciąg nagrania; mikrofon odzywa się dopiero później
        wideo.zablokuj().dodaj(pakiet(1050, false, 1));
        gra.zablokuj().dodaj(pakiet(1100, true, 2));
        mik.zablokuj().dodaj(pakiet(1200, true, 3));

        let gotowe = film.zbierz();
        assert_eq!(gotowe.len(), 1);
        let g = &gotowe[0];
        assert_eq!(g.poczatek, 1000 * 10_000);
        assert_eq!(g.klatki, 4);
        assert_eq!(std::fs::read(&g.wideo).unwrap().len(), 4 * 2);
        // Dźwięk gry: ramki od 1000 ms (5 z bufora + 1 nowa); mikrofon: jedna ramka w 1200 ms
        assert_eq!(g.nazwy(), vec!["Gra", "Mikrofon"]);
        assert_eq!(g.audio[0].2, 1000 * 10_000);
        assert_eq!(std::fs::read(&g.audio[0].1).unwrap().len(), 6 * 2);
        assert_eq!(g.audio[1].2, 1200 * 10_000);
        // Po zakończeniu bufory działają dalej bez odbiornika
        wideo.zablokuj().dodaj(pakiet(1066, false, 1));
        film.posprzataj();
        assert!(!film.tmp.exists());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Segment od `od` do `do_` sekund (60 FPS, bez dźwięku)
    fn segment(format: &'static str, od: f64, do_: f64) -> Gotowy {
        let klatki = ((do_ - od) * 60.0) as u64 + 1;
        Gotowy { poczatek: (od * 1e7) as i64, koniec: (do_ * 1e7) as i64, klatki, ..gotowy(format, &["Gra"]) }
    }

    #[test]
    fn znaczniki_trafiaja_we_wlasciwe_miejsce_pliku() {
        // Plik 1: dwa segmenty (pauza od 10 do 20 s), plik 2: inny format od 40 s
        let s = [segment("h264", 0.0, 10.0), segment("h264", 20.0, 30.0), segment("hevc", 40.0, 50.0)];
        let grupy = grupuj(&s);
        assert_eq!(grupy, vec![0..2, 2..3]);
        let dl = 601.0 / 60.0;
        let sek = |x: f64| (x * 1e7) as i64;
        let p = pozycje_znacznikow(&s, &grupy, &[5.0, 10.5, 15.0, 25.0, 35.0, 60.0].map(sek));
        let oczekiwane = [vec![5.0, dl, dl, dl + 5.0], vec![0.0, dl]];
        for (wynik, ocz) in p.iter().zip(&oczekiwane) {
            assert_eq!(wynik.len(), ocz.len(), "{p:?}");
            assert!(wynik.iter().zip(ocz).all(|(a, b)| (a - b).abs() < 0.01), "{p:?}");
        }
        // Znacznik spóźniony o ułamek sekundy po końcu segmentu (10,5 s) zostaje na końcu pierwszej części;
        // znacznik w pauzie (15 s) trafia na początek drugiej
        assert!(pozycje_znacznikow(&s, &[], &[sek(1.0)]).is_empty());
    }

    #[test]
    fn rozdzialy_dla_ffmpeg() {
        assert_eq!(
            opis_rozdzialow(&[3.0, 7.25], 10.0),
            ";FFMETADATA1\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=3000\ntitle=Początek\n\
             [CHAPTER]\nTIMEBASE=1/1000\nSTART=3000\nEND=7250\ntitle=Znacznik 1\n\
             [CHAPTER]\nTIMEBASE=1/1000\nSTART=7250\nEND=10000\ntitle=Znacznik 2\n"
        );
        // Znacznik na samym początku: bez osobnego rozdziału „Początek”
        assert!(!opis_rozdzialow(&[0.0], 5.0).contains("Początek"));
    }

    #[test]
    fn postep_zapisu_obrazu() {
        let p = Postep { pierwszy: 10, ostatni: 500, klatki: 121, bajty: 4096 };
        assert_eq!(Postep::z_tekstu(&p.tekst()), Some(p));
        // Uszkodzony albo pusty postęp nie nadaje się do odzyskania
        assert_eq!(Postep::z_tekstu("1 2"), None);
        assert_eq!(Postep::z_tekstu("10 500 0 4096"), None);
        assert_eq!(Postep::z_tekstu(""), None);
    }

    #[test]
    fn przerwany_film_da_sie_odczytac_z_dysku() {
        let folder = std::env::temp_dir().join(format!("clipeon-test-film-awaria-{}", std::process::id()));
        let mut film = Film::nowy(&folder, Some("Gra testowa".into())).unwrap();
        let (wideo, gra, mik) = (Bufor::nowy(30), Bufor::nowy(30), Bufor::nowy(30));
        film.dodaj_segment(obraz("h264"), &wideo, &[("Gra", gra.clone()), ("Mikrofon", mik.clone())]).unwrap();
        // Wątki zapisują w tle — czekamy, aż plik pojawi się na dysku
        let czekaj = |plik: PathBuf| {
            let start = Instant::now();
            while !plik.is_file() && start.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(20));
            }
        };
        wideo.zablokuj().dodaj(pakiet(1000, true, 1));
        // Dźwięk czeka, aż obraz ogłosi swój początek (postęp zapisuje się zaraz po nim)
        czekaj(film.tmp.join("s0-postep.txt"));
        gra.zablokuj().dodaj(pakiet(1000, true, 2));
        czekaj(film.tmp.join("s0-dzwiek0.poczatek"));
        film.dodaj_znacznik();
        // „Awaria”: nikt nie woła zakoncz(); przy następnym starcie folder jest na liście do odzyskania
        assert_eq!(przerwane(&folder), vec![film.tmp.clone()]);
        let segmenty = wczytaj_segmenty(&film.tmp);
        assert_eq!(segmenty.len(), 1);
        let s = &segmenty[0];
        assert_eq!((s.poczatek, s.klatki, s.obraz.clone()), (1000 * 10_000, 1, obraz("h264")));
        // Mikrofon nic nie nagrał (brak początku) — zostaje sama gra
        assert_eq!(s.nazwy(), vec!["Gra"]);
        assert_eq!(std::fs::read(&s.wideo).unwrap().len(), 2);
        assert_eq!(std::fs::read_to_string(film.tmp.join(ZNACZNIKI)).unwrap().lines().count(), 1);
        let opis: OpisFilmu = serde_json::from_str(&std::fs::read_to_string(film.tmp.join(OPIS_FILMU)).unwrap()).unwrap();
        assert_eq!((opis.znacznik.as_str(), opis.gra.as_deref()), (film.znacznik.as_str(), Some("Gra testowa")));
        // Po nieudanej próbie odzyskania folder nie wraca na listę
        std::fs::write(film.tmp.join(PROBA_ODZYSKANIA), "").unwrap();
        assert!(przerwane(&folder).is_empty());
        film.zbierz();
        film.posprzataj();
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn obraz_jest_ucinany_do_zapisanego_postepu() {
        let tmp = std::env::temp_dir().join(format!("clipeon-test-film-postep-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let opis = OpisSegmentu {
            format: "hevc".into(),
            fps: 60,
            szerokosc: 1920,
            wysokosc: 1080,
            enkoder: "hevc_amf".into(),
            dzwiek: vec!["Gra".into(), "Discord i reszta".into()],
        };
        zapisz_json(&tmp.join("s0.json"), &opis).unwrap();
        // Za ostatnim postępem (1000 bajtów) jest niepełna klatka — po odczycie jej nie ma
        std::fs::write(tmp.join("s0-obraz.tmp"), vec![7u8; 1500]).unwrap();
        zapisz_postep(&tmp.join("s0-postep.txt"), &Postep { pierwszy: 5, ostatni: 900, klatki: 241, bajty: 1000 });
        std::fs::write(tmp.join("s0-dzwiek1.aac"), [1, 2, 3]).unwrap();
        std::fs::write(tmp.join("s0-dzwiek1.poczatek"), "7").unwrap();
        // Drugi segment bez postępu (padło przed pierwszą klatką kluczową) jest pomijany
        zapisz_json(&tmp.join("s1.json"), &opis).unwrap();
        let s = wczytaj_segmenty(&tmp);
        assert_eq!(s.len(), 1);
        assert_eq!(std::fs::metadata(&s[0].wideo).unwrap().len(), 1000);
        assert_eq!((s[0].klatki, s[0].koniec, s[0].obraz.enkoder), (241, 900, "hevc_amf"));
        assert_eq!(s[0].nazwy(), vec!["Discord i reszta"]);
        assert_eq!(s[0].audio[0].2, 7);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn segment_bez_obrazu_jest_pomijany() {
        let folder = std::env::temp_dir().join(format!("clipeon-test-film-pusty-{}", std::process::id()));
        let mut film = Film::nowy(&folder, None).unwrap();
        let (wideo, gra) = (Bufor::nowy(30), Bufor::nowy(30));
        film.dodaj_segment(obraz("h264"), &wideo, &[("Gra", gra.clone())]).unwrap();
        // Tylko zwykłe klatki (bez kluczowej) i dźwięk: z tego nie da się zrobić filmu
        wideo.zablokuj().dodaj(pakiet(10, false, 1));
        gra.zablokuj().dodaj(pakiet(10, true, 2));
        assert!(film.zbierz().is_empty());
        film.posprzataj();
        let _ = std::fs::remove_dir_all(&folder);
    }
}
