# Zmiany

## 1.4.4

- **Zamknięcie albo uruchomienie gry nie pokazuje już błędu dźwięku.** Gdy Windows zmienia albo przebudowuje
  urządzenie dźwięku (gra się zamyka lub startuje, podłączasz inne słuchawki, usługa dźwięku się restartuje),
  nagrywanie dźwięku musi otworzyć urządzenie od nowa. Dawniej Clipeon pokazywał wtedy czerwony błąd
  „Nie można nagrywać (dźwięk systemowy): 0x88890004” i wpisywał `[BŁĄD]` do dziennika, choć po sekundzie
  nagrywał dalej. Teraz kropka robi się na chwilę żółta z napisem „Przełączam…”, a w dzienniku zostaje
  zwykła informacja. Obraz nagrywa się przez cały czas. Błąd pojawia się
  dopiero wtedy, gdy urządzenia nie da się otworzyć przez 10 sekund.
- **Dźwięk po przełączeniu dalej zgadza się z obrazem.** Dawniej sekundowa dziura w dźwięku znikała z klipu,
  więc wszystko po niej było słychać o sekundę za wcześnie (w klipie zapisanym tuż po zamknięciu lub starcie
  gry). Teraz przełączenie trwa ułamek sekundy, a przerwa jest wypełniana ciszą.
- Przełączenie na inne domyślne urządzenie (np. słuchawki) też jest natychmiastowe i bez dziury w klipie.
- **Edytor: linia głośności nie przeskakuje.** Gdy złapało się ścieżkę „Gra” albo „Mikrofon” w innym miejscu
  niż linia, linia od razu skakała pod kursor (np. z 100% na prawie 200%). Teraz rusza z miejsca, w którym
  stoi, i przesuwa się o tyle, o ile ruszy się myszka. Etykieta z procentem przy 0% i 200% zsuwa się płynnie
  zamiast przeskakiwać nad linię i pod nią.
- **Edytor: wykres dźwięku stoi w miejscu.** Dawniej słupki rosły i malały razem z linią głośności. Teraz
  głośność zmienia ich jasność (0% blade, 200% najjaśniejsze), a wysokość tylko odrobinę.

## 1.4.3

Porządki w kodzie — bez zmian w działaniu programu.

- Szybszy start: lista enkoderów z ffmpeg jest pobierana raz (dawniej dwa razy, osobnym procesem ffmpeg).
- Mniej pracy przy nagrywaniu: każda zakodowana klatka obrazu i ramka dźwięku jest kopiowana w pamięci raz
  zamiast dwóch, a cisza w dźwięku nie zajmuje co chwilę nowej pamięci. Możliwości Windows Graphics Capture
  są sprawdzane raz na uruchomienie, a nie przy każdym odświeżeniu stanu.
- Dziennik: zapisane klipy, sprzątanie starych klipów i błędy otwierania okna, dymka i przeciągania trafiają
  teraz do `clipeon.log` (dawniej szły na niewidoczną konsolę).
- Krótszy i czytelniejszy kod (wspólne funkcje zamiast powtórzeń, poprawione mylące nazwy i komentarze).
- Poprawka: po chwilowej utracie obrazu w nagrywaniu „Desktop Duplication przez RAM” pierwsza klatka po
  powrocie mogła być czarna.
- Poprawka: przy szybkiej zmianie klipu w edytorze podgląd dźwięku mógł wczytać ścieżkę z poprzedniego klipu.
- Poprawka: gdy nie udało się uruchomić wątku odczytu obrazu, ffmpeg zostawał w tle aż do zamknięcia Clipeona.
- Nazwa gry na wierzchu jest czytana z pliku programu raz, a nie co sekundę.
- Poprawka: w trybie Pulpit dymki o aktualizacjach pokazywały się na ekranie z trybu Gra (teraz każdy dymek
  jest na nagrywanym ekranie).
- Poprawka: przerwany eksport GIF albo WebP zostawiał w folderze klipów plik `.gif.tmp` / `.webp.tmp`, którego
  sprzątanie nie usuwało.
