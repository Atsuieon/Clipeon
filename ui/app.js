// Okno Clipeona: zakładki Klipy, Filmy (długie nagrania) i Ustawienia. Rozmawia z rdzeniem przez komendy Tauri.
// Animowane tło rysuje osobny plik tlo.js (window.Tlo).
const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);

// Ikony Lucide (linie 1.5 px)
const IKONY = {
  zapisz: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  pauza: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
  graj: '<polygon points="6 3 20 12 6 21 6 3"/>',
  pelny: '<path d="M8 3H5a2 2 0 0 0-2 2v3"/><path d="M16 3h3a2 2 0 0 1 2 2v3"/><path d="M8 21H5a2 2 0 0 1-2-2v-3"/><path d="M16 21h3a2 2 0 0 0 2-2v-3"/>',
  okno: '<path d="M3 8h3a2 2 0 0 0 2-2V3"/><path d="M21 8h-3a2 2 0 0 1-2-2V3"/><path d="M3 16h3a2 2 0 0 1 2 2v3"/><path d="M21 16h-3a2 2 0 0 0-2 2v3"/>',
  folder: '<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/>',
  kosz: '<path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
  zamknij: '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>',
  odswiez: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
  minimalizuj: '<path d="M5 12h14"/>',
  maksymalizuj: '<rect width="16" height="16" x="4" y="4" rx="2"/>',
  przywroc: '<rect width="12" height="12" x="8" y="4" rx="2"/><path d="M16 16v2a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2h2"/>',
  nozyczki: '<circle cx="6" cy="6" r="3"/><path d="M8.12 8.12 12 12"/><path d="M20 4 8.12 15.88"/><circle cx="6" cy="18" r="3"/><path d="M14.8 14.8 20 20"/>',
  kompresja: '<path d="m15 15 6 6"/><path d="m15 9 6-6"/><path d="M21 16v5h-5"/><path d="M21 8V3h-5"/><path d="M3 16v5h5"/><path d="m3 21 6-6"/><path d="M3 8V3h5"/><path d="M9 9 3 3"/>',
  przeciagnij: '<circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/>',
  ptaszek: '<path d="M20 6 9 17l-5-5"/>',
  pobierz: '<path d="M12 15V3"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/>',
  monitor: '<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>',
  strzalka: '<path d="m6 9 6 6 6-6"/>',
  gra: '<line x1="6" x2="10" y1="11" y2="11"/><line x1="8" x2="8" y1="9" y2="13"/><line x1="15" x2="15.01" y1="12" y2="12"/><line x1="18" x2="18.01" y1="10" y2="10"/><path d="M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z"/>',
  mikrofon: '<path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" x2="12" y1="19" y2="22"/>',
  mikrofonWyl: '<line x1="2" x2="22" y1="2" y2="22"/><path d="M18.89 13.23A7.12 7.12 0 0 0 19 12v-2"/><path d="M5 10v2a7 7 0 0 0 12 5"/><path d="M15 9.34V5a3 3 0 0 0-5.68-1.33"/><path d="M9 9v3a3 3 0 0 0 5.12 2.12"/><line x1="12" x2="12" y1="19" y2="22"/>',
  glosnik: '<path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.364 18.364a9 9 0 0 0 0-12.728"/>',
  glosnikWyl: '<path d="M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z"/><line x1="22" x2="16" y1="9" y2="15"/><line x1="16" x2="22" y1="9" y2="15"/>',
  kadr: '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M18 22V8a2 2 0 0 0-2-2H2"/>',
  nagraj: '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="3.5" fill="currentColor"/>',
  stop: '<rect width="12" height="12" x="6" y="6" rx="2" fill="currentColor"/>',
  film: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M7 3v18"/><path d="M3 7.5h4"/><path d="M3 12h18"/><path d="M3 16.5h4"/><path d="M17 3v18"/><path d="M17 7.5h4"/><path d="M17 16.5h4"/>',
};
const ikona = (n) => `<svg class="ikona" viewBox="0 0 24 24" aria-hidden="true">${IKONY[n]}</svg>`;
const zIkona = (n, tekst) => `${ikona(n)}<span>${tekst}</span>`;

let cfg = null; // ostatnio wczytany config
let stan = { nagrywa: false, pauza: false, bledy: [], tryb: "gra", ekran: "", monitor: "" };
let biblioteka = "klipy"; // co pokazuje lista: "klipy" albo "filmy"

// ---------- Start ----------
// Stan nagrywania i przyciski okna ustawiamy na samej górze: gdyby dalsza część skryptu
// rzuciła błąd, okno i tak pokaże, czy Clipeon nagrywa, i da się je zamknąć.

listen("stan", (e) => pokazStan(e.payload));
invoke("stan").then(pokazStan);

// ---------- Pasek tytułu ----------

const okno = window.__TAURI__.window.getCurrentWindow();
$("okno-min").innerHTML = ikona("minimalizuj");
$("okno-zamknij").innerHTML = ikona("zamknij");
$("okno-min").addEventListener("click", () => okno.minimize());
$("okno-max").addEventListener("click", () => okno.toggleMaximize());
// Zamknięcie niszczy okno (i WebView); nagrywanie działa dalej w zasobniku
$("okno-zamknij").addEventListener("click", () => okno.close());

async function odswiezMaksymalizacje() {
  const max = await okno.isMaximized();
  $("okno-max").innerHTML = ikona(max ? "przywroc" : "maksymalizuj");
  $("okno-max").title = max ? "Przywróć" : "Maksymalizuj";
  $("okno-max").setAttribute("aria-label", $("okno-max").title);
}
odswiezMaksymalizacje();
okno.onResized(odswiezMaksymalizacje);

// ---------- Drobne pomocnicze ----------

function dymek(tekst) {
  const d = $("dymek");
  d.textContent = tekst;
  d.hidden = false;
  clearTimeout(dymek.t);
  dymek.t = setTimeout(() => (d.hidden = true), 3000);
}

const formatMB = (b) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);
const formatCzas = (s) => {
  s = Math.round(s);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};
