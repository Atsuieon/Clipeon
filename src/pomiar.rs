// Pomiar zużycia procesora: Clipeon i jego procesy ffmpeg. Wynik co minutę trafia do dziennika,
// żeby było widać, ile kosztuje nagrywanie i czy zmiany coś dały.
use windows::Win32::Foundation::{CloseHandle, FILETIME, STILL_ACTIVE};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

fn sto_ns(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

fn czas_uchwytu(uchwyt: windows::Win32::Foundation::HANDLE) -> Option<u64> {
    let (mut a, mut b, mut jadro, mut uzytk) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetProcessTimes(uchwyt, &mut a, &mut b, &mut jadro, &mut uzytk).ok()? };
    Some(sto_ns(jadro) + sto_ns(uzytk))
}

/// Łączny czas procesora (jednostki 100 ns) procesu o danym numerze; None = procesu już nie ma.
pub fn czas_cpu(pid: u32) -> Option<u64> {
    unsafe {
        let uchwyt = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let wynik = czas_uchwytu(uchwyt);
        let _ = CloseHandle(uchwyt);
        wynik
    }
}

/// Czas procesora bieżącego procesu (Clipeon).
pub fn czas_cpu_wlasny() -> u64 {
    czas_uchwytu(unsafe { GetCurrentProcess() }).unwrap_or(0)
}

/// Czy proces jeszcze działa.
pub fn zyje(pid: u32) -> bool {
    unsafe {
        let Ok(uchwyt) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut kod = 0u32;
        let ok = GetExitCodeProcess(uchwyt, &mut kod).is_ok();
        let _ = CloseHandle(uchwyt);
        ok && kod == STILL_ACTIVE.0 as u32
    }
}

/// Procent jednego rdzenia × liczba rdzeni = procent całego procesora (jak w Menedżerze zadań).
pub fn procent(delta_cpu: u64, delta_sciany: std::time::Duration) -> f64 {
    let rdzenie = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let sciana = delta_sciany.as_nanos() as f64 / 100.0;
    if sciana <= 0.0 { 0.0 } else { delta_cpu as f64 / sciana / rdzenie * 100.0 }
}

/// Stan poprzedniego pomiaru (żeby liczyć różnice co minutę).
#[derive(Default)]
pub struct Licznik {
    kiedy: Option<std::time::Instant>,
    wlasny: u64,
    procesy: std::collections::HashMap<u32, u64>,
}

impl Licznik {
    /// Opis zużycia od poprzedniego wywołania, np. „Clipeon 0,4%, ffmpeg obraz 1,2%, ffmpeg dźwięk 0,1%”.
    /// Pierwsze wywołanie tylko zapamiętuje stan (zwraca None).
    pub fn pomiar(&mut self) -> Option<String> {
        let teraz = std::time::Instant::now();
        let wlasny = czas_cpu_wlasny();
        let mut procesy = std::collections::HashMap::new();
        let mut opisy = Vec::new();
        let poprzedni = self.kiedy.replace(teraz);
        for (pid, opis) in crate::ffmpeg::procesy() {
            let Some(c) = czas_cpu(pid) else { continue };
            procesy.insert(pid, c);
            if let (Some(p), Some(stary)) = (poprzedni, self.procesy.get(&pid)) {
                opisy.push(format!("ffmpeg {opis} {:.1}%", procent(c.saturating_sub(*stary), teraz - p)));
            }
        }
        let wynik = poprzedni.map(|p| {
            let mut t = format!("Clipeon {:.1}%", procent(wlasny.saturating_sub(self.wlasny), teraz - p));
            for o in &opisy {
                t.push_str(", ");
                t.push_str(o);
            }
            t.replace('.', ",")
        });
        self.wlasny = wlasny;
        self.procesy = procesy;
        wynik
    }
}
