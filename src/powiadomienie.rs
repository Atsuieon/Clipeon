// Dymek w prawym górnym rogu monitora z grą: natywne okno warstwowe (UpdateLayeredWindow) rysowane
// w pamięci. Po zapisie klipu pokazuje kadr z niego, jego długość i nazwę gry; pozostałe dymki
// (błędy, alarm, aktualizacja) to tytuł z opisem i kropką stanu. Wygląd liczy rysunek_dymka.rs. Bez WebView — lekkie i niezawodne nad grą:
// zawsze na wierzchu, bez aktywacji (gra nie traci fokusu), przezroczyste dla myszy,
// pomijane przez menedżery okien kafelkowych (styl okna-narzędzia).
pub use crate::rysunek_dymka::{Kadr, czas, rozmiar_kadru};
use crate::rysunek_dymka::{Prostokat, Uklad, rysuj, rysuj_pasek};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS,
    DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW,
    GetDC, GetTextExtentPoint32W, GetTextFaceW, HDC, HGDIOBJ, OUT_DEFAULT_PRECIS, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
    RegisterClassW, SW_SHOWNOACTIVATE, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, ShowWindow, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow, WNDCLASSW,
    WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

/// Numer bieżącego dymka; starszy znika, gdy pojawi się nowszy.
static NUMER: AtomicU64 = AtomicU64::new(0);
/// Numer klikniętego dymka (0 = żaden)
static KLIKNIETY: AtomicU64 = AtomicU64::new(0);

/// Ostatnio zamówiony dymek. Dymek po zapisie czeka na kadr z klipu (ffmpeg), więc w tym czasie może
/// przyjść nowszy (kolejny zapis, błąd) — wtedy spóźniony nie wyskakuje już nad nim.
static ZAMOWIONY: AtomicU64 = AtomicU64::new(0);

/// Chowa bieżący dymek (np. alarm, gdy problem minął).
pub fn ukryj() {
    ZAMOWIONY.fetch_add(1, Ordering::SeqCst);
    NUMER.fetch_add(1, Ordering::SeqCst);
}

/// Rezerwuje miejsce w kolejce dymków; wywołać od razu przy zdarzeniu, zanim zacznie się coś długiego.
pub fn zamow() -> u64 {
    ZAMOWIONY.fetch_add(1, Ordering::SeqCst) + 1
}

/// Czy zamówiony dymek jest wciąż najnowszy (nic po nim nie zamówiono ani nie schowano)?
pub fn aktualny(zamowienie: u64) -> bool {
    ZAMOWIONY.load(Ordering::SeqCst) == zamowienie
}

/// Krój napisów: „Segoe UI Variable Text” z Windows 11, a gdzie go nie ma (Windows 10) — zwykłe
/// „Segoe UI”. Bez tego GDI podstawia jakiś inny krój i napisy wychodzą krzywe albo za szerokie.
fn kroj(dc: HDC) -> windows::core::PCWSTR {
    static WINDOWS_11: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let jest = *WINDOWS_11.get_or_init(|| unsafe {
        let f = CreateFontW(
            -12, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, ANTIALIASED_QUALITY, 0,
            w!("Segoe UI Variable Text"),
        );
        let stara = SelectObject(dc, HGDIOBJ(f.0));
        let mut nazwa = [0u16; 64];
        let n = GetTextFaceW(dc, Some(&mut nazwa)).max(0) as usize;
        SelectObject(dc, stara);
        let _ = DeleteObject(HGDIOBJ(f.0));
        String::from_utf16_lossy(&nazwa[..n.min(nazwa.len())]).trim_end_matches('\0').eq_ignore_ascii_case("Segoe UI Variable Text")
    });
    if jest { w!("Segoe UI Variable Text") } else { w!("Segoe UI") }
}

/// Prawy górny róg monitora (piksele fizyczne) i skala DPI.
pub struct Miejsce {
    pub prawa: i32,
    pub gora: i32,
    pub skala: f32,
}