const formatData = (ms) =>
  new Date(ms).toLocaleString("pl-PL", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

// Dwa przyciski „tak | nie” zamiast przełącznika (np. „Z moim głosem | Bez głosu”).
// Stan trzyma ukryte pole wyboru z data-pole, więc reszta kodu czyta je jak zwykły przełącznik;
// kto ustawia je z kodu, wysyła potem zdarzenie input.
function odswiezDwojke(kont) {
  const pole = $(kont.dataset.pole);
  for (const b of kont.querySelectorAll("button")) {
    const wybrany = (b.dataset.wartosc === "tak") === pole.checked;
    b.classList.toggle("wybrany", wybrany);
    b.setAttribute("aria-checked", String(wybrany));
    b.disabled = pole.disabled;
  }
}

for (const kont of document.querySelectorAll(".dwojka")) {
  const pole = $(kont.dataset.pole);
  for (const b of kont.querySelectorAll("button")) {
    b.setAttribute("role", "radio");
    b.addEventListener("click", () => {
      pole.checked = b.dataset.wartosc === "tak";
      pole.dispatchEvent(new Event("input"));
    });
  }
  pole.addEventListener("input", () => odswiezDwojke(kont));
  odswiezDwojke(kont);
}

// ---------- Pigułka: podświetlenie przesuwające się między przyciskami grupy ----------
// Jeden element na grupę, przesuwany transformacją (karta graficzna, bez przeliczania układu).
// Każde kliknięcie ustawia tylko cel, a przejście CSS rusza z miejsca, w którym pigułka akurat jest,
// więc szybkie klikanie nie zostawia jej w połowie drogi. Zmiana rozmiaru okna, czcionki albo
// napisu przestawia ją bez animacji (ResizeObserver).

const PIGULKI = new Map(); // kontener -> selektor wybranego przycisku

const obserwatorPigulek = new ResizeObserver((wpisy) => {
  const kontenery = new Set(wpisy.map((w) => w.target.closest("[data-pigulka]")));
  for (const k of kontenery) if (k) ulozPigulke(k, false);
});

function dodajPigulke(kontener, selektor) {
  const p = document.createElement("span");
  p.className = "pigulka";
  p.setAttribute("aria-hidden", "true");
  kontener.prepend(p);
  kontener.dataset.pigulka = "";
  PIGULKI.set(kontener, selektor);
  obserwatorPigulek.observe(kontener);
  for (const b of kontener.children) if (b !== p) obserwatorPigulek.observe(b);
}

function ulozPigulke(kontener, animuj = true) {
  const p = kontener.querySelector(":scope > .pigulka");
  const cel = kontener.querySelector(PIGULKI.get(kontener));
  if (!p) return;
  if (!cel || !cel.offsetWidth) {
    p.classList.remove("widoczna");
    return;
  }
  // Pierwsze pokazanie i zmiana rozmiaru: od razu na miejsce, bez przejazdu przez pół paska
  const skok = !animuj || !p.classList.contains("widoczna");
  if (skok) p.classList.add("bez-animacji");
  p.style.width = `${cel.offsetWidth}px`;
  p.style.height = `${cel.offsetHeight}px`;
  p.style.transform = `translate(${cel.offsetLeft}px, ${cel.offsetTop}px)`;
  p.classList.add("widoczna");
  if (skok) {
    p.getBoundingClientRect(); // zatwierdza pozycję, zanim wrócą przejścia
    p.classList.remove("bez-animacji");
  }
}

// ---------- Zakładki ----------

dodajPigulke(document.querySelector(".zakladki"), '.zakladka[aria-selected="true"]');

// Klipy i Filmy to ta sama lista (widok-klipy), tylko z innymi plikami
function pokazZakladke(nazwa) {
  if (nazwa !== "ustawienia" && nazwa !== "filmy") nazwa = "klipy";
  for (const b of document.querySelectorAll(".zakladka")) {
    b.setAttribute("aria-selected", String(b.dataset.zakladka === nazwa));
  }
  ulozPigulke(document.querySelector(".zakladki"));
  const lista = nazwa !== "ustawienia";
  $("widok-klipy").hidden = !lista;
  $("widok-ustawienia").hidden = lista;
  if (lista) {
    biblioteka = nazwa;
    wczytajKlipy();
  } else wczytajUstawienia();
}

for (const b of document.querySelectorAll(".zakladka")) {
  b.addEventListener("click", () => pokazZakladke(b.dataset.zakladka));
}

// ---------- Stan nagrywania ----------

function pokazStan(s) {
  stan = s;
  const kropka = $("kropka");
  const klasa = s.bledy.length ? "blad" : s.wznawianie ? "wznawianie" : s.nagrywa ? "nagrywa" : "";
  kropka.className = "kropka " + klasa;
  $("status").className = "status " + klasa;
  const pulpit = s.tryb === "pulpit";
  const tekst = s.bledy.length
    ? "Nie nagrywa"
    : s.wznawianie
      ? "Wznawiam…"
      : s.pauza
        ? "Wstrzymane"
        : s.czeka_na_gre
          ? `Czekam na ${s.wybrana_gra}`
          : pulpit
            ? "Nagrywa pulpit"
            : s.gra
              ? `Nagrywa: ${s.gra}`
              : "Nagrywa";
  // Pasek z wyjaśnieniem i przyciskami, gdy coś nie działa
  const problem = s.bledy[0] || (s.wznawianie && `Wznawiam nagrywanie: ${s.wznawianie}`);
  $("baner").hidden = !problem;
  if (problem) {
    $("baner-tytul").textContent = s.bledy.length ? "Clipeon ma problem" : "Clipeon wznawia nagrywanie";
    $("baner-opis").textContent = problem;
    $("baner-kropka").className = "kropka " + (s.bledy.length ? "blad" : "wznawianie");
  }
  $("status-tekst").textContent = tekst;
  // Przekreślony mikrofon, gdy Clipeon nagrywa bez mikrofonu
  $("status-mik").innerHTML = ikona("mikrofonWyl");
  $("status-mik").hidden = s.mikrofon !== false;
  $("status").title = s.bledy.join("\n");
  $("pauza").innerHTML = s.pauza ? zIkona("graj", "Wznów") : zIkona("pauza", "Wstrzymaj");
  $("zapisz-klip").disabled = !s.nagrywa;
  $("status").classList.toggle("czeka", !!s.czeka_na_gre);
  pokazFilm(s);
  pokazDroge(s);
  pokazTryb(s);
}

// ---------- Film (długie nagrywanie) ----------

let filmOd = null; // kiedy zaczął się trwający film (zegar okna), null = film nie trwa
let zegarFilmu = 0;

function pokazFilm(s) {
  const trwa = s.film != null;
  filmOd = trwa ? Date.now() - s.film * 1000 : null;
  const b = $("film");
  b.classList.toggle("trwa", trwa);
  b.setAttribute("aria-pressed", String(trwa));
  b.disabled = !!s.film_zapis || (!trwa && !s.nagrywa);
  const skrot = cfg && cfg.skrot_filmu ? ` (${cfg.skrot_filmu})` : "";
  b.title = s.film_zapis
    ? "Składam nagranie w jeden plik"
    : trwa
      ? `Zakończ film i zapisz go${skrot}`
      : `Nagraj film: nagrywa bez przerwy, aż klikniesz jeszcze raz${skrot}. Klipy działają w tym czasie jak zwykle.`;
  b.setAttribute("aria-label", s.film_zapis ? "Zapisuję film" : trwa ? "Zakończ film" : "Nagraj film");
  odswiezPrzyciskFilmu();
  // Zegar tyka tylko, gdy film trwa
  if (trwa && !zegarFilmu) zegarFilmu = setInterval(odswiezPrzyciskFilmu, 1000);
  if (!trwa && zegarFilmu) {
    clearInterval(zegarFilmu);
    zegarFilmu = 0;
  }
  // Zajęty skrót filmu: uwaga w ustawieniach (film da się wtedy włączyć przyciskiem)
  $("skrot-filmu-uwaga").hidden = !s.skrot_filmu_blad;
  $("skrot-filmu-uwaga").textContent = s.skrot_filmu_blad || "";
  // Zapisany film jest zmniejszany w tle do formatu z ustawień
  $("film-zmniejszanie").hidden = s.film_zmniejszanie == null;
  $("film-zmniejszanie").textContent = `Zmniejszam ostatni film: ${s.film_zmniejszanie ?? 0}%. Do końca jest w bibliotece w pełnej jakości.`;
}

const formatDlugi = (s) => {
  s = Math.max(0, Math.floor(s));
  const g = Math.floor(s / 3600);
  const reszta = `${String(Math.floor((s % 3600) / 60)).padStart(g ? 2 : 1, "0")}:${String(s % 60).padStart(2, "0")}`;
  return g ? `${g}:${reszta}` : reszta;
};

function odswiezPrzyciskFilmu() {
  $("film").innerHTML = stan.film_zapis
    ? zIkona("film", "Zapisuję film…")
    : filmOd != null
      ? zIkona("stop", formatDlugi((Date.now() - filmOd) / 1000))
      : zIkona("nagraj", "Nagraj film");
}

$("film").addEventListener("click", () => invoke("przelacz_film"));
listen("film-zapisany", () => {
  dymek("Film zapisany");
  if (!$("widok-klipy").hidden) wczytajKlipy();
});
listen("film-zmniejszony", () => {
  dymek("Film zmniejszony");
  if (!$("widok-klipy").hidden) wczytajKlipy();
});
listen("film-blad", (e) => dymek(String(e.payload)));

// Którym sposobem Clipeon naprawdę przechwytuje obraz (przy „Automatycznie” wybiera sam)
const DROGI = { dda: "Desktop Duplication", gfx: "Windows Graphics Capture", potok: "Tryb zgodności" };

function pokazDroge(s) {
  const nazwa = DROGI[s.droga];
  $("metoda-teraz").textContent = !nazwa
    ? ""
    : s.ramka
      ? `Teraz: ${nazwa}. Ten sposób pokazuje na tym Windows ramkę wokół nagrywanego obrazu; Clipeon wróci do Desktop Duplication, gdy tylko znów zadziała.`
      : `Teraz: ${nazwa}.`;
}

// ---------- Tryb (Gra | Pulpit), wybór ekranu i gry ----------

let zmianaTrybu = false; // przełącznik czeka na nowy stan z rdzenia
let ostatniWybor = null;

const przyciskiTrybu = () => $("tryb").querySelectorAll("[data-tryb]");
dodajPigulke($("tryb"), ".segment.wybrany");

function zaznaczTryb(tryb) {
  for (const b of przyciskiTrybu()) {
    const wybrany = b.dataset.tryb === tryb;
    b.classList.toggle("wybrany", wybrany);
    b.setAttribute("aria-checked", String(wybrany));
  }
  ulozPigulke($("tryb"));
}

function pokazTryb(s) {
  zmianaTrybu = false;
  zaznaczTryb(s.tryb);
  for (const b of przyciskiTrybu()) b.disabled = false;
  // Obok przełącznika: w trybie pulpitu wybór ekranu, w trybie gry wybór gry
  const pulpit = s.tryb === "pulpit";
  const opis = pulpit ? s.ekran || "Ekran" : s.wybrana_gra || "Automatycznie";
  $("wybor").innerHTML = `${ikona(pulpit ? "monitor" : "gra")}<span class="etykieta-wyboru"></span><svg class="ikona strzalka" viewBox="0 0 24 24" aria-hidden="true">${IKONY.strzalka}</svg>`;
  $("wybor").querySelector(".etykieta-wyboru").textContent = opis; // nazwa gry pochodzi z okna programu
  $("wybor").title = pulpit ? `Nagrywany ekran: ${opis} (zmień)` : `Nagrywana gra: ${opis} (zmień)`;
  $("wybor").setAttribute("aria-label", $("wybor").title);
  $("lista-wyboru").setAttribute("aria-label", pulpit ? "Który ekran nagrywać" : "Którą grę nagrywać");
  // Tryb lub ekran zmienione z paska / zasobnika — formularz ustawień nie może ich potem cofnąć
  const wybor = `${s.tryb}|${s.monitor}`;
  if (cfg && wybor !== ostatniWybor) {
    ostatniWybor = wybor;
    cfg.tryb = s.tryb;
    const pole = pulpit ? "monitor_pulpitu" : "monitor";
    cfg[pole] = s.monitor;
    const select = $(pulpit ? "monitor-pulpitu" : "monitor");
    if ([...select.options].some((o) => o.value === s.monitor)) select.value = s.monitor;
  }
}

async function ustawTryb(zmiana) {
  if (zmianaTrybu) return;
  zmianaTrybu = true;
  // Pigułka jedzie od razu; gdyby rdzeń odmówił, nowy stan przesunie ją z powrotem
  if (zmiana.tryb) zaznaczTryb(zmiana.tryb);
  for (const b of przyciskiTrybu()) b.disabled = true;
  try {
    await invoke("ustaw_tryb", { tryb: zmiana.tryb ?? null, monitor: zmiana.monitor ?? null });
  } catch (e) {
    dymek(String(e));
  } finally {
    // Nowy stan zwykle przychodzi zdarzeniem; gdyby nic się nie zmieniło, odblokowujemy sami
    pokazStan(await invoke("stan"));
  }
}

for (const b of przyciskiTrybu()) {
  b.addEventListener("click", () => {
    if (b.dataset.tryb === stan.tryb) return;
    ustawTryb({ tryb: b.dataset.tryb });
    dymek(b.dataset.tryb === "pulpit" ? "Nagrywam pulpit…" : "Nagrywam grę…");
  });
}

function zamknijListeWyboru() {
  $("lista-wyboru").hidden = true;
  $("wybor").setAttribute("aria-expanded", "false");
}

// Pozycje listy na pasku: ekrany (tryb pulpitu) albo działające gry (tryb gry)
async function pozycjeWyboru() {
  if (stan.tryb === "pulpit") {
    const monitory = await invoke("monitory");
    const biezacy = stan.monitor || monitory.find((m) => m.glowny)?.id;
    return monitory.map((m) => ({
      tytul: `Ekran ${m.numer}${m.glowny ? " · główny" : ""}`,
      opis: `${m.nazwa} — ${m.szerokosc}×${m.wysokosc}${m.hz ? `, ${m.hz} Hz` : ""}${m.hdr ? ", HDR" : ""}`,
      wybrany: m.id === biezacy,
      wybierz: () => ustawTryb({ monitor: m.id }),
    }));
  }
  const gra = (proces, nazwa, tytul, opis) => ({
    tytul,
    opis,
    wybrany: proces.toLowerCase() === (stan.gra_proces || "").toLowerCase(),
    wybierz: async () => {
      await invoke("ustaw_gre", { proces, nazwa });
      pokazStan(await invoke("stan"));
    },
  });
  const gry = await invoke("gry");
  return [
    gra("", "", "Automatycznie", "Nagrywa cały czas, nazwę bierze z okna na wierzchu"),
    ...gry.map((g) => gra(g.proces, g.nazwa, g.nazwa, `Nagrywa tylko, gdy działa ${g.proces}`)),
  ];
}

$("wybor").addEventListener("click", async (e) => {
  e.stopPropagation();
  const lista = $("lista-wyboru");
  if (!lista.hidden) return zamknijListeWyboru();
  const pozycje = await pozycjeWyboru();
  lista.replaceChildren(
    ...pozycje.map((p) => {
      const b = document.createElement("button");
      b.type = "button";
      b.setAttribute("role", "option");
      b.setAttribute("aria-selected", String(p.wybrany));
      const tytul = document.createElement("span");
      tytul.textContent = p.tytul;
      const opis = document.createElement("span");
      opis.className = "pomoc";
      opis.textContent = p.opis;
      b.append(tytul, opis);
      b.addEventListener("click", () => {
        zamknijListeWyboru();
        if (!p.wybrany) p.wybierz();
      });
      return b;
    })
  );
  if (!pozycje.length) lista.textContent = "Nie znaleziono monitorów";
  lista.hidden = false;
  $("wybor").setAttribute("aria-expanded", "true");
  lista.querySelector('[aria-selected="true"]')?.focus();
});

document.addEventListener("click", (e) => {
  if (!e.target.closest(".wybor")) zamknijListeWyboru();
});
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && !$("lista-wyboru").hidden) {
    zamknijListeWyboru();
    $("wybor").focus();
  }
});

// ---------- Diagnostyka do zgłoszenia błędu ----------

async function kopiujDiagnostyke() {
  try {
    const tekst = await invoke("diagnostyka");
    await navigator.clipboard.writeText(tekst);
    dymek("Skopiowano diagnostykę — wklej ją do zgłoszenia");
  } catch (e) {
    dymek(`Nie udało się skopiować: ${e}`);
  }
}
$("baner-diagnostyka").addEventListener("click", kopiujDiagnostyke);
$("kopiuj-diagnostyke").addEventListener("click", kopiujDiagnostyke);
$("pokaz-dziennik").addEventListener("click", () => invoke("otworz_log"));

$("pauza").addEventListener("click", () => invoke("przelacz_pauze"));
$("baner-ponow").addEventListener("click", () => {
  invoke("uruchom_ponownie");
  dymek("Uruchamiam nagrywanie od nowa…");
});
$("baner-log").addEventListener("click", () => invoke("otworz_log"));
$("zapisz-klip").addEventListener("click", () => {
  invoke("zapisz_klip");
  dymek("Zapisuję klip…");
});

// ---------- Klipy ----------

let kolejkaMiniatur = [];
let aktywneMiniatury = 0;

// Miniatury generuje ffmpeg — najwyżej dwie naraz, żeby nie obciążać komputera
function pobierzMiniature(klip, karta) {
  kolejkaMiniatur.push({ klip, karta });
  pompujMiniatury();
}

function pompujMiniatury() {
  while (aktywneMiniatury < 2 && kolejkaMiniatur.length) {
    const { klip, karta } = kolejkaMiniatur.shift();
    aktywneMiniatury++;
    invoke("miniatura", { sciezka: klip.sciezka })
      .then((m) => {
        const img = karta.querySelector("img");
        img.onload = () => img.classList.add("gotowa");
        img.src = m.obraz;
        if (m.dlugosc > 0) {
          klip.dlugosc = m.dlugosc;
          const d = karta.querySelector(".dlugosc");
          d.textContent = formatCzas(m.dlugosc);
          d.hidden = false;
        }
        // Gra zapisana w metadanych klipu — pokazujemy ją zamiast samej nazwy pliku
        if (m.gra) {
          klip.gra = m.gra;
          karta.querySelector(".pomoc").textContent = `${formatMB(klip.rozmiar)} · ${m.gra}`;
          karta.querySelector(".pomoc").title = klip.nazwa;
        }
      })
      .catch(() => {})
      .finally(() => {
        aktywneMiniatury--;
        pompujMiniatury();
      });
  }
}

