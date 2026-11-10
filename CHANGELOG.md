# Zmiany

## 1.2.0

**Aktualizacje**
- **Aktualizacja jednym kliknięciem:** Clipeon sam sprawdza, czy jest nowa wersja, i pokazuje przycisk
  „Aktualizuj” (w oknie, w ustawieniach i w menu ikony przy zegarze). Po kliknięciu sam pobiera instalator,
  sprawdza jego podpis, instaluje go i uruchamia się ponownie — bez wchodzenia na stronę GitHuba.
  O nowej wersji znalezionej w tle informuje też dymek (raz na wersję).

**Nagrywanie u każdego**
- Działa też na starszym Windows 10: gdy system nie pozwala wyłączyć żółtej ramki przechwytywania
  (albo nie obsługuje innych nowszych opcji), Clipeon nagrywa dalej z ustawieniami domyślnymi zamiast przerywać.
- Karty NVIDIA: FFmpeg w zestawie działa ze sterownikami od wersji 570 (wcześniej wymagał 610 i przy starszym
  sterowniku nagrywanie spadało na procesor). Przy za starym sterowniku Clipeon pokazuje, że trzeba go zaktualizować.
- Kilka kart graficznych (np. grafika w procesorze i osobna karta): Clipeon koduje na najmocniejszej.
- Sterowniki: wersja każdej karty trafia do dziennika, a zmiana sterownika jest odnotowana. Gdy enkoder
  przestanie działać (np. po aktualizacji sterownika), Clipeon sprawdza go od nowa i w razie potrzeby bierze inny.
- Wykrywanie enkoderów jest odporniejsze: więcej czasu na odpowiedź karty przy starcie Windows, druga próba
  i przyczyna w dzienniku, gdy enkoder nie działa.
- Jakość dopasowana do ekranu: ustawienie dotyczy 1080p, przy 1440p i 4K Clipeon sam podnosi bitrate.

**Bezpieczeństwo klipów**
- Brak miejsca na dysku nie zostawi uszkodzonego klipu: Clipeon sprawdza wolne miejsce przed zapisem,
  zapisuje do pliku tymczasowego i nadaje nazwę dopiero po udanym zapisie. To samo przy przycinaniu i zmniejszaniu.
- Po awarii Clipeon sam uruchamia się ponownie i wraca do nagrywania (z dymkiem, że wrócił).

## 1.1.0

- Nowa nazwa: **Clipeon** (wcześniej Clipper). Instalator sam usuwa starą wersję, a Clipeon przejmuje
  ustawienia, autostart i folder z klipami — nic nie trzeba robić ręcznie.
- Nowe klipy mają nazwy `Clipeon_RRRR-MM-DD_GG-MM-SS.mp4`; dotychczasowe zostają bez zmian.
- Repozytorium: https://github.com/Atsuieon/Clipeon (stary adres przekierowuje).

## 1.0.0 — pierwsze wydanie (jako Clipper)

- Nagrywanie w tle ostatnich 10–300 s gry (domyślnie 30 s) i zapis klipu jednym skrótem (domyślnie Alt+F10).
- Kodowanie na karcie graficznej: AMD AMF, NVIDIA NVENC, Intel Quick Sync — wykrywane automatycznie;
  awaryjnie procesor (Windows Media Foundation). Formaty H.264, HEVC i AV1.
- Dźwięk z gry i mikrofon jako osobne ścieżki (plus wspólny miks).
- Biblioteka klipów z podziałem na dni, miniaturami, zaznaczaniem wielu klipów i usuwaniem do Kosza.
- Edytor: przycinanie, zmniejszanie do 10 / 20 / 50 / 100 MB, głośność gry i mikrofonu, „z moim głosem / bez”,
  przeciąganie gotowego pliku prosto do Discorda, Eksploratora lub przeglądarki.
- Dźwięk i powiadomienie w rogu ekranu po zapisie (nie zabiera fokusu z gry).
- Strażnik nagrywania: sam wznawia nagrywanie po awarii enkodera, zmianie rozdzielczości czy uśpieniu komputera;
  alarm, gdy nagrywanie nie działa, i dziennik zdarzeń.
- Automatyczne porządki w folderze klipów (opcjonalnie).
- Sprawdzanie aktualizacji na GitHubie (można wyłączyć).
- Instalator krok po kroku (bez uprawnień administratora), FFmpeg w zestawie.
