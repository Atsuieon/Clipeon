// Długie nagrywanie („film”): ten sam zakodowany obraz i dźwięk, które idą do bufora powtórki,
// są dodatkowo dopisywane do plików tymczasowych na dysku. Po zatrzymaniu ffmpeg składa z nich MP4
// bez ponownego kodowania obrazu. Nie ma drugiego enkodera — film kosztuje tylko miejsce na dysku.
// Każdy restart sesji nagrywania (gra zmieniła tryb ekranu, pauza) zaczyna nowy segment; segmenty
// o tych samych parametrach są na końcu sklejane w jeden plik, a inne dają kolejne pliki.
use crate::Zablokuj;
use crate::bufor::{Bufor, Pakiet, WspolnyBufor, teraz_100ns};
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
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
/// Do tego rozmiaru film dostaje „szybki start” (nagłówek na początku pliku — wygodne do wysyłania);
/// większe pliki musiałyby być przepisywane drugi raz
const SZYBKI_START_DO: u64 = 2_000_000_000;

/// Parametry obrazu jednej sesji nagrywania (segmenty o tych samych da się skleić bez kodowania).
#[derive(Clone, PartialEq, Debug)]
pub struct Obraz {
    /// Format surowego strumienia dla ffmpeg (h264 / hevc / obu)
    pub format: &'static str,
    pub fps: u32,
    pub rozmiar: (u32, u32),
    pub enkoder: &'static str,
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
        Ok(Film { folder: folder.to_path_buf(), tmp, znacznik, start: Instant::now(), gra, segmenty: Vec::new(), wspolne: Arc::default() })
    }

    /// Podłącza film do buforów nowej sesji nagrywania (nowy segment).
    pub fn dodaj_segment(&mut self, obraz: Obraz, wideo: &WspolnyBufor, audio: &[(&'static str, WspolnyBufor)]) -> Result<(), String> {
        let nr = self.segmenty.len();
        let poczatek = Arc::new(AtomicI64::new(0));
        let blad_pliku = |e: std::io::Error| format!("Nie mogę zapisać pliku filmu: {e}");

        let plik_wideo = self.tmp.join(format!("s{nr}-obraz.tmp"));
        let (tx, rx) = channel();
        let watek = pisz_obraz(std::fs::File::create(&plik_wideo).map_err(blad_pliku)?, rx, poczatek.clone(), self.wspolne.clone())?;
        // Film zaczyna się od ostatniej klatki kluczowej w buforze (najwyżej sekundę wstecz)
        let od = wideo.zablokuj().podlacz_od_klucza(tx).unwrap_or_else(teraz_100ns);
        let tor_wideo = Tor { nazwa: "", plik: plik_wideo, watek };

        let mut tory = Vec::new();
        for (i, (nazwa, bufor)) in audio.iter().enumerate() {
            let plik = self.tmp.join(format!("s{nr}-dzwiek{i}.aac"));
            let (tx, rx) = channel();
            let watek = pisz_dzwiek(std::fs::File::create(&plik).map_err(blad_pliku)?, rx, poczatek.clone(), self.wspolne.clone())?;
            bufor.zablokuj().podlacz_od_czasu(tx, od);
            tory.push(Tor { nazwa, plik, watek });
        }
        let bufory = std::iter::once(wideo).chain(audio.iter().map(|(_, b)| b)).map(Arc::downgrade).collect();
        self.segmenty.push(Segment { obraz, wideo: tor_wideo, audio: tory, bufory });
        Ok(())
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
        let folder_miesiaca = self.folder.join(crate::zapis::miesiac(&self.znacznik));
        std::fs::create_dir_all(&folder_miesiaca).map_err(|e| format!("Nie mogę utworzyć {}: {e}", folder_miesiaca.display()))?;

        let mut pliki = Vec::new();
        for (nr, grupa) in grupuj(&gotowe).into_iter().enumerate() {
            let segmenty = &gotowe[grupa];
            let plik = crate::zapis::nowa_nazwa(&folder_miesiaca, PRZEDROSTEK, &self.znacznik);
            // Jak przy klipach: najpierw plik tymczasowy, nazwa docelowa dopiero po udanym zapisie
            let tymczasowy = plik.with_extension("mp4.tmp");
            let wynik = self.zloz_grupe(nr, segmenty, &tymczasowy, szybki_start).and_then(|()| {
                std::fs::rename(&tymczasowy, &plik).map_err(|e| format!("Nie udało się nadać nazwy filmowi: {e}"))
            });
            if let Err(e) = wynik {
                let _ = std::fs::remove_file(&tymczasowy);
                // Pliki tymczasowe zostają (to jedyna kopia nagrania); sprzątanie usunie je po kilku dniach
                crate::dziennik::blad(&format!("Nie udało się złożyć filmu z {}: {e}", self.tmp.display()));
                return Err(if crate::zapis::wolne_miejsce(&self.folder).is_some_and(|w| w < 64 * 1024 * 1024) {
                    format!("Za mało miejsca na dysku z folderem {} — zwolnij miejsce", self.folder.display())
                } else {
                    e
                });
            }
            pliki.push(plik);
        }
        self.posprzataj();
        crate::dziennik::info(&format!(
            "Zapisano film: {} ({:.0} s, części: {}) w {:.1} s",
            pliki[0].display(),
            sekundy,
            pliki.len(),
            start.elapsed().as_secs_f64()
        ));
        Ok(Wynik { pliki, sekundy })
    }

    /// Jeden plik wynikowy z grupy zgodnych segmentów (kilka segmentów: każdy do MP4, potem sklejenie).
    fn zloz_grupe(&self, nr: usize, segmenty: &[Gotowy], wyjscie: &Path, szybki_start: bool) -> Result<(), String> {
        let gra = self.gra.as_deref().filter(|g| !g.is_empty());
        if let [jeden] = segmenty {
            return zloz_segment(jeden, gra, wyjscie, szybki_start);
        }
        let mut lista = String::new();
        for (i, s) in segmenty.iter().enumerate() {
            let czesc = self.tmp.join(format!("g{nr}-czesc{i}.mp4"));
            zloz_segment(s, None, &czesc, false)?;
            lista.push_str(&wpis_listy(&czesc));
        }
        let plik_listy = self.tmp.join(format!("g{nr}-lista.txt"));
        std::fs::write(&plik_listy, lista).map_err(|e| format!("Nie mogę zapisać listy części filmu: {e}"))?;
        let mut cmd = crate::ffmpeg::komenda();
        cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i"]).arg(&plik_listy);
        cmd.args(["-map", "0", "-c", "copy"]);
        opisz_sciezki(&mut cmd, &segmenty[0].nazwy());
        wykonaj(dokoncz(cmd, gra, wyjscie, szybki_start))
    }

    /// Usuwa pliki tymczasowe tego filmu.
    fn posprzataj(&self) {
        usun_folder(&self.tmp);
        // Pusty folder .nagrywanie też znika
        if let Some(rodzic) = self.tmp.parent() {
            let _ = std::fs::remove_dir(rodzic);
        }
    }
}

/// Wątek zapisujący obraz: zaczyna od klatki kluczowej i ogłasza jej czas (dźwięk zaczyna się od niego).
fn pisz_obraz(plik: std::fs::File, rx: Receiver<Pakiet>, poczatek: Arc<AtomicI64>, wspolne: Arc<Wspolne>) -> Result<JoinHandle<WynikToru>, String> {
    std::thread::Builder::new()
        .name("film-obraz".into())
        .spawn(move || {
            let mut plik = std::io::BufWriter::with_capacity(1 << 20, plik);
            let mut w = WynikToru { pierwszy: None, ostatni: 0, pakiety: 0 };
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
            }
            zamknij(plik, &wspolne);
            w
        })
        .map_err(|e| format!("Nie udało się uruchomić zapisu filmu: {e}"))
}

