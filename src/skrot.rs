// Globalny skrót klawiszowy (RegisterHotKey) — działa także, gdy gra ma fokus.
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

/// Zamienia tekst typu "Alt+F10" na modyfikatory i kod klawisza.
fn parsuj(skrot: &str) -> Result<(HOT_KEY_MODIFIERS, u32), String> {
    let mut modyfikatory = MOD_NOREPEAT;
    let mut klawisz = None;
    for czesc in skrot.split('+').map(|c| c.trim().to_ascii_uppercase()) {
        match czesc.as_str() {
            "ALT" => modyfikatory |= MOD_ALT,
            "CTRL" | "CONTROL" => modyfikatory |= MOD_CONTROL,
            "SHIFT" => modyfikatory |= MOD_SHIFT,
            "WIN" => modyfikatory |= MOD_WIN,
            k => {
                let kod = match k.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
                    Some(n @ 1..=24) => 0x70 + n - 1, // VK_F1..VK_F24
                    _ if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => k.as_bytes()[0] as u32,
                    _ => return Err(format!("Nieznany klawisz \"{k}\" w skrócie \"{skrot}\"")),
                };
                if klawisz.replace(kod).is_some() {
                    return Err(format!("Skrót \"{skrot}\" ma więcej niż jeden zwykły klawisz"));
                }
            }
        }
    }
    Ok((modyfikatory, klawisz.ok_or(format!("Skrót \"{skrot}\" nie ma zwykłego klawisza"))?))
}

/// Rejestruje skrót i blokuje bieżący wątek, wywołując `akcja` przy każdym naciśnięciu.
pub fn nasluchuj(skrot: &str, mut akcja: impl FnMut()) -> Result<(), String> {
    let (modyfikatory, klawisz) = parsuj(skrot)?;
    unsafe { RegisterHotKey(None, 1, modyfikatory, klawisz) }
        .map_err(|e| format!("Nie mogę zarejestrować skrótu {skrot} (zajęty przez inny program?): {e}"))?;
    println!("Naciśnij {skrot}, aby zapisać klip.");

    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        if msg.message == WM_HOTKEY {
            akcja();
        }
    }
    Ok(())
}