- Poprawka: dwa eksporty tego samego klipu naraz mogły dostać tę samą nazwę i pisać do jednego pliku.
- Dziennik: błędy ustawiania ikony przy zegarze też trafiają do `clipeon.log`.

## 1.4.2

- **Powtórka nie przepada, gdy nagrywanie się wznawia.** Gdy gra (np. Fightcade) co chwilę przełącza tryb
  ekranu, nagrywanie na moment traci obraz i Clipeon uruchamia je od nowa — dawniej za każdym razem z pustym
  buforem, więc Alt+F10 dawał klip kilkusekundowy. Teraz nagrywanie wraca do tego samego bufora: klip ma pełną
  długość, a w miejscu przerwy jest krótki przeskok obrazu (dźwięk z przerwy jest wycinany, żeby zgadzał się
  z obrazem). Alt+F10 w trakcie wznawiania też zapisuje to, co już było nagrane.
- **Nowy sposób nagrywania: Desktop Duplication przez RAM.** Clipeon sam pobiera obraz z ekranu (bez
  ffmpeg i bez Windows Graphics Capture, więc **bez żółtej ramki** na Windows 10) i dorysowuje kursor. Gdy
  Windows na chwilę odbierze obraz (gra przełącza pełny ekran, okno UAC), nagrywanie się nie restartuje: do
  klipu idzie ostatnia klatka, a Clipeon w tle odzyskuje obraz — bez przeskoków i bez utraty powtórki.
  Na Windows 10 Clipeon przechodzi na niego przy pierwszej utracie obrazu przez zwykłe Desktop Duplication
  (bez restartowania w kółko) i zapamiętuje to dla tego ekranu, więc po restarcie komputera od razu
  nagrywa w ten sposób; działa z każdym enkoderem. Można go też
  wybrać na stałe w Ustawieniach → Zaawansowane.
- Gdy Desktop Duplication gubi obraz raz za razem, a każdy inny sposób nagrywania rysowałby żółtą ramkę,
  Clipeon zostaje przy nagrywaniu bez ramki, dopóki ono startuje.
- W dzienniku przy awarii nagrywania są trzy ostatnie komunikaty ffmpeg zamiast jednego (łatwiej znaleźć przyczynę).
- **Żółta ramka nie zostaje już na stałe po zmianie trybu ekranu (NVIDIA i inne karty).** Gdy gra albo
  Windows przełączał tryb ekranu, nagrywanie Desktop Duplication na chwilę traciło obraz. Clipeon brał to za
  awarię enkodera, sprawdzał go od nowa akurat w trakcie zmiany trybu, uznawał, że enkoder nie bierze obrazu
  z karty, i do końca uruchomienia nagrywał przez Windows Graphics Capture — na Windows 10 z żółtą ramką
  wokół ekranu. Teraz enkoder, który już działał, nie jest za to obwiniany: Clipeon ponawia Desktop
  Duplication, a gdy na chwilę musi przejść na tryb zgodności, wraca na drogę bez ramki, gdy tylko ekran
  znów da obraz.

## 1.4.1

- **Tło płynie jak dym albo tusz w wodzie.** Ruch myszki wlewa jasne znaki i popycha „płyn” pod tłem: znaki
  ciągną się za kursorem, zawijają w wiry i powoli się rozmywają (prawdziwa symulacja płynu, liczona tylko
  wokół miejsca, gdzie coś płynie). Myszka, która stoi, niczego nie dolewa, więc efekt sam gaśnie. Kolory bez zmian.
- Długość filmów w bibliotece pokazuje godziny („1:06:52” zamiast „66:52”).
- „Klatki i jakość” w Ustawieniach: pola leżą pod nazwą, więc nazwa nie łamie się już w wąskiej kolumnie.
- Nowe zrzuty ekranu w README.

## 1.4.0