async function wczytajKlipy() {
  const [wszystkie, u] = await Promise.all([invoke("klipy"), cfg ? Promise.resolve(null) : invoke("ustawienia")]);
  if (u) cfg = u.cfg;
  // Zakładka Filmy pokazuje długie nagrania, Klipy — całą resztę
  const filmy = biblioteka === "filmy";
  const klipy = wszystkie.filter((k) => !!k.film === filmy);
  $("biblioteka-tytul").textContent = filmy ? "Filmy" : "Klipy";
  $("pusto-tytul").textContent = filmy ? "Brak filmów" : "Brak klipów";
  kolejkaMiniatur = [];
  // Zaznaczenie przeżywa odświeżenie listy (bez klipów, których już nie ma)
  kolejnosc = klipy;
  const istniejace = new Set(klipy.map((k) => k.sciezka));
  for (const s of [...zaznaczone]) if (!istniejace.has(s)) zaznaczone.delete(s);
  const kontener = $("siatka");
  kontener.replaceChildren();
  let siatka = null;
  let sekcja = null;
  const suma = klipy.reduce((a, k) => a + k.rozmiar, 0);
  $("podsumowanie").textContent = klipy.length
    ? `${klipy.length} ${odmiana(klipy.length)} · ${formatMB(suma)}`
    : "";
  $("pusto").hidden = klipy.length > 0;
  $("pusto-tekst").textContent = !filmy
    ? `Naciśnij ${cfg.skrot} w grze, aby zapisać ostatnie ${cfg.bufor_sekundy} s.`
    : cfg.skrot_filmu
      ? `Naciśnij ${cfg.skrot_filmu} albo kliknij „Nagraj film”, aby nagrywać bez przerwy. Ten sam skrót kończy nagranie.`
      : "Kliknij „Nagraj film”, aby nagrywać bez przerwy, aż klikniesz jeszcze raz.";

  for (const klip of klipy) {
    // Nowa sekcja daty (klipy są posortowane od najnowszego)
    const nazwaSekcji = sekcjaDaty(klip.czas_ms);
    if (nazwaSekcji !== sekcja) {
      sekcja = nazwaSekcji;
      const naglowek = document.createElement("h2");
      naglowek.className = "sekcja";
      naglowek.textContent = sekcja;
      siatka = document.createElement("div");
      siatka.className = "siatka";
      kontener.append(naglowek, siatka);
    }
    const karta = document.createElement("button");
    karta.className = "karta klip";
    karta.innerHTML = `
      <div class="obraz"><img alt=""><span class="dlugosc" hidden></span><span class="graj">${ikona("graj")}</span>
        <span class="zaznacz" role="checkbox" aria-checked="false" aria-label="Zaznacz" tabindex="-1">${ikona("ptaszek")}</span>
        ${klip.pulpit ? '<span class="znaczek" title="Nagrane w trybie pulpitu">Pulpit</span>' : ""}</div>
      <div class="opis"><h3></h3><p class="pomoc"></p></div>`;
    karta.querySelector("h3").textContent = formatData(klip.czas_ms);
    karta.querySelector(".pomoc").textContent = `${formatMB(klip.rozmiar)} · ${klip.nazwa}`;
    karta.dataset.sciezka = klip.sciezka;
    karta.addEventListener("click", (e) => {
      if (poPrzeciaganiu) return;
      // Ctrl: przełącz, Shift: zakres od ostatnio klikniętego, a gdy coś jest zaznaczone — zwykły klik też zaznacza
      if (e.ctrlKey || e.metaKey) przelaczZaznaczenie(klip);
      else if (e.shiftKey && kotwica) zaznaczZakres(kotwica, klip);
      else if (zaznaczone.size || e.target.closest(".zaznacz")) przelaczZaznaczenie(klip);
      else return odtworz(klip);
      kotwica = e.shiftKey ? kotwica : klip;
    });
    przeciagalny(karta, () => klip.sciezka);
    karta.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      pokazMenu(e.clientX, e.clientY, klip);
    });
    siatka.append(karta);
    pobierzMiniature(klip, karta);
  }
  odswiezZaznaczenie();
}

// ---------- Zaznaczanie wielu klipów ----------

const zaznaczone = new Set();
let kolejnosc = [];
let kotwica = null;

function przelaczZaznaczenie(klip) {
  if (zaznaczone.has(klip.sciezka)) zaznaczone.delete(klip.sciezka);
  else zaznaczone.add(klip.sciezka);
  odswiezZaznaczenie();
}

function zaznaczZakres(od, doKlipu) {
  const a = kolejnosc.findIndex((k) => k.sciezka === od.sciezka);
  const b = kolejnosc.findIndex((k) => k.sciezka === doKlipu.sciezka);
  if (a < 0 || b < 0) return przelaczZaznaczenie(doKlipu);
  for (const k of kolejnosc.slice(Math.min(a, b), Math.max(a, b) + 1)) zaznaczone.add(k.sciezka);
  odswiezZaznaczenie();
}

function wyczyscZaznaczenie() {
  zaznaczone.clear();
  kotwica = null;
  odswiezZaznaczenie();
}

function odmianaZaznaczonych(n) {
  const co = biblioteka === "filmy" ? "film" : "klip";
  if (n === 1) return `1 ${co} zaznaczony`;
  const r = n % 10, s = n % 100;
  return r >= 2 && r <= 4 && (s < 12 || s > 14) ? `${n} ${co}y zaznaczone` : `${n} ${co}ów zaznaczonych`;
}

function odswiezZaznaczenie() {
  for (const karta of document.querySelectorAll(".klip")) {
    const tak = zaznaczone.has(karta.dataset.sciezka);
    karta.classList.toggle("zaznaczony", tak);
    karta.querySelector(".zaznacz").setAttribute("aria-checked", String(tak));
  }
  const n = zaznaczone.size;
  $("siatka").classList.toggle("tryb-zaznaczania", n > 0);
  $("pasek-zaznaczenia").hidden = n === 0;
  $("ile-zaznaczonych").textContent = odmianaZaznaczonych(n);
  $("usun-zaznaczone").innerHTML = zIkona("kosz", "Usuń zaznaczone");
}

$("zaznacz-wszystko").addEventListener("click", () => {
  for (const k of kolejnosc) zaznaczone.add(k.sciezka);
  odswiezZaznaczenie();
});
$("odznacz").addEventListener("click", wyczyscZaznaczenie);
$("usun-zaznaczone").addEventListener("click", async () => {
  const lista = [...zaznaczone];
  if (!lista.length) return;
  if (!confirm(`Przenieść do Kosza: ${odmianaZaznaczonych(lista.length).replace(/ zaznacz\S+$/, "")}? Da się je przywrócić z Kosza.`)) return;
  let bledy = 0;
  for (const sciezka of lista) {
    try {
      await invoke("usun_klip", { sciezka });
      zaznaczone.delete(sciezka);
    } catch {
      bledy++;
    }
  }
  dymek(bledy ? `Nie udało się usunąć ${bledy} z ${lista.length} (są w użyciu)` : "Przeniesiono do Kosza");
  wczytajKlipy();
});

// Nazwa sekcji dla daty klipu: Dzisiaj, Wczoraj, 2–6 dni temu, W tym miesiącu, potem miesiąc i rok
const MIESIACE = ["Styczeń", "Luty", "Marzec", "Kwiecień", "Maj", "Czerwiec", "Lipiec", "Sierpień", "Wrzesień", "Październik", "Listopad", "Grudzień"];

function sekcjaDaty(ms) {
  const d = new Date(ms);
  const dzis = new Date();
  const poczatekDnia = (x) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const dni = Math.round((poczatekDnia(dzis) - poczatekDnia(d)) / 86400000);
  if (dni <= 0) return "Dzisiaj";
  if (dni === 1) return "Wczoraj";
  if (dni <= 6) return `${dni} dni temu`;
  if (d.getFullYear() === dzis.getFullYear() && d.getMonth() === dzis.getMonth()) return "Wcześniej w tym miesiącu";
  return d.getFullYear() === dzis.getFullYear() ? MIESIACE[d.getMonth()] : `${MIESIACE[d.getMonth()]} ${d.getFullYear()}`;
}

function odmiana(n) {
  const co = biblioteka === "filmy" ? "film" : "klip";
  if (n === 1) return co;
  const r = n % 10, s = n % 100;
  return r >= 2 && r <= 4 && (s < 12 || s > 14) ? `${co}y` : `${co}ów`;
}

$("odswiez").addEventListener("click", wczytajKlipy);
$("otworz-folder").addEventListener("click", () => invoke("otworz_folder"));

// ---------- Przeciąganie pliku do innych aplikacji ----------

// Po przesunięciu myszy o kilka pikseli z wciśniętym przyciskiem system przejmuje przeciąganie
// prawdziwego pliku (Discord, Eksplorator, przeglądarka). Zwykły klik działa jak dotąd.
// `zrodlo` zwraca ścieżkę pliku (albo obietnicę — np. gdy trzeba najpierw zapisać przyciętą wersję).
let poPrzeciaganiu = false;
let myszWcisnieta = false;
let przeciaganyEl = null;
addEventListener("pointerdown", () => (myszWcisnieta = true), true);
addEventListener("pointerup", () => (myszWcisnieta = false), true);

function koniecPrzeciagania() {
  myszWcisnieta = false;
  if (przeciaganyEl) przeciaganyEl.classList.remove("ciagniety");
  przeciaganyEl = null;
}
listen("przeciaganie-koniec", koniecPrzeciagania);

function przeciagalny(el, zrodlo) {
  el.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    const [x0, y0] = [e.clientX, e.clientY];
    const ruch = async (m) => {
      if (Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
      koniec();
      poPrzeciaganiu = true;
      setTimeout(() => (poPrzeciaganiu = false), 400);
      // Podgląd maleje i blednie — pod kursorem jest już miniatura pliku
      el.classList.add("ciagniety");
      przeciaganyEl = el;
      try {
        const sciezka = await zrodlo();
        if (!sciezka) return koniecPrzeciagania();
        if (!myszWcisnieta) {
          // Przygotowanie pliku trwało dłużej niż trzymanie myszy
          koniecPrzeciagania();
          return dymek("Plik gotowy — przeciągnij jeszcze raz");
        }
        await invoke("przeciagnij", { sciezka });
      } catch (err) {
        koniecPrzeciagania();
        dymek(String(err));
      }
    };
    const koniec = () => {
      removeEventListener("pointermove", ruch);
      removeEventListener("pointerup", koniec);
    };
    addEventListener("pointermove", ruch);
    addEventListener("pointerup", koniec);
  });
  // Bez obrazka przeciągania z przeglądarki
  el.addEventListener("dragstart", (e) => e.preventDefault());
}

// ---------- Edytor: odtwarzacz, przycinanie, kompresja ----------

let odtwarzany = null;
let zakres = { start: 0, koniec: 0 };
let dlugosc = 0;
let rozmiarDocelowy = null; // null = bez zmian, inaczej MB
let eksportTrwa = false;
let formatZapisu = "mp4"; // "mp4", "gif" albo "webp"
let infoKlipu = { sciezki_audio: 1, fps: 0, szerokosc: 0, wysokosc: 0, webp: false };
// GIF i WebP: najdłuższy fragment (tak samo pilnuje tego rdzeń)
const ANIMACJA_MAX = 15;
const animacja = () => formatZapisu !== "mp4";
// Kadr w pikselach klipu { x, y, szer, wys } (null = cały obraz); kadrowanie = ramka do przesuwania na obrazie
let kadr = null;
let kadrowanie = false;
let proporcjeKadru = 0; // 0 = dowolne, inaczej szerokość / wysokość
// Podgląd głośności na żywo: osobne ścieżki dźwięku przez Web Audio
const dzwiek = { ctx: null, sciezki: [], klip: null };

const ROZMIARY = [null, 10, 20, 50, 100];

function odtworz(klip) {
  odtwarzany = klip;
  $("odtw-tytul").textContent = formatData(klip.czas_ms);
  $("odtw-opis").textContent = `${klip.nazwa} · ${formatMB(klip.rozmiar)}`;
  $("wynik").hidden = true;
  $("postep").hidden = true;
  $("eksport-info").textContent = "";
  $("zachowaj").checked = false;
  rozmiarDocelowy = null;
  formatZapisu = "mp4";
  kadr = null;
  kadrowanie = false;
  dlugosc = 0;
  ostatniEksport = null;
  zatrzymajDzwiekNaZywo();
  przygotujGlosnosc(klip);
  const v = $("wideo");
  // Znacznik w adresie omija pamięć podręczną WebView — po „Zastąp oryginał” plik ma tę samą nazwę,
  // ale inną treść, a stary, zapamiętany rozmiar zostawiał pusty podgląd
  v.src = `${convertFileSrc(klip.sciezka)}?v=${Date.now()}`;
  $("odtwarzacz").hidden = false;
  window.Tlo?.odswiez(); // tło stoi, gdy zasłania je edytor
  v.play().catch(() => {});
  $("odtw-zamknij").focus();
}

function zamknijOdtwarzacz() {
  if (eksportTrwa) return dymek("Poczekaj, aż skończy się zapisywanie");
  if (wPelnym()) document.exitFullscreen().catch(() => {});
  const v = $("wideo");
  v.pause();
  v.removeAttribute("src");
  v.load(); // zwalnia plik, żeby dało się go usunąć
  zatrzymajDzwiekNaZywo();
  $("odtwarzacz").hidden = true;
  window.Tlo?.odswiez();
  odtwarzany = null;
}

