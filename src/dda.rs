// Droga „DDA przez RAM”: Desktop Duplication prowadzone przez Clipeona, a nie przez filtr ddagrab w ffmpeg.
// Obraz idzie do ffmpeg przez pamięć RAM (jak w trybie zgodności), ale bez Windows Graphics Capture —
// więc bez żółtej ramki na Windows 10. Najważniejsze: gdy Windows odbierze dostęp do obrazu (gra przełącza
// pełny ekran / rozdzielczość, okno UAC), ffmpeg dalej koduje ostatnią klatkę, a Clipeon w tle otwiera
// duplikację od nowa. Nagrywanie się nie restartuje, więc w klipie nie ma przerw.
use crate::Zablokuj;
use crate::wideo::{OstatniaKlatka, Statystyki};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY, DXGI_MODE_ROTATION_UNSPECIFIED, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTDUPL_POINTER_SHAPE_INFO, IDXGIAdapter1,
    IDXGIFactory1, IDXGIOutput, IDXGIOutput1, IDXGIOutput5, IDXGIOutputDuplication, IDXGIResource,
};
use windows::core::Interface;

/// Typy kształtu kursora z DXGI_OUTDUPL_POINTER_SHAPE_TYPE.
const KURSOR_MONOCHROMATYCZNY: u32 = 1;
const KURSOR_KOLOROWY: u32 = 2;
const KURSOR_MASKOWANY: u32 = 4;

/// Po tylu sekundach bez obrazu mówimy strażnikowi, że ekran jest niedostępny (do tego czasu ffmpeg
/// koduje ostatnią klatkę, a my próbujemy odzyskać obraz po cichu).
const BEZ_OBRAZU_ZGLOS_PO: Duration = Duration::from_secs(120);

/// Uchwyt wątku przechwytywania; zniszczenie go zatrzymuje wątek.
pub struct Przechwyt {
    stop: Arc<AtomicBool>,
    watek: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Przechwyt {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.watek.take() {
            let _ = w.join();
        }
    }
}

/// Otwarta duplikacja ekranu z teksturami pomocniczymi.
struct Zrodlo {
    kontekst: ID3D11DeviceContext,
    duplikacja: IDXGIOutputDuplication,
    /// Kopia obrazu na karcie (tania, przy każdej zmianie ekranu)
    kopia: ID3D11Texture2D,
    /// Tekstura do odczytu przez procesor (tylko gdy pora na klatkę)
    odczyt: ID3D11Texture2D,
}

/// Wyjście karty (DXGI) o danej nazwie urządzenia, np. \\.\DISPLAY1, razem z jego kartą.
fn znajdz_wyjscie(nazwa: &str) -> Result<(IDXGIAdapter1, IDXGIOutput), String> {
    unsafe {
        let fabryka = CreateDXGIFactory1::<IDXGIFactory1>().map_err(|e| format!("DXGI: {e}"))?;
        let mut a = 0;
        while let Ok(karta) = fabryka.EnumAdapters1(a) {
            let mut o = 0;
            while let Ok(wyjscie) = karta.EnumOutputs(o) {
                if let Ok(opis) = wyjscie.GetDesc() {
                    let dl = opis.DeviceName.iter().position(|&c| c == 0).unwrap_or(opis.DeviceName.len());
                    if String::from_utf16_lossy(&opis.DeviceName[..dl]).eq_ignore_ascii_case(nazwa) {
                        return Ok((karta, wyjscie));
                    }
                }
                o += 1;
            }
            a += 1;
        }
    }
    Err(format!("nie ma ekranu {nazwa} na żadnej karcie"))
}

fn tekstura(urzadzenie: &ID3D11Device, (w, h): (u32, u32), do_odczytu: bool) -> Result<ID3D11Texture2D, String> {
    let opis = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: if do_odczytu { D3D11_USAGE_STAGING } else { D3D11_USAGE_DEFAULT },
        BindFlags: 0,
        CPUAccessFlags: if do_odczytu { D3D11_CPU_ACCESS_READ.0 as u32 } else { 0 },
        MiscFlags: 0,
    };
    let mut t = None;
    unsafe { urzadzenie.CreateTexture2D(&opis, None, Some(&mut t)) }.map_err(|e| format!("tekstura: {e}"))?;
    t.ok_or_else(|| "tekstura: brak".to_string())
}

