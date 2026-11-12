// Rozpoznawanie gry: jaki program jest teraz na wierzchu nagrywanego ekranu i jak się ładnie nazywa.
// Clipeon nagrywa cały monitor, więc nazwa gry służy do pokazania „co nagrywam” i do opcji
// „nagrywaj tylko wtedy, gdy działa ta gra”.
use std::path::Path;
use windows::Win32::Foundation::{HWND, LPARAM, TRUE};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFOEXW, MonitorFromWindow};
use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, MONITORINFOF_PRIMARY,
};
use windows::core::BOOL;
use windows::core::PCWSTR;

/// Program, który Clipeon uznaje za grę.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Gra {
    /// Ładna nazwa z pliku .exe albo tytuł okna, np. „Cyberpunk 2077”
    pub nazwa: String,
    /// Nazwa pliku programu, np. „Cyberpunk2077.exe” (po niej zapamiętujemy wybór)
    pub proces: String,
}

/// Programy, które nigdy nie są grą: pulpit, powłoka Windows, przeglądarki i sam Clipeon.
const POMIJANE: &[&str] = &[
    "explorer.exe",
    "searchhost.exe",
    "searchapp.exe",
    "shellexperiencehost.exe",
    "startmenuexperiencehost.exe",
    "applicationframehost.exe",
    "textinputhost.exe",
    "lockapp.exe",
    "taskmgr.exe",
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "opera.exe",
    "opera_gx.exe",
    "brave.exe",
    "vivaldi.exe",
    "clipeon.exe",
];

/// Czy ten program może być grą (nazwa pliku, bez względu na wielkość liter).
pub fn mozliwa_gra(proces: &str) -> bool {
    let p = proces.to_ascii_lowercase();
    !p.is_empty() && !POMIJANE.contains(&p.as_str())
}

/// Czy dwie nazwy procesów to ten sam program.
pub fn ten_sam(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn szeroki(tekst: &str) -> Vec<u16> {
    tekst.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Gra z okna na pierwszym planie; `monitor` ogranicza ją do nagrywanego ekranu
/// (pusty = ekran główny). None, gdy na wierzchu jest pulpit, przeglądarka albo sam Clipeon.
pub fn na_pierwszym_planie(monitor: &str) -> Option<Gra> {
    let okno = unsafe { GetForegroundWindow() };
    if okno.is_invalid() || !na_monitorze(okno, monitor) {
        return None;
    }
    z_okna(okno)
}

/// Czy wybrany program jest uruchomiony. Sprawdzamy listę procesów, nie okien: gra (np. emulator)
/// przy przełączaniu pełnego ekranu albo po zminimalizowaniu na chwilę chowa okno, a to nie znaczy,
/// że się zamknęła.
pub fn dziala(proces: &str) -> bool {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let Ok(migawka) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            // Bez listy procesów zostaje stary sposób: widoczne okna
            return lista().iter().any(|g| ten_sam(&g.proces, proces));
        };
        let mut wpis = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut jest = false;
        let mut dalej = Process32FirstW(migawka, &mut wpis).is_ok();
        while dalej {
            if ten_sam(&crate::tekst_utf16(&wpis.szExeFile), proces) {
                jest = true;
                break;
            }
            dalej = Process32NextW(migawka, &mut wpis).is_ok();
        }
        let _ = CloseHandle(migawka);
        jest
    }
}

/// Programy do wyboru w ustawieniach: widoczne, niezminimalizowane okna z tytułem.
pub fn lista() -> Vec<Gra> {
    let mut zebrane: Vec<Gra> = Vec::new();
    let wskaznik = &mut zebrane as *mut Vec<Gra>;
    let _ = unsafe { EnumWindows(Some(zbierz), LPARAM(wskaznik as isize)) };
    zebrane.sort_by_key(|g| g.nazwa.to_lowercase());
    zebrane
}

unsafe extern "system" fn zbierz(okno: HWND, dane: LPARAM) -> BOOL {
    let zebrane = unsafe { &mut *(dane.0 as *mut Vec<Gra>) };
    if let Some(g) = z_okna(okno)
        && !zebrane.iter().any(|x| ten_sam(&x.proces, &g.proces))
    {
        zebrane.push(g);
    }
    TRUE
}

/// Gra z konkretnego okna (None, gdy okno jest niewidoczne, bez tytułu albo nie jest grą).
fn z_okna(okno: HWND) -> Option<Gra> {
    unsafe {
        if !IsWindowVisible(okno).as_bool() || IsIconic(okno).as_bool() {
            return None;
        }
        let mut tytul = [0u16; 256];
        let dlugosc = GetWindowTextW(okno, &mut tytul);
        let tytul = String::from_utf16_lossy(&tytul[..dlugosc.max(0) as usize]);
        if tytul.trim().is_empty() {
            return None;
        }
        let sciezka = sciezka_programu(okno)?;
        let proces = Path::new(&sciezka).file_name()?.to_string_lossy().into_owned();
        if !mozliwa_gra(&proces) {
            return None;
        }
        let nazwa = opis_pliku(&sciezka).unwrap_or_else(|| tytul.trim().to_string());
        Some(Gra { nazwa, proces })
    }
}

