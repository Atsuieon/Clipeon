# Windows blokuje ffmpeg („Zasady kontroli aplikacji zablokowały ten plik”)

*English below.*

Clipeon nagrywa i zapisuje klipy programem **ffmpeg** (`ffmpeg\ffmpeg.exe` w folderze Clipeona).
Jeśli Clipeon pokazuje *„Windows zablokował ffmpeg… Smart App Control”* albo w dzienniku jest
*„Zasady kontroli aplikacji zablokowały ten plik (os error 4551)”*, to **Windows nie pozwolił go uruchomić**.
Clipeon nie może tego obejść — to zabezpieczenie systemu.

## Dlaczego

Najczęściej to **Smart App Control** (Windows 11). Wpuszcza tylko programy podpisane certyfikatem albo takie,
które chmura Microsoftu uzna za znane i bezpieczne. Clipeon i jego ffmpeg nie są jeszcze podpisane, więc decyzja
zależy od chmury i bywa różna: raz ffmpeg rusza, a raz nie (dlatego nagrywanie czasem działa, a zapis klipu już nie).

## Co zrobić

1. Otwórz **Zabezpieczenia Windows → Kontrola aplikacji i przeglądarki → Ustawienia funkcji Smart App Control**
   (albo wpisz „Smart App Control” w menu Start).
2. Jeśli jest **Włączone** — to ono blokuje ffmpeg. Możesz przełączyć je na **Wyłączone** i uruchomić Clipeon ponownie.
   - Uwaga: Smart App Control nie ma wyjątków dla pojedynczych programów — wyłącza się je w całości.
   - Na części wersji Windows 11 po wyłączeniu nie da się go włączyć z powrotem bez ponownej instalacji systemu.
     Antywirus (Microsoft Defender) i SmartScreen działają dalej.
3. Jeśli Smart App Control jest **Wyłączone** albo go nie ma, blokadę ustawia firma lub szkoła (komputer służbowy) —
   trzeba poprosić administratora o zezwolenie na `ffmpeg.exe` z folderu Clipeona.
4. Jeśli nie chcesz nic wyłączać: poczekaj na wersję Clipeona podpisaną certyfikatem — podpisany program
   Smart App Control wpuszcza.

---

# Windows blocks ffmpeg (“An Application Control policy has blocked this file”)

Clipeon records and saves clips with **ffmpeg** (`ffmpeg\ffmpeg.exe` in the Clipeon folder). If Clipeon says
*“Windows blocked ffmpeg… Smart App Control”* or the log shows *os error 4551*, **Windows refused to start it**.
Clipeon can't work around this; it is a system protection.

**Why:** usually **Smart App Control** (Windows 11). It only allows signed programs or ones Microsoft's cloud
considers known and safe. Clipeon and its ffmpeg are not code-signed yet, so the cloud's answer varies: sometimes
ffmpeg starts, sometimes not.

**What to do:**

1. Open **Windows Security → App & browser control → Smart App Control settings**.
2. If it is **On**, it is blocking ffmpeg. You can switch it **Off** and restart Clipeon. Smart App Control has no
   per-app exceptions, and on some Windows 11 versions it can't be turned back on without reinstalling Windows.
   Microsoft Defender and SmartScreen keep working.
3. If it is **Off** or missing, the block comes from your organization (work or school PC): ask the administrator
   to allow `ffmpeg.exe` from the Clipeon folder.
4. Or wait for a code-signed Clipeon release, which Smart App Control allows.