/// Wątek zapisujący jedną ścieżkę dźwięku: pomija ramki sprzed pierwszej klatki obrazu.
fn pisz_dzwiek(plik: std::fs::File, rx: Receiver<Pakiet>, poczatek: Arc<AtomicI64>, wspolne: Arc<Wspolne>) -> Result<JoinHandle<WynikToru>, String> {
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
                    w.pierwszy.get_or_insert(p.czas);
                    w.pakiety += 1;
                }
            }
            zamknij(plik, &wspolne);
            w
        })
        .map_err(|e| format!("Nie udało się uruchomić zapisu filmu: {e}"))
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
        _ => std::iter::once("Gra + mikrofon").chain(nazwy.iter().copied()).collect(),
    };
    for (i, t) in tytuly.iter().enumerate() {
        cmd.args([format!("-metadata:s:a:{i}"), format!("title={t}")])
            .args([format!("-metadata:s:a:{i}"), format!("handler_name={t}")])
            .args([format!("-disposition:a:{i}"), if i == 0 { "default" } else { "0" }.to_string()]);
    }
}

/// Jeden segment -> MP4: obraz bez ponownego kodowania, dźwięk przesunięty względem pierwszej klatki.
fn zloz_segment(s: &Gotowy, gra: Option<&str>, wyjscie: &Path, szybki_start: bool) -> Result<(), String> {
    let mut cmd = crate::ffmpeg::komenda();
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-fflags", "+genpts", "-f", s.obraz.format, "-framerate", &format!("{:.6}", tempo(s)), "-i"])
        .arg(&s.wideo);
    for (_, plik, pierwszy) in &s.audio {
        let przesuniecie = (pierwszy - s.poczatek).max(0) as f64 / 1e7;
        cmd.args(["-itsoffset", &format!("{przesuniecie:.6}"), "-f", "aac", "-i"]).arg(plik);
    }
    cmd.args(["-map", "0:v", "-c:v", "copy"]);
    crate::zapis::mapuj_dzwiek(&mut cmd, &s.nazwy());
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
