# Zmiany

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
