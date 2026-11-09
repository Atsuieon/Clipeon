// Uruchamianie ffmpeg bez okna konsoli. Kolejność szukania: ffmpeg dostarczony z instalatorem
// (folder „ffmpeg” obok Clipper.exe), potem ffmpeg z PATH (z pominięciem pośrednika chocolatey).
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Ścieżka do ffmpeg.exe, ustalana raz.
fn sciezka() -> &'static PathBuf {
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

/// Nowe polecenie ffmpeg z ukrytym oknem konsoli.
pub fn komenda() -> Command {
    let mut cmd = Command::new(sciezka());
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