/// Pełna ścieżka pliku .exe programu, do którego należy okno.
fn sciezka_programu(okno: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(okno, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        // Gry pod anty-cheatem nie wpuszczają nas głębiej, ale sama nazwa pliku zwykle się udaje
        let uchwyt = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut bufor = [0u16; 520];
        let mut dlugosc = bufor.len() as u32;
        let wynik = QueryFullProcessImageNameW(
            uchwyt,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(bufor.as_mut_ptr()),
            &mut dlugosc,
        );
        let _ = CloseHandle(uchwyt);
        wynik.ok()?;
        Some(String::from_utf16_lossy(&bufor[..dlugosc as usize]))
    }
}

/// Ładna nazwa programu z zasobów pliku .exe (pole FileDescription, to samo, co widać
/// w Menedżerze zadań). None, gdy plik nie ma opisu. Strażnik pyta o grę na wierzchu co sekundę,
/// więc opis każdego pliku czytamy z dysku tylko raz.
fn opis_pliku(sciezka: &str) -> Option<String> {
    use crate::Zablokuj;
    static ZNANE: std::sync::Mutex<Vec<(String, Option<String>)>> = std::sync::Mutex::new(Vec::new());
    if let Some((_, opis)) = ZNANE.zablokuj().iter().find(|(s, _)| s == sciezka) {
        return opis.clone();
    }
    let opis = opis_z_zasobow(sciezka).filter(|n| !n.is_empty());
    let mut znane = ZNANE.zablokuj();
    // Lista programów w ustawieniach przechodzi przez wszystkie okna — pamięć nie rośnie bez końca
    if znane.len() >= 64 {
        znane.clear();
    }
    znane.push((sciezka.to_string(), opis.clone()));
    opis
}

fn opis_z_zasobow(sciezka: &str) -> Option<String> {
    unsafe {
        let plik = szeroki(sciezka);
        let rozmiar = GetFileVersionInfoSizeW(PCWSTR(plik.as_ptr()), None);
        if rozmiar == 0 {
            return None;
        }
        let mut dane = vec![0u8; rozmiar as usize];
        GetFileVersionInfoW(PCWSTR(plik.as_ptr()), None, rozmiar, dane.as_mut_ptr() as *mut _).ok()?;

        // Najpierw język i strona kodowa opisu, potem samo pole FileDescription
        let mut wskaznik = std::ptr::null_mut();
        let mut bajtow = 0u32;
        let zapytanie = szeroki(r"\VarFileInfo\Translation");
        if !VerQueryValueW(dane.as_ptr() as *const _, PCWSTR(zapytanie.as_ptr()), &mut wskaznik, &mut bajtow).as_bool()
            || bajtow < 4
        {
            return None;
        }
        let tlumaczenia = std::slice::from_raw_parts(wskaznik as *const u16, (bajtow / 2) as usize);
        let (jezyk, strona) = (tlumaczenia[0], tlumaczenia[1]);
        let zapytanie = szeroki(&format!("\\StringFileInfo\\{jezyk:04x}{strona:04x}\\FileDescription"));
        let mut znakow = 0u32;
        if !VerQueryValueW(dane.as_ptr() as *const _, PCWSTR(zapytanie.as_ptr()), &mut wskaznik, &mut znakow).as_bool()
            || znakow == 0
        {
            return None;
        }
        let opis = std::slice::from_raw_parts(wskaznik as *const u16, znakow as usize);
        Some(String::from_utf16_lossy(opis).trim_end_matches('\0').trim().to_string())
    }
}

/// Czy okno jest na nagrywanym ekranie (`monitor` pusty = ekran główny).
fn na_monitorze(okno: HWND, monitor: &str) -> bool {
    unsafe {
        let uchwyt: HMONITOR = MonitorFromWindow(okno, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(uchwyt, &mut info as *mut _ as *mut _).as_bool() {
            return true; // nie wiemy — lepiej pokazać nazwę niż jej nie pokazać
        }
        if monitor.is_empty() {
            return info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;
        }
        crate::tekst_utf16(&info.szDevice).eq_ignore_ascii_case(monitor)
    }
}

#[cfg(test)]
mod testy {
    use super::{mozliwa_gra, ten_sam};

    #[test]
    fn powloka_i_przegladarki_nie_sa_gra() {
        assert!(!mozliwa_gra("explorer.exe"));
        assert!(!mozliwa_gra("Explorer.exe"));
        assert!(!mozliwa_gra("chrome.exe"));
        assert!(!mozliwa_gra("Clipeon.exe"));
        assert!(!mozliwa_gra(""));
        assert!(mozliwa_gra("Cyberpunk2077.exe"));
        assert!(mozliwa_gra("csgo.exe"));
    }

    #[test]
    fn nazwy_procesow_porownujemy_bez_wzgledu_na_wielkosc_liter() {
        assert!(ten_sam("Cyberpunk2077.exe", "cyberpunk2077.EXE"));
        assert!(!ten_sam("csgo.exe", "cs2.exe"));
    }
}
