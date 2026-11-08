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
        img.src = convertFileSrc(m.obraz);
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
  const siatka = $("siatka");
  siatka.replaceChildren();
  const suma = klipy.reduce((a, k) => a + k.rozmiar, 0);
  $("podsumowanie").textContent = klipy.length
    ? `${klipy.length} ${odmiana(klipy.length)} · ${formatMB(suma)}`
    : "";
  $("pusto").hidden = klipy.length > 0;
  $("pusto-tekst").textContent = `Naciśnij ${cfg.skrot} w grze, aby zapisać ostatnie ${cfg.bufor_sekundy} s.`;

  for (const klip of klipy) {
    const karta = document.createElement("button");
    karta.className = "karta klip";
    karta.innerHTML = `
      <div class="obraz"><img alt=""><span class="dlugosc" hidden></span><span class="graj">${ikona("graj")}</span></div>
      <div class="opis"><h3></h3><p class="pomoc"></p></div>`;
    karta.querySelector("h3").textContent = formatData(klip.czas_ms);
    karta.querySelector(".pomoc").textContent = `${formatMB(klip.rozmiar)} · ${klip.nazwa}`;
    karta.addEventListener("click", () => odtworz(klip));
    karta.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      pokazMenu(e.clientX, e.clientY, klip);
    });
    siatka.append(karta);
    pobierzMiniature(klip, karta);
  }
}

function odmiana(n) {
  if (n === 1) return "klip";
  const r = n % 10, s = n % 100;
  return r >= 2 && r <= 4 && (s < 12 || s > 14) ? "klipy" : "klipów";
}

$("odswiez").addEventListener("click", wczytajKlipy);
$("otworz-folder").addEventListener("click", () => invoke("otworz_folder"));

// Odtwarzacz
let odtwarzany = null;

function odtworz(klip) {
  odtwarzany = klip;
  $("odtw-tytul").textContent = formatData(klip.czas_ms);
  $("odtw-opis").textContent = `${klip.nazwa} · ${formatMB(klip.rozmiar)}`;
  const v = $("wideo");
  v.src = convertFileSrc(klip.sciezka);
  $("odtwarzacz").hidden = false;
  v.play().catch(() => {});
  $("odtw-zamknij").focus();
}

function zamknijOdtwarzacz() {
  const v = $("wideo");
  v.pause();
  v.removeAttribute("src");
  v.load(); // zwalnia plik, żeby dało się go usunąć
  $("odtwarzacz").hidden = true;
  odtwarzany = null;
}

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

// Menu kontekstowe
let klipMenu = null;

function pokazMenu(x, y, klip) {
  klipMenu = klip;
  const m = $("menu");
  m.hidden = false;
  const { width, height } = m.getBoundingClientRect();
  m.style.left = `${Math.min(x, innerWidth - width - 8)}px`;
  m.style.top = `${Math.min(y, innerHeight - height - 8)}px`;
  m.querySelector("button").focus();
}

const ukryjMenu = () => ($("menu").hidden = true);

$("menu").querySelector('[data-akcja="odtworz"]').innerHTML = zIkona("graj", "Odtwórz");
$("menu").querySelector('[data-akcja="folder"]').innerHTML = zIkona("folder", "Pokaż w folderze");
$("menu").querySelector('[data-akcja="usun"]').innerHTML = zIkona("kosz", "Usuń");
$("menu").addEventListener("click", (e) => {
  const b = e.target.closest("button");
  if (!b || !klipMenu) return;
  ukryjMenu();
  const akcja = { odtworz, folder: pokazWFolderze, usun }[b.dataset.akcja];
  akcja(klipMenu);
});
document.addEventListener("click", (e) => {
  if (!e.target.closest("#menu")) ukryjMenu();
});
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape" || nasluchSkrotu) return;
  ukryjMenu();
  if (!$("odtwarzacz").hidden) zamknijOdtwarzacz();
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
