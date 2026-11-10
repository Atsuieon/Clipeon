// Powrót po awarii: gdy Clipeon padnie, uruchamia się ponownie sam i wraca do nagrywania.
// - awarie natywne (np. sterownik, WebView2): Windows Application Restart (po co najmniej 60 s działania),
// - awaria głównego wątku Rusta (panic): sami uruchamiamy nową kopię, która czeka na zamknięcie starej.
// Nagrania sprzed awarii nie da się odzyskać (bufor jest w RAM) — wraca samo nagrywanie.
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const FLAGA: &str = "--po-awarii";

fn start() -> &'static Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now)
}

/// Wywoływane na samym początku programu. Zwraca true, gdy to uruchomienie po awarii.
pub fn przy_starcie() -> bool {
    start();
    let argumenty: Vec<String> = std::env::args().collect();
    let po_awarii = argumenty.iter().any(|a| a == FLAGA);
    if po_awarii {
        // Nowa kopia z wątku awarii: czekamy, aż stara się zamknie (inaczej zablokuje ją pojedyncza instancja)
        if let Some(pid) = argumenty.iter().skip_while(|a| *a != FLAGA).nth(1).and_then(|p| p.parse::<u32>().ok()) {
            if !czekaj_na_koniec(pid, Duration::from_secs(15)) {
                // Stara kopia jednak działa dalej — nie dublujemy programu
                std::process::exit(0);
            }
        }
        crate::dziennik::info("Clipeon uruchomiony ponownie po awarii");
    }
    // Windows uruchomi Clipeona ponownie po awarii lub zawieszeniu (nie po aktualizacji ani restarcie systemu —
    // to załatwia instalator i autostart)
    unsafe {
        use windows::Win32::System::Recovery::{RESTART_NO_PATCH, RESTART_NO_REBOOT, RegisterApplicationRestart};
        let linia: Vec<u16> = FLAGA.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = RegisterApplicationRestart(windows::core::PCWSTR(linia.as_ptr()), RESTART_NO_PATCH | RESTART_NO_REBOOT);
    }
    po_awarii
}

/// Czeka na zakończenie procesu; true = zakończył się (albo już go nie ma).
fn czekaj_na_koniec(pid: u32, limit: Duration) -> bool {
    use windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
    unsafe {
        let Ok(uchwyt) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else { return true };
        let wynik = WaitForSingleObject(uchwyt, limit.as_millis() as u32);
        let _ = CloseHandle(uchwyt);
        wynik != WAIT_TIMEOUT
    }
}

/// Awaria (panic) głównego wątku: program zaraz się zamknie, więc uruchamiamy nową kopię.
/// Tylko gdy działał co najmniej minutę — inaczej to błąd przy starcie i wpadlibyśmy w pętlę.
pub fn po_awarii_glownego_watku() {
    if start().elapsed() < Duration::from_secs(60) {
        crate::dziennik::blad("Awaria krótko po starcie — bez ponownego uruchamiania");
        return;
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).args([FLAGA, &std::process::id().to_string()]).spawn();
    }
}
