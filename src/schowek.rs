// Schowek: klip jako plik (CF_HDROP), tak jak po „Kopiuj” w Eksploratorze. Potem Ctrl+V w Discordzie,
// w Eksploratorze albo w przeglądarce wkleja sam plik.
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_HDROP;
use windows::Win32::UI::Shell::DROPFILES;
use windows::core::w;

/// Kładzie plik do schowka. Schowek bywa na chwilę zajęty przez inny program — wtedy próbujemy kilka razy.
pub fn kopiuj_plik(plik: &Path) -> Result<(), String> {
    let lista = lista_plikow(plik);
    for proba in 0..10 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            let wynik = unsafe { wypelnij(&lista) };
            unsafe {
                let _ = CloseClipboard();
            }
            return wynik.map_err(|e| format!("Nie udało się skopiować klipu: {e}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(20 * (proba + 1)));
    }
    Err("Schowek jest zajęty przez inny program — spróbuj jeszcze raz".into())
}

/// Ścieżka w UTF-16 zakończona dwoma zerami (lista plików z jednym plikiem).
fn lista_plikow(plik: &Path) -> Vec<u16> {
    plik.as_os_str().encode_wide().chain([0, 0]).collect()
}

/// Wypełnia otwarty schowek: CF_HDROP (DROPFILES + lista plików) i „Preferred DropEffect” = kopiuj.
unsafe fn wypelnij(lista: &[u16]) -> windows::core::Result<()> {
    unsafe {
        EmptyClipboard()?;
        let naglowek = std::mem::size_of::<DROPFILES>();
        let pliki = globalny(naglowek + std::mem::size_of_val(lista), |wsk| {
            let d = DROPFILES { pFiles: naglowek as u32, fWide: true.into(), ..Default::default() };
            std::ptr::write_unaligned(wsk as *mut DROPFILES, d);
            std::ptr::copy_nonoverlapping(lista.as_ptr(), wsk.add(naglowek) as *mut u16, lista.len());
        })?;
        if let Err(e) = SetClipboardData(CF_HDROP.0 as u32, Some(HANDLE(pliki.0))) {
            let _ = GlobalFree(Some(pliki));
            return Err(e);
        }
        // Eksplorator przy wklejaniu kopiuje plik (a nie przenosi go z folderu klipów)
        let efekt = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if efekt != 0
            && let Ok(h) = globalny(4, |wsk| std::ptr::write_unaligned(wsk as *mut u32, 1))
            && SetClipboardData(efekt, Some(HANDLE(h.0))).is_err()
        {
            let _ = GlobalFree(Some(h));
        }
        Ok(())
    }
}

/// Pamięć dla schowka (po udanym SetClipboardData należy do systemu).
unsafe fn globalny(rozmiar: usize, wypelnij: impl FnOnce(*mut u8)) -> windows::core::Result<HGLOBAL> {
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, rozmiar)?;
        let wsk = GlobalLock(h) as *mut u8;
        if wsk.is_null() {
            let _ = GlobalFree(Some(h));
            return Err(windows::core::Error::from_thread());
        }
        std::ptr::write_bytes(wsk, 0, rozmiar);
        wypelnij(wsk);
        let _ = GlobalUnlock(h);
        Ok(h)
    }
}

#[cfg(test)]
mod testy {
    #[test]
    fn lista_plikow_konczy_sie_dwoma_zerami() {
        let l = super::lista_plikow(std::path::Path::new(r"C:\Klipy\ą.mp4"));
        assert_eq!(&l[l.len() - 2..], &[0, 0]);
        assert_eq!(String::from_utf16_lossy(&l[..l.len() - 2]), r"C:\Klipy\ą.mp4");
    }
}
