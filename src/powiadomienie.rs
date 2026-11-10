// Dymek „Klip zapisany” w prawym górnym rogu monitora z grą: natywne okno warstwowe
// (UpdateLayeredWindow) rysowane w pamięci. Bez WebView — lekkie i niezawodne nad grą:
// zawsze na wierzchu, bez aktywacji (gra nie traci fokusu), przezroczyste dla myszy,
// pomijane przez menedżery okien kafelkowych (styl okna-narzędzia).
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    CLIP_DEFAULT_PRECIS, CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DIB_RGB_COLORS,
    DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, GetDC,
    HGDIOBJ, OUT_DEFAULT_PRECIS, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
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

/// Chowa bieżący dymek (np. alarm, gdy problem minął).
pub fn ukryj() {
    NUMER.fetch_add(1, Ordering::SeqCst);
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
/// `klik` = co zrobić po kliknięciu (wtedy dymek przyjmuje kliknięcia zamiast przepuszczać je do gry).
pub fn pokaz(miejsce: Miejsce, tytul: &str, opis: &str, blad: bool, trwale: bool, klik: Option<Box<dyn FnOnce() + Send>>) {
    let numer = NUMER.fetch_add(1, Ordering::SeqCst) + 1;
    let s = miejsce.skala;
    let px = |v: f32| (v * s).round() as i32;
    let (szer, wys) = (px(360.0), px(68.0));
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
            return eprintln!("Nie udało się utworzyć dymka");
        };
        // Dymek nie trafia do nagrania (tryb pulpitu, ddagrab i Windows.Graphics.Capture go pomijają);
        // Windows 10 starszy niż 2004 nie zna tej flagi — wtedy dymek po prostu bywa widoczny w klipie
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);

        // Obraz dymka w pamięci (32 bit, od góry)
        let ekran = GetDC(None);
        let dc = CreateCompatibleDC(Some(ekran));
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: szer,
            biHeight: -wys,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
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
        let pokrycie = rysuj_tlo(piksele, szer, wys, s, blad);

        // Tekst (GDI, wygładzanie w odcieniach szarości — ClearType nie pasuje do przezroczystości)
        SetBkMode(dc, TRANSPARENT);
        let czcionka = |rozmiar: f32| {
            CreateFontW(
                -px(rozmiar), 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY, 0, w!("Segoe UI Variable Text"),
            )
        };
        let (f_tytul, f_opis) = (czcionka(15.0), czcionka(12.5));
        let lewa = px(40.0);
        let prawa = szer - px(16.0);
        let tekst = |f, kolor: u32, tekst: &str, gora: i32, dol: i32| {
            SelectObject(dc, HGDIOBJ(f));
            SetTextColor(dc, COLORREF(kolor));
            let mut t = szeroki(tekst);
            let mut r = RECT { left: lewa, top: gora, right: prawa, bottom: dol };
            DrawTextW(dc, &mut t, &mut r, DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX);
        };
        tekst(f_tytul.0, 0x00ED_EDED, tytul, px(10.0), px(36.0));
        tekst(f_opis.0, 0x009A_9A9A, opis, px(34.0), px(58.0));

        // GDI zeruje kanał alfa tam, gdzie pisze — ustawiamy go z kształtu i mnożymy kolory (premultiplied)
        for (p, c) in piksele.iter_mut().zip(&pokrycie) {
            let a = (c * 245.0) as u32;
            let skaluj = |v: u32| v * a / 255;
            let (r, g, b) = ((*p >> 16) & 0xff, (*p >> 8) & 0xff, *p & 0xff);
            *p = (a << 24) | (skaluj(r) << 16) | (skaluj(g) << 8) | skaluj(b);
        }

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
        let _ = DeleteDC(dc);
        ReleaseDC(None, ekran);
    }
}

/// Tło dymka: zaokrąglony prostokąt #0A0A0A z obwódką #262626 i kropką stanu.
/// Zwraca pokrycie kształtu (0..1) dla każdego piksela — potrzebne do kanału alfa.
fn rysuj_tlo(piksele: &mut [u32], szer: i32, wys: i32, s: f32, blad: bool) -> Vec<f32> {
    let (w, h) = (szer as f32, wys as f32);
    let r = 18.0 * s;
    let kropka = if blad { (239.0, 68.0, 68.0) } else { (34.0, 197.0, 94.0) };
    let (kx, ky, kr) = (22.0 * s, h / 2.0, 4.5 * s);
    let mut pokrycie = vec![0.0f32; piksele.len()];
    for y in 0..wys {
        for x in 0..szer {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Odległość od krawędzi zaokrąglonego prostokąta (ujemna w środku)
            let qx = (px - w / 2.0).abs() - (w / 2.0 - r);
            let qy = (py - h / 2.0).abs() - (h / 2.0 - r);
            let d = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r;
            let c = (0.5 - d).clamp(0.0, 1.0);
            // Obwódka 1 px przy krawędzi
            let ramka = (0.5 - (d + s).abs() + 0.5 * s).clamp(0.0, 1.0);
            let mut kol = [10.0f32 + ramka * 28.0; 3];
            let dk = ((px - kx).powi(2) + (py - ky).powi(2)).sqrt() - kr;
            let k = (0.5 - dk).clamp(0.0, 1.0);
            kol[0] = kol[0] * (1.0 - k) + kropka.0 * k;
            kol[1] = kol[1] * (1.0 - k) + kropka.1 * k;
            kol[2] = kol[2] * (1.0 - k) + kropka.2 * k;
            let i = (y * szer + x) as usize;
            piksele[i] = ((kol[0] as u32) << 16) | ((kol[1] as u32) << 8) | kol[2] as u32;
            pokrycie[i] = c;
        }
    }
    pokrycie
}