- **Filmy — długie nagrywanie.** Nowy przycisk „Nagraj film” na pasku (i w menu ikony przy zegarze) albo skrót
  **Alt+F9** zaczyna nagrywanie bez limitu czasu; to samo kończy film. Na przycisku tyka czas nagrania.
  Filmy mają własną zakładkę **Filmy** obok Klipów, zapisują się do tego samego folderu jako
  `Clipeon_Film_…mp4`, mają te same ścieżki dźwięku co klipy (razem, Gra, Mikrofon) i otwierają się
  w edytorze, więc da się z nich wyciąć klip albo GIF.
  - Film nie obciąża gry dodatkowo: korzysta z tego samego obrazu, który Clipeon i tak nagrywa do bufora.
    Klipy (Alt+F10) działają w trakcie filmu jak zwykle.
  - Automatyczne sprzątanie nigdy nie usuwa filmów.
  - Film kończy się sam, gdy na dysku zostaje mniej niż 500 MB, gdy zmienisz ustawienia nagrywania albo
    zamkniesz Clipeona — i zawsze zapisuje to, co zdążył nagrać. Skrót filmu zmienisz (albo usuniesz)
    w Ustawieniach → Zapisywanie klipów.
- **Mniejsze filmy.** Ustawienia → Zapisywanie klipów → „Rozmiar filmów”: po zakończeniu nagrania Clipeon
  może zapisać film od nowa jako HEVC (ok. połowa rozmiaru) albo AV1 (ok. jedna trzecia). Robi to karta
  graficzna, w tle i z niskim priorytetem, więc nagrywanie nie jest przez to cięższe; godzina filmu to
  zwykle kilka–kilkanaście minut. Postęp widać w bibliotece, a do końca film leży tam w pełnej jakości.
  Opcje pojawiają się tylko wtedy, gdy karta umie dany format. Domyślnie film zostaje bez zmian.
- **Żółta ramka przy zmianie pełny ekran ↔ okno.** Gdy gra przełączała tryb ekranu, Desktop Duplication
  na chwilę gubiło obraz, a Clipeon po trzech takich potknięciach przechodził na stałe na tryb zgodności,
  który na Windows 10 rysuje żółtą ramkę. Teraz Clipeon po prostu ponawia Desktop Duplication, aż ekran się
  uspokoi. Jeśli naprawdę nie działa dłużej niż 30 s, przechodzi na inny sposób tylko na chwilę i sam wraca
  do Desktop Duplication (pierwsza próba po minucie). Zapamiętany „sposób z ramką” nie wygrywa już
  z działającym Desktop Duplication po ponownym uruchomieniu.
- W Ustawieniach → Zaawansowane widać, którym sposobem Clipeon przechwytuje obraz **w tej chwili**.
  Zniknęły napisy o żółtej ramce (także z listy sposobów).
- **Ustawianie skrótu nie zapisuje już klipów.** Gdy klikniesz pole skrótu, skróty Clipeona są wyłączone,
  dopóki nie wciśniesz nowej kombinacji, nie anulujesz (Esc) albo nie przejdziesz do innego okna.
- **Podgląd wciskanych klawiszy.** Przy ustawianiu skrótu pole pokazuje na szaro to, co już trzymasz
  („Ctrl + Shift + …”), a gotowy skrót robi się biały.
- **Animowane tło.** Większość okna jest czarna, a tu i tam leżą plamy napisu CLIPEON, które na brzegach
  rzedną w plusy, dwukropki i rozproszone kropki. Plamy bardzo powoli zmieniają kształt i przy każdym
  uruchomieniu układają się inaczej. Ruch myszki zostawia smugę gęstych znaków: im szybszy ruch, tym
  jaśniejsza, a przy powolnym prawie jej nie widać. Smuga nigdzie nie leci — strzępi się i powoli gaśnie tam,
  gdzie powstała, więc gdy myszka stoi, efekt sam znika. Nad przyciskami, ustawieniami i klipami smuga nie
  powstaje. Tło działa tylko wtedy, gdy okno Clipeona jest otwarte i aktywne (w grze nie kosztuje nic),
  a wyłączysz je w Ustawieniach → System → „Animowane tło”.
- **Napis „Clipeon” z zorzą** — po białych literach wędrują kolorowe plamy (napis nie jest już pogrubiony).
  Kolory wyłączysz w Ustawieniach → System → „Kolorowy napis Clipeon”.