// Komunikat tylko przy prawdziwym braku obsługi kodeka (nie przy celowym zwolnieniu pliku w trakcie zapisu)
const BRAK_PODGLADU = "Ten klip nie odtwarza się w podglądzie (HEVC wymaga rozszerzenia HEVC z Microsoft Store). Przycinanie i kompresja nadal działają.";
$("wideo").addEventListener("error", () => {
  const v = $("wideo");
  if (!odtwarzany || !v.getAttribute("src") || eksportTrwa || !v.error || v.error.code !== 4) return;
  $("eksport-info").textContent = BRAK_PODGLADU;
});
$("wideo").addEventListener("loadeddata", () => {
  if ($("eksport-info").textContent === BRAK_PODGLADU) $("eksport-info").textContent = "";
});

$("wideo").addEventListener("loadedmetadata", () => {
  dlugosc = $("wideo").duration || 0;
  zakres = { start: 0, koniec: dlugosc };
  odswiezEdytor();
  ulozKadr();
});

// Odtwarzanie tylko w zaznaczonym zakresie (w pętli)
$("wideo").addEventListener("timeupdate", () => {
  const v = $("wideo");
  if (v.currentTime >= zakres.koniec - 0.02 && !v.paused) v.currentTime = zakres.start;
  odswiezGlowice();
});
$("wideo").addEventListener("play", odswiezPrzyciskGraj);
$("wideo").addEventListener("pause", odswiezPrzyciskGraj);
$("wideo").addEventListener("click", () => !poPrzeciaganiu && przelaczGraj());

// ---------- Pełny ekran w podglądzie ----------

const wPelnym = () => !!document.fullscreenElement;

function przelaczPelnyEkran() {
  const okno = $("okno-odtwarzacza");
  if (wPelnym()) document.exitFullscreen().catch(() => {});
  // Pełny ekran bierze całą nakładkę, więc oś czasu i przyciski zostają widoczne
  else okno.requestFullscreen().catch(() => dymek("Pełny ekran jest niedostępny"));
}

function odswiezPelnyEkran() {
  const pelny = wPelnym();
  $("pelny-ekran").innerHTML = ikona(pelny ? "okno" : "pelny");
  $("pelny-ekran").title = pelny ? "Wyjdź z pełnego ekranu (F)" : "Pełny ekran (F)";
  $("pelny-ekran").setAttribute("aria-label", $("pelny-ekran").title);
  $("okno-odtwarzacza").classList.toggle("pelny", pelny);
  if (pelny) obudzSterowanie();
}

document.addEventListener("fullscreenchange", odswiezPelnyEkran);
odswiezPelnyEkran();
$("pelny-ekran").addEventListener("click", przelaczPelnyEkran);
$("wideo").addEventListener("dblclick", przelaczPelnyEkran);

// W pełnym ekranie sterowanie chowa się po 2 s bez ruchu myszy
let ukryjSterowanie = null;

function obudzSterowanie() {
  const okno = $("okno-odtwarzacza");
  okno.classList.remove("bez-sterowania");
  clearTimeout(ukryjSterowanie);
  if (!wPelnym()) return;
  ukryjSterowanie = setTimeout(() => {
    if (wPelnym() && !$("wideo").paused) okno.classList.add("bez-sterowania");
  }, 2000);
}

$("okno-odtwarzacza").addEventListener("mousemove", obudzSterowanie);
$("okno-odtwarzacza").addEventListener("mouseleave", () => clearTimeout(ukryjSterowanie));

function przelaczGraj() {
  const v = $("wideo");
  if (v.paused) {
    if (v.currentTime < zakres.start || v.currentTime >= zakres.koniec - 0.05) v.currentTime = zakres.start;
    v.play().catch(() => {});
  } else v.pause();
}
$("graj").addEventListener("click", przelaczGraj);

function odswiezPrzyciskGraj() {
  const p = $("wideo").paused;
  $("graj").innerHTML = ikona(p ? "graj" : "pauza");
  $("graj").title = p ? "Odtwórz (spacja)" : "Pauza (spacja)";
}

const procent = (t) => (dlugosc ? (t / dlugosc) * 100 : 0);

function odswiezGlowice() {
  const t = $("wideo").currentTime;
  $("os-glowica").style.left = `${procent(t)}%`;
  for (const s of sciezkiGlosu) s.pole.querySelector(".fala-glowica").style.left = `${procent(t)}%`;
  $("czas").textContent = `${formatCzas(t)} / ${formatCzas(dlugosc)}`;
}

function przyciete() {
  return zakres.start > 0.05 || zakres.koniec < dlugosc - 0.05;
}

function odswiezEdytor() {
  $("uchwyt-start").style.left = `${procent(zakres.start)}%`;
  $("uchwyt-koniec").style.left = `${procent(zakres.koniec)}%`;
  $("os-zakres").style.left = `${procent(zakres.start)}%`;
  $("os-zakres").style.width = `${procent(zakres.koniec - zakres.start)}%`;
  $("uchwyt-start").setAttribute("aria-valuetext", formatCzas(zakres.start));
  $("uchwyt-koniec").setAttribute("aria-valuetext", formatCzas(zakres.koniec));
  const dl = zakres.koniec - zakres.start;
  $("zakres-tekst").textContent = przyciete() ? `Zaznaczono ${dl.toFixed(1)} s` : "Cały klip";
  // Dokładne cięcie ma znaczenie tylko bez kompresji i kadru (one i tak kodują od nowa)
  $("dokladnie-wiersz").hidden = !przyciete() || rozmiarDocelowy !== null || !!kadr;
  $("eksport-opcje").classList.toggle("animacja", animacja());
  for (const b of $("formaty").children) {
    b.classList.toggle("wybrany", b.dataset.format === formatZapisu);
    b.setAttribute("aria-checked", b.dataset.format === formatZapisu);
  }
  const webp = $("formaty").querySelector('[data-format="webp"]');
  webp.disabled = !infoKlipu.webp;
  if (!infoKlipu.webp) webp.title = "Ten ffmpeg nie umie zapisać WebP";
  $("format-uwaga").textContent = animacja() ? `Zapętlony, bez dźwięku, najwyżej ${ANIMACJA_MAX} s` : "";
  $("kadr-grupa").hidden = !kadrowanie && !kadr;
  $("kadruj").setAttribute("aria-pressed", kadrowanie);

  // Rozmiary większe od (przyciętego) oryginału nie mają sensu; GIF / WebP: bez limitu albo do 10 MB
  const szacunek = animacja() ? Infinity : odtwarzany ? odtwarzany.rozmiar * (dlugosc ? dl / dlugosc : 1) : 0;
  if (rozmiarDocelowy !== null && rozmiarDocelowy * 1e6 >= szacunek) rozmiarDocelowy = null;
  const kontener = $("rozmiary");
  kontener.replaceChildren();
  for (const mb of animacja() ? [null, 10] : ROZMIARY) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "segment" + (mb === rozmiarDocelowy ? " wybrany" : "");
    b.textContent = mb === null ? (animacja() ? "Bez limitu" : "Bez zmian") : animacja() ? "Do 10 MB (Discord)" : `${mb} MB`;
    if (mb !== null && mb * 1e6 >= szacunek) {
      b.disabled = true;
      b.title = `Plik ma już ${formatMB(szacunek)}`;
    }
    b.addEventListener("click", () => {
      rozmiarDocelowy = mb;
      odswiezEdytor();
    });
    kontener.append(b);
  }

  // Zmieniona wersja MP4 zastępuje oryginał, chyba że zaznaczono „Zachowaj oryginał”
  $("zachowaj-wiersz").hidden = !zmiany() || animacja();

  const b = $("eksportuj");
  const zaDlugi = animacja() && dl > ANIMACJA_MAX + 0.05;
  if (animacja()) b.textContent = `Zapisz jako ${formatZapisu.toUpperCase()}`;
  else if (rozmiarDocelowy !== null) b.textContent = `Zapisz jako ${rozmiarDocelowy} MB`;
  else if (kadr) b.textContent = "Zapisz wykadrowany";
  else if (przyciete()) b.textContent = "Zapisz przycięty";
  else if (maMikrofon && glosnosci()[1] === 0 && glosnosci()[0] === 1) b.textContent = "Zapisz bez mojego głosu";
  else if (glosnosciZmienione()) b.textContent = "Zapisz z nową głośnością";
  else b.textContent = "Zapisz";
  b.disabled = eksportTrwa || !zmiany() || zaDlugi;
  if (!eksportTrwa) {
    $("eksport-info").textContent = zaDlugi
      ? `${formatZapisu.toUpperCase()} może mieć najwyżej ${ANIMACJA_MAX} s — przytnij zaznaczenie`
      : b.disabled
        ? "Przytnij klip, wykadruj, wybierz rozmiar albo zmień dźwięk, żeby zapisać nową wersję."
        : !animacja() && !$("zachowaj").checked
          ? "Nowa wersja zastąpi oryginał (oryginał trafi do Kosza)."
          : "";
  }
  // GIF / WebP nie mają dźwięku
  $("sciezki-dzwieku").hidden = !sciezkiGlosu.length || animacja();
  odswiezZakresSciezek();
  ustawGlosnoscNaZywo();
  odswiezGlowice();
}

// Uchwyty i klikanie w oś
function czasZPozycji(clientX) {
  const r = $("os").getBoundingClientRect();
  return Math.min(Math.max((clientX - r.left) / r.width, 0), 1) * dlugosc;
}

function przesun(ktory, t) {
  const min = 0.5;
  // GIF / WebP: zaznaczenie najwyżej 15 s (krótsze — dowolnie)
  const max = animacja() ? ANIMACJA_MAX : Infinity;
  if (ktory === "start") zakres.start = Math.min(Math.max(t, 0, zakres.koniec - max), zakres.koniec - min);
  else zakres.koniec = Math.max(Math.min(t, dlugosc, zakres.start + max), zakres.start + min);
  $("wideo").currentTime = ktory === "start" ? zakres.start : Math.max(zakres.koniec - 0.05, zakres.start);
  odswiezEdytor();
}

for (const [id, ktory] of [["uchwyt-start", "start"], ["uchwyt-koniec", "koniec"]]) {
  const u = $(id);
  u.addEventListener("pointerdown", (e) => {
    e.stopPropagation();
    u.setPointerCapture(e.pointerId);
    $("wideo").pause();
    const ruch = (m) => przesun(ktory, czasZPozycji(m.clientX));
    u.addEventListener("pointermove", ruch);
    u.addEventListener("pointerup", () => u.removeEventListener("pointermove", ruch), { once: true });
  });
  u.addEventListener("keydown", (e) => {
    const krok = e.shiftKey ? 1 : 0.1;
    if (e.key === "ArrowLeft") przesun(ktory, zakres[ktory] - krok);
    else if (e.key === "ArrowRight") przesun(ktory, zakres[ktory] + krok);
    else return;
    e.preventDefault();
  });
}

$("os").addEventListener("pointerdown", (e) => {
  const os = $("os");
  os.setPointerCapture(e.pointerId);
  const idz = (m) => {
    $("wideo").currentTime = Math.min(Math.max(czasZPozycji(m.clientX), zakres.start), zakres.koniec);
    odswiezGlowice();
  };
  idz(e);
  os.addEventListener("pointermove", idz);
  os.addEventListener("pointerup", () => os.removeEventListener("pointermove", idz), { once: true });
});

$("ustaw-start").addEventListener("click", () => przesun("start", $("wideo").currentTime));
$("ustaw-koniec").addEventListener("click", () => przesun("koniec", $("wideo").currentTime));
$("resetuj").addEventListener("click", () => {
  zakres = { start: 0, koniec: dlugosc };
  odswiezEdytor();
});

// Zapis (przycięcie i/lub kompresja)
listen("postep", (e) => {
  if (!eksportTrwa || $("odtwarzacz").hidden) return;
  const p = Math.round(e.payload * 100);
  $("postep-pasek").style.width = `${p}%`;
  $("eksport-info").textContent = `Zapisuję… ${p}%`;
});

// ---------- Edytor: ścieżki dźwięku (Gra / Mikrofon albo jedna) ----------
// Każda ścieżka to wykres głośności na osi czasu klipu, z poziomą linią głośności (0–200%).
// Linię przeciąga się w górę i w dół, a zmianę słychać od razu (patrz „głośność słychać od razu”).
// Każdy wpis: { nazwa, glosnosc (0..2), przed (głośność sprzed wyciszenia), fala (0–255 albo null), el, … }
let sciezkiGlosu = [];
let maMikrofon = false;
const KUBELKI_FALI = 800;

