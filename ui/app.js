// Okno Clippera: zakładki Klipy i Ustawienia. Rozmawia z rdzeniem przez komendy Tauri.
const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);

// Ikony Lucide (linie 1.5 px)
const IKONY = {
  zapisz: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/>',
  pauza: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
  graj: '<polygon points="6 3 20 12 6 21 6 3"/>',
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
  film: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="M7 3v18"/><path d="M3 7.5h4"/><path d="M3 12h18"/><path d="M3 16.5h4"/><path d="M17 3v18"/><path d="M17 7.5h4"/><path d="M17 16.5h4"/>',
};
const ikona = (n) => `<svg class="ikona" viewBox="0 0 24 24" aria-hidden="true">${IKONY[n]}</svg>`;
const zIkona = (n, tekst) => `${ikona(n)}<span>${tekst}</span>`;

let cfg = null; // ostatnio wczytany config
let stan = { nagrywa: false, pauza: false, bledy: [] };

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

// ---------- Zakładki ----------

function pokazZakladke(nazwa) {
  if (nazwa !== "ustawienia") nazwa = "klipy";
  for (const b of document.querySelectorAll(".zakladka")) {
    b.setAttribute("aria-selected", String(b.dataset.zakladka === nazwa));
  }
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
  kropka.className = "kropka" + (s.bledy.length ? " blad" : s.nagrywa ? " nagrywa" : "");
  const tekst = s.bledy.length ? s.bledy[0] : s.pauza ? "Bufor wstrzymany" : "Nagrywa";
  $("status-tekst").textContent = tekst;
  $("status").title = s.bledy.join("\n");
  $("pauza").innerHTML = s.pauza ? zIkona("graj", "Wznów") : zIkona("pauza", "Wstrzymaj");
  $("zapisz-klip").disabled = !s.nagrywa;
}