- **Ustawienia czytelniej.** Listy wyboru i pola skrótów leżą pod opisem i zajmują szerokość karty (opis nie
  jest już ściśnięty w wąską kolumnę). „Zaawansowane” są zaraz pod „Systemem” i widać je bez rozwijania.
  „Ikona zawsze widoczna przy zegarze” nazywa się teraz „Ikona zawsze widoczna na pasku zadań”.
- **Suwak „Ile ostatnich sekund zapisywać”** jest na całą szerokość karty, ma kreski co minutę i przy
  przeciąganiu przyskakuje do równych wartości (30 s, 1 min, 1 min 30 s…). Strzałki dalej zmieniają co 5 s.
- Zniknęło ustawienie „Edytor zaczyna od: Z moim głosem / Bez głosu”. Edytor zawsze zaczyna z głosem,
  a mikrofon wyciszasz przyciskiem przy jego ścieżce.
- **Okno bez ramki.** Treść i tło sięgają do brzegów okna, a pasek z zakładkami oddziela tylko cienka linia.
- Okno otwiera się trochę szersze (1200 px), żeby pasek z nową zakładką i przyciskiem filmu się mieścił.
- **Poprawka:** ponowne uruchomienie po instalacji aktualizacji nie jest już zapisywane w dzienniku jako
  „uruchomiony ponownie po awarii”.

## 1.3.9

- **GIF i WebP z klipu.** W edytorze nowy wybór „Format: MP4 | GIF | WebP”. Animacja jest zapętlona
  (na Discordzie gra w kółko), bez dźwięku i ma najwyżej 15 s (krótsza — dowolnie). Opcja
  „Do 10 MB (Discord)” sama zmniejsza płynność, rozmiar i liczbę kolorów, aż plik się zmieści.
- **Kadrowanie obrazu.** Przycisk kadru przy odtwarzaniu (albo klawisz `C`): ramkę na obrazie przesuwasz
  i rozciągasz dowolnie, z opcjonalnymi proporcjami 16:9, 9:16 i 1:1. Kadr działa przy zapisie MP4, GIF i WebP.
- **Ścieżki dźwięku w edytorze.** Pod osią czasu są dwie ścieżki, „Gra” i „Mikrofon”, z wykresem, na którym
  widać, w którym momencie klipu co słychać. Linię na ścieżce przeciągasz w górę lub w dół (0–200%, obok
  procent), ikona przy nazwie wycisza ścieżkę, dwuklik wraca do 100%, kliknięcie przewija klip. Zastępuje to
  suwaki „Głośność” i przyciski „Mój głos”.
- **Głośność słychać od razu** — zmiana na ścieżce działa już w podglądzie, także powyżej 100%.
- **Przekreślony mikrofon przy statusie** („Nagrywa”), gdy Clipeon nie nagrywa mikrofonu.
- **Przewijanie klawiaturą:** `←` / `→` o 5 s, `,` / `.` (czyli `<` / `>`) o jedną klatkę.
- **Pełny ekran:** wysokie przyciemnienie pod sterowaniem, więc przyciski i oś czasu widać na każdym obrazie.
  Kadrowanie i ścieżki dźwięku chowają się na czas pełnego ekranu i wracają po wyjściu.
- **„Zachowaj oryginał” zamiast „Zastąp oryginał”.** Domyślnie zapisana wersja zastępuje oryginał
  (oryginał trafia do Kosza, bez okienka z pytaniem). Zaznacz „Zachowaj oryginał”, żeby zapisać obok.
- **Płynniejszy podgląd 60 FPS:** bez rozmycia tła pod odtwarzaczem. `Ctrl+Shift+D` w podglądzie pokazuje,
  ile klatek na sekundę naprawdę wyświetla okno.
- **Poprawka: tryb Pulpit czekał na grę.** Gra wybrana w trybie Gra nie zatrzymuje już nagrywania pulpitu.
- **Poprawka: gry, które chowają okno** (np. emulatory przy przełączaniu pełnego ekranu), nie są już brane
  za zamknięte — Clipeon sprawdza, czy działa proces, i daje grze 10 s zapasu, zanim wyrzuci bufor.