async function przygotujGlosnosc(klip) {
  const kont = $("sciezki-dzwieku");
  kont.replaceChildren();
  sciezkiGlosu = [];
  let info = { sciezki_audio: 1, fps: 0, szerokosc: 0, wysokosc: 0, webp: false };
  try {
    info = await invoke("info_klipu", { sciezka: klip.sciezka });
  } catch {}
  if (odtwarzany !== klip) return;
  infoKlipu = info;
  // Gdy podgląd nie zna długości (kodek nieobsługiwany przez WebView), bierzemy ją z pliku
  if (!dlugosc && info.dlugosc) {
    dlugosc = info.dlugosc;
    zakres = { start: 0, koniec: dlugosc };
  }
  // Klip z osobnymi ścieżkami ma: miks, gra, mikrofon
  maMikrofon = info.sciezki_audio >= 3;
  const opisy = maMikrofon ? [["Gra", "glosnik"], ["Mikrofon", "mikrofon"]] : info.sciezki_audio >= 1 ? [["Dźwięk", "glosnik"]] : [];
  const numery = maMikrofon ? [1, 2] : [0];
  const moje = opisy.map(([nazwa, ikonaNazwa], i) => {
    // Ustawienie „Edytor zaczyna od: Bez głosu” = ścieżka mikrofonu od razu wyciszona
    const bezGlosu = maMikrofon && i === 1 && cfg && !cfg.glos_w_edytorze;
    const s = { nazwa, ikonaNazwa, glosnosc: bezGlosu ? 0 : 1, przed: 1, fala: null };
    s.el = document.createElement("div");
    s.el.className = "sciezka";
    s.el.innerHTML =
      `<div class="sciezka-opis"><span>${nazwa}</span><button type="button" class="wycisz"></button></div>` +
      `<div class="fala" role="slider" tabindex="0" aria-valuemin="0" aria-valuemax="200" aria-label="Głośność: ${nazwa} (przeciągnij w górę lub w dół, strzałki ↑ ↓)">` +
      `<canvas></canvas><div class="fala-poza"></div><div class="fala-poza"></div><div class="fala-glowica"></div>` +
      `<div class="fala-linia"><span class="fala-procent"></span></div></div><div></div>`;
    s.pole = s.el.querySelector(".fala");
    s.plotno = s.el.querySelector("canvas");
    s.linia = s.el.querySelector(".fala-linia");
    s.wycisz = s.el.querySelector(".wycisz");
    s.wycisz.addEventListener("click", () => ustawGlosnosc(s, s.glosnosc === 0 ? s.przed : 0));
    podepnijFale(s);
    kont.append(s.el);
    return s;
  });
  sciezkiGlosu = moje;
  odswiezEdytor();
  ulozSciezki();
  odswiezSciezki();
  przygotujDzwiekNaZywo(klip, info);
  // Wykresy dochodzą po chwili (ffmpeg musi przejść przez dźwięk klipu); bez nich zostaje sama linia głośności
  moje.forEach(async (s, i) => {
    try {
      const fala = await invoke("fala_dzwieku", { sciezka: klip.sciezka, nr: numery[i], kubelki: KUBELKI_FALI });
      if (odtwarzany !== klip || !fala) return;
      s.fala = Array.from(fala);
      rysujFale(s);
    } catch {}
  });
}

function ustawGlosnosc(s, v) {
  // Co 5%, od 0 do 200%
  s.glosnosc = Math.min(Math.max(Math.round(v * 20) / 20, 0), 2);
  if (s.glosnosc > 0) s.przed = s.glosnosc;
  odswiezSciezki();
  odswiezEdytor();
}

function odswiezSciezki() {
  for (const s of sciezkiGlosu) {
    const p = Math.round(s.glosnosc * 100);
    s.linia.style.bottom = `${p / 2}%`;
    s.linia.classList.toggle("nisko", p < 45);
    s.linia.classList.toggle("wysoko", p > 165);
    s.linia.firstChild.textContent = `${p}%`;
    s.pole.setAttribute("aria-valuenow", p);
    s.pole.setAttribute("aria-valuetext", p === 0 ? "wyciszona" : `${p}%`);
    s.el.classList.toggle("wyciszona", p === 0);
    s.wycisz.innerHTML = ikona(p === 0 ? (s.ikonaNazwa === "mikrofon" ? "mikrofonWyl" : "glosnikWyl") : s.ikonaNazwa);
    s.wycisz.title = p === 0 ? `Włącz: ${s.nazwa}` : `Wycisz: ${s.nazwa}`;
    s.wycisz.setAttribute("aria-label", s.wycisz.title);
    rysujFale(s);
  }
}

// Wykres: słupki od dołu; pełna wysokość = 200%, więc przy 100% najgłośniejsze miejsca sięgają linii
function rysujFale(s) {
  const c = s.plotno;
  const dpr = window.devicePixelRatio || 1;
  const [w, h] = [c.clientWidth, c.clientHeight];
  if (!w || !h) return;
  if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
    c.width = Math.round(w * dpr);
    c.height = Math.round(h * dpr);
  }
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  if (!s.fala) return;
  // Wyciszona ścieżka dalej pokazuje (blado), gdzie jest dźwięk
  const glosnosc = s.glosnosc === 0 ? 1 : s.glosnosc;
  g.fillStyle = s.glosnosc === 0 ? "rgba(255, 255, 255, 0.16)" : "rgba(255, 255, 255, 0.5)";
  const krok = 3;
  for (let x = 0; x < w; x += krok) {
    // Najgłośniejsze miejsce z kawałka klipu pod tym słupkiem
    const od = Math.floor((x / w) * s.fala.length);
    const ile = Math.max(1, Math.ceil((krok / w) * s.fala.length));
    let szczyt = 0;
    for (let i = od; i < od + ile && i < s.fala.length; i++) szczyt = Math.max(szczyt, s.fala[i]);
    // Pierwiastek, żeby ciche dźwięki (rozmowa, kroki) też było widać
    const wys = Math.min(h, Math.sqrt(szczyt / 255) * glosnosc * (h / 2));
    if (wys >= 0.5) g.fillRect(x, h - wys, krok - 1, wys);
  }
}

// Przeciąganie w górę / w dół = głośność; samo kliknięcie = przewinięcie w to miejsce klipu
function podepnijFale(s) {
  const pole = s.pole;
  pole.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    pole.setPointerCapture(e.pointerId);
    const y0 = e.clientY;
    let ciagnie = false;
    const ruch = (m) => {
      if (!ciagnie && Math.abs(m.clientY - y0) < 3) return;
      ciagnie = true;
      const r = pole.getBoundingClientRect();
      ustawGlosnosc(s, ((r.bottom - m.clientY) / r.height) * 2);
    };
    const koniec = (k) => {
      pole.removeEventListener("pointermove", ruch);
      pole.removeEventListener("pointerup", koniec);
      pole.removeEventListener("pointercancel", koniec);
      if (ciagnie || k.type !== "pointerup" || !dlugosc) return;
      const r = pole.getBoundingClientRect();
      const t = Math.min(Math.max((k.clientX - r.left) / r.width, 0), 1) * dlugosc;
      $("wideo").currentTime = Math.min(Math.max(t, zakres.start), Math.max(zakres.koniec - 0.05, zakres.start));
      odswiezGlowice();
    };
    pole.addEventListener("pointermove", ruch);
    pole.addEventListener("pointerup", koniec);
    pole.addEventListener("pointercancel", koniec);
  });
  pole.addEventListener("dblclick", () => ustawGlosnosc(s, 1));
  pole.addEventListener("keydown", (e) => {
    if (e.key === "ArrowUp") ustawGlosnosc(s, s.glosnosc + 0.05);
    else if (e.key === "ArrowDown") ustawGlosnosc(s, s.glosnosc - 0.05);
    else if (e.key === "Home") ustawGlosnosc(s, 1);
    else return;
    e.preventDefault();
  });
}

// Wykresy leżą dokładnie pod osią czasu: ta sama lewa krawędź i szerokość
function ulozSciezki() {
  const kont = $("sciezki-dzwieku");
  const os = $("os").getBoundingClientRect();
  const k = kont.getBoundingClientRect();
  if (!os.width || !k.width) return;
  kont.style.setProperty("--lewo", `${os.left - k.left}px`);
  kont.style.setProperty("--szer", `${os.width}px`);
  for (const s of sciezkiGlosu) rysujFale(s);
}
new ResizeObserver(ulozSciezki).observe($("os"));

// Przygaszenie poza zaznaczonym fragmentem i głowica odtwarzania na ścieżkach
function odswiezZakresSciezek() {
  for (const s of sciezkiGlosu) {
    const [lewa, prawa] = s.pole.querySelectorAll(".fala-poza");
    Object.assign(lewa.style, { left: "0", width: `${procent(zakres.start)}%` });
    Object.assign(prawa.style, { left: `${procent(zakres.koniec)}%`, right: "0" });
  }
}

const glosnosci = () => sciezkiGlosu.map((s) => s.glosnosc);
const glosnosciZmienione = () => glosnosci().some((g) => g !== 1);
const zmiany = () => przyciete() || rozmiarDocelowy !== null || glosnosciZmienione() || !!kadr || animacja();

function biezaceZlecenie() {
  return {
    start: zakres.start,
    koniec: zakres.koniec,
    przyciete: przyciete(),
    dokladnie: $("dokladnie").checked,
    rozmiar_mb: rozmiarDocelowy,
    zastap: zmiany() && !animacja() && !$("zachowaj").checked,
    // GIF / WebP są bez dźwięku
    glosnosci: glosnosciZmienione() && !animacja() ? glosnosci() : [],
    kadr: kadr && { x: Math.round(kadr.x), y: Math.round(kadr.y), szer: Math.round(kadr.szer), wys: Math.round(kadr.wys) },
    format: formatZapisu,
  };
}

// ---------- Edytor: przewijanie strzałkami i o klatkę ----------

function przewin(sekundy) {
  const v = $("wideo");
  if (!dlugosc) return;
  v.currentTime = Math.min(Math.max(v.currentTime + sekundy, zakres.start), Math.max(zakres.koniec - 0.05, zakres.start));
  odswiezGlowice();
}

function klatka(ile) {
  const v = $("wideo");
  if (!dlugosc) return;
  v.pause();
  const fps = infoKlipu.fps > 0 ? infoKlipu.fps : 60;
  // Numer bieżącej klatki, a cel w jej środku — zaokrąglenia nie zostawią obrazu na tej samej klatce
  const nr = Math.floor(v.currentTime * fps + 0.01) + ile;
  v.currentTime = Math.min(Math.max((nr + 0.5) / fps, 0), Math.max(dlugosc - 0.5 / fps, 0));
  odswiezGlowice();
}

// ---------- Edytor: format zapisu (MP4 / GIF / WebP) ----------

for (const b of $("formaty").children) {
  b.addEventListener("click", () => {
    if (b.disabled || eksportTrwa) return;
    formatZapisu = b.dataset.format;
    rozmiarDocelowy = null;
    // Dłuższe zaznaczenie przycinamy do 15 s od miejsca odtwarzania (krótsze zostaje, jak jest)
    if (animacja() && zakres.koniec - zakres.start > ANIMACJA_MAX) {
      const v = $("wideo");
      const start = Math.min(Math.max(v.currentTime, zakres.start), Math.max(zakres.koniec - ANIMACJA_MAX, 0));
      zakres = { start, koniec: Math.min(start + ANIMACJA_MAX, dlugosc) };
      v.currentTime = zakres.start;
      dymek(`Zaznaczyłem ${ANIMACJA_MAX} s — przesuń uchwyty, żeby wybrać inny fragment`);
    }
    odswiezEdytor();
  });
}

// ---------- Edytor: kadrowanie ----------

// Wymiary obrazu klipu (z odtwarzacza, a gdy ten nie zna — z pliku)
function wymiaryKlipu() {
  const v = $("wideo");
  return [v.videoWidth || infoKlipu.szerokosc, v.videoHeight || infoKlipu.wysokosc];
}

// Gdzie na ekranie jest obraz w elemencie <video> (z czarnymi pasami po bokach albo u góry)
function prostokatObrazu() {
  const v = $("wideo");
  const [w, h] = wymiaryKlipu();
  const sz = v.clientWidth, wy = v.clientHeight;
  if (!w || !h || !sz || !wy) return null;
  const skala = Math.min(sz / w, wy / h);
  return { x: v.offsetLeft + (sz - w * skala) / 2, y: v.offsetTop + (wy - h * skala) / 2, szer: w * skala, wys: h * skala, skala };
}

function ulozKadr() {
  const warstwa = $("kadr-warstwa");
  const p = prostokatObrazu();
  warstwa.hidden = !(kadr && p);
  warstwa.classList.toggle("podglad", !kadrowanie);
  if (warstwa.hidden) return;
  Object.assign(warstwa.style, { left: `${p.x}px`, top: `${p.y}px`, width: `${p.szer}px`, height: `${p.wys}px` });
  Object.assign($("kadr").style, {
    left: `${kadr.x * p.skala}px`,
    top: `${kadr.y * p.skala}px`,
    width: `${kadr.szer * p.skala}px`,
    height: `${kadr.wys * p.skala}px`,
  });
  $("kadr-wymiary").textContent = `${Math.round(kadr.szer) & ~1} × ${Math.round(kadr.wys) & ~1}`;
}