/// Otwiera Desktop Duplication ekranu. Obraz zawsze w BGRA 8 bit (Windows przelicza HDR / 10 bit sam,
/// jeśli umie — DuplicateOutput1); obrócony ekran i inny rozmiar niż `rozmiar` to błąd.
fn otworz(nazwa: &str, rozmiar: (u32, u32)) -> Result<Zrodlo, String> {
    let (karta, wyjscie) = znajdz_wyjscie(nazwa)?;
    let (mut urzadzenie, mut kontekst) = (None, None);
    unsafe {
        D3D11CreateDevice(
            &karta,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut urzadzenie),
            None,
            Some(&mut kontekst),
        )
    }
    .map_err(|e| format!("Direct3D: {e}"))?;
    let (urzadzenie, kontekst) = urzadzenie.zip(kontekst).ok_or("Direct3D: brak urządzenia")?;
    let duplikacja = unsafe {
        let nowa = wyjscie
            .cast::<IDXGIOutput5>()
            .ok()
            .and_then(|o5| o5.DuplicateOutput1(&urzadzenie, 0, &[DXGI_FORMAT_B8G8R8A8_UNORM]).ok());
        match nowa {
            Some(d) => d,
            None => wyjscie
                .cast::<IDXGIOutput1>()
                .map_err(|e| format!("DXGI 1.2: {e}"))?
                .DuplicateOutput(&urzadzenie)
                .map_err(|e| format!("DuplicateOutput: {e}"))?,
        }
    };
    let opis = unsafe { duplikacja.GetDesc() };
    if opis.Rotation != DXGI_MODE_ROTATION_IDENTITY && opis.Rotation != DXGI_MODE_ROTATION_UNSPECIFIED {
        return Err("ekran jest obrócony (pionowo) — tej drogi nie używamy".into());
    }
    if opis.ModeDesc.Format != DXGI_FORMAT_B8G8R8A8_UNORM {
        return Err(format!("nieobsługiwany format obrazu ekranu ({:?})", opis.ModeDesc.Format));
    }
    let teraz = (opis.ModeDesc.Width, opis.ModeDesc.Height);
    if teraz != rozmiar {
        return Err(format!("rozdzielczość ekranu zmieniła się na {}x{}", teraz.0, teraz.1));
    }
    Ok(Zrodlo { kopia: tekstura(&urzadzenie, rozmiar, false)?, odczyt: tekstura(&urzadzenie, rozmiar, true)?, kontekst, duplikacja })
}

/// Kursor do dorysowania (Desktop Duplication daje obraz bez kursora).
#[derive(Default)]
struct Kursor {
    ksztalt: Vec<u8>,
    info: DXGI_OUTDUPL_POINTER_SHAPE_INFO,
    widoczny: bool,
    pozycja: (i32, i32),
}

