// Globalny skrót klawiszowy (RegisterHotKey) — działa także, gdy gra ma fokus.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_HOTKEY, WM_QUIT, WM_USER,
};
use windows::Win32::Foundation::{LPARAM, WPARAM};

/// Zamienia tekst typu "Alt+F10" na modyfikatory i kod klawisza.
pub fn parsuj(skrot: &str) -> Result<(HOT_KEY_MODIFIERS, u32), String> {
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

/// Działający nasłuch skrótu; zniszczenie go zwalnia skrót.
pub struct Nasluch {
    stop: Arc<AtomicBool>,
    watek: Arc<AtomicU32>,
    uchwyt: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Nasluch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let id = self.watek.load(Ordering::SeqCst);
        if id != 0 {
            let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
        // Czekamy, aż wątek zwolni skrót — inaczej nowy nasłuch nie mógłby go zarejestrować
        if let Some(u) = self.uchwyt.take() {
            let _ = u.join();
        }
    }
}

/// Rejestruje skrót w osobnym wątku i wywołuje `akcja` przy każdym naciśnięciu.
/// Gdy skrót jest zajęty przez inny program, zgłasza błąd przez `zglos` i ponawia co 3 s.
pub fn nasluchuj(
    skrot: &str,
    akcja: impl Fn() + Send + 'static,
    zglos: impl Fn(Option<String>) + Send + 'static,
) -> Result<Nasluch, String> {
    let (modyfikatory, klawisz) = parsuj(skrot)?;
    let (stop, watek) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicU32::new(0)));
    let (stop_watku, watek_id) = (stop.clone(), watek.clone());
    let skrot = skrot.to_string();

    let uchwyt = std::thread::spawn(move || unsafe {
        let (stop, watek) = (stop_watku, watek_id);
        // Kolejka komunikatów musi istnieć, zanim ktoś wyśle nam WM_QUIT
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);
        watek.store(GetCurrentThreadId(), Ordering::SeqCst);

        loop {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            match RegisterHotKey(None, 1, modyfikatory, klawisz) {
                Ok(()) => break,
                Err(e) => {
                    zglos(Some(format!("Skrót {skrot} jest zajęty przez inny program ({})", e.message())));
                    for _ in 0..30 {
                        if stop.load(Ordering::SeqCst) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        }
        zglos(None);
        println!("Naciśnij {skrot}, aby zapisać klip.");

        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                akcja();
            }
        }
        let _ = UnregisterHotKey(None, 1);
    });
    Ok(Nasluch { stop, watek, uchwyt: Some(uchwyt) })
}