// Kadr w granicach obrazu, nie mniejszy niż 32 px, z zachowaniem proporcji (gdy wybrane)
function poprawKadr(k, kotwica) {
  const [w, h] = wymiaryKlipu();
  const min = 32;
  let { x, y, szer, wys } = k;
  szer = Math.min(Math.max(szer, min), w);
  wys = Math.min(Math.max(wys, min), h);
  if (proporcjeKadru) {
    // Dopasowanie do proporcji: zmieniamy bok, którego nie ciągnięto
    if (kotwica === "pion") szer = wys * proporcjeKadru;
    else wys = szer / proporcjeKadru;
    if (szer > w) [szer, wys] = [w, w / proporcjeKadru];
    if (wys > h) [szer, wys] = [h * proporcjeKadru, h];
  }
  x = Math.min(Math.max(x, 0), w - szer);
  y = Math.min(Math.max(y, 0), h - wys);
  return { x, y, szer, wys };
}

function kadrNaSrodku() {
  const [w, h] = wymiaryKlipu();
  let szer = w * 0.8, wys = h * 0.8;
  if (proporcjeKadru) {
    wys = Math.min(h * 0.9, szer / proporcjeKadru);
    szer = wys * proporcjeKadru;
  }
  return poprawKadr({ x: (w - szer) / 2, y: (h - wys) / 2, szer, wys });
}

function przelaczKadrowanie() {
  // W pełnym ekranie kadrowanie jest schowane (ramka wraca po wyjściu z pełnego ekranu)
  if (wPelnym()) return;
  if (!wymiaryKlipu()[0]) return dymek("Nie znam rozmiaru obrazu tego klipu — kadrowanie niedostępne");
  kadrowanie = !kadrowanie;
  if (kadrowanie && !kadr) kadr = kadrNaSrodku();
  if (kadrowanie) $("wideo").pause();
  ulozKadr();
  odswiezEdytor();
}
$("kadruj").innerHTML = ikona("kadr");
$("kadruj").addEventListener("click", przelaczKadrowanie);
$("kadr-usun").addEventListener("click", () => {
  kadr = null;
  kadrowanie = false;
  ulozKadr();
  odswiezEdytor();
});
for (const b of $("proporcje").children) {
  b.addEventListener("click", () => {
    proporcjeKadru = Number(b.dataset.proporcje);
    for (const x of $("proporcje").children) x.classList.toggle("wybrany", x === b);
    if (kadr) {
      // Nowe proporcje wokół środka obecnego kadru
      const sx = kadr.x + kadr.szer / 2, sy = kadr.y + kadr.wys / 2;
      const k = poprawKadr({ ...kadr });
      kadr = poprawKadr({ ...k, x: sx - k.szer / 2, y: sy - k.wys / 2 });
    }
    ulozKadr();
    odswiezEdytor();
  });
}

// Przeciąganie ramki (środek = przesuwanie) i uchwytów (zmiana rozmiaru)
$("kadr").addEventListener("pointerdown", (e) => {
  if (!kadrowanie || e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();
  const p = prostokatObrazu();
  if (!p) return;
  const u = e.target.dataset.u || "";
  const start = { ...kadr };
  const [x0, y0] = [e.clientX, e.clientY];
  const el = $("kadr");
  el.setPointerCapture(e.pointerId);
  const ruch = (m) => {
    const dx = (m.clientX - x0) / p.skala, dy = (m.clientY - y0) / p.skala;
    let k = { ...start };
    if (!u) {
      k.x += dx;
      k.y += dy;
      kadr = poprawKadr(k);
    } else {
      if (u.includes("w")) [k.x, k.szer] = [start.x + Math.min(dx, start.szer - 32), start.szer - Math.min(dx, start.szer - 32)];
      if (u.includes("e")) k.szer = start.szer + dx;
      if (u.includes("n")) [k.y, k.wys] = [start.y + Math.min(dy, start.wys - 32), start.wys - Math.min(dy, start.wys - 32)];
      if (u.includes("s")) k.wys = start.wys + dy;
      // Ciągnięty bok nie może wyjść poza obraz
      const [w, h] = wymiaryKlipu();
      if (k.x < 0) [k.szer, k.x] = [k.szer + k.x, 0];
      if (k.y < 0) [k.wys, k.y] = [k.wys + k.y, 0];
      k.szer = Math.min(k.szer, w - k.x);
      k.wys = Math.min(k.wys, h - k.y);
      const pion = u === "n" || u === "s";
      k = poprawKadr(k, pion ? "pion" : "poziom");
      // Przy proporcjach ramka rośnie od przeciwległego rogu / boku
      if (proporcjeKadru) {
        if (u.includes("w")) k.x = start.x + start.szer - k.szer;
        if (u.includes("n")) k.y = start.y + start.wys - k.wys;
        k = poprawKadr(k, pion ? "pion" : "poziom");
      }
      kadr = k;
    }
    ulozKadr();
  };
  const koniec = () => {
    el.removeEventListener("pointermove", ruch);
    el.removeEventListener("pointerup", koniec);
    // Kadr na cały obraz = bez kadru
    const [w, h] = wymiaryKlipu();
    if (kadr && kadr.szer >= w - 1 && kadr.wys >= h - 1) kadr = { ...kadr, x: 0, y: 0, szer: w, wys: h };
    odswiezEdytor();
  };
  el.addEventListener("pointermove", ruch);
  el.addEventListener("pointerup", koniec);
});
new ResizeObserver(ulozKadr).observe($("scena"));
document.addEventListener("fullscreenchange", () =>
  requestAnimationFrame(() => {
    ulozKadr();
    ulozSciezki();
  }),
);

// ---------- Edytor: głośność słychać od razu ----------
// Ścieżki gry i mikrofonu (albo jedyna ścieżka) grają osobno, każda przez własny „gain” (0–200%),
// a wideo jest wyciszone. Bez tego suwaki działałyby dopiero w zapisanym pliku.

async function przygotujDzwiekNaZywo(klip, info) {
  zatrzymajDzwiekNaZywo();
  const numery = info.sciezki_audio >= 3 ? [1, 2] : info.sciezki_audio >= 1 ? [0] : [];
  if (!numery.length) return;
  dzwiek.klip = klip;
  try {
    const dane = await Promise.all(numery.map((nr) => invoke("dzwiek_podgladu", { sciezka: klip.sciezka, nr })));
    if (odtwarzany !== klip || dzwiek.klip !== klip) return;
    dzwiek.ctx ||= new AudioContext();
    dzwiek.sciezki = dane.map((d) => {
      const adres = URL.createObjectURL(new Blob([d], { type: "audio/mp4" }));
      const a = new Audio(adres);
      a.preload = "auto";
      const zrodlo = dzwiek.ctx.createMediaElementSource(a);
      const g = dzwiek.ctx.createGain();
      zrodlo.connect(g).connect(dzwiek.ctx.destination);
      return { a, g, zrodlo, adres };
    });
    $("wideo").muted = true;
    ustawGlosnoscNaZywo(true);
    synchronizujDzwiek(true);
  } catch {
    // Bez podglądu na żywo zostaje dźwięk samego wideo (głośność słychać po zapisie)
    zatrzymajDzwiekNaZywo();
  }
}

function zatrzymajDzwiekNaZywo() {
  for (const s of dzwiek.sciezki) {
    s.a.pause();
    s.zrodlo.disconnect();
    s.a.removeAttribute("src");
    URL.revokeObjectURL(s.adres);
  }
  dzwiek.sciezki = [];
  dzwiek.klip = null;
  $("wideo").muted = false;
}

function ustawGlosnoscNaZywo(odRazu = false) {
  if (!dzwiek.sciezki.length || !dzwiek.ctx) return;
  const g = glosnosci();
  dzwiek.sciezki.forEach((s, i) => {
    const cel = g[i] ?? 1;
    // Krótkie wygładzenie zamiast skoku — bez trzasków przy przesuwaniu suwaka
    if (odRazu) s.g.gain.value = cel;
    else s.g.gain.setTargetAtTime(cel, dzwiek.ctx.currentTime, 0.015);
  });
}

function synchronizujDzwiek(wymus = false) {
  const v = $("wideo");
  for (const s of dzwiek.sciezki) {
    if (wymus || Math.abs(s.a.currentTime - v.currentTime) > 0.08) {
      try {
        s.a.currentTime = v.currentTime;
      } catch {}
    }
    s.a.playbackRate = v.playbackRate;
    if (v.paused || v.seeking) s.a.pause();
    else if (s.a.paused) {
      dzwiek.ctx.resume().catch(() => {});
      s.a.play().catch(() => {});
    }
  }
}

for (const zd of ["play", "playing", "pause", "seeked", "ratechange"]) {
  $("wideo").addEventListener(zd, () => synchronizujDzwiek(zd !== "pause"));
}
$("wideo").addEventListener("seeking", () => dzwiek.sciezki.forEach((s) => s.a.pause()));
$("wideo").addEventListener("timeupdate", () => dzwiek.sciezki.length && synchronizujDzwiek());

// ---------- Edytor: licznik klatek podglądu (Ctrl+Shift+D) ----------
// Pokazuje, ile klatek na sekundę naprawdę wyświetla okno i ile gubi — do sprawdzania płynności podglądu.

let licznik = null;

function przelaczLicznik() {
  const el = $("licznik-klatek");
  const v = $("wideo");
  if (licznik) {
    clearInterval(licznik.zegar);
    licznik = null;
    el.hidden = true;
    return;
  }
  licznik = { klatki: 0, zegar: 0 };
  el.hidden = false;
  el.textContent = "Mierzę…";
  const nastepna = () => licznik && v.requestVideoFrameCallback?.(() => {
    if (!licznik) return;
    licznik.klatki++;
    nastepna();
  });
  nastepna();
  let poprzednia = performance.now();
  licznik.zegar = setInterval(() => {
    const teraz = performance.now();
    const fps = (licznik.klatki * 1000) / (teraz - poprzednia);
    licznik.klatki = 0;
    poprzednia = teraz;
    const q = v.getVideoPlaybackQuality?.() || {};
    el.textContent =
      `Podgląd: ${v.paused ? "pauza" : fps.toFixed(1) + " kl./s"}\n` +
      `Klip: ${infoKlipu.fps ? infoKlipu.fps + " kl./s" : "?"}, ${wymiaryKlipu().join("×")}\n` +
      `Zgubione: ${q.droppedVideoFrames ?? "?"} z ${q.totalVideoFrames ?? "?"}`;
    if (!v.requestVideoFrameCallback) el.textContent += "\n(brak requestVideoFrameCallback)";
  }, 1000);
}

// Ostatni wynik, żeby przeciąganie tej samej wersji nie zapisywało jej drugi raz
let ostatniEksport = null;

// Zapisuje wersję według zlecenia (z paskiem postępu); zwraca nowy klip albo rzuca błąd
async function eksportujTeraz(klip, zlecenie) {
  $("wideo").pause();
  eksportTrwa = true;
  $("postep").hidden = false;
  $("postep-pasek").style.width = "0%";
  $("wynik").hidden = true;
  odswiezEdytor();
  $("eksport-info").textContent = "Zapisuję…";
  let blad = null;
  try {
    if (zlecenie.zastap) {
      // Odtwarzacz musi puścić plik, zanim zastąpimy go nową wersją
      zatrzymajDzwiekNaZywo();
      $("wideo").removeAttribute("src");
      $("wideo").load();
    }
    const { sciezka, rozmiar } = await invoke("eksportuj", { sciezka: klip.sciezka, zlecenie });
    // GIF i WebP nie trafiają do biblioteki — opis składamy z wyniku
    const lista = await invoke("klipy");
    return lista.find((k) => k.sciezka === sciezka) || { ...klip, sciezka, rozmiar, nazwa: sciezka.split("\\").pop() };
  } catch (e) {
    blad = String(e);
    throw e;
  } finally {
    eksportTrwa = false;
    $("postep").hidden = true;
    odswiezEdytor();
    if (blad) $("eksport-info").textContent = blad;
  }
}

$("eksportuj").addEventListener("click", async () => {
  if (!odtwarzany || eksportTrwa) return;
  const klip = odtwarzany;
  const zlecenie = biezaceZlecenie();
  try {
    const nowy = await eksportujTeraz(klip, zlecenie);
    if (zlecenie.zastap) {
      odtworz(nowy);
      dymek("Zapisano. Oryginał jest w Koszu");
    } else ostatniEksport = { klucz: klip.sciezka + JSON.stringify(zlecenie), sciezka: nowy.sciezka };
    pokazWynik(nowy);
    wczytajKlipy();
  } catch {}
});

// Co przeciągamy z podglądu: oryginał albo (przy zmianach) najpierw zapisaną wersję
async function plikDoPrzeciagniecia() {
  if (!odtwarzany) return null;
  if (!zmiany()) return odtwarzany.sciezka;
  if (eksportTrwa) throw "Poczekaj, aż skończy się zapisywanie";
  const klip = odtwarzany;
  const zlecenie = { ...biezaceZlecenie(), zastap: false };
  const klucz = klip.sciezka + JSON.stringify(zlecenie);
  if (ostatniEksport && ostatniEksport.klucz === klucz) return ostatniEksport.sciezka;
  dymek("Przygotowuję plik do przeciągnięcia…");
  const nowy = await eksportujTeraz(klip, zlecenie);
  ostatniEksport = { klucz, sciezka: nowy.sciezka };
  pokazWynik(nowy);
  wczytajKlipy();
  return nowy.sciezka;
}

let wynikKlip = null;

function pokazWynik(klip) {
  wynikKlip = klip;
  $("wynik-nazwa").textContent = klip.nazwa;
  $("wynik-opis").textContent = `${formatMB(klip.rozmiar)} · przeciągnij do Discorda, Eksploratora lub przeglądarki`;
  // GIF / WebP nie odtworzy się w podglądzie klipu
  $("wynik-otworz").hidden = !/\.mp4$/i.test(klip.sciezka);
  $("wynik").hidden = false;
}

przeciagalny($("wynik-przeciagnij"), () => wynikKlip && wynikKlip.sciezka);
$("wynik-folder").addEventListener("click", () => wynikKlip && pokazWFolderze(wynikKlip));
$("wynik-otworz").addEventListener("click", () => wynikKlip && odtworz(wynikKlip));
przeciagalny($("wideo"), plikDoPrzeciagniecia);

$("odtw-zamknij").addEventListener("click", zamknijOdtwarzacz);
$("odtwarzacz").addEventListener("click", (e) => {
  if (e.target === $("odtwarzacz")) zamknijOdtwarzacz();
});
$("odtw-folder").addEventListener("click", () => odtwarzany && pokazWFolderze(odtwarzany));
$("odtw-usun").addEventListener("click", () => odtwarzany && usun(odtwarzany));

async function pokazWFolderze(klip) {
  try {
    await invoke("pokaz_w_folderze", { sciezka: klip.sciezka });
  } catch (e) {
    dymek(String(e));
  }
}

async function usun(klip) {
  if (!confirm(`Przenieść klip „${klip.nazwa}” do Kosza?`)) return;
  if (odtwarzany && odtwarzany.sciezka === klip.sciezka) zamknijOdtwarzacz();
  try {
    await invoke("usun_klip", { sciezka: klip.sciezka });
    dymek("Klip przeniesiony do Kosza");
    wczytajKlipy();
  } catch (e) {
    dymek(String(e));
  }
}

// Szybka kompresja całego klipu z menu karty; postęp w dymku
async function kompresuj(klip, mb) {
  if (eksportTrwa) return dymek("Poczekaj, aż skończy się poprzedni zapis");
  eksportTrwa = true;
  const sluchacz = await listen("postep", (e) => dymek(`Kompresja do ${mb} MB… ${Math.round(e.payload * 100)}%`));
  dymek(`Kompresja do ${mb} MB…`);
  try {
    // koniec = 0 oznacza „do końca klipu” (długość ustala rdzeń)
    const zlecenie = { start: 0, koniec: 0, przyciete: false, dokladnie: false, rozmiar_mb: mb, zastap: false, glosnosci: [] };
    const { sciezka } = await invoke("eksportuj", { sciezka: klip.sciezka, zlecenie });
    sluchacz();
    dymek(`Gotowe: ${sciezka.split("\\").pop()}`);
    wczytajKlipy();
  } catch (e) {
    sluchacz();
    dymek(String(e));
  } finally {
    eksportTrwa = false;
  }
}

// Menu kontekstowe
let klipMenu = null;

function pokazMenu(x, y, klip) {
  klipMenu = klip;
  const m = $("menu");
  m.replaceChildren();
  const pozycja = (ikonaNazwa, tekst, akcja) => {
    const b = document.createElement("button");
    b.setAttribute("role", "menuitem");
    b.innerHTML = zIkona(ikonaNazwa, tekst);
    b.addEventListener("click", () => {
      ukryjMenu();
      akcja();
    });
    m.append(b);
  };
  pozycja("nozyczki", "Przytnij…", () => odtworz(klip));
  for (const mb of ROZMIARY.slice(1)) {
    if (mb * 1e6 < klip.rozmiar) pozycja("kompresja", `Kompresuj do ${mb} MB`, () => kompresuj(klip, mb));
  }
  pozycja("folder", "Pokaż w folderze", () => pokazWFolderze(klip));
  pozycja("kosz", "Usuń", () => usun(klip));
  m.hidden = false;
  const { width, height } = m.getBoundingClientRect();
  m.style.left = `${Math.min(x, innerWidth - width - 8)}px`;
  m.style.top = `${Math.min(y, innerHeight - height - 8)}px`;
  m.querySelector("button").focus();
}

const ukryjMenu = () => ($("menu").hidden = true);

document.addEventListener("click", (e) => {
  if (!e.target.closest("#menu")) ukryjMenu();
});
document.addEventListener("keydown", (e) => {
  if (nasluchSkrotu) return;
  if (e.key === "Escape") {
    ukryjMenu();
    // Pełny ekran zamyka się sam (Esc należy do przeglądarki) — nakładkę zostawiamy otwartą
    if (wPelnym()) return;
    if (!$("odtwarzacz").hidden) zamknijOdtwarzacz();
    else if (zaznaczone.size) wyczyscZaznaczenie();
  } else if (e.key === " " && !$("odtwarzacz").hidden && !e.target.closest("input, select, button, .uchwyt")) {
    e.preventDefault();
    przelaczGraj();
  } else if ((e.key === "f" || e.key === "F") && !$("odtwarzacz").hidden && !e.target.closest("input, select")) {
    e.preventDefault();
    przelaczPelnyEkran();
  } else if (!$("odtwarzacz").hidden && !e.target.closest("input, select, textarea, .uchwyt") && !e.ctrlKey && !e.altKey) {
    // ← / → o 5 s, , / . (czyli < / >) o jedną klatkę, C = kadrowanie
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") przewin(e.key === "ArrowLeft" ? -5 : 5);
    else if (e.key === "," || e.key === "<") klatka(-1);
    else if (e.key === "." || e.key === ">") klatka(1);
    else if (e.key === "c" || e.key === "C") przelaczKadrowanie();
    else return;
    e.preventDefault();
    obudzSterowanie();
  } else if (!$("odtwarzacz").hidden && e.ctrlKey && e.shiftKey && (e.key === "D" || e.key === "d")) {
    e.preventDefault();
    przelaczLicznik();
  }
});
// Bez domyślnego menu przeglądarki
document.addEventListener("contextmenu", (e) => e.preventDefault());

// ---------- Ustawienia ----------

const PRESETY = [
  { nazwa: "Lekka — mniejsze pliki", fps: 30, bitrate: 10 },
  { nazwa: "Standard — płynnie (polecana)", fps: 60, bitrate: 20 },
  { nazwa: "Najwyższa — duże pliki", fps: 60, bitrate: 40 },
];

for (const p of PRESETY) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "segment";
  b.textContent = p.nazwa;
  b.title = `${p.fps} klatek na sekundę, ${p.bitrate} Mb/s`;
  b.addEventListener("click", () => {
    $("fps").value = p.fps;
    $("bitrate").value = p.bitrate;
    odswiezFormularz();
  });
  b.dataset.fps = p.fps;
  b.dataset.bitrate = p.bitrate;
  $("presety").append(b);
}