- **Poprawka: Desktop Duplication po zmianie trybu ekranu w grze.** Taki błąd jest teraz chwilowy: Clipeon
  od razu wznawia nagrywanie tym samym enkoderem, a po trzech takich awariach w 2 minuty przechodzi na
  Windows Graphics Capture. Czerwony błąd nie wisi już w czasie czekania na grę.
- **Poprawka: suwak „Ile ostatnich sekund” nie skacze** w górę i w dół przy przesuwaniu.
- W ustawieniach (Zaawansowane) podpowiedź, jak pozbyć się żółtej ramki wokół ekranu na Windows 10.

## 1.3.4

- **Okno znowu działa od startu.** W 1.3.2 jeden błąd w oknie zatrzymywał je zaraz po uruchomieniu:
  status zostawał na „Łączenie…”, znikały przyciski minimalizuj / powiększ / zamknij, nie dało się wybrać
  mikrofonu ani zapisać ustawień. Teraz Clipeon od razu pokazuje „Nagrywa”, bez klikania.
- **Wybór gry na pasku.** W trybie Gra obok przełącznika jest przycisk z nazwą gry, który rozwija listę
  działających gier i programów (oraz „Automatycznie”). Wybór działa od razu. W trybie Pulpit w tym miejscu
  jest jak dotąd wybór ekranu. Pole „Gra” zniknęło z ustawień.
- **Mikrofon:** lista urządzeń jest zawsze aktywna, a wybranie mikrofonu sam go włącza.
  W dzienniku widać, ile mikrofonów znalazł Windows.
- **„Mój głos” prościej:** w edytorze dwa przyciski „Z moim głosem | Bez mojego głosu” zamiast przełącznika,
  a w ustawieniach „Edytor zaczyna od: Z moim głosem | Bez głosu”.
- **„Zaawansowane”** to zwijany wiersz w karcie, tak jak reszta ustawień.
- Przyciski jakości obrazu („Lekka / Standard / Najwyższa”) są pod opisem i już go nie ściskają.
- **Szybszy start nagrywania na kartach AMD.** h264_amf nie przyjmuje obrazu prosto z karty (DDA, WGC), więc
  Clipeon sprawdza to jedną krótką próbą, pamięta wynik do zmiany sterownika lub ffmpeg i od razu nagrywa
  trybem zgodności. Nagrywanie rusza po około 1 s zamiast po kilku sekundach nieudanych prób.
- **Poprawne kolory w plikach HEVC z karty AMD:** plik ma znaczniki BT.709 zamiast „RGB”, więc odtwarzacze
  nie przekłamują barw.
- Porządki w kodzie i nowy automatyczny test okna, który łapie błędy takie jak ten z 1.3.2.

## 1.3.2

- **Widać, jaka gra się nagrywa** — przy statusie, w podpowiedzi ikony przy zegarze i przy zapisanym klipie
  w bibliotece. Nazwa bierze się z okna na wierzchu nagrywanego ekranu.
- **Można wybrać grę do nagrywania** (Ustawienia → Nagrywanie → „Nagrywaj”). Przy wybranej grze bufor działa
  tylko wtedy, gdy gra jest uruchomiona, a poza tym Clipeon czeka i nie zużywa procesora.
  Uwaga: nagrywany jest cały ekran, więc wybór gry decyduje o tym, *kiedy* Clipeon nagrywa.
- **Można wybrać mikrofon** zamiast zawsze domyślnego systemowego. Gdy wybrany mikrofon jest odłączony,
  Clipeon nagrywa z domyślnego i wraca do wybranego, gdy ten się pojawi.
- **Klipy trafiają do podfolderów z miesiącem** (`2026-11`, `2026-12`…). Biblioteka i automatyczne porządki
  widzą zarówno je, jak i starsze klipy leżące luzem w folderze.
- **Pełny ekran w podglądzie klipu**: przycisk przy odtwarzaniu, klawisz `F` albo podwójne kliknięcie w obraz.
  Sterowanie chowa się po chwili bez ruchu myszy, a `Esc` wychodzi najpierw z pełnego ekranu.