unsafe extern "system" fn procedura(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // Kliknięcie w dymek (tylko gdy dymek jest klikalny — inaczej mysz przechodzi do gry)
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP {
        KLIKNIETY.store(NUMER.load(Ordering::SeqCst), Ordering::SeqCst);
        return LRESULT(0);
    }
    // Kliknięcie nie aktywuje okna (gra nie traci fokusu, dopóki użytkownik sam nie otworzy Clipeona)
    if msg == windows::Win32::UI::WindowsAndMessaging::WM_MOUSEACTIVATE {
        return LRESULT(windows::Win32::UI::WindowsAndMessaging::MA_NOACTIVATE as isize);
    }
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

fn szeroki(t: &str) -> Vec<u16> {
    t.encode_utf16().collect()
}

/// Pokazuje dymek (blokuje bieżący wątek do jego zniknięcia — wywoływać z osobnego wątku).
/// `trwale` = wisi, dopóki nie zastąpi go inny dymek, `ukryj()` albo kliknięcie;
/// `klik` = co zrobić po kliknięciu (wtedy dymek przyjmuje kliknięcia zamiast przepuszczać je do gry);
/// `kadr` = obrazek z zapisanego klipu po lewej stronie (o rozmiarze `rozmiar_kadru(miejsce.skala)`).
pub fn pokaz(
    miejsce: Miejsce,
    tytul: &str,
    opis: &str,
    blad: bool,
    trwale: bool,
    klik: Option<Box<dyn FnOnce() + Send>>,
    kadr: Option<Kadr>,
) {
    let numer = NUMER.fetch_add(1, Ordering::SeqCst) + 1;
    let s = miejsce.skala;
    let px = |v: f32| (v * s).round() as i32;
    let mut uklad = Uklad::nowy(s, kadr.is_some(), !opis.is_empty());
    let (szer, wys) = (uklad.szer, uklad.wys);
    let akcent = if blad { crate::ikona::CZERWONY } else { crate::ikona::ZIELONY };
    let x_cel = miejsce.prawa - szer - px(14.0);
    let y = miejsce.gora + px(14.0);

    unsafe {
        let instancja = GetModuleHandleW(None).unwrap_or_default();
        let klasa = w!("ClipeonPowiadomienie");
        let wc = WNDCLASSW { lpfnWndProc: Some(procedura), hInstance: instancja.into(), lpszClassName: klasa, ..Default::default() };
        RegisterClassW(&wc); // druga rejestracja tej samej klasy po prostu się nie udaje — to w porządku
        let Ok(hwnd) = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
                | if klik.is_some() { Default::default() } else { WS_EX_TRANSPARENT },
            klasa,
            w!("Clipeon"),
            WS_POPUP,
            x_cel,
            y,
            szer,
            wys,
            None,
            None,
            Some(instancja.into()),
            None,
        ) else {
            return crate::dziennik::blad("Nie udało się utworzyć dymka");
        };
        // Dymek nie trafia do nagrania (tryb pulpitu, ddagrab i Windows.Graphics.Capture go pomijają);
        // Windows 10 starszy niż 2004 nie zna tej flagi — wtedy dymek po prostu bywa widoczny w klipie
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);

        // Obraz dymka w pamięci (32 bit, od góry)
        let ekran = GetDC(None);
        let dc = CreateCompatibleDC(Some(ekran));
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: szer,
                biHeight: -wys,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bity: *mut std::ffi::c_void = std::ptr::null_mut();
        let Ok(bitmapa) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bity, None, 0) else {
            let _ = DeleteDC(dc);
            ReleaseDC(None, ekran);
            let _ = DestroyWindow(hwnd);
            return;
        };
        let stara = SelectObject(dc, HGDIOBJ(bitmapa.0));
        let piksele = std::slice::from_raw_parts_mut(bity as *mut u32, (szer * wys) as usize);

        // Tekst (GDI, wygładzanie w odcieniach szarości — ClearType nie pasuje do przezroczystości)
        SetBkMode(dc, TRANSPARENT);
        let nazwa_kroju = kroj(dc);
        let czcionka = |rozmiar: f32, waga: i32| {
            CreateFontW(
                -px(rozmiar), 0, 0, 0, waga, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY, 0, nazwa_kroju,
            )
        };
        // Z kadrem tytuł jest krótki („Klip zapisany”), więc może być grubszy; długie tytuły zostają lekkie
        let (f_tytul, f_opis, f_dlugosc) =
            (czcionka(if kadr.is_some() { 14.5 } else { 15.0 }, if kadr.is_some() { 600 } else { 400 }), czcionka(12.5, 400), czcionka(11.0, 600));
        // Plakietka z długością klipu ma szerokość swojego napisu
        let dlugosc = kadr.as_ref().map(|k| szeroki(&k.dlugosc)).filter(|d| !d.is_empty());
        if let Some(d) = &dlugosc {
            SelectObject(dc, HGDIOBJ(f_dlugosc.0));
            let mut rozmiar = SIZE::default();
            if GetTextExtentPoint32W(dc, d, &mut rozmiar).as_bool() {
                uklad.plakietka(rozmiar.cx as f32);
            }
        }
        let pokrycie = rysuj(piksele, &uklad, akcent, kadr.as_ref());

        let tekst = |f, kolor: u32, tekst: &str, gdzie: Prostokat, wyrownanie| {
            SelectObject(dc, HGDIOBJ(f));
            SetTextColor(dc, COLORREF(kolor));
            let mut t = szeroki(tekst);
            let mut r = RECT {
                left: gdzie.x as i32,
                top: gdzie.y as i32,
                right: (gdzie.x + gdzie.w) as i32,
                bottom: (gdzie.y + gdzie.h) as i32,
            };
            DrawTextW(dc, &mut t, &mut r, wyrownanie | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
        };
        tekst(f_tytul.0, 0x00ED_EDED, tytul, uklad.tytul, DT_LEFT);
        if !opis.is_empty() {
            tekst(f_opis.0, 0x009A_9A9A, opis, uklad.opis, DT_LEFT);
        }
        if let (Some(k), Some(p)) = (&kadr, uklad.plakietka) {
            tekst(f_dlugosc.0, 0x00F2_F2F2, &k.dlugosc, p, DT_CENTER);
        }

        // GDI zeruje kanał alfa tam, gdzie pisze — ustawiamy go z kształtu i mnożymy kolory (premultiplied)
        for (p, c) in piksele.iter_mut().zip(&pokrycie) {
            let a = (c * 255.0) as u32;
            let skaluj = |v: u32| v * a / 255;
            let (r, g, b) = ((*p >> 16) & 0xff, (*p >> 8) & 0xff, *p & 0xff);
            *p = (a << 24) | (skaluj(r) << 16) | (skaluj(g) << 8) | skaluj(b);
        }

        // Pasek czasu rysujemy co klatkę na czystej kopii jego wierszy
        let wiersze_paska = (uklad.pasek().2 * szer) as usize..(uklad.pasek().3 * szer) as usize;
        let pod_paskiem = piksele[wiersze_paska].to_vec();

        // Animacja: wjazd z prawej z pojawieniem się, ~2 s postoju, płynne zniknięcie
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let start = Instant::now();
        let (wejscie, wyjscie) = (0.22f32, 0.3f32);
        let mut postoj = if trwale { f32::INFINITY } else { 2.0f32 };
        let mut klik = klik;
        loop {
            let mut t = start.elapsed().as_secs_f32();
            // Nowszy dymek zastępuje ten od razu
            if NUMER.load(Ordering::SeqCst) != numer {
                break;
            }
            // Kliknięcie: akcja i płynne zniknięcie
            if KLIKNIETY.load(Ordering::SeqCst) == numer {
                KLIKNIETY.store(0, Ordering::SeqCst);
                if let Some(k) = klik.take() {
                    k();
                }
                postoj = (t - wejscie).max(0.0);
                t = start.elapsed().as_secs_f32();
            }
            if t > wejscie + postoj + wyjscie {
                break;
            }
            let (alfa, przesuniecie) = if t < wejscie {
                let k = 1.0 - (1.0 - t / wejscie).powi(3);
                (k, (1.0 - k) * 24.0)
            } else if t < wejscie + postoj {
                (1.0, 0.0)
            } else {
                let k = (t - wejscie - postoj) / wyjscie;
                (1.0 - k, k * k * 12.0)
            };
            // Dymek, który sam zniknie, pokazuje paskiem, ile mu zostało
            if postoj.is_finite() {
                let zostalo = 1.0 - ((t - wejscie) / postoj.max(0.01)).clamp(0.0, 1.0);
                rysuj_pasek(piksele, &pod_paskiem, &uklad, akcent, zostalo);
            }
            let poz = POINT { x: x_cel + px(przesuniecie), y };
            let rozmiar = SIZE { cx: szer, cy: wys };
            let zrodlo = POINT { x: 0, y: 0 };
            let mieszanie = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: (alfa.clamp(0.0, 1.0) * 255.0) as u8,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                hwnd, Some(ekran), Some(&poz), Some(&rozmiar), Some(dc), Some(&zrodlo), COLORREF(0), Some(&mieszanie), ULW_ALPHA,
            );
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, Some(hwnd), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(15));
        }

        let _ = DestroyWindow(hwnd);
        SelectObject(dc, stara);
        let _ = DeleteObject(HGDIOBJ(bitmapa.0));
        let _ = DeleteObject(HGDIOBJ(f_tytul.0));
        let _ = DeleteObject(HGDIOBJ(f_opis.0));
        let _ = DeleteObject(HGDIOBJ(f_dlugosc.0));
        let _ = DeleteDC(dc);
        ReleaseDC(None, ekran);
    }
}