// Pola formularza i ich klucze w konfiguracji: [id pola, klucz, rodzaj].
// Rodzaj: "tak" = przełącznik, "liczba" = pole liczbowe, "tekst" = lista wyboru.
// Nowe ustawienie to jeden wiersz tutaj (i pole w index.html).
const POLA = [
  ["bufor", "bufor_sekundy", "liczba"],
  ["fps", "fps", "liczba"],
  ["bitrate", "bitrate_mbps", "liczba"],
  ["fps-pulpitu", "fps_pulpitu", "liczba"],
  ["bitrate-pulpitu", "bitrate_pulpitu_mbps", "liczba"],
  ["dzwiek", "dzwiek_systemowy", "tak"],
  ["mikrofon", "mikrofon", "tak"],
  ["glos-w-edytorze", "glos_w_edytorze", "tak"],
  ["dzwiek-zapisu", "dzwiek_zapisu", "tak"],
  ["powiadomienie-zapisu", "powiadomienie_zapisu", "tak"],
  ["ikona-zegar", "ikona_przy_zegarze", "tak"],
  ["aktualizacje", "sprawdzaj_aktualizacje", "tak"],
  ["animowane-tlo", "animowane_tlo", "tak"],
  ["format-filmu", "format_filmu", "tekst"],
  ["enkoder", "enkoder", "tekst"],
  ["metoda", "metoda_przechwytywania", "tekst"],
  ["monitor", "monitor", "tekst"],
  ["monitor-pulpitu", "monitor_pulpitu", "tekst"],
  ["mikrofon-urzadzenie", "mikrofon_urzadzenie", "tekst"],
];

function wpiszPola(c) {
  for (const [id, klucz, rodzaj] of POLA) {
    if (rodzaj === "tak") $(id).checked = c[klucz];
    else $(id).value = c[klucz];
    $(id).dispatchEvent(new Event("input")); // np. pary przycisków odświeżają się same
  }
}

function odczytajPola() {
  const c = {};
  for (const [id, klucz, rodzaj] of POLA) {
    c[klucz] = rodzaj === "tak" ? $(id).checked : rodzaj === "liczba" ? Number($(id).value) : $(id).value;
  }
  return c;
}

// Lista wyboru z zapisaną wartością. Zapisane urządzenie, którego teraz nie ma (odłączony monitor
// albo mikrofon), dostaje własną pozycję, żeby wybór nie zmienił się po cichu przy zapisie.
function wypelnijListe(select, opcje, zapisana, opisBraku) {
  select.replaceChildren(...opcje.map(([wartosc, opis]) => new Option(opis, wartosc)));
  select.value = zapisana;
  if (select.value !== zapisana && opisBraku) {
    select.append(new Option(opisBraku, zapisana));
    select.value = zapisana;
  }
}

async function wczytajUstawienia() {
  const [u, monitory, mikrofony] = await Promise.all([invoke("ustawienia"), invoke("monitory"), invoke("mikrofony")]);
  cfg = u.cfg;
  ostatniWybor = `${stan.tryb}|${stan.monitor}`;

  // Listy wypełniamy przed wpisaniem wartości, żeby było z czego wybierać
  const domyslny = mikrofony.find((m) => m.domyslny);
  wypelnijListe(
    $("mikrofon-urzadzenie"),
    [["", domyslny ? `Domyślny systemowy (${domyslny.nazwa})` : "Domyślny systemowy"], ...mikrofony.map((m) => [m.id, m.nazwa])],
    cfg.mikrofon_urzadzenie,
    "Wybrany mikrofon (niepodłączony)",
  );
  const ekrany = [["", "Główny monitor"], ...monitory.map((m) => [m.id, m.opis + (m.hdr ? " · HDR" : "")])];
  wypelnijListe($("monitor"), ekrany, cfg.monitor, `${cfg.monitor} (niepodłączony)`);
  wypelnijListe($("monitor-pulpitu"), ekrany, cfg.monitor_pulpitu, `${cfg.monitor_pulpitu} (niepodłączony)`);
  $("monitor").dataset.hdr = JSON.stringify(monitory.filter((m) => m.hdr).map((m) => m.id));
  wypelnijListe($("enkoder"), u.enkodery, cfg.enkoder);
  wypelnijListe($("format-filmu"), u.formaty_filmu, cfg.format_filmu, "Wybrany format (ta karta go nie zapisuje)");
  wpiszPola(cfg);

  $("sprz-dni-wl").checked = cfg.sprzatanie_dni > 0;
  $("sprz-dni").value = cfg.sprzatanie_dni || 30;
  $("sprz-gb-wl").checked = cfg.sprzatanie_gb > 0;
  $("sprz-gb").value = cfg.sprzatanie_gb || 50;
  ustawSkrot($("skrot"), cfg.skrot);
  ustawSkrot($("skrot-filmu"), cfg.skrot_filmu);
  $("folder").textContent = cfg.folder_klipow;
  $("autostart").checked = u.autostart;
  $("wersja").textContent = u.wersja;
  $("komunikat").textContent = "";
  odswiezFormularz();
}