- **Nowy dźwięk po zapisaniu klipu** („Kropla”) i pasujący do niego dźwięk błędu.
- **Ustawienia zajmują o połowę mniej miejsca**: zamiast kart z opisami jest wiersz na ustawienie
  (nazwa z lewej, pole z prawej), dwie kolumny w szerokim oknie, a format wideo, sposób przechwytywania
  i diagnostyka siedzą w zwijanej sekcji „Zaawansowane”.
- Logo „Clipeon” jest odrobinę większe i grubsze.

## 1.3.1

- Przesuwane podświetlenie zakładek i przełącznika Gra | Pulpit działa też wtedy, gdy w Windows są wyłączone
  animacje (wtedy jest tylko trochę szybsze).

## 1.3.0

**Tryb pulpitu**
- Na pasku u góry jest przełącznik **Gra | Pulpit** i wybór ekranu (także w menu ikony przy zegarze).
  Pulpit ma własny ekran i własne ustawienia (domyślnie 30 FPS i 12 Mb/s), więc przełączanie nie rusza ustawień gry.
- Klipy z pulpitu mają nazwy `Clipeon_Pulpit_…` i znaczek „Pulpit” w bibliotece.
- Dymki Clipeona nie trafiają do nagrania.

**Mniej pracy dla procesora**
- Obraz zostaje na karcie graficznej: przechwytuje go FFmpeg (Desktop Duplication albo Windows Graphics Capture)
  i od razu koduje enkoder karty, bez kopiowania każdej klatki przez pamięć RAM. Stary sposób zostaje jako
  „tryb zgodności”. Clipeon sam przechodzi na następny sposób, gdy któryś nie działa, i zapamiętuje ten, który działa.
- Dźwięk czeka na dane od Windows zamiast sprawdzać je co 10 ms.
- Gdy enkoder nie nadąża za grą, Clipeon przechodzi na lżejsze ustawienia kodowania i mówi o tym raz.
- Co minutę dziennik zapisuje, ile procesora zużywa Clipeon z FFmpeg i ile klatek zakodował.

**Więcej kart i komputerów**
- Enkodery są sprawdzane w prawdziwej rozdzielczości ekranu, a wyniki zostają zapamiętane do zmiany sterownika
  (szybszy start). Za duży ekran dla H.264 nagrywa się w HEVC albo w zmniejszonej rozdzielczości.
- Nowe enkodery zapasowe: sprzętowy przez Windows (Media Foundation) i OpenH264.
- Klipy mają poprawnie opisane kolory (BT.709), więc nie wyglądają na wyblakłe w odtwarzaczach.
- Monitor z HDR: Ustawienia mówią, że klip będzie w zwykłych kolorach (SDR).
- Wybrany monitor jest rozpoznawany po stałym identyfikatorze, a nie tylko po numerze, który Windows potrafi zmienić.
  Gdy monitor jest odłączony, Clipeon nagrywa główny i wraca do wybranego, gdy ten znów się pojawi.
- Okno UAC i ekran blokady nie wywołują już alarmu od razu: Clipeon czeka do minuty, aż ekran znów będzie dostępny.
- Nowe ustawienie „Sposób przechwytywania obrazu” (Automatycznie / Desktop Duplication / Windows Graphics Capture /
  tryb zgodności) na wypadek kłopotów.

**Zgłaszanie błędów**
- Przycisk **Kopiuj diagnostykę** (w Ustawieniach i na czerwonym pasku) kopiuje opis sprzętu, ustawień
  i ostatnich zdarzeń do wklejenia w zgłoszeniu. Nic nie jest wysyłane automatycznie.

**Wygląd**
- Podświetlenie zakładek Klipy/Ustawienia i przełącznika Gra | Pulpit płynnie przesuwa się na wybraną opcję.

**Poprawki**
- „Zastąp oryginał” w edytorze jest dostępne przy każdej zmianie (np. sama kompresja do 10 MB albo inna głośność),
  a nie tylko po przycięciu.
- Bufor można ustawić od 5 s do 10 min (suwak kończył się na 5 min).
- Pliki tymczasowe i opisy w programie używają już nazwy Clipeon zamiast Clipper.

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
