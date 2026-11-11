<p align="center">
  <img src="icons/128x128@2x.png" width="88" alt="">
</p>

<h1 align="center">Clipeon</h1>

<p align="center">Zapisuj najlepsze momenty z gier jednym skrótem — bez nagrywania wszystkiego na dysk.</p>

<p align="center">
  <a href="https://github.com/Atsuieon/Clipeon/releases/latest"><img alt="Pobierz dla Windows" src="https://img.shields.io/badge/Pobierz-Windows-ffffff?style=for-the-badge&labelColor=000000"></a>
  <a href="https://github.com/Atsuieon/Clipeon/releases/latest"><img alt="Wersja" src="https://img.shields.io/github/v/release/Atsuieon/Clipeon?style=for-the-badge&label=wersja&labelColor=000000&color=1f1f1f"></a>
</p>

<p align="center"><img src="docs/screenshots/biblioteka.png" width="860" alt="Biblioteka klipów Clipeona"></p>

## Funkcje

- **Ostatnie sekundy gry jednym skrótem** — wciśnij **Alt+F10**, a ostatnie 30 s (do ustawienia od 5 s do 10 min) zapisze się jako MP4.
- **Lekki** — obraz przechwytuje i koduje karta graficzna (AMD, NVIDIA, Intel), bez kopiowania klatek przez procesor; nic nie trafia na dysk, dopóki nie zapiszesz.
- **Tryb pulpitu** — przełącznik „Gra | Pulpit” na pasku: pulpit ma własny ekran i lżejsze ustawienia (domyślnie 30 FPS, 12 Mb/s), a ekran wybierzesz jednym kliknięciem.
- **Dźwięk gry i mikrofon osobno** — w edytorze dwie ścieżki z wykresem dźwięku: ściszysz albo wyciszysz jedno lub drugie i od razu to słychać.
- **Edytor** — przycinanie, kadrowanie, zmniejszanie do 10 / 20 / 50 / 100 MB i zapis jako zapętlony GIF / WebP, np. pod Discorda. Głośność słychać od razu, a `←` / `→` przewijają o 5 s, `,` / `.` o klatkę.
- **Przeciągnij i upuść** — gotowy klip przeciągasz prosto do Discorda, przeglądarki albo folderu.
- **Biblioteka** — klipy pogrupowane według dni, zaznaczanie wielu naraz, usuwanie do Kosza.
- **Nie przestaje nagrywać po cichu** — sam wznawia nagrywanie po problemie ze sterownikiem, przechodzi na inny sposób przechwytywania, gdy któryś nie działa, i ostrzega, gdy coś jest nie tak.

## Wymagania

- Windows 10 (wersja 2004 lub nowsza) albo Windows 11, 64-bit
- Zalecana karta graficzna AMD Radeon RX 400+, NVIDIA GTX 600+ lub grafika Intel HD 500+ (bez niej Clipeon koduje procesorem)

## Szybki start

1. Pobierz `Clipeon_x.y.z_x64-setup.exe` z [najnowszego wydania](https://github.com/Atsuieon/Clipeon/releases/latest) i zainstaluj.
2. Clipeon działa w tle — ikona przy zegarze. Kropka jest zielona, gdy nagrywa.
3. Graj i wciśnij **Alt+F10**, gdy wydarzy się coś wartego zapisania. Klik w ikonę otwiera bibliotekę klipów.

## Pytania

<details>
<summary><b>Windows pokazuje „System Windows ochronił ten komputer”.</b></summary>

Instalator nie jest podpisany płatnym certyfikatem, więc SmartScreen go nie zna. Kliknij **Więcej informacji → Uruchom mimo to**.
Instalator jest budowany automatycznie z kodu w tym repozytorium.
</details>

<details>
<summary><b>Gdzie są moje klipy?</b></summary>

W folderze `Wideo\Clipeon`, w podfolderze z miesiącem (np. `2026-11`); starsze klipy leżą prosto w folderze. Folder zmienisz w Ustawieniach.
Odinstalowanie nigdy nie usuwa klipów.
</details>

<details>
<summary><b>Jak zmienić skrót?</b></summary>

Ustawienia → Zapisywanie klipów → Skrót do zapisania klipu. Kliknij pole i wciśnij nową kombinację klawiszy.
</details>

<details>
<summary><b>Jak nagrać pulpit zamiast gry?</b></summary>

Na pasku u góry okna kliknij **Pulpit** (albo w menu ikony przy zegarze „Tryb: Pulpit”). Obok wybierzesz ekran.
Pulpit ma osobne ustawienia w Ustawieniach → Tryb pulpitu, więc przełączanie nie zmienia ustawień gry.
Klipy z pulpitu mają nazwy `Clipeon_Pulpit_…` i znaczek „Pulpit” w bibliotece.
</details>

<details>
<summary><b>Coś nie działa — jak zgłosić błąd?</b></summary>

Ustawienia → Zgłaszanie problemów → **Kopiuj diagnostykę** i wklej tekst do [zgłoszenia](https://github.com/Atsuieon/Clipeon/issues/new/choose).
Diagnostyka opisuje kartę graficzną, monitory, ustawienia i ostatnie zdarzenia. Nic nie jest wysyłane automatycznie.
</details>

<details>
<summary><b>Używałem Clippera — co teraz?</b></summary>

Clipeon to ta sama aplikacja pod nową nazwą. Zainstaluj go: instalator sam usunie Clippera, a ustawienia, autostart i klipy zostaną.
</details>

**Prywatność:** wszystko zostaje na Twoim komputerze — jedyne połączenie z internetem to sprawdzanie aktualizacji na GitHubie (można wyłączyć).

## English

Clipeon keeps the last seconds of your gameplay in memory and saves them as an MP4 when you press **Alt+F10**.
GPU capture and encoding (AMD / NVIDIA / Intel), a desktop recording mode, separate game and mic tracks, trimming, shrink-to-size for Discord,
drag-and-drop clips into other apps. Windows 10 2004+ / 11. [Download the latest release](https://github.com/Atsuieon/Clipeon/releases/latest)
— the installer is unsigned, so SmartScreen may warn: *More info → Run anyway*. The interface is in Polish.

## Licencja

[MIT](LICENSE) © 2026 Atsuieon. FFmpeg w instalatorze: LGPL 2.1+ — zob. [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
