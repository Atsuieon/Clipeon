// Uruchamianie ffmpeg bez okna konsoli. Kolejność szukania: ffmpeg dostarczony z instalatorem
// (folder „ffmpeg” obok Clipeon.exe), potem ffmpeg z PATH (z pominięciem pośrednika chocolatey).
// Procesy nagrywania (obraz, dźwięk) dostają wyższy priorytet i nie są dławione przez Windows 11,
// a ich numery trafiają na listę, z której liczymy zużycie procesora do dziennika.
use crate::Zablokuj;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x0000_8000;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;

/// Ścieżka do ffmpeg.exe, ustalana raz.
pub fn sciezka() -> &'static PathBuf {
    static SCIEZKA: OnceLock<PathBuf> = OnceLock::new();
    SCIEZKA.get_or_init(|| {
        if let Some(folder) = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            for kandydat in [folder.join("ffmpeg").join("ffmpeg.exe"), folder.join("ffmpeg.exe")] {
                if kandydat.is_file() {
                    return kandydat;
                }
            }
        }
        let Some(znaleziony) = std::env::var_os("PATH")
            .and_then(|p| std::env::split_paths(&p).map(|d| d.join("ffmpeg.exe")).find(|p| p.is_file()))
        else {
            return PathBuf::from("ffmpeg");
        };
        // Chocolatey wkłada do PATH tylko pośrednika (osobny proces, ~19 MB RAM); prawdziwy exe leży w lib\
        let bin = znaleziony.parent().unwrap();
        if bin.ends_with("chocolatey\\bin") {
            let prawdziwy = bin.parent().unwrap().join(r"lib\ffmpeg\tools\ffmpeg\bin\ffmpeg.exe");
            if prawdziwy.is_file() {
                return prawdziwy;
            }
        }
        znaleziony
    })
}

/// Odcisk pliku ffmpeg.exe (rozmiar + data zmiany): inny ffmpeg = wyniki prób enkoderów od nowa.
pub fn odcisk() -> String {
    std::fs::metadata(sciezka())
        .map(|m| {
            let czas = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
            format!("{}:{}", m.len(), czas.unwrap_or(0))
        })
        .unwrap_or_else(|_| "brak".into())
}

/// Nowe polecenie ffmpeg z ukrytym oknem konsoli.
pub fn komenda() -> Command {
    komenda_z_flagami(0)
}

/// Polecenie ffmpeg do długiej pracy w tle (zmniejszanie filmu): priorytet poniżej normalnego,
/// żeby nie zabierało procesora grze.
pub fn komenda_w_tle() -> Command {
    komenda_z_flagami(BELOW_NORMAL_PRIORITY_CLASS)
}

/// Polecenie ffmpeg do nagrywania (obraz albo dźwięk w tle): priorytet powyżej normalnego, żeby gra
/// obciążająca procesor nie zagłodziła enkodera. Uruchamiać przez `uruchom_nagrywanie`.
pub fn komenda_nagrywania() -> Command {
    komenda_z_flagami(ABOVE_NORMAL_PRIORITY_CLASS)
}

fn komenda_z_flagami(flagi: u32) -> Command {
    let mut cmd = Command::new(sciezka());
    cmd.creation_flags(CREATE_NO_WINDOW | flagi);
    cmd
}

/// Procesy nagrywania (numer procesu, opis) — do pomiaru zużycia procesora.
static PROCESY: Mutex<Vec<(u32, &'static str)>> = Mutex::new(Vec::new());

/// Uruchamia ffmpeg nagrywania: wyłącza dławienie przez Windows i zapisuje proces na liście pomiarów.
pub fn uruchom_nagrywanie(cmd: &mut Command, opis: &'static str) -> std::io::Result<Child> {
    let proces = cmd.spawn()?;
    {
        use std::os::windows::io::AsRawHandle;
        bez_dlawienia(windows::Win32::Foundation::HANDLE(proces.as_raw_handle()));
    }
    let mut l = PROCESY.zablokuj();
    l.retain(|(pid, _)| crate::pomiar::zyje(*pid));
    l.push((proces.id(), opis));
    Ok(proces)
}

/// Działające procesy nagrywania (martwe usuwa z listy).
pub fn procesy() -> Vec<(u32, &'static str)> {
    let mut l = PROCESY.zablokuj();
    l.retain(|(pid, _)| crate::pomiar::zyje(*pid));
    l.clone()
}

/// Windows 11 (EcoQoS) spowalnia procesy, których okno nie jest na wierzchu — czyli zawsze nas,
/// gdy gramy. Wyłączamy to dla procesu; na starszym Windows wywołanie po prostu się nie udaje.
pub fn bez_dlawienia(proces: windows::Win32::Foundation::HANDLE) {
    use windows::Win32::System::Threading::{
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetProcessInformation,
    };
    let stan = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    unsafe {
        let _ = SetProcessInformation(
            proces,
            ProcessPowerThrottling,
            &stan as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

/// Filtry dostępne w tym ffmpeg (lista z `ffmpeg -filters`, sprawdzana raz).
pub fn ma_filtr(nazwa: &str) -> bool {
    static LISTA: OnceLock<String> = OnceLock::new();
    na_liscie(LISTA.get_or_init(|| wypisz("-filters")), nazwa)
}

/// Enkodery dostępne w tym ffmpeg (lista z `ffmpeg -encoders`, sprawdzana raz).
pub fn ma_enkoder(nazwa: &str) -> bool {
    static LISTA: OnceLock<String> = OnceLock::new();
    na_liscie(LISTA.get_or_init(|| wypisz("-encoders")), nazwa)
}

/// Wynik `ffmpeg -filters` / `-encoders` (pusty, gdy ffmpeg się nie uruchomił).
fn wypisz(co: &str) -> String {
    komenda()
        .args(["-hide_banner", co])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Czy nazwa jest na liście ffmpeg (druga kolumna wiersza, np. „ V....D h264_amf  AMD AMF H.264 Encoder”).
fn na_liscie(lista: &str, nazwa: &str) -> bool {
    lista.lines().any(|l| l.split_whitespace().nth(1) == Some(nazwa))
}

/// Pierwsza linia `ffmpeg -version` (do diagnostyki).
pub fn wersja() -> String {
    komenda()
        .arg("-version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(str::to_string))
        .unwrap_or_else(|| "nie udało się uruchomić ffmpeg".into())
}
