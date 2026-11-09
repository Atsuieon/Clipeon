# Komponenty zewnętrzne / Third-party notices

Clipper (licencja MIT) korzysta z poniższych komponentów. Ten plik jest dołączany do instalatora.

## FFmpeg (LGPL 2.1 lub nowsza)

Instalator Clippera zawiera niezmieniony plik `ffmpeg.exe` z projektu **FFmpeg** (https://ffmpeg.org),
zbudowany w wariancie **LGPL** (bez komponentów GPL, m.in. bez x264) przez projekt
**FFmpeg-Builds** (https://github.com/FFmpeg-Builds/FFmpeg-Builds), wydanie `ffmpeg-n9.0-latest-win64-lgpl-9.0`.

- Dokładna wersja i konfiguracja builda są zapisane w pliku `ffmpeg/WERSJA.txt` w folderze instalacji
  (to wynik `ffmpeg -version`) oraz w opisie każdego wydania Clippera na GitHubie.
- Pełna treść licencji FFmpeg: `ffmpeg/LICENSE.txt` w folderze instalacji
  oraz https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html.
- Kod źródłowy FFmpeg 9.0: https://github.com/FFmpeg/FFmpeg/tree/release/9.0
  (archiwa: https://ffmpeg.org/download.html#releases).
- Skrypty budujące użyte do wykonania tego builda: https://github.com/FFmpeg-Builds/FFmpeg-Builds.

FFmpeg jest uruchamiany przez Clippera jako osobny program (nie jest z nim linkowany). Możesz go podmienić
na własny build: wystarczy zastąpić plik `ffmpeg\ffmpeg.exe` w folderze instalacji albo usunąć go —
wtedy Clipper użyje `ffmpeg` dostępnego w zmiennej PATH.

FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg project.

Build zawiera biblioteki dostawców sprzętu w wersjach dopuszczonych w buildach LGPL
(m.in. AMD AMF, NVIDIA NVENC/nv-codec-headers, Intel oneVPL/libvpl) — zob. licencje w repozytorium
FFmpeg-Builds.

## Tauri i biblioteki Rust (MIT / Apache-2.0 i zgodne)

Aplikacja jest zbudowana na frameworku **Tauri 2** (https://tauri.app, MIT / Apache-2.0) i korzysta z bibliotek
Rust z crates.io, m.in.: `windows` i `windows-capture` (MIT), `serde`, `serde_json`, `toml` (MIT / Apache-2.0),
`ureq` (MIT / Apache-2.0), `trash` (MIT), `drag` (MIT / Apache-2.0), `base64` (MIT / Apache-2.0),
`tauri-plugin-autostart`, `tauri-plugin-dialog`, `tauri-plugin-single-instance` (MIT / Apache-2.0).
Pełna lista zależności i ich licencji: `Cargo.lock` oraz `cargo about` / `cargo license` w repozytorium.

## Microsoft WebView2

Okno aplikacji używa systemowego komponentu Microsoft Edge WebView2 (część Windows 10/11).

## Ikony

Ikony interfejsu pochodzą z zestawu **Lucide** (https://lucide.dev, licencja ISC).
