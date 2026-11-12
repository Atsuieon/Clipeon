// Ikona zawsze przy zegarze. Windows 11 nie ma do tego API, ale trzyma ustawienie w rejestrze:
// HKCU\Control Panel\NotifyIconSettings\<id>, wartość IsPromoted (1 = widoczna przy zegarze).
// Wpis powstaje dopiero, gdy ikona pokaże się pierwszy raz, więc ustawiamy go z ponawianiem.
use std::time::Duration;
use windows_registry::CURRENT_USER;

const KLUCZ: &str = r"Control Panel\NotifyIconSettings";

/// Czy ścieżka z rejestru wskazuje nasz plik. Programy w folderach znanych (np. Program Files)
/// są zapisane jako {GUID}\reszta, pozostałe pełną ścieżką.
fn pasuje(zapisana: &str, nasza: &str) -> bool {
    let (z, n) = (zapisana.to_lowercase(), nasza.to_lowercase());
    z == n || (z.starts_with('{') && z.find("}\\").is_some_and(|i| n.ends_with(&z[i + 1..])))
}

/// Ustawia widoczność ikony; Ok(false), gdy Windows nie utworzył jeszcze wpisu.
fn ustaw(widoczna: bool) -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.to_string_lossy();
    let ustawienia = CURRENT_USER.open(KLUCZ).map_err(|e| e.to_string())?;
    for nazwa in ustawienia.keys().map_err(|e| e.to_string())? {
        let Ok(k) = CURRENT_USER.options().read().write().open(format!(r"{KLUCZ}\{nazwa}")) else { continue };
        if k.get_string("ExecutablePath").is_ok_and(|p| pasuje(&p, &exe)) {
            k.set_u32("IsPromoted", widoczna as u32).map_err(|e| e.to_string())?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Ustawia widoczność w tle (do ~30 s prób); `gotowe` wywołuje się po udanym zapisie.
pub fn ustaw_w_tle(widoczna: bool, gotowe: impl FnOnce() + Send + 'static) {
    std::thread::spawn(move || {
        for _ in 0..15 {
            match ustaw(widoczna) {
                Ok(true) => {
                    crate::dziennik::info(&format!("Ikona w zasobniku: {}", if widoczna { "zawsze przy zegarze" } else { "w ukrytych ikonach" }));
                    gotowe();
                    return;
                }
                Ok(false) => std::thread::sleep(Duration::from_secs(2)),
                Err(e) => {
                    crate::dziennik::blad(&format!("Nie udało się ustawić widoczności ikony: {e}"));
                    return;
                }
            }
        }
        crate::dziennik::info("Windows nie utworzył wpisu ikony w rejestrze — widoczność zostaje bez zmian");
    });
}