// Zależne części formularza: wyłączone pola, uwagi, szacunek RAM, podświetlony preset
function odswiezFormularz() {
  $("sprz-dni").disabled = !$("sprz-dni-wl").checked;
  $("sprz-gb").disabled = !$("sprz-gb-wl").checked;
  $("mikrofon-uwaga").textContent = $("mikrofon").checked ? "" : "Mikrofon jest wyłączony. Wybranie urządzenia go włączy.";
  const uwagi = {
    hevc_amf: "Podgląd w Clipeonie wymaga rozszerzenia HEVC z Microsoft Store, a Discord w przeglądarce może takiego klipu nie odtworzyć.",
    av1_amf: "Karty Radeon RX 7000 zapisują ten format w 1920×1082 — na dole klipu są 2 czarne linie.",
  };
  // Monitor z HDR: klip jest w zwykłych kolorach (SDR), więc może wyglądać inaczej niż gra
  const hdr = JSON.parse($("monitor").dataset.hdr || "[]");
  const zHdr = [$("monitor").value, $("monitor-pulpitu").value].some((v) => hdr.includes(v));
  $("monitor-uwaga").textContent = zHdr
    ? "Ten monitor ma włączony HDR. Klipy zapisują się w zwykłych kolorach (SDR), więc mogą wyglądać na bledsze niż w grze."
    : "";
  // Ile mniej więcej zajmie godzina filmu w wybranym formacie (bitrate trybu gry + dźwięk)
  const naGodzine = (udzial) => formatMB(((Number($("bitrate").value) || 0) * 1e6 * udzial + 480e3) / 8 * 3600);
  const film = {
    bez_zmian: `Godzina filmu zajmie ok. ${naGodzine(1)}.`,
    hevc: `Godzina filmu zajmie ok. ${naGodzine(0.45)} zamiast ${naGodzine(1)}. Podgląd HEVC w Clipeonie wymaga rozszerzenia HEVC z Microsoft Store.`,
    av1: `Godzina filmu zajmie ok. ${naGodzine(0.33)} zamiast ${naGodzine(1)}. AV1 odtwarza Clipeon, przeglądarki i Discord; starsze programy mogą go nie otworzyć.`,
  };
  $("format-filmu-uwaga").textContent = film[$("format-filmu").value] || "";
  // Szacunek RAM bufora: obraz (bitrate) + dźwięk (160 kb/s na ścieżkę)
  const s = Number($("bufor").value);
  const sciezki = Number($("dzwiek").checked) + Number($("mikrofon").checked);
  const ram = (mbps) => formatMB((s * ((Number(mbps) || 0) * 1e6 + sciezki * 160e3)) / 8);
  $("ram-pulpitu").textContent = `Bufor pulpitu zajmuje ok. ${ram($("bitrate-pulpitu").value)} pamięci RAM.`;
  $("ram").textContent = `Zajmuje to ok. ${ram($("bitrate").value)} pamięci RAM.`;
  const wybrany = $("enkoder").value;
  $("enkoder-uwaga").textContent = uwagi[wybrany] || (wybrany.startsWith("hevc") ? uwagi.hevc_amf : "");
  $("bufor-wartosc").textContent = s >= 60 ? `${Math.floor(s / 60)} min${s % 60 ? ` ${s % 60} s` : ""}` : `${s} s`;
  for (const b of $("presety").children) {
    b.classList.toggle("wybrany", b.dataset.fps === $("fps").value && b.dataset.bitrate === $("bitrate").value);
  }
}

// Każda zmiana w formularzu (zdarzenia wpisywania wędrują w górę do formularza)
$("formularz").addEventListener("input", odswiezFormularz);

// Wybrany mikrofon ma być słychać, więc wybór urządzenia włącza „Mój mikrofon”
$("mikrofon-urzadzenie").addEventListener("change", () => {
  $("mikrofon").checked = true;
  odswiezFormularz();
});

$("zmien-folder").addEventListener("click", async () => {
  const f = await invoke("wybierz_folder");
  if (f) $("folder").textContent = f;
});

// ---------- Ustawianie skrótu ----------
// Pola z data-skrot: kliknięcie zaczyna nasłuch, wciśnięta kombinacja go kończy. Na czas nasłuchu
// rdzeń wyłącza skróty Clipeona — inaczej wciśnięcie obecnego skrótu zapisałoby klip (albo zaczęło
// film) zamiast trafić do pola. Wciskane klawisze widać od razu na szaro („Ctrl + Alt + …”).

let nasluchSkrotu = null; // pole, które właśnie czeka na kombinację

function ustawSkrot(pole, wartosc) {
  pole.dataset.wartosc = wartosc;
  pole.textContent = wartosc || pole.dataset.pusty || "";
  pole.classList.toggle("pusty", !wartosc);
}

function zacznijNasluch(pole) {
  if (nasluchSkrotu === pole) return;
  if (nasluchSkrotu) koniecNasluchu();
  nasluchSkrotu = pole;
  uwagaSkrotu = false;
  pole.classList.add("nasluch");
  pole.textContent = "Naciśnij skrót…";
  invoke("wstrzymaj_skroty", { wstrzymaj: true });
}

// `wartosc`: nowy skrót, "" = bez skrótu, undefined = bez zmian (anulowanie)
function koniecNasluchu(wartosc) {
  const pole = nasluchSkrotu;
  if (!pole) return;
  nasluchSkrotu = null;
  pole.classList.remove("nasluch");
  ustawSkrot(pole, wartosc ?? pole.dataset.wartosc);
  invoke("wstrzymaj_skroty", { wstrzymaj: false });
}

for (const pole of document.querySelectorAll("[data-skrot]")) {
  pole.addEventListener("click", () => zacznijNasluch(pole));
  pole.addEventListener("blur", () => nasluchSkrotu === pole && koniecNasluchu());
}
// Okno straciło fokus (Alt+Tab, klik w grę): skróty muszą znów działać
window.addEventListener("blur", () => koniecNasluchu());

const modyfikatory = (e) => [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win"].filter(Boolean);

// Podgląd na szaro: to, co jest już wciśnięte, i wielokropek w miejscu brakującego klawisza.
// `uwaga` (np. „jest już zajęty”) zostaje do następnego wciśnięcia — puszczanie klawiszy jej nie zmazuje.
let uwagaSkrotu = false;

function pokazWciskane(e, uwaga) {
  const mod = modyfikatory(e);
  uwagaSkrotu = !!uwaga;
  nasluchSkrotu.textContent = uwaga || (mod.length ? `${mod.join(" + ")} + …` : "Naciśnij skrót…");
}

document.addEventListener("keydown", (e) => {
  if (!nasluchSkrotu) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") return koniecNasluchu();
  // Delete / Backspace czyści skrót tam, gdzie wolno go nie mieć (film)
  if ((e.key === "Delete" || e.key === "Backspace") && nasluchSkrotu.dataset.pusty) return koniecNasluchu("");
  let klawisz = null;
  if (/^F([1-9]|1\d|2[0-4])$/.test(e.code)) klawisz = e.code;
  else if (/^Key[A-Z]$/.test(e.code)) klawisz = e.code.slice(3);
  else if (/^Digit\d$/.test(e.code)) klawisz = e.code.slice(5);
  if (!klawisz) return pokazWciskane(e); // sam modyfikator — czekamy na zwykły klawisz
  const mod = modyfikatory(e);
  // Litera lub cyfra bez modyfikatora zablokowałaby pisanie w całym systemie
  if (!mod.length && !klawisz.startsWith("F")) return pokazWciskane(e, `${klawisz} — dodaj Ctrl, Alt lub Shift…`);
  const skrot = [...mod, klawisz].join("+");
  // Ten sam skrót w obu polach: rdzeń i tak by go odrzucił, więc mówimy od razu
  const zajety = [...document.querySelectorAll("[data-skrot]")].some((p) => p !== nasluchSkrotu && p.dataset.wartosc === skrot);
  if (zajety) return pokazWciskane(e, `${skrot.replaceAll("+", " + ")} jest już zajęty…`);
  koniecNasluchu(skrot);
}, true);

document.addEventListener("keyup", (e) => {
  if (!nasluchSkrotu) return;
  e.preventDefault();
  if (!uwagaSkrotu) pokazWciskane(e);
}, true);

$("zapisz-ustawienia").addEventListener("click", async () => {
  const nowy = {
    ...cfg,
    ...odczytajPola(),
    skrot: $("skrot").dataset.wartosc,
    skrot_filmu: $("skrot-filmu").dataset.wartosc,
    folder_klipow: $("folder").textContent,
    sprzatanie_dni: $("sprz-dni-wl").checked ? Number($("sprz-dni").value) || 30 : 0,
    sprzatanie_gb: $("sprz-gb-wl").checked ? Number($("sprz-gb").value) || 50 : 0,
  };

  const przycisk = $("zapisz-ustawienia");
  przycisk.disabled = true;
  $("komunikat").textContent = "Zapisuję…";
  try {
    await invoke("zapisz_ustawienia", { cfg: nowy, autostart: $("autostart").checked });
    cfg = nowy;
    $("komunikat").textContent = "Zapisano. Nagrywanie wystartowało z nowymi ustawieniami.";
    odswiezPrzyciskZapisu();
    pokazFilm(stan); // podpowiedź przycisku filmu pokazuje skrót
  } catch (e) {
    $("komunikat").textContent = String(e);
  } finally {
    przycisk.disabled = false;
  }
});

function odswiezPrzyciskZapisu() {
  $("zapisz-klip").innerHTML = zIkona("zapisz", `Zapisz ${cfg ? cfg.bufor_sekundy : 30} s`);
  $("zapisz-klip").title = cfg ? `Skrót: ${cfg.skrot}` : "";
  window.Tlo?.wlacz(!cfg || cfg.animowane_tlo !== false);
}

// ---------- Ikony i zakładka startowa ----------

$("odswiez").innerHTML = ikona("odswiez");
$("otworz-folder").innerHTML = zIkona("folder", "Folder");
$("odtw-folder").innerHTML = zIkona("folder", "Pokaż w folderze");
$("odtw-usun").innerHTML = zIkona("kosz", "Usuń");
$("odtw-zamknij").innerHTML = ikona("zamknij");
$("pusto-ikona").innerHTML = ikona("film");
$("wynik-ikona").innerHTML = ikona("przeciagnij");
odswiezPrzyciskGraj();

// Zakładka startowa (wczytuje też ustawienia)
pokazZakladke(location.hash.slice(1));
invoke("ustawienia").then((u) => {
  cfg ??= u.cfg;
  odswiezPrzyciskZapisu();
  pokazFilm(stan);
});

// ---------- Aktualizacje ----------

let instalujemy = false; // trwa pobieranie albo instalacja aktualizacji

function pokazAktualizacje(w) {
  if (instalujemy) return;
  const b = $("aktualizacja");
  b.hidden = !w;
  b.disabled = false;
  $("aktualizuj-teraz").hidden = !w;
  if (w) {
    b.innerHTML = zIkona("pobierz", `Aktualizuj do ${w.wersja}`);
    b.title = "Pobierz i zainstaluj nową wersję. Clipeon uruchomi się ponownie sam.";
    $("akt-wynik").textContent = `Dostępna wersja ${w.wersja}`;
  }
}

async function aktualizuj() {
  if (instalujemy) return;
  instalujemy = true;
  pokazPostep({ etap: "pobieranie", procent: 0 });
  try {
    await invoke("zainstaluj_aktualizacje");
  } catch (e) {
    instalujemy = false;
    dymek(String(e));
    pokazAktualizacje(await invoke("aktualizacja"));
  }
}

function pokazPostep(p) {
  if (p.etap === "blad") return; // komunikat pokazuje aktualizuj()
  const tekst =
    p.etap === "instalowanie"
      ? "Instalowanie… Clipeon uruchomi się ponownie"
      : `Pobieranie aktualizacji…${p.procent == null ? "" : ` ${p.procent}%`}`;
  for (const b of [$("aktualizacja"), $("aktualizuj-teraz")]) b.disabled = true;
  $("aktualizacja").hidden = false;
  $("aktualizacja").innerHTML = zIkona("pobierz", tekst);
  $("akt-wynik").textContent = tekst;
}

$("aktualizacja").addEventListener("click", aktualizuj);
$("aktualizuj-teraz").addEventListener("click", aktualizuj);
listen("aktualizacja-postep", (e) => {
  if (e.payload.etap !== "blad") {
    instalujemy = true; // instalację mogło też zacząć menu ikony przy zegarze
    pokazPostep(e.payload);
  } else {
    instalujemy = false;
    invoke("aktualizacja").then(pokazAktualizacje);
  }
});
$("autor").addEventListener("click", (e) => {
  e.preventDefault();
  invoke("otworz_repozytorium");
});
$("sprawdz-teraz").addEventListener("click", async () => {
  const b = $("sprawdz-teraz");
  b.disabled = true;
  $("akt-wynik").textContent = "Sprawdzam…";
  try {
    const w = await invoke("sprawdz_aktualizacje");
    $("akt-wynik").textContent = w ? `Dostępna wersja ${w.wersja}` : "Masz najnowszą wersję.";
    pokazAktualizacje(w);
  } catch (e) {
    $("akt-wynik").textContent = String(e);
  } finally {
    b.disabled = false;
  }
});
invoke("aktualizacja").then(pokazAktualizacje);
listen("aktualizacja", (e) => pokazAktualizacje(e.payload));

listen("zakladka", (e) => pokazZakladke(e.payload));
listen("klip-zapisany", () => {
  dymek("Klip zapisany");
  if (!$("widok-klipy").hidden) wczytajKlipy();
});
