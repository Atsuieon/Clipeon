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

/// Kody błędów Windows przy uruchamianiu pliku, który zablokowały zasady kontroli aplikacji: Smart App
/// Control, App Control for Business (WDAC) albo AppLocker. Smart App Control pyta chmurę Microsoftu o każdy
/// niepodpisany program, więc mało znany ffmpeg.exe bywa raz wpuszczany, a raz nie (dziennik testera
/// z 2026-11-16: nagrywanie ruszyło o 15:14, a zapis klipu o 18:48 już nie, kod 4551).
const KODY_ZASAD: [i32; 9] = [
    4551, // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION
    4556, // ERROR_SYSTEM_INTEGRITY_REPUTATION_MALICIOUS
    4557, // ERROR_SYSTEM_INTEGRITY_REPUTATION_PUA
    4558, // ERROR_SYSTEM_INTEGRITY_REPUTATION_DANGEROUS_EXT
    4559, // ERROR_SYSTEM_INTEGRITY_REPUTATION_OFFLINE
    4580, // ERROR_SYSTEM_INTEGRITY_REPUTATION_UNFRIENDLY_FILE
    4581, // ERROR_SYSTEM_INTEGRITY_REPUTATION_UNATTAINABLE
    4582, // ERROR_SYSTEM_INTEGRITY_REPUTATION_EXPLICIT_DENY_FILE
    1260, // ERROR_ACCESS_DISABLED_BY_POLICY (AppLocker, zasady ograniczeń oprogramowania)
];
/// Antywirus uznał plik za groźny (ERROR_VIRUS_INFECTED, ERROR_VIRUS_DELETED).
const KODY_ANTYWIRUSA: [i32; 2] = [225, 226];

/// Strona z instrukcją, co zrobić, gdy Windows blokuje ffmpeg (dymek po kliknięciu ją otwiera).
pub const POMOC_BLOKADY: &str = "https://github.com/Atsuieon/Clipeon/blob/main/docs/windows-blokuje-ffmpeg.md";

/// Znak rozpoznawczy blokady w opisie błędu (po polsku i po angielsku): dymek z nim otwiera `POMOC_BLOKADY`.
const ZNAK_BLOKADY: &str = "Smart App Control";

/// Opis błędu uruchomienia ffmpeg: blokada przez Windows albo antywirusa po ludzku, z kodem do dziennika;
/// każdy inny błąd jak dotąd („Nie udało się uruchomić ffmpeg: …”).
pub fn blad_uruchomienia(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(kod) if KODY_ZASAD.contains(&kod) => crate::jezyk::wybierz(
            format!(
                "Windows zablokował ffmpeg, którego Clipeon używa do zapisu ({ZNAK_BLOKADY} albo inne zasady kontroli aplikacji, kod {kod}). Kliknij, żeby zobaczyć, co zrobić."
            ),
            format!(
                "Windows blocked ffmpeg, which Clipeon uses to save clips ({ZNAK_BLOKADY} or another app control policy, code {kod}). Click to see what to do."
            ),
        ),
        Some(kod) if KODY_ANTYWIRUSA.contains(&kod) => crate::jezyk::wybierz(
            format!("Antywirus zablokował ffmpeg, którego Clipeon używa do zapisu (kod {kod}). Dodaj folder Clipeona do wyjątków antywirusa."),
            format!("Your antivirus blocked ffmpeg, which Clipeon uses to save clips (code {kod}). Add the Clipeon folder to its exclusions."),
        ),
        _ => format!("Nie udało się uruchomić ffmpeg: {e}"),
    }
}

/// Czy opis błędu to blokada ffmpeg przez zasady kontroli aplikacji (z `blad_uruchomienia`).
pub fn to_blokada(opis: &str) -> bool {
    opis.contains(ZNAK_BLOKADY)
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

#[cfg(test)]
mod testy {
    use super::{blad_uruchomienia, to_blokada};

    #[test]
    fn blokada_przez_zasady_jest_rozpoznawana() {
        let opis = blad_uruchomienia(&std::io::Error::from_raw_os_error(4551));
        assert!(to_blokada(&opis), "{opis}");
        assert!(opis.contains("4551"));
        assert!(to_blokada(&blad_uruchomienia(&std::io::Error::from_raw_os_error(1260))));
    }

    #[test]
    fn zwykly_blad_zostaje_jak_dawniej() {
        let opis = blad_uruchomienia(&std::io::Error::from_raw_os_error(2));
        assert!(opis.starts_with("Nie udało się uruchomić ffmpeg: "), "{opis}");
        assert!(!to_blokada(&opis));
        assert!(!to_blokada(&blad_uruchomienia(&std::io::Error::from_raw_os_error(225))));
    }
}