/// Startuje przechwytywanie ekranu `nazwa` (rozmiar = rozmiar ekranu). Pierwsze otwarcie jest od razu —
/// błąd (np. obrócony ekran) wraca tutaj i Clipeon bierze następną drogę.
pub fn uruchom(
    nazwa: &str,
    rozmiar: (u32, u32),
    fps: u32,
    wspolna: Arc<Mutex<OstatniaKlatka>>,
    statystyki: Arc<Statystyki>,
    zglos: impl Fn(String) + Send + 'static,
) -> Result<Przechwyt, String> {
    // Otwieramy w wątku przechwytywania (obiekty COM zostają w nim), ale na wynik pierwszej próby czekamy
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let stop = Arc::new(AtomicBool::new(false));
    let nazwa = nazwa.to_string();
    let stop_w = stop.clone();
    let watek = std::thread::Builder::new()
        .name("wideo-dda".into())
        .spawn(move || {
            let zrodlo = match otworz(&nazwa, rozmiar) {
                Ok(z) => {
                    let _ = tx.send(Ok(()));
                    z
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            };
            petla(zrodlo, &nazwa, rozmiar, fps, &wspolna, &statystyki, &stop_w, &zglos);
        })
        .map_err(|e| format!("wątek przechwytywania: {e}"))?;
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(())) => Ok(Przechwyt { stop, watek: Some(watek) }),
        Ok(Err(e)) => {
            let _ = watek.join();
            Err(e)
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            Err("Desktop Duplication nie odpowiada".into())
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn petla(
    zrodlo: Zrodlo,
    nazwa: &str,
    rozmiar: (u32, u32),
    fps: u32,
    wspolna: &Mutex<OstatniaKlatka>,
    statystyki: &Statystyki,
    stop: &AtomicBool,
    zglos: &dyn Fn(String),
) {
    let (w, h) = rozmiar;
    let okres = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
    let czekaj_ms = (okres.as_millis() as u32).clamp(5, 50);
    let mut zrodlo = Some(zrodlo);
    let mut czysty = vec![0u8; (w * h * 4) as usize];
    let mut wlasna = vec![0u8; (w * h * 4) as usize];
    let mut kursor = Kursor::default();
    // Obraz na karcie jest nowszy niż `czysty` / kursor zmienił się od ostatniej klatki
    let (mut brudny, mut zmiana) = (false, false);
    let mut ostatnia: Option<Instant> = None;
    // Od kiedy nie mamy obrazu (utracony dostęp) i czy już to zgłosiliśmy
    let mut bez_obrazu: Option<Instant> = None;
    let mut zgloszono = false;

    while !stop.load(Ordering::SeqCst) {
        let Some(z) = &zrodlo else {
            // Odzyskiwanie po utracie dostępu: nowa duplikacja co 100 ms, po 5 s co sekundę (ekran blokady
            // potrafi trwać długo); ffmpeg koduje w tym czasie ostatnią klatkę
            let dlugo = bez_obrazu.is_some_and(|t| t.elapsed() > Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(if dlugo { 1000 } else { 100 }));
            match otworz(nazwa, rozmiar) {
                Ok(nowe) => {
                    let ile = bez_obrazu.take().map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0);
                    crate::dziennik::info(&format!("Desktop Duplication: obraz wrócił po {ile:.1} s — nagrywanie trwa bez przerwy"));
                    zrodlo = Some(nowe);
                    zgloszono = false;
                }
                Err(e) if e.starts_with("rozdzielczość") || e.starts_with("ekran jest obrócony") || e.starts_with("nie ma ekranu") => {
                    // Inny rozmiar ekranu: enkoder ma stały rozmiar, więc nagrywanie musi ruszyć od nowa
                    zglos(e);
                    return;
                }
                Err(e) => {
                    if !zgloszono && bez_obrazu.is_some_and(|t| t.elapsed() >= BEZ_OBRAZU_ZGLOS_PO) {
                        zgloszono = true;
                        zglos(format!("chwilowa utrata obrazu z ekranu ({e})"));
                    }
                }
            }
            continue;
        };

        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut zasob: Option<IDXGIResource> = None;
        match unsafe { z.duplikacja.AcquireNextFrame(czekaj_ms, &mut info, &mut zasob) } {
            Ok(()) => {
                if info.LastPresentTime != 0
                    && let Some(t) = zasob.as_ref().and_then(|r| r.cast::<ID3D11Texture2D>().ok())
                {
                    unsafe { z.kontekst.CopyResource(&z.kopia, &t) };
                    brudny = true;
                }
                if info.LastMouseUpdateTime != 0 {
                    kursor.widoczny = info.PointerPosition.Visible.as_bool();
                    kursor.pozycja = (info.PointerPosition.Position.x, info.PointerPosition.Position.y);
                    zmiana = true;
                }
                if info.PointerShapeBufferSize > 0 {
                    kursor.ksztalt.resize(info.PointerShapeBufferSize as usize, 0);
                    let mut potrzeba = 0u32;
                    let mut nowe = DXGI_OUTDUPL_POINTER_SHAPE_INFO::default();
                    let ok = unsafe {
                        z.duplikacja.GetFramePointerShape(
                            kursor.ksztalt.len() as u32,
                            kursor.ksztalt.as_mut_ptr().cast(),
                            &mut potrzeba,
                            &mut nowe,
                        )
                    };
                    if ok.is_ok() {
                        kursor.info = nowe;
                    } else {
                        kursor.ksztalt.clear();
                    }
                    zmiana = true;
                }
                drop(zasob);
                let _ = unsafe { z.duplikacja.ReleaseFrame() };
            }
            Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => {}
            Err(e) => {
                // Utrata dostępu (zmiana trybu ekranu, UAC, blokada) albo karty: otwieramy od nowa w tle
                crate::dziennik::info(&format!("Desktop Duplication: obraz chwilowo niedostępny ({e}) — odzyskuję bez przerywania nagrywania"));
                zrodlo = None;
                // Skopiowany, a jeszcze nieodczytany obraz był w teksturze starej duplikacji. Nowa ma pustą
                // teksturę — bez tego pierwsza klatka po powrocie mogłaby być czarna
                brudny = false;
                bez_obrazu.get_or_insert_with(Instant::now);
                continue;
            }
        }

        // Klatka nie częściej niż nasze FPS (wątek taktujący i tak wysyła równo, powtarzając ostatnią)
        let pora = ostatnia.is_none_or(|t| t.elapsed() >= okres.mul_f64(0.9));
        if !(brudny || zmiana) || !pora {
            continue;
        }
        if brudny {
            if let Err(e) = odczytaj(z, &mut czysty, rozmiar) {
                crate::dziennik::info(&format!("Desktop Duplication: nie udało się odczytać obrazu ({e}) — odzyskuję"));
                zrodlo = None;
                brudny = false;
                bez_obrazu.get_or_insert_with(Instant::now);
                continue;
            }
            brudny = false;
        }
        zmiana = false;
        wlasna.copy_from_slice(&czysty);
        if kursor.widoczny && !kursor.ksztalt.is_empty() {
            let i = &kursor.info;
            narysuj_kursor(&mut wlasna, rozmiar, &kursor.ksztalt, i.Type, (i.Width, i.Height, i.Pitch), kursor.pozycja);
        }
        {
            let mut g = wspolna.zablokuj();
            std::mem::swap(&mut g.piksele, &mut wlasna);
            g.nowa = true;
        }
        statystyki.przechwycone.fetch_add(1, Ordering::Relaxed);
        ostatnia = Some(Instant::now());
    }
}

/// Kopiuje obraz z karty do `cel` (BGRA, wiersz po wierszu — wiersz tekstury bywa dłuższy).
fn odczytaj(z: &Zrodlo, cel: &mut [u8], (w, h): (u32, u32)) -> Result<(), String> {
    unsafe {
        z.kontekst.CopyResource(&z.odczyt, &z.kopia);
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        z.kontekst.Map(&z.odczyt, 0, D3D11_MAP_READ, 0, Some(&mut m)).map_err(|e| e.to_string())?;
        let wiersz = (w * 4) as usize;
        let zrodlo = m.pData as *const u8;
        for y in 0..h as usize {
            let src = std::slice::from_raw_parts(zrodlo.add(y * m.RowPitch as usize), wiersz);
            cel[y * wiersz..(y + 1) * wiersz].copy_from_slice(src);
        }
        z.kontekst.Unmap(&z.odczyt, 0);
    }
    Ok(())
}

/// Dorysowuje kursor na obrazie BGRA. `rozmiar_ksztaltu` = (szerokość, wysokość, bajtów na wiersz) z DXGI;
/// `poz` = lewy górny róg kształtu na ekranie. Trzy rodzaje kursora jak w DXGI: monochromatyczny (maski
/// AND i XOR po 1 bicie), kolorowy (BGRA z przezroczystością) i maskowany (alfa 0 = kolor, 0xFF = XOR).
pub fn narysuj_kursor(obraz: &mut [u8], (w, h): (u32, u32), ksztalt: &[u8], typ: u32, rozmiar_ksztaltu: (u32, u32, u32), poz: (i32, i32)) {
    let (kw, kh, pitch) = (rozmiar_ksztaltu.0 as i32, rozmiar_ksztaltu.1 as i32, rozmiar_ksztaltu.2 as usize);
    let kh = if typ == KURSOR_MONOCHROMATYCZNY { kh / 2 } else { kh };
    for y in 0..kh {
        let ey = poz.1 + y;
        if ey < 0 || ey >= h as i32 {
            continue;
        }
        for x in 0..kw {
            let ex = poz.0 + x;
            if ex < 0 || ex >= w as i32 {
                continue;
            }
            let i = ((ey as u32 * w + ex as u32) * 4) as usize;
            let Some(px) = obraz.get_mut(i..i + 3) else { continue };
            match typ {
                KURSOR_MONOCHROMATYCZNY => {
                    let bit = |wiersz: i32| {
                        let b = ksztalt.get(wiersz as usize * pitch + (x / 8) as usize).copied().unwrap_or(0);
                        (b >> (7 - (x % 8))) & 1 == 1
                    };
                    let (and, xor) = (bit(y), bit(y + kh));
                    for c in px.iter_mut() {
                        if !and {
                            *c = 0;
                        }
                        if xor {
                            *c ^= 0xFF;
                        }
                    }
                }
                KURSOR_KOLOROWY | KURSOR_MASKOWANY => {
                    let j = y as usize * pitch + x as usize * 4;
                    let Some(k) = ksztalt.get(j..j + 4) else { continue };
                    let a = k[3] as u32;
                    if typ == KURSOR_MASKOWANY {
                        if a == 0 {
                            px.copy_from_slice(&k[..3]);
                        } else {
                            for (c, s) in px.iter_mut().zip(&k[..3]) {
                                *c ^= s;
                            }
                        }
                    } else {
                        for (c, s) in px.iter_mut().zip(&k[..3]) {
                            *c = ((*s as u32 * a + *c as u32 * (255 - a)) / 255) as u8;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn kolorowy_kursor_z_przezroczystoscia() {
        let mut obraz = vec![100u8; 4 * 4 * 4];
        // Kursor 2×1: piksel nieprzezroczysty biały i całkiem przezroczysty
        let ksztalt = [255, 255, 255, 255, 0, 0, 0, 0];
        narysuj_kursor(&mut obraz, (4, 4), &ksztalt, KURSOR_KOLOROWY, (2, 1, 8), (1, 1));
        let p = |x: usize, y: usize| &obraz[(y * 4 + x) * 4..(y * 4 + x) * 4 + 3];
        assert_eq!(p(1, 1), &[255, 255, 255]);
        assert_eq!(p(2, 1), &[100, 100, 100]);
        assert_eq!(p(0, 0), &[100, 100, 100]);
    }

    #[test]
    fn kursor_przy_krawedzi_nie_wychodzi_poza_obraz() {
        let mut obraz = vec![0u8; 2 * 2 * 4];
        let ksztalt = vec![255u8; 4 * 4 * 4];
        narysuj_kursor(&mut obraz, (2, 2), &ksztalt, KURSOR_KOLOROWY, (4, 4, 16), (1, -1));
        assert_eq!(&obraz[4..7], &[255, 255, 255]);
        assert_eq!(&obraz[0..3], &[0, 0, 0]);
    }

    #[test]
    fn monochromatyczny_kursor_maski_and_xor() {
        let mut obraz = vec![0x40u8; 4 * 4];
        // Szerokość 4, wysokość 2 (1 wiersz AND + 1 wiersz XOR), 1 bajt na wiersz.
        // AND: 0011 (dwa pierwsze piksele czyszczone), XOR: 0101
        let ksztalt = [0b0011_0000, 0b0101_0000];
        narysuj_kursor(&mut obraz, (4, 1), &ksztalt, KURSOR_MONOCHROMATYCZNY, (4, 2, 1), (0, 0));
        let p = |x: usize| obraz[x * 4];
        assert_eq!(p(0), 0x00); // AND 0, XOR 0 -> czarny
        assert_eq!(p(1), 0xFF); // AND 0, XOR 1 -> biały
        assert_eq!(p(2), 0x40); // AND 1, XOR 0 -> bez zmian
        assert_eq!(p(3), 0x40 ^ 0xFF); // AND 1, XOR 1 -> odwrócony
    }
}