$("pauza").addEventListener("click", () => invoke("przelacz_pauze"));
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
      <div class="obraz"><img alt=""><span class="dlugosc" hidden></span><span class="graj">${ikona("graj")}</span></div>
      <div class="opis"><h3></h3><p class="pomoc"></p></div>`;
    karta.querySelector("h3").textContent = formatData(klip.czas_ms);
    karta.querySelector(".pomoc").textContent = `${formatMB(klip.rozmiar)} · ${klip.nazwa}`;
    karta.addEventListener("click", () => !poPrzeciaganiu && odtworz(klip));
    przeciagalny(karta, () => klip.sciezka);
    karta.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      pokazMenu(e.clientX, e.clientY, klip);
    });
    siatka.append(karta);
    pobierzMiniature(klip, karta);
  }
}

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
  v.src = convertFileSrc(klip.sciezka);
  $("odtwarzacz").hidden = false;
  v.play().catch(() => {});
  $("odtw-zamknij").focus();
}

function zamknijOdtwarzacz() {
  if (eksportTrwa) return dymek("Poczekaj, aż skończy się zapisywanie");
  const v = $("wideo");
  v.pause();
  v.removeAttribute("src");
  v.load(); // zwalnia plik, żeby dało się go usunąć
  $("odtwarzacz").hidden = true;
  odtwarzany = null;
}

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
  $("zastap-wiersz").hidden = !przyciete();
  if (!przyciete()) $("zastap").checked = false;

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

  const b = $("eksportuj");
  if (rozmiarDocelowy !== null) b.textContent = `Zapisz jako ${rozmiarDocelowy} MB`;
  else if (przyciete()) b.textContent = "Zapisz przycięty";
  else if (glosnosciZmienione()) b.textContent = "Zapisz z nową głośnością";
  else b.textContent = "Zapisz";
  b.disabled = eksportTrwa || !zmiany();
  if (!eksportTrwa) {
    $("eksport-info").textContent = b.disabled ? "Przytnij, wybierz rozmiar albo zmień głośność." : "";
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
  // Klip z osobnymi ścieżkami ma: miks, gra, mikrofon
  const nazwy = info.sciezki_audio >= 3 ? ["Gra", "Mikrofon"] : info.sciezki_audio >= 1 ? ["Dźwięk"] : [];
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
}

const glosnosci = () => suwakiGlosu.map((s) => Number(s.value) / 100);
const glosnosciZmienione = () => glosnosci().some((g) => g !== 1);
const zmiany = () => przyciete() || rozmiarDocelowy !== null || glosnosciZmienione();

function biezaceZlecenie() {
  return {
    start: zakres.start,
    koniec: zakres.koniec,
    przyciete: przyciete(),
    dokladnie: $("dokladnie").checked,
    rozmiar_mb: rozmiarDocelowy,
    zastap: przyciete() && $("zastap").checked,
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
  if (zlecenie.zastap && !confirm("Zastąpić oryginał przyciętą wersją? Oryginał trafi do Kosza.")) return;
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
    if (!$("odtwarzacz").hidden) zamknijOdtwarzacz();
  } else if (e.key === " " && !$("odtwarzacz").hidden && !e.target.closest("input, select, button, .uchwyt")) {
    e.preventDefault();
    przelaczGraj();
  }
});
// Bez domyślnego menu przeglądarki
document.addEventListener("contextmenu", (e) => e.preventDefault());

// ---------- Ustawienia ----------

const PRESETY = [
  { nazwa: "Wydajność", fps: 30, bitrate: 10 },
  { nazwa: "Zrównoważona", fps: 60, bitrate: 20 },
  { nazwa: "Wysoka", fps: 60, bitrate: 40 },
];

for (const p of PRESETY) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "segment";
  b.textContent = `${p.nazwa} · ${p.fps} FPS · ${p.bitrate} Mb/s`;
  b.addEventListener("click", () => {
    $("fps").value = p.fps;
    $("bitrate").value = p.bitrate;
    odswiezFormularz();
  });
  b.dataset.fps = p.fps;
  b.dataset.bitrate = p.bitrate;
  $("presety").append(b);
}

async function wczytajUstawienia() {
  const u = await invoke("ustawienia");
  cfg = u.cfg;
  $("enkoder").replaceChildren(
    ...u.enkodery.map(([wartosc, opis]) => new Option(opis, wartosc))
  );
  $("bufor").value = cfg.bufor_sekundy;
  $("fps").value = cfg.fps;
  $("bitrate").value = cfg.bitrate_mbps;
  $("enkoder").value = cfg.enkoder;
  $("dzwiek").checked = cfg.dzwiek_systemowy;
  $("mikrofon").checked = cfg.mikrofon;
  $("skrot").textContent = cfg.skrot;
  $("skrot").dataset.wartosc = cfg.skrot;
  $("folder").textContent = cfg.folder_klipow;
  $("autostart").checked = u.autostart;
  $("ikona-zegar").checked = cfg.ikona_przy_zegarze;
  $("komunikat").textContent = "";
  odswiezFormularz();
}

// Szacunek RAM bufora: obraz (bitrate) + dźwięk (160 kb/s na ścieżkę)
function odswiezFormularz() {
  const s = Number($("bufor").value);
  const mbps = Number($("bitrate").value) || 0;
  const sciezki = Number($("dzwiek").checked) + Number($("mikrofon").checked);
  const bajty = s * (mbps * 1e6 + sciezki * 160e3) / 8;
  $("bufor-wartosc").textContent = s >= 60 ? `${Math.floor(s / 60)} min${s % 60 ? ` ${s % 60} s` : ""}` : `${s} s`;
  $("ram").textContent = `Bufor zajmie ok. ${formatMB(bajty)} RAM.`;
  for (const b of $("presety").children) {
    b.classList.toggle("wybrany", b.dataset.fps === $("fps").value && b.dataset.bitrate === $("bitrate").value);
  }
}

for (const id of ["bufor", "fps", "bitrate", "dzwiek", "mikrofon"]) {
  $(id).addEventListener("input", odswiezFormularz);
}

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
    bufor_sekundy: Number($("bufor").value),
    fps: Number($("fps").value),
    bitrate_mbps: Number($("bitrate").value),
    enkoder: $("enkoder").value,
    dzwiek_systemowy: $("dzwiek").checked,
    mikrofon: $("mikrofon").checked,
    skrot: $("skrot").dataset.wartosc,
    folder_klipow: $("folder").textContent,
    ikona_przy_zegarze: $("ikona-zegar").checked,
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

// ---------- Start ----------

$("odswiez").innerHTML = ikona("odswiez");
$("otworz-folder").innerHTML = zIkona("folder", "Folder");
$("odtw-folder").innerHTML = zIkona("folder", "Pokaż w folderze");
$("odtw-usun").innerHTML = zIkona("kosz", "Usuń");
$("odtw-zamknij").innerHTML = ikona("zamknij");
$("pusto-ikona").innerHTML = ikona("film");
$("wynik-ikona").innerHTML = ikona("przeciagnij");
odswiezPrzyciskGraj();

(async () => {
  const u = await invoke("ustawienia");
  cfg = u.cfg;
  odswiezPrzyciskZapisu();
  pokazStan(await invoke("stan"));
  pokazZakladke(location.hash.slice(1));
})();

listen("stan", (e) => pokazStan(e.payload));
listen("zakladka", (e) => pokazZakladke(e.payload));
listen("klip-zapisany", () => {
  dymek("Klip zapisany");
  if (!$("widok-klipy").hidden) wczytajKlipy();
});
