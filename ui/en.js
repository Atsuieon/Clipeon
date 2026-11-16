// Słownik polski → angielski. Plik jest zwykłym skryptem okna (bez pobierania w trakcie działania),
// a rdzeń wczytuje ten sam plik przy kompilacji (src/jezyk.rs): wszystko między pierwszym „{” a ostatnim „}” to JSON.
window.SLOWNIK_EN = {
 "_opis": "Słownik polski → angielski dla okna (jezyk.js) i rdzenia (src/jezyk.rs). Klucz = polski tekst z okna albo z kodu Rust, „wzory” = wyrażenia regularne z $1, $2 (części są tłumaczone dalej).",
 "teksty": {
  "Minimalizuj": "Minimize",
  "Maksymalizuj": "Maximize",
  "Przywróć": "Restore",
  "Zamknij": "Close",
  "Klipy": "Clips",
  "Filmy": "Films",
  "Ustawienia": "Settings",
  "Co nagrywać": "What to record",
  "Nagrywanie gry: ustawienia z karty „Ekran gry”": "Game recording: settings from the “Game screen” card",
  "Gra": "Game",
  "Pulpit": "Desktop",
  "Nagrywanie pulpitu: osobny ekran, mniej klatek i mniejsze pliki": "Desktop recording: separate screen, fewer frames and smaller files",
  "Łączenie…": "Connecting…",
  "Mikrofon nie jest nagrywany — włączysz go w Ustawieniach → Dźwięk": "The microphone is not being recorded — turn it on in Settings → Audio",
  "Spróbuj ponownie": "Try again",
  "Pokaż dziennik": "Show log",
  "Kopiuje opis sprzętu, ustawień i ostatnich zdarzeń — wklej go do zgłoszenia błędu": "Copies a description of your hardware, settings and recent events — paste it into a bug report",
  "Kopiuj diagnostykę": "Copy diagnostics",
  "Odśwież": "Refresh",
  "Zaznaczone klipy": "Selected clips",
  "Zaznacz wszystko": "Select all",
  "Odznacz": "Deselect",
  "Brak klipów": "No clips yet",
  "Brak filmów": "No films yet",
  "Po zapisaniu zmian nagrywanie zaczyna się od nowa — wcześniejsze sekundy nie trafią do klipu.": "After saving, recording starts over — earlier seconds will not be in the next clip.",
  "Pokazuje opisy, liczby klatek i Mb/s oraz ustawienia zaawansowane": "Shows descriptions, frame rates, Mb/s and advanced settings",
  "Dla nerdów": "For nerds",
  "Jakość nagrywania": "Recording quality",
  "„Gra” i „Pulpit” to przełącznik u góry okna. Każdy tryb ma własne klatki i jakość — zmieniasz tylko kartę, w której klikasz.": "“Game” and “Desktop” is the switch at the top of the window. Each mode has its own frame rate and quality — you only change the card you click in.",
  "Klipy, gdy u góry wybrane jest „Gra”": "Clips when “Game” is selected at the top",
  "Nagrywa teraz": "Recording now",
  "Gotowe ustawienia gry": "Game presets",
  "Klatki/s gry": "Game fps",
  "Mb/s gry": "Game Mb/s",
  "Ekran gry": "Game screen",
  "Dotyczy ekranu 1080p — przy 1440p i 4K Clipeon sam podnosi Mb/s.": "For a 1080p screen — at 1440p and 4K Clipeon raises Mb/s by itself.",
  "Klipy, gdy u góry wybrany jest „Pulpit”": "Clips when “Desktop” is selected at the top",
  "Gotowe ustawienia pulpitu": "Desktop presets",
  "Klatki/s pulpitu": "Desktop fps",
  "Mb/s pulpitu": "Desktop Mb/s",
  "Ekran pulpitu": "Desktop screen",
  "Długie nagrania przyciskiem „Nagraj film”": "Long recordings with the “Record film” button",
  "Rozmiar pliku": "File size",
  "Rozmiar filmów": "Film size",
  "Po nagraniu Clipeon w tle zapisuje film od nowa w mniejszym formacie. Do tego czasu film jest w bibliotece w pełnej jakości.": "After recording, Clipeon re-encodes the film into a smaller format in the background. Until then the film is in the library in full quality.",
  "Nagrywanie": "Recording",
  "Ile ostatnich sekund zapisywać": "How many last seconds to save",
  "Dźwięk": "Audio",
  "Dźwięk z gry i komputera": "Game and computer audio",
  "Zapisuje się osobno od głosu, więc w edytorze ściszysz jedno albo drugie.": "Saved separately from your voice, so in the editor you can turn down either one.",
  "Gra osobno od Discorda": "Game separate from Discord",
  "Dwie ścieżki: sama gra oraz Discord i reszta komputera. W edytorze wyciszysz Discorda, a grę zostawisz.": "Two tracks: the game alone, and Discord plus the rest of the computer. In the editor you can mute Discord and keep the game.",
  "Mój mikrofon": "My microphone",
  "Który mikrofon": "Which microphone",
  "Który mikrofon nagrywać": "Which microphone to record",
  "Zapisywanie klipów": "Saving clips",
  "Skrót do zapisania klipu": "Shortcut to save a clip",
  "Kliknij i wciśnij nową kombinację (Esc anuluje). W tym czasie skróty Clipeona są wyłączone.": "Click and press a new combination (Esc cancels). Clipeon's shortcuts are off meanwhile.",
  "Skrót do nagrywania filmu": "Shortcut to record a film",
  "Zaczyna i kończy długie nagrywanie. Filmy znajdziesz w zakładce Filmy. Delete albo Backspace usuwa skrót.": "Starts and stops a long recording. Films are in the Films tab. Delete or Backspace removes the shortcut.",
  "Bez skrótu": "No shortcut",
  "Skrót znacznika w filmie": "Film marker shortcut",
  "W trakcie filmu zaznacza ważną chwilę. W edytorze kliknij kreskę znacznika na osi czasu albo naciśnij [ lub ], żeby do niej skoczyć.": "Marks an important moment during a film. In the editor click the marker line on the timeline or press [ or ] to jump to it.",
  "Gdzie zapisywać klipy": "Where to save clips",
  "Nowe klipy trafiają do podfolderów z miesiącem, np. 2026-11.": "New clips go into monthly subfolders, e.g. 2026-11.",
  "Zmień folder…": "Change folder…",
  "Dźwięk po zapisaniu": "Sound after saving",
  "Żebyś wiedział, że się udało, bez wychodzenia z gry.": "So you know it worked without leaving the game.",
  "Powiadomienie w rogu ekranu": "Notification in the screen corner",
  "Usuwaj klipy starsze niż": "Delete clips older than",
  "Stare klipy idą do Kosza, więc da się je przywrócić.": "Old clips go to the Recycle Bin, so you can restore them.",
  "Liczba dni": "Number of days",
  "dni": "days",
  "Klipy zajmą najwyżej": "Clips take up at most",
  "Limit w GB": "Limit in GB",
  "System": "System",
  "Uruchamiaj razem z Windows": "Start with Windows",
  "Clipeon działa w tle jako ikona na pasku zadań, obok zegara.": "Clipeon runs in the background as an icon on the taskbar, next to the clock.",
  "Ikona zawsze widoczna na pasku zadań": "Icon always visible on the taskbar",
  "Obok zegara, a nie schowana pod strzałką „^”.": "Next to the clock, not hidden under the “^” arrow.",
  "Sprawdzaj, czy jest nowa wersja": "Check for new versions",
  "Raz dziennie pytamy GitHuba. To jedyne połączenie z internetem — klipy nigdy nie opuszczają komputera.": "Once a day we ask GitHub. That is the only internet connection — clips never leave your computer.",
  "Animowane tło": "Animated background",
  "Znaki w tle okna i smuga, którą zostawia ruch myszki. Działa tylko, gdy okno Clipeona jest otwarte i aktywne.": "Characters in the window background and a trail left by the mouse. Runs only while the Clipeon window is open and active.",
  "Napis „Clipeon” z cząsteczek": "“Clipeon” title made of particles",
  "Napis w rogu okna złożony z kropek: zlatują się przy otwarciu okna, odsuwają przed kursorem i rozpryskują po kliknięciu. Po wyłączeniu napis jest zwykły.": "The title in the window corner made of dots: they fly in when the window opens, move away from the cursor and scatter on click. When off, the title is plain.",
  "Aktualizacja": "Update",
  "Sprawdź teraz": "Check now",
  "Aktualizuj teraz": "Update now",
  "Zaawansowane": "Advanced",
  "Format wideo": "Video format",
  "Sposób przechwytywania obrazu": "Screen capture method",
  "Zostaw „Automatycznie”, chyba że nagrania się zacinają albo są czarne.": "Leave “Automatic” unless recordings stutter or are black.",
  "Automatycznie (polecane)": "Automatic (recommended)",
  "Desktop Duplication — najlżejszy": "Desktop Duplication — lightest",
  "Desktop Duplication przez RAM — bez ramki, odporny na zmiany ekranu": "Desktop Duplication via RAM — no border, survives screen changes",
  "Tryb zgodności": "Compatibility mode",
  "Zgłaszanie problemów": "Reporting problems",
  "Kopiuje opis sprzętu, ustawień i ostatnich zdarzeń. Nic nie jest wysyłane automatycznie.": "Copies a description of your hardware, settings and recent events. Nothing is sent automatically.",
  "· by": "· by",
  "Clipeon jest darmowy i taki zostanie. Jeśli chcesz pomóc, postaw kawę na Ko-fi (otworzy się w przeglądarce).": "Clipeon is free and will stay free. If you want to help, buy a coffee on Ko-fi (opens in your browser).",
  "Cofnij": "Undo",
  "Zapisz zmiany": "Save changes",
  "Zapisać zmiany?": "Save changes?",
  "Te ustawienia są zmienione, ale jeszcze niezapisane. Bez zapisu Clipeon nagrywa dalej po staremu.": "These settings are changed but not saved yet. Without saving, Clipeon keeps recording the old way.",
  "Wróć do ustawień": "Back to settings",
  "Nie zapisuj": "Don't save",
  "Zapisz": "Save",
  "Kliknij: odtwórz / pauza · Przeciągnij: wrzuć plik do Discorda, Eksploratora lub przeglądarki": "Click: play / pause · Drag: drop the file into Discord, Explorer or a browser",
  "Pełny ekran (F)": "Full screen (F)",
  "Kadruj obraz (C)": "Crop the picture (C)",
  "Kliknij, żeby przewinąć": "Click to seek",
  "Początek klipu (strzałki przesuwają)": "Clip start (arrow keys move it)",
  "Koniec klipu (strzałki przesuwają)": "Clip end (arrow keys move it)",
  "Przycinanie": "Trimming",
  "Początek tutaj": "Start here",
  "Koniec tutaj": "End here",
  "Cały klip": "Whole clip",
  "Format": "Format",
  "Format zapisu": "Save format",
  "Zapętlony, bez dźwięku, do 15 s": "Looping, no sound, up to 15 s",
  "Zapętlony, bez dźwięku, do 15 s — mniejszy niż GIF": "Looping, no sound, up to 15 s — smaller than GIF",
  "Kadr": "Crop",
  "Proporcje kadru": "Crop proportions",
  "Dowolny": "Free",
  "Bez kadru": "No crop",
  "Bez tego cięcie jest natychmiastowe, ale zaczyna się od najbliższej klatki kluczowej (do 1 s wcześniej)": "Without it the cut is instant, but starts at the nearest keyframe (up to 1 s earlier)",
  "Dokładne cięcie": "Exact cut",
  "Wyłączone: nowa wersja zastępuje oryginał, a oryginał trafia do Kosza": "Off: the new version replaces the original, and the original goes to the Recycle Bin",
  "Zachowaj oryginał": "Keep the original",
  "Przeciągnij do Discorda, Eksploratora lub przeglądarki": "Drag into Discord, Explorer or a browser",
  "Otwórz": "Open",
  "Potem Ctrl+V w Discordzie": "Then Ctrl+V in Discord",
  "Kopiuj": "Copy",
  "Pokaż w folderze": "Show in folder",
  "Język": "Language",
  "Język okna, dymków i menu przy zegarze.": "Language of the window, notifications and tray menu.",
  "Jak Windows": "Same as Windows",
  "Język / Language": "Language",
  "Nie nagrywa": "Not recording",
  "Wznawiam…": "Resuming…",
  "Przełączam…": "Switching…",
  "Wstrzymane": "Paused",
  "Nagrywa pulpit": "Recording the desktop",
  "Nagrywa": "Recording",
  "Clipeon ma problem": "Clipeon has a problem",
  "Clipeon wznawia nagrywanie": "Clipeon is resuming recording",
  "Wznów": "Resume",
  "Wstrzymaj": "Pause",
  "Składam nagranie w jeden plik": "Joining the recording into one file",
  "Zapisuję film": "Saving the film",
  "Zakończ film": "End film",
  "Nagraj film": "Record film",
  "Zapisuję film…": "Saving the film…",
  "Film zapisany": "Film saved",
  "Film zmniejszony": "Film shrunk",
  "Desktop Duplication przez RAM": "Desktop Duplication via RAM",
  "Ekran": "Screen",
  "Automatycznie": "Automatic",
  "Który ekran nagrywać": "Which screen to record",
  "Którą grę nagrywać": "Which game to record",
  "Nagrywa cały czas, nazwę bierze z okna na wierzchu": "Records all the time, takes the name from the window on top",
  "Nie znaleziono monitorów": "No monitors found",
  "Skopiowano diagnostykę — wklej ją do zgłoszenia": "Diagnostics copied — paste them into the report",
  "Uruchamiam nagrywanie od nowa…": "Restarting recording…",
  "Zapisuję klip…": "Saving the clip…",
  "Kliknij „Nagraj film”, aby nagrywać bez przerwy, aż klikniesz jeszcze raz.": "Click “Record film” to record non-stop until you click again.",
  "Usuń zaznaczone": "Delete selected",
  "Przeniesiono do Kosza": "Moved to the Recycle Bin",
  "Styczeń": "January",
  "Luty": "February",
  "Marzec": "March",
  "Kwiecień": "April",
  "Maj": "May",
  "Czerwiec": "June",
  "Lipiec": "July",
  "Sierpień": "August",
  "Wrzesień": "September",
  "Październik": "October",
  "Listopad": "November",
  "Grudzień": "December",
  "Dzisiaj": "Today",
  "Wczoraj": "Yesterday",
  "Wcześniej w tym miesiącu": "Earlier this month",
  "Plik gotowy — przeciągnij jeszcze raz": "File ready — drag again",
  "Poczekaj, aż skończy się zapisywanie": "Wait until saving finishes",
  "Ten klip nie odtwarza się w podglądzie (HEVC wymaga rozszerzenia HEVC z Microsoft Store). Przycinanie i kompresja nadal działają.": "This clip does not play in the preview (HEVC needs the HEVC extension from the Microsoft Store). Trimming and compression still work.",
  "Pełny ekran jest niedostępny": "Full screen is not available",
  "Wyjdź z pełnego ekranu (F)": "Exit full screen (F)",
  "Odtwórz (spacja)": "Play (space)",
  "Pauza (spacja)": "Pause (space)",
  "Ten ffmpeg nie umie zapisać WebP": "This ffmpeg cannot write WebP",
  "Bez limitu": "No limit",
  "Bez zmian": "No change",
  "Do 10 MB (Discord)": "Up to 10 MB (Discord)",
  "Zapisz wykadrowany": "Save cropped",
  "Zapisz przycięty": "Save trimmed",
  "Zapisz z nową głośnością": "Save with new volume",
  "Przytnij klip, wykadruj, wybierz rozmiar albo zmień dźwięk, żeby zapisać nową wersję.": "Trim, crop, pick a size or change the audio to save a new version.",
  "Nowa wersja zastąpi oryginał (oryginał trafi do Kosza).": "The new version will replace the original (the original goes to the Recycle Bin).",
  "Mikrofon": "Microphone",
  "Discord i reszta": "Discord and the rest",
  "mojego głosu": "my voice",
  "Discorda i reszty": "Discord and the rest",
  "dźwięku gry": "game audio",
  "Dalej nie ma znaczników": "No more markers",
  "Ten film nie ma znaczników": "This film has no markers",
  "Nie znam rozmiaru obrazu tego klipu — kadrowanie niedostępne": "Unknown picture size of this clip — cropping unavailable",
  "Mierzę…": "Measuring…",
  "Przygotowuję plik…": "Preparing the file…",
  "Zapisuję…": "Saving…",
  "Zapisano. Oryginał jest w Koszu": "Saved. The original is in the Recycle Bin",
  "Przygotowuję plik do przeciągnięcia…": "Preparing the file for dragging…",
  "Skopiowano — wklej Ctrl+V w Discordzie": "Copied — paste with Ctrl+V in Discord",
  "Klip przeniesiony do Kosza": "Clip moved to the Recycle Bin",
  "Poczekaj, aż skończy się poprzedni zapis": "Wait until the previous save finishes",
  "Przytnij…": "Trim…",
  "Kopiuj (wklej Ctrl+V w Discordzie)": "Copy (paste with Ctrl+V in Discord)",
  "Usuń": "Delete",
  "Lekka": "Light",
  "Standard": "Standard",
  "Najwyższa": "Highest",
  "Płynna": "Smooth",
  "Własne": "Custom",
  "Własne klatki/s i Mb/s — zmienisz je po włączeniu „Dla nerdów”": "Custom fps and Mb/s — change them after turning on “For nerds”",
  "Oryginał": "Original",
  "Mniejszy": "Smaller",
  "HEVC · ok. ½": "HEVC · about ½",
  "Najmniejszy": "Smallest",
  "AV1 · ok. ⅓": "AV1 · about ⅓",
  "Domyślny systemowy": "System default",
  "Wybrany mikrofon (niepodłączony)": "Selected microphone (disconnected)",
  "Główny monitor": "Main monitor",
  "Wybrany format (ta karta go nie zapisuje)": "Selected format (this card can't write it)",
  "Mikrofon jest wyłączony. Wybranie urządzenia go włączy.": "The microphone is off. Picking a device turns it on.",
  "Podgląd w Clipeonie wymaga rozszerzenia HEVC z Microsoft Store, a Discord w przeglądarce może takiego klipu nie odtworzyć.": "Preview in Clipeon needs the HEVC extension from the Microsoft Store, and Discord in a browser may not play such a clip.",
  "Karty Radeon RX 7000 zapisują ten format w 1920×1082 — na dole klipu są 2 czarne linie.": "Radeon RX 7000 cards write this format at 1920×1082 — there are 2 black lines at the bottom of the clip.",
  "Ten monitor ma włączony HDR. Klipy zapisują się w zwykłych kolorach (SDR), więc mogą wyglądać na bledsze niż w grze.": "This monitor has HDR on. Clips are saved in normal colours (SDR), so they may look paler than in the game.",
  "Film ma jakość trybu, który akurat nagrywa. Teraz:": "A film has the quality of the mode that is recording. Now:",
  "Gra: klatki/s": "Game: fps",
  "Gra: Mb/s": "Game: Mb/s",
  "Pulpit: klatki/s": "Desktop: fps",
  "Pulpit: Mb/s": "Desktop: Mb/s",
  "Ikona przy zegarze": "Icon next to the clock",
  "Sprawdzanie nowej wersji": "Checking for new versions",
  "Napis z cząsteczek": "Particle title",
  "Usuwanie starych klipów": "Deleting old clips",
  "Limit miejsca na klipy": "Space limit for clips",
  "Uruchamianie razem z Windows": "Start with Windows",
  "Zapisano. Nagrywanie wystartowało z nowymi ustawieniami.": "Saved. Recording restarted with the new settings.",
  "Zapisano. Nowe ustawienia działają.": "Saved. The new settings are active.",
  "Naciśnij skrót…": "Press a shortcut…",
  "Folder": "Folder",
  "Kopiuj plik klipu (Ctrl+C) — potem Ctrl+V w Discordzie": "Copy the clip file (Ctrl+C) — then Ctrl+V in Discord",
  "Pobierz i zainstaluj nową wersję. Clipeon uruchomi się ponownie sam.": "Download and install the new version. Clipeon restarts by itself.",
  "Instalowanie… Clipeon uruchomi się ponownie": "Installing… Clipeon will restart",
  "Sprawdzam…": "Checking…",
  "Masz najnowszą wersję.": "You have the latest version.",
  "Klip zapisany": "Clip saved",
  "Zaznacz": "Select",
  "Nagrane w trybie pulpitu": "Recorded in desktop mode",
  " · HDR": " · HDR",
  "H.264 — karta AMD (działa wszędzie)": "H.264 — AMD card (plays everywhere)",
  "H.264 — karta NVIDIA (działa wszędzie)": "H.264 — NVIDIA card (plays everywhere)",
  "H.264 — grafika Intel (działa wszędzie)": "H.264 — Intel graphics (plays everywhere)",
  "HEVC — karta AMD (mniejsze pliki)": "HEVC — AMD card (smaller files)",
  "HEVC — karta NVIDIA (mniejsze pliki)": "HEVC — NVIDIA card (smaller files)",
  "HEVC — grafika Intel (mniejsze pliki)": "HEVC — Intel graphics (smaller files)",
  "AV1 — karta AMD (najmniejsze pliki)": "AV1 — AMD card (smallest files)",
  "AV1 — karta NVIDIA (najmniejsze pliki)": "AV1 — NVIDIA card (smallest files)",
  "AV1 — grafika Intel (najmniejsze pliki)": "AV1 — Intel graphics (smallest files)",
  "H.264 — karta przez Windows (starszy sterownik)": "H.264 — card through Windows (older driver)",
  "H.264 — procesor (wolniejsze, gdy karta nie działa)": "H.264 — processor (slower, when the card doesn't work)",
  "Pełna jakość — bez zmian (największy plik)": "Full quality — unchanged (largest file)",
  "Mniejszy plik — HEVC, ok. połowa rozmiaru": "Smaller file — HEVC, about half the size",
  "Najmniejszy plik — AV1, ok. 1/3 rozmiaru": "Smallest file — AV1, about 1/3 of the size",
  "Pokaż dziennik zdarzeń": "Show event log",
  "Sprawdź aktualizacje": "Check for updates",
  "Wstrzymaj nagrywanie": "Pause recording",
  "Wznów nagrywanie": "Resume recording",
  "Tryb: Gra": "Mode: Game",
  "Tryb: Pulpit": "Mode: Desktop",
  "Otwórz folder klipów": "Open clips folder",
  "Biblioteka…": "Library…",
  "Ustawienia…": "Settings…",
  "Zakończ": "Quit",
  "○ Uruchamiam nagrywanie…": "○ Starting recording…",
  "Nie udało się zapisać klipu": "Couldn't save the clip",
  "Clipeon nie nagrywa": "Clipeon is not recording",
  "Problem z mikrofonem": "Microphone problem",
  "Nagrywanie znów działa": "Recording works again",
  "Clipeon sam wznowił nagrywanie": "Clipeon resumed recording by itself",
  "Nagrywam film": "Recording a film",
  "Film zakończony": "Film ended",
  "Koniec filmu": "End of film",
  "Zapisuję plik…": "Saving the file…",
  "Nie nagrywasz filmu": "You are not recording a film",
  "Film został w pełnej jakości": "The film stayed in full quality",
  "Nie udało się nagrać filmu": "Couldn't record the film",
  "Nagrywanie działa wolniej": "Recording runs slower",
  "Zakończysz go w oknie Clipeona albo w menu przy zegarze": "End it in the Clipeon window or in the menu next to the clock",
  "Znaczniki działają w trakcie filmu — zacznij go w oknie Clipeona": "Markers work during a film — start one in the Clipeon window",
  "Schowek jest zajęty przez inny program — spróbuj jeszcze raz": "The clipboard is busy in another program — try again",
  "Nic do zrobienia — przytnij klip, wybierz rozmiar albo zmień głośność": "Nothing to do — trim the clip, pick a size or change the volume",
  "Zaznaczony fragment jest za krótki (minimum 0,5 s)": "The selected part is too short (at least 0.5 s)",
  "części": "parts",
  "film": "film",
  "Nagrywanie wstrzymane": "Recording paused",
  "Kliknij, żeby otworzyć Clipeona i zaktualizować": "Click to open Clipeon and update",
  "Aktualizacja się nie udała": "The update failed",
  "Pobieranie aktualizacji…": "Downloading the update…",
  "Wybrana gra": "Selected game",
  "Nagrywaj tylko grę": "Record only the game",
  "Po Alt+Tab obraz zatrzymuje się na ostatniej klatce gry, więc Discord ani przeglądarka nie trafią do klipu. Dźwięk nagrywa się dalej.": "After Alt+Tab the picture freezes on the last game frame, so Discord or your browser never end up in a clip. Sound keeps recording.",
  "Zapisz klip": "Save clip",
  "Znacznik w filmie": "Film marker",
  "Skróty": "Shortcuts"
 },
 "wzory": [
  [
   "^Czekam na (.+)$",
   "Waiting for $1"
  ],
  [
   "^Nagrywa: (.+)$",
   "Recording: $1"
  ],
  [
   "^Wznawiam nagrywanie: (.+)$",
   "Resuming recording: $1"
  ],
  [
   "^(.*?)(\\d+) kl\\./s(.*)$",
   "$1$2 fps$3"
  ],
  [
   "^Windows zmienił urządzenie dźwięku \\(np\\. po zamknięciu gry\\)\\. Clipeon przełącza na nowe: (.+)\\. Obraz nagrywa się dalej\\.$",
   "Windows changed the audio device (e.g. after closing a game). Clipeon is switching to the new one: $1. The picture keeps recording."
  ],
  [
   "^Zakończ film i zapisz go(.*)$",
   "End the film and save it$1"
  ],
  [
   "^Nagraj film: nagrywa bez przerwy, aż klikniesz jeszcze raz(.*)\\. Klipy działają w tym czasie jak zwykle\\.$",
   "Record film: records non-stop until you click again$1. Clips work as usual meanwhile."
  ],
  [
   "^Zmniejszam ostatni film: (\\d+)%\\. Do końca jest w bibliotece w pełnej jakości\\.$",
   "Shrinking the last film: $1%. Until it's done it is in the library in full quality."
  ],
  [
   "^Teraz: (.+)\\. Ten sposób pokazuje na tym Windows ramkę wokół nagrywanego obrazu; Clipeon wróci do Desktop Duplication, gdy tylko znów zadziała\\.$",
   "Now: $1. On this Windows this method shows a border around the recorded picture; Clipeon goes back to Desktop Duplication as soon as it works again."
  ],
  [
   "^Teraz: (.+)\\.$",
   "Now: $1."
  ],
  [
   "^Nagrywany ekran: (.+) \\(zmień\\)$",
   "Recorded screen: $1 (change)"
  ],
  [
   "^Nagrywana gra: (.+) \\(zmień\\)$",
   "Recorded game: $1 (change)"
  ],
  [
   "^Nagrywam pulpit(.*)$",
   "Recording the desktop$1"
  ],
  [
   "^Nagrywam grę(.*)$",
   "Recording the game$1"
  ],
  [
   "^Ekran (\\d+) · główny$",
   "Screen $1 · main"
  ],
  [
   "^Ekran (\\d+)$",
   "Screen $1"
  ],
  [
   "^Nagrywa tylko, gdy działa (.+)$",
   "Records only while $1 is running"
  ],
  [
   "^Nie udało się skopiować: (.+)$",
   "Couldn't copy: $1"
  ],
  [
   "^1 (klip|film) · (.+)$",
   "1 $1 · $2"
  ],
  [
   "^(\\d+) klip(?:y|ów) · (.+)$",
   "$1 clips · $2"
  ],
  [
   "^(\\d+) film(?:y|ów) · (.+)$",
   "$1 films · $2"
  ],
  [
   "^1 klip zaznaczony$",
   "1 clip selected"
  ],
  [
   "^1 film zaznaczony$",
   "1 film selected"
  ],
  [
   "^(\\d+) klip(?:y zaznaczone|ów zaznaczonych)$",
   "$1 clips selected"
  ],
  [
   "^(\\d+) film(?:y zaznaczone|ów zaznaczonych)$",
   "$1 films selected"
  ],
  [
   "^1 klip$",
   "1 clip"
  ],
  [
   "^1 film$",
   "1 film"
  ],
  [
   "^(\\d+) klip(?:y|ów)$",
   "$1 clips"
  ],
  [
   "^(\\d+) film(?:y|ów)$",
   "$1 films"
  ],
  [
   "^Naciśnij (.+) w grze, aby zapisać ostatnie (\\d+) s\\.$",
   "Press $1 in game to save the last $2 s."
  ],
  [
   "^Naciśnij (.+) albo kliknij „Nagraj film”, aby nagrywać bez przerwy\\. Ten sam skrót kończy nagranie\\.$",
   "Press $1 or click “Record film” to record non-stop. The same shortcut ends the recording."
  ],
  [
   "^Przenieść do Kosza: (.+)\\? Da się je przywrócić z Kosza\\.$",
   "Move to the Recycle Bin: $1? You can restore them from the Recycle Bin."
  ],
  [
   "^Nie udało się usunąć (\\d+) z (\\d+) \\(są w użyciu\\)$",
   "Couldn't delete $1 of $2 (they are in use)"
  ],
  [
   "^(Styczeń|Luty|Marzec|Kwiecień|Maj|Czerwiec|Lipiec|Sierpień|Wrzesień|Październik|Listopad|Grudzień) (\\d{4})$",
   "$1 $2"
  ],
  [
   "^(\\d+) dni temu$",
   "$1 days ago"
  ],
  [
   "^Zaznaczono (.+) s$",
   "Selected $1 s"
  ],
  [
   "^Zapętlony, bez dźwięku, najwyżej (\\d+) s$",
   "Looping, no sound, at most $1 s"
  ],
  [
   "^Plik ma już (.+)$",
   "The file is already $1"
  ],
  [
   "^Zapisz jako (.+)$",
   "Save as $1"
  ],
  [
   "^Zapisz bez (.+)$",
   "Save without $1"
  ],
  [
   "^(\\w+) może mieć najwyżej (\\d+) s — przytnij zaznaczenie$",
   "$1 can be at most $2 s — trim the selection"
  ],
  [
   "^Zapisuję… (\\d+)%$",
   "Saving… $1%"
  ],
  [
   "^Ścieżka (\\d+)$",
   "Track $1"
  ],
  [
   "^Głośność: (.+)$",
   "Volume: $1"
  ],
  [
   "^Włącz: (.+)$",
   "Unmute: $1"
  ],
  [
   "^Wycisz: (.+)$",
   "Mute: $1"
  ],
  [
   "^Znacznik (\\d+) \\((.+)\\)$",
   "Marker $1 ($2)"
  ],
  [
   "^Zaznaczyłem (\\d+) s — przesuń uchwyty, żeby wybrać inny fragment$",
   "Selected $1 s — move the handles to pick another part"
  ],
  [
   "^(.+) · przeciągnij do Discorda, Eksploratora lub przeglądarki$",
   "$1 · drag into Discord, Explorer or a browser"
  ],
  [
   "^Przenieść klip „(.+)” do Kosza\\?$",
   "Move the clip “$1” to the Recycle Bin?"
  ],
  [
   "^Kompresja do (\\d+) MB…(.*)$",
   "Compressing to $1 MB…$2"
  ],
  [
   "^Gotowe: (.+)$",
   "Done: $1"
  ],
  [
   "^Kompresuj do (\\d+) MB$",
   "Compress to $1 MB"
  ],
  [
   "^(\\d+) klatek na sekundę, (\\d+) Mb/s \\(polecane\\)$",
   "$1 frames per second, $2 Mb/s (recommended)"
  ],
  [
   "^(\\d+) klatek na sekundę, (\\d+) Mb/s$",
   "$1 frames per second, $2 Mb/s"
  ],
  [
   "^Domyślny systemowy \\((.+)\\)$",
   "System default ($1)"
  ],
  [
   "^(.+) \\(niepodłączony\\)$",
   "$1 (disconnected)"
  ],
  [
   "^Godzina filmu zajmie ok\\. (.+) zamiast (.+)\\. Podgląd HEVC w Clipeonie wymaga rozszerzenia HEVC z Microsoft Store\\.$",
   "An hour of film takes about $1 instead of $2. HEVC preview in Clipeon needs the HEVC extension from the Microsoft Store."
  ],
  [
   "^Godzina filmu zajmie ok\\. (.+) zamiast (.+)\\. AV1 odtwarza Clipeon, przeglądarki i Discord; starsze programy mogą go nie otworzyć\\.$",
   "An hour of film takes about $1 instead of $2. AV1 plays in Clipeon, browsers and Discord; older programs may not open it."
  ],
  [
   "^Godzina filmu zajmie ok\\. (.+)\\.$",
   "An hour of film takes about $1."
  ],
  [
   "^Zajmuje to ok\\. (.+) pamięci RAM w trybie Gra i (.+) w trybie Pulpit\\.$",
   "This takes about $1 of RAM in Game mode and $2 in Desktop mode."
  ],
  [
   "^Ten ekran odświeża się (\\d+) razy na sekundę, więc nagranie i tak będzie miało najwyżej (\\d+) różnych klatek na sekundę\\.$",
   "This screen refreshes $1 times per second, so the recording will have at most $2 different frames per second anyway."
  ],
  [
   "^Niezapisana zmiana: (.+)$",
   "Unsaved change: $1"
  ],
  [
   "^Niezapisane zmiany: (\\d+)$",
   "Unsaved changes: $1"
  ],
  [
   "^i (\\d+) więcej$",
   "and $1 more"
  ],
  [
   "^(.+) — dodaj Ctrl, Alt lub Shift…$",
   "$1 — add Ctrl, Alt or Shift…"
  ],
  [
   "^(.+) jest już zajęty…$",
   "$1 is already taken…"
  ],
  [
   "^Zapisz (\\d+) s$",
   "Save $1 s"
  ],
  [
   "^Skrót: (.+)$",
   "Shortcut: $1"
  ],
  [
   "^Aktualizuj do (.+)$",
   "Update to $1"
  ],
  [
   "^Dostępna wersja (.+)$",
   "Version $1 available"
  ],
  [
   "^Pobieranie aktualizacji…(.*)$",
   "Downloading the update…$1"
  ],
  [
   "^Automatycznie — teraz: (.+)$",
   "Automatic — now: $1"
  ],
  [
   "^Ostatnie (.+)$",
   "Last $1"
  ],
  [
   "^(.+) godz\\. (.+)$",
   "$1 h $2"
  ]
 ]
};
