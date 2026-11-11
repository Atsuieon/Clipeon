// Okno Clipeona: zakładki Klipy i Ustawienia. Rozmawia z rdzeniem przez komendy Tauri.
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
  film: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M7 3v18"/><path d="M3 7.5h4"/><path d="M3 12h18"/><path d="M3 16.5h4"/><path d="M17 3v18"/><path d="M17 7.5h4"/><path d="M17 16.5h4"/>',
};
const ikona = (n) => `<svg class="ikona" viewBox="0 0 24 24" aria-hidden="true">${IKONY[n]}</svg>`;
const zIkona = (n, tekst) => `${ikona(n)}<span>${tekst}</span>`;

let cfg = null; // ostatnio wczytany config
let stan = { nagrywa: false, pauza: false, bledy: [], tryb: "gra", ekran: "", monitor: "" };

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

function pokazZakladke(nazwa) {
  if (nazwa !== "ustawienia") nazwa = "klipy";
  for (const b of document.querySelectorAll(".zakladka")) {
    b.setAttribute("aria-selected", String(b.dataset.zakladka === nazwa));
  }
  ulozPigulke(document.querySelector(".zakladki"));
  $("widok-klipy").hidden = nazwa !== "klipy";
  $("widok-ustawienia").hidden = nazwa !== "ustawienia";
  if (nazwa === "klipy") wczytajKlipy();
  else wczytajUstawienia();
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
  $("status").title = s.bledy.join("\n");
  $("pauza").innerHTML = s.pauza ? zIkona("graj", "Wznów") : zIkona("pauza", "Wstrzymaj");
  $("zapisz-klip").disabled = !s.nagrywa;
  $("status").classList.toggle("czeka", !!s.czeka_na_gre);
  pokazTryb(s);
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
  const [klipy, u] = await Promise.all([invoke("klipy"), cfg ? Promise.resolve(null) : invoke("ustawienia")]);
  if (u) cfg = u.cfg;
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
  $("pusto-tekst").textContent = `Naciśnij ${cfg.skrot} w grze, aby zapisać ostatnie ${cfg.bufor_sekundy} s.`;

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
        <span class="zaznacz" role="checkbox" aria-checked="false" aria-label="Zaznacz klip" tabindex="-1">${ikona("ptaszek")}</span>
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
  if (n === 1) return "1 klip zaznaczony";
  const r = n % 10, s = n % 100;
  return r >= 2 && r <= 4 && (s < 12 || s > 14) ? `${n} klipy zaznaczone` : `${n} klipów zaznaczonych`;
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
  if (n === 1) return "klip";
  const r = n % 10, s = n % 100;
  return r >= 2 && r <= 4 && (s < 12 || s > 14) ? "klipy" : "klipów";
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

const ROZMIARY = [null, 10, 20, 50, 100];

function odtworz(klip) {
  odtwarzany = klip;
  $("odtw-tytul").textContent = formatData(klip.czas_ms);
  $("odtw-opis").textContent = `${klip.nazwa} · ${formatMB(klip.rozmiar)}`;
  $("wynik").hidden = true;
  $("postep").hidden = true;
  $("eksport-info").textContent = "";
  $("zastap").checked = false;
  rozmiarDocelowy = null;
  dlugosc = 0;
  ostatniEksport = null;
  przygotujGlosnosc(klip);
  const v = $("wideo");
  // Znacznik w adresie omija pamięć podręczną WebView — po „Zastąp oryginał” plik ma tę samą nazwę,
  // ale inną treść, a stary, zapamiętany rozmiar zostawiał pusty podgląd
  v.src = `${convertFileSrc(klip.sciezka)}?v=${Date.now()}`;
  $("odtwarzacz").hidden = false;
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
  $("odtwarzacz").hidden = true;
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
  // Dokładne cięcie ma znaczenie tylko bez kompresji (kompresja i tak koduje od nowa)
  $("dokladnie-wiersz").hidden = !przyciete() || rozmiarDocelowy !== null;

  // Rozmiary większe od (przyciętego) oryginału nie mają sensu
  const szacunek = odtwarzany ? odtwarzany.rozmiar * (dlugosc ? dl / dlugosc : 1) : 0;
  if (rozmiarDocelowy !== null && rozmiarDocelowy * 1e6 >= szacunek) rozmiarDocelowy = null;
  const kontener = $("rozmiary");
  kontener.replaceChildren();
  for (const mb of ROZMIARY) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "segment" + (mb === rozmiarDocelowy ? " wybrany" : "");
    b.textContent = mb === null ? "Bez zmian" : `${mb} MB`;
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

  // Zastąpić można każdą zmienioną wersją: przyciętą, skompresowaną albo z inną głośnością
  $("zastap-wiersz").hidden = !zmiany();
  if (!zmiany()) $("zastap").checked = false;

  const b = $("eksportuj");
  if (rozmiarDocelowy !== null) b.textContent = `Zapisz jako ${rozmiarDocelowy} MB`;
  else if (przyciete()) b.textContent = "Zapisz przycięty";
  else if (maMikrofon && !zGlosem() && glosnosci()[0] === 1) b.textContent = "Zapisz bez mojego głosu";
  else if (glosnosciZmienione()) b.textContent = "Zapisz z nową głośnością";
  else b.textContent = "Zapisz";
  b.disabled = eksportTrwa || !zmiany();
  if (!eksportTrwa) {
    $("eksport-info").textContent = b.disabled ? "Przytnij klip, wybierz rozmiar albo zmień dźwięk, żeby zapisać nową wersję." : "";
  }
  odswiezGlowice();
}

// Uchwyty i klikanie w oś
function czasZPozycji(clientX) {
  const r = $("os").getBoundingClientRect();
  return Math.min(Math.max((clientX - r.left) / r.width, 0), 1) * dlugosc;
}

function przesun(ktory, t) {
  const min = 0.5;
  if (ktory === "start") zakres.start = Math.min(Math.max(t, 0), zakres.koniec - min);
  else zakres.koniec = Math.max(Math.min(t, dlugosc), zakres.start + min);
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

// Głośność ścieżek dźwięku (Gra / Mikrofon albo jedna ścieżka), 0–200%
let suwakiGlosu = [];

async function przygotujGlosnosc(klip) {
  const kont = $("glosnosci");
  kont.replaceChildren();
  suwakiGlosu = [];
  let info = { sciezki_audio: 1 };
  try {
    info = await invoke("info_klipu", { sciezka: klip.sciezka });
  } catch {}
  if (odtwarzany !== klip) return;
  // Gdy podgląd nie zna długości (kodek nieobsługiwany przez WebView), bierzemy ją z pliku
  if (!dlugosc && info.dlugosc) {
    dlugosc = info.dlugosc;
    zakres = { start: 0, koniec: dlugosc };
    odswiezEdytor();
  }
  // Klip z osobnymi ścieżkami ma: miks, gra, mikrofon
  maMikrofon = info.sciezki_audio >= 3;
  const nazwy = maMikrofon ? ["Gra", "Mikrofon"] : info.sciezki_audio >= 1 ? ["Dźwięk"] : [];
  $("mik-grupa").hidden = !nazwy.length;
  $("z-mikrofonem").disabled = !maMikrofon;
  $("z-mikrofonem").checked = maMikrofon && (!cfg || cfg.glos_w_edytorze);
  $("mik-uwaga").textContent = maMikrofon ? "" : "Ten klip nagrano bez mikrofonu.";
  $("glos-wiersz").hidden = !nazwy.length;
  for (const nazwa of nazwy) {
    const wiersz = document.createElement("label");
    wiersz.className = "glos";
    const opis = document.createElement("span");
    opis.textContent = nazwa;
    const suwak = document.createElement("input");
    suwak.type = "range";
    suwak.min = 0;
    suwak.max = 200;
    suwak.step = 5;
    suwak.value = 100;
    suwak.setAttribute("aria-label", `Głośność: ${nazwa}`);
    const wartosc = document.createElement("span");
    wartosc.className = "wartosc-glosu";
    const wycisz = document.createElement("button");
    wycisz.type = "button";
    wycisz.className = "segment";
    const odswiez = () => {
      wartosc.textContent = `${suwak.value}%`;
      wycisz.textContent = Number(suwak.value) === 0 ? "Przywróć" : "Wycisz";
      odswiezEdytor();
    };
    suwak.addEventListener("input", odswiez);
    wycisz.addEventListener("click", () => {
      suwak.value = Number(suwak.value) === 0 ? 100 : 0;
      odswiez();
    });
    wiersz.append(opis, suwak, wartosc, wycisz);
    kont.append(wiersz);
    suwakiGlosu.push(suwak);
    odswiez();
  }
  $("z-mikrofonem").dispatchEvent(new Event("input"));
}

// Bez głosu = ścieżka mikrofonu wyciszona przy zapisie
let maMikrofon = false;
const zGlosem = () => !maMikrofon || $("z-mikrofonem").checked;
const glosnosci = () => suwakiGlosu.map((s, i) => (maMikrofon && i === 1 && !zGlosem() ? 0 : Number(s.value) / 100));
const glosnosciZmienione = () => glosnosci().some((g) => g !== 1);

$("z-mikrofonem").addEventListener("input", () => {
  // Suwak głosu nie ma sensu, gdy głos jest wyłączony
  if (suwakiGlosu[1]) suwakiGlosu[1].closest(".glos").classList.toggle("wylaczony", !zGlosem());
  odswiezEdytor();
});
const zmiany = () => przyciete() || rozmiarDocelowy !== null || glosnosciZmienione();

function biezaceZlecenie() {
  return {
    start: zakres.start,
    koniec: zakres.koniec,
    przyciete: przyciete(),
    dokladnie: $("dokladnie").checked,
    rozmiar_mb: rozmiarDocelowy,
    zastap: zmiany() && $("zastap").checked,
    glosnosci: glosnosciZmienione() ? glosnosci() : [],
  };
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
      $("wideo").removeAttribute("src");
      $("wideo").load();
    }
    const sciezka = await invoke("eksportuj", { sciezka: klip.sciezka, zlecenie });
    const lista = await invoke("klipy");
    return lista.find((k) => k.sciezka === sciezka) || { ...klip, sciezka, nazwa: sciezka.split("\\").pop() };
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
  if (zlecenie.zastap && !confirm("Zastąpić oryginał nową wersją? Oryginał trafi do Kosza.")) return;
  try {
    const nowy = await eksportujTeraz(klip, zlecenie);
    if (zlecenie.zastap) odtworz(nowy);
    else ostatniEksport = { klucz: klip.sciezka + JSON.stringify(zlecenie), sciezka: nowy.sciezka };
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
    const sciezka = await invoke("eksportuj", { sciezka: klip.sciezka, zlecenie });
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
  wpiszPola(cfg);

  $("sprz-dni-wl").checked = cfg.sprzatanie_dni > 0;
  $("sprz-dni").value = cfg.sprzatanie_dni || 30;
  $("sprz-gb-wl").checked = cfg.sprzatanie_gb > 0;
  $("sprz-gb").value = cfg.sprzatanie_gb || 50;
  $("skrot").textContent = cfg.skrot;
  $("skrot").dataset.wartosc = cfg.skrot;
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

// Przechwytywanie skrótu
let nasluchSkrotu = false;

$("skrot").addEventListener("click", () => {
  nasluchSkrotu = true;
  $("skrot").classList.add("nasluch");
  $("skrot").textContent = "Naciśnij skrót…";
});

function koniecNasluchu(wartosc) {
  nasluchSkrotu = false;
  const pole = $("skrot");
  pole.classList.remove("nasluch");
  if (wartosc) pole.dataset.wartosc = wartosc;
  pole.textContent = pole.dataset.wartosc;
}

$("skrot").addEventListener("blur", () => nasluchSkrotu && koniecNasluchu());

document.addEventListener("keydown", (e) => {
  if (!nasluchSkrotu) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") return koniecNasluchu();
  let klawisz = null;
  if (/^F([1-9]|1\d|2[0-4])$/.test(e.code)) klawisz = e.code;
  else if (/^Key[A-Z]$/.test(e.code)) klawisz = e.code.slice(3);
  else if (/^Digit\d$/.test(e.code)) klawisz = e.code.slice(5);
  if (!klawisz) return; // sam modyfikator — czekamy na zwykły klawisz
  const mod = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Win"].filter(Boolean);
  // Litera lub cyfra bez modyfikatora zablokowałaby pisanie w całym systemie
  if (!mod.length && !klawisz.startsWith("F")) {
    $("skrot").textContent = "Dodaj Ctrl, Alt lub Shift…";
    return;
  }
  koniecNasluchu([...mod, klawisz].join("+"));
}, true);

$("zapisz-ustawienia").addEventListener("click", async () => {
  const nowy = {
    ...cfg,
    ...odczytajPola(),
    skrot: $("skrot").dataset.wartosc,
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
  } catch (e) {
    $("komunikat").textContent = String(e);
  } finally {
    przycisk.disabled = false;
  }
});

function odswiezPrzyciskZapisu() {
  $("zapisz-klip").innerHTML = zIkona("zapisz", `Zapisz ${cfg ? cfg.bufor_sekundy : 30} s`);
  $("zapisz-klip").title = cfg ? `Skrót: ${cfg.skrot}` : "";
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
