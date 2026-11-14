// Test okna Clipeona bez Windowsa: otwiera ui/index.html w Chromium z udawanym Tauri
// i sprawdza, że skrypt startuje bez błędów, a najważniejsze elementy działają.
// Uruchomienie: npm install --no-save playwright && npx playwright install chromium && node testy/okno.mjs
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");
const ui = fileURLToPath(new URL("../ui/", import.meta.url));
const bledy = [];
// ZRZUTY=folder zapisuje zrzuty okna do obejrzenia (np. ZRZUTY=/tmp node testy/okno.mjs)
const zrzut = async (nazwa) => process.env.ZRZUTY && strona.screenshot({ path: `${process.env.ZRZUTY}/${nazwa}.png`, fullPage: true });
const sprawdz = (warunek, opis) => {
  if (!warunek) bledy.push(opis);
};

// 1. Każdy $("id") z app.js istnieje w index.html (tak zepsuło się okno w 1.3.2)
const js = readFileSync(ui + "app.js", "utf8");
const html = readFileSync(ui + "index.html", "utf8");
for (const id of new Set([...js.matchAll(/\$\("([\w-]+)"\)/g)].map((m) => m[1]))) {
  sprawdz(html.includes(`id="${id}"`), `app.js odwołuje się do #${id}, którego nie ma w index.html`);
}

// Podgląd głośności na żywo gra ścieżki z adresów blob: — polityka bezpieczeństwa okna musi je wpuszczać
const csp = JSON.parse(readFileSync(new URL("../tauri.conf.json", import.meta.url), "utf8")).app.security.csp;
sprawdz(/media-src[^;]*\bblob:/.test(csp), "CSP w tauri.conf.json nie wpuszcza blob: w media-src (dźwięk na żywo w edytorze byłby zablokowany)");

// 2. Okno w przeglądarce z udawanym rdzeniem
function udawanyTauri() {
  const cfg = {
    bufor_sekundy: 30, fps: 60, bitrate_mbps: 20, enkoder: "auto", dzwiek_systemowy: true, mikrofon: false,
    folder_klipow: "C:\\Klipy", skrot: "Alt+F10", ikona_przy_zegarze: true, ikona_ustawiona: true,
    dzwiek_zapisu: true, powiadomienie_zapisu: true, sprzatanie_dni: 0, sprzatanie_gb: 0,
    sprawdzaj_aktualizacje: true, monitor: "", monitor_id: "", tryb: "gra", monitor_pulpitu: "",
    monitor_pulpitu_id: "", fps_pulpitu: 30, bitrate_pulpitu_mbps: 12, metoda_przechwytywania: "auto",
    mikrofon_urzadzenie: "", gra: "", gra_nazwa: "", skrot_filmu: "Alt+F9", animowane_tlo: true, kolorowy_napis: true,
    format_filmu: "bez_zmian",
  };
  const stan = {
    nagrywa: true, pauza: false, bledy: [], wznawianie: null, przelaczanie: null, tryb: "gra", ekran: "Ekran 1", monitor: "",
    droga: "dda", gra: "Gra testowa", wybrana_gra: "", gra_proces: "", czeka_na_gre: false, mikrofon: true,
    ramka: false, film: null, film_zapis: false, skrot_filmu_blad: null, film_zmniejszanie: null,
  };
  const odpowiedzi = {
    stan: () => stan,
    ustawienia: () => ({ cfg: structuredClone(cfg), autostart: false, enkodery: [["auto", "Automatycznie"]], formaty_filmu: [["bez_zmian", "Pełna jakość — bez zmian (największy plik)"], ["hevc", "Mniejszy plik — HEVC, ok. połowa rozmiaru"], ["av1", "Najmniejszy plik — AV1, ok. 1/3 rozmiaru"]], wersja: "test", repozytorium: "" }),
    mikrofony: () => [
      { id: "m1", nazwa: "Mikrofon (USB)", domyslny: true },
      { id: "m2", nazwa: "Mikrofon (Zestaw)", domyslny: false },
    ],
    gry: () => [{ nazwa: "Gra testowa", proces: "gra.exe" }, { nazwa: "Druga gra", proces: "druga.exe" }],
    monitory: () => [{ id: "\\\\.\\DISPLAY1", numer: 1, glowny: true, nazwa: "Monitor", opis: "Ekran 1", szerokosc: 1920, wysokosc: 1080, hz: 144, hdr: false }],
    klipy: () => window.__klipy || [],
    przelacz_film: () => {
      window.__filmy = (window.__filmy || 0) + 1;
    },
    wstrzymaj_skroty: (a) => {
      window.__wstrzymania = [...(window.__wstrzymania || []), a.wstrzymaj];
    },
    zapisz_ustawienia: (a) => {
      window.__zapisane = a;
    },
    info_klipu: () => ({ dlugosc: 20, sciezki_audio: 3, fps: 60, szerokosc: 192, wysokosc: 108, webp: true, ...window.__info }),
    kopiuj_klip: (a) => {
      window.__skopiowane = a.sciezka;
    },
    dzwiek_podgladu: () => {
      const b = atob(window.__dzwiek);
      const u = new Uint8Array(b.length);
      for (let i = 0; i < b.length; i++) u[i] = b.charCodeAt(i);
      return u.buffer;
    },
    // Wykres głośności: cisza, potem coraz głośniej
    fala_dzwieku: (a) => Array.from({ length: a.kubelki }, (_, i) => (i < a.kubelki / 4 ? 0 : Math.round((i / a.kubelki) * 255))),
    eksportuj: (a) => {
      window.__zlecenia = [...(window.__zlecenia || []), a.zlecenie];
      return { sciezka: a.zlecenie.format === "gif" ? "C:\\Klipy\\klip_cut.gif" : a.sciezka, rozmiar: 1234567 };
    },
    ustaw_gre: (a) => {
      stan.gra_proces = a.proces;
      stan.wybrana_gra = a.nazwa;
      window.__wybranaGra = a;
    },
  };
  window.__TAURI__ = {
    core: { invoke: async (k, a) => (odpowiedzi[k] ? odpowiedzi[k](a) : null), convertFileSrc: (x) => x },
    event: { listen: async () => () => {} },
    window: {
      getCurrentWindow: () => ({
        minimize() {}, toggleMaximize() {}, close() {}, isMaximized: async () => false, onResized() {},
        // Zamknięcie okna: test woła zapamiętaną obsługę sam, tak jak Tauri przy krzyżyku albo Alt+F4
        onCloseRequested(h) { window.__przyZamknieciu = h; },
        destroy() { window.__zniszczone = (window.__zniszczone || 0) + 1; },
      }),
    },
  };
}

const przegladarka = await chromium.launch();
const strona = await przegladarka.newPage({ viewport: { width: 1200, height: 800 } });
strona.on("pageerror", (e) => bledy.push(`błąd skryptu: ${e.message}`));
strona.on("console", (m) => m.type() === "error" && bledy.push(`konsola: ${m.text()}`));
await strona.addInitScript(udawanyTauri);
await strona.addInitScript((d) => (window.__dzwiek = d), readFileSync(new URL("dzwiek.webm", import.meta.url)).toString("base64"));
await strona.goto(pathToFileURL(ui + "index.html").href);
await strona.waitForTimeout(500);

sprawdz((await strona.textContent("#status-tekst")) !== "Łączenie…", "status został na „Łączenie…”");
for (const id of ["okno-min", "okno-max", "okno-zamknij"]) {
  sprawdz((await strona.locator(`#${id} svg`).count()) === 1, `przycisk #${id} nie ma ikony`);
}

// Różne stany rdzenia nie mogą wywrócić okna
for (const zmiana of [
  { pauza: true, nagrywa: false },
  { czeka_na_gre: true, nagrywa: false, wybrana_gra: "Gra <b>testowa</b>", gra_proces: "gra.exe" },
  { bledy: ["Nie udało się uruchomić nagrywania"], nagrywa: false },
  { wznawianie: "brak nowych klatek" },
  { film: 75 },
  { film: null, film_zapis: true },
  { droga: "potok", ramka: true, skrot_filmu_blad: "Skrót Alt+F9 jest zajęty przez inny program" },
  { tryb: "pulpit", ekran: "Ekran 2" },
]) {
  await strona.evaluate((z) => pokazStan({ ...stan, ...z }), zmiana);
}
sprawdz((await strona.textContent("#wybor")).includes("Ekran 2"), "w trybie pulpitu przycisk nie pokazuje ekranu");
// Przekreślony mikrofon przy statusie tylko wtedy, gdy mikrofon nie jest nagrywany
sprawdz(await strona.isHidden("#status-mik"), "ikona wyłączonego mikrofonu widoczna przy włączonym mikrofonie");
await strona.evaluate(() => pokazStan({ ...stan, mikrofon: false }));
sprawdz((await strona.isVisible("#status-mik")) && (await strona.locator("#status-mik svg").count()) === 1, "brak ikony przekreślonego mikrofonu przy wyłączonym mikrofonie");
await zrzut("0-mikrofon-wylaczony");
await strona.evaluate(() => pokazStan({ ...stan, mikrofon: true }));
await strona.evaluate(() => pokazStan({ ...stan, tryb: "gra", pauza: false, czeka_na_gre: false, bledy: [], wznawianie: null, nagrywa: true, film: null, film_zapis: false, droga: "dda", ramka: false, skrot_filmu_blad: null }));

// Windows zmienił urządzenie dźwięku (np. po zamknięciu gry): żółta kropka z napisem, bez paska z błędem
await strona.evaluate(() => pokazStan({ ...stan, przelaczanie: "dźwięk" }));
sprawdz((await strona.textContent("#status-tekst")) === "Przełączam…", "przełączanie dźwięku nie zmienia napisu przy kropce");
sprawdz((await strona.locator("#kropka.wznawianie").count()) === 1, "przełączanie dźwięku nie daje żółtej kropki");
sprawdz(await strona.isHidden("#baner"), "przełączanie dźwięku pokazuje pasek z problemem");
await zrzut("0-przelaczanie-dzwieku");
await strona.evaluate(() => pokazStan({ ...stan, przelaczanie: null }));
sprawdz((await strona.textContent("#status-tekst")) === "Nagrywa: Gra testowa · 60 kl./s", "po przełączeniu dźwięku status nie wraca do „Nagrywa”");


// ---------- Tło ze znaków i płyn pod myszką ----------
// Jasne znaki (przezroczystość ≥ 0,4) rysuje tylko smuga — liczymy je wokół podanego punktu
const jasneZnaki = (x, y) =>
  strona.evaluate(([x, y]) => {
    const c = document.getElementById("tlo");
    const s = c.width / window.innerWidth;
    const d = c.getContext("2d").getImageData((x - 80) * s, (y - 80) * s, 160 * s, 160 * s).data;
    let n = 0;
    for (let i = 3; i < d.length; i += 4) if (d[i] >= 95) n++;
    return n;
  }, [x, y]);
sprawdz(await strona.evaluate(() => {
  const c = document.getElementById("tlo");
  const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
  for (let i = 3; i < d.length; i += 4) if (d[i]) return true;
  return false;
}), "tło ze znaków jest puste");
// Tło nie zajmuje całego okna: spora część jest czarna (plamy napisu, kropki na brzegach, potem nic).
// Liczymy kwadraty 12 × 12 px, w których nie ma ani jednego znaku.
const czarne = await strona.evaluate(() => {
  const c = document.getElementById("tlo");
  const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
  const K = 12;
  let [puste, wszystkie] = [0, 0];
  for (let by = 0; by + K <= c.height; by += K) {
    for (let bx = 0; bx + K <= c.width; bx += K) {
      let jest = false;
      for (let y = by; y < by + K && !jest; y++) for (let x = bx; x < bx + K; x++) if (d[(y * c.width + x) * 4 + 3]) { jest = true; break; }
      wszystkie++;
      if (!jest) puste++;
    }
  }
  return puste / wszystkie;
});
sprawdz(czarne > 0.2, `tło jest za gęste: tylko ${(czarne * 100).toFixed(0)}% okna jest czarne`);
// Szybki ruch nad pustym miejscem zostawia smugę na całej drodze…
await strona.mouse.move(440, 640);
await strona.waitForTimeout(100);
for (let i = 1; i <= 8; i++) {
  await strona.mouse.move(440 + i * 30, 640 + (i % 3) * 4);
  await strona.waitForTimeout(12);
}
await strona.waitForTimeout(60);
const [poczatek, koniec] = [await jasneZnaki(500, 640), await jasneZnaki(640, 640)];
sprawdz(poczatek > 20 && koniec > 20, `szybki ruch myszki nie zostawia smugi: ${poczatek}, ${koniec}`);
await zrzut("0a-tlo-smuga");
// …myszka, która stoi, niczego nie podtrzymuje: smuga sama gaśnie
await strona.waitForTimeout(6000);
sprawdz((await jasneZnaki(500, 640)) === 0 && (await jasneZnaki(640, 640)) === 0, "smuga nie gaśnie, gdy myszka stoi");
// Powolny ruch zostawia najwyżej maleńką plamkę (szybki — szeroką strugę)
for (let i = 1; i <= 30; i++) {
  await strona.mouse.move(440 + i * 2, 660);
  await strona.waitForTimeout(25);
}
await strona.waitForTimeout(60);
const wolno = await jasneZnaki(490, 660);
sprawdz(wolno * 4 < Math.max(poczatek, koniec), `powolny ruch myszki zostawia za dużą plamę: ${wolno} (szybki: ${poczatek}, ${koniec})`);
await strona.waitForTimeout(6000);
sprawdz((await jasneZnaki(490, 660)) === 0, "plamka po powolnym ruchu nie gaśnie");
// Nad przyciskiem smuga nie powstaje
const przyciskFolder = await strona.locator("#otworz-folder").boundingBox();
await strona.mouse.move(przyciskFolder.x + 4, przyciskFolder.y + 10);
await strona.waitForTimeout(50);
for (let i = 1; i <= 6; i++) {
  await strona.mouse.move(przyciskFolder.x + 4 + i * 12, przyciskFolder.y + 10 + (i % 2) * 6);
  await strona.waitForTimeout(12);
}
await strona.waitForTimeout(60);
sprawdz((await jasneZnaki(przyciskFolder.x + 40, przyciskFolder.y + 10)) === 0, "smuga powstaje, gdy mysz jest nad przyciskiem");
// Napis „Clipeon” ma zorzę (kolorowe tło przycięte do liter)
const marka = await strona.evaluate(() => {
  const s = getComputedStyle(document.querySelector(".marka"));
  return [s.backgroundClip || s.webkitBackgroundClip, s.animationName, s.backgroundImage.includes("radial-gradient")];
});
sprawdz(marka[0].startsWith("text") && marka[1] === "zorza" && marka[2], `napis „Clipeon” nie ma zorzy: ${JSON.stringify(marka)}`);
// Ustawienie „Kolorowy napis” wyłączone: zwykły biały napis bez animacji
await strona.evaluate(() => { cfg.kolorowy_napis = false; odswiezPrzyciskZapisu(); });
const bialy = await strona.evaluate(() => {
  const s = getComputedStyle(document.querySelector(".marka"));
  return [s.animationName, s.backgroundImage, s.color];
});
sprawdz(bialy[0] === "none" && bialy[1] === "none" && bialy[2] !== "rgba(0, 0, 0, 0)", `wyłączony kolorowy napis dalej ma zorzę: ${JSON.stringify(bialy)}`);
await strona.evaluate(() => { cfg.kolorowy_napis = true; odswiezPrzyciskZapisu(); });

// ---------- Film (długie nagrywanie) ----------
sprawdz((await strona.textContent("#film")).trim() === "Nagraj film", `przycisk filmu: „${await strona.textContent("#film")}”`);
await strona.click("#film");
sprawdz((await strona.evaluate(() => window.__filmy)) === 1, "„Nagraj film” nie trafia do rdzenia");
await strona.evaluate(() => pokazStan({ ...stan, film: 3725 }));
sprawdz((await strona.textContent("#film")).trim() === "1:02:05" && (await strona.locator("#film.trwa").count()) === 1, `trwający film: „${await strona.textContent("#film")}”`);
await strona.waitForTimeout(1100);
sprawdz((await strona.textContent("#film")).trim() !== "1:02:05", "czas filmu na przycisku stoi w miejscu");
await zrzut("0b-film-trwa");
await strona.evaluate(() => pokazStan({ ...stan, film: null, film_zapis: true }));
sprawdz(await strona.isDisabled("#film"), "przycisk filmu jest klikalny w trakcie składania pliku");
await strona.evaluate(() => pokazStan({ ...stan, film_zapis: false }));
// Zakładka Filmy pokazuje tylko długie nagrania, Klipy — resztę
await strona.evaluate(() => {
  const teraz = Date.now();
  window.__klipy = [
    { sciezka: "C:\\Klipy\\Clipeon_Film_2026-11-11_20-00-00.mp4", nazwa: "Clipeon_Film_2026-11-11_20-00-00.mp4", rozmiar: 4.2e9, czas_ms: teraz, pulpit: false, film: true },
    { sciezka: "C:\\Klipy\\Clipeon_2026-11-11_19-00-00.mp4", nazwa: "Clipeon_2026-11-11_19-00-00.mp4", rozmiar: 50e6, czas_ms: teraz, pulpit: false, film: false },
    { sciezka: "C:\\Klipy\\Clipeon_2026-11-11_18-00-00.mp4", nazwa: "Clipeon_2026-11-11_18-00-00.mp4", rozmiar: 50e6, czas_ms: teraz, pulpit: true, film: false },
  ];
});
await strona.click('[data-zakladka="filmy"]');
await strona.waitForTimeout(200);
sprawdz((await strona.textContent("#biblioteka-tytul")) === "Filmy" && (await strona.locator(".klip").count()) === 1, "zakładka Filmy nie pokazuje samych filmów");
sprawdz((await strona.textContent("#podsumowanie")).startsWith("1 film "), `podsumowanie filmów: „${await strona.textContent("#podsumowanie")}”`);
await zrzut("0c-filmy");
await strona.click('[data-zakladka="klipy"]');
await strona.waitForTimeout(200);
sprawdz((await strona.textContent("#biblioteka-tytul")) === "Klipy" && (await strona.locator(".klip").count()) === 2, "zakładka Klipy pokazuje filmy albo gubi klipy");
sprawdz((await strona.textContent("#podsumowanie")).startsWith("2 klipy "), `podsumowanie klipów: „${await strona.textContent("#podsumowanie")}”`);

// Mleczne szkło: karty rozmywają tło, ale kafelki klipów zostają pełne (koszt karty graficznej)
{
  const szklo = await strona.evaluate(() => {
    const filtr = (sel) => { const el = document.querySelector(sel); return el ? getComputedStyle(el).backdropFilter : "brak elementu " + sel; };
    return { karta: filtr(".karta:not(.klip)"), klip: filtr(".karta.klip"), gora: filtr(".gora") };
  });
  sprawdz(szklo.karta.includes("blur"), `karta nie rozmywa tła: ${szklo.karta}`);
  sprawdz(szklo.klip === "none", `kafelek klipu rozmywa tło (koszt GPU): ${szklo.klip}`);
  sprawdz(szklo.gora.includes("blur"), `pasek górny nie rozmywa tła: ${szklo.gora}`);
}
await strona.evaluate(() => (window.__klipy = []));
await strona.click('[data-zakladka="filmy"]');
await strona.waitForTimeout(200);
sprawdz((await strona.textContent("#pusto-tytul")) === "Brak filmów" && (await strona.textContent("#pusto-tekst")).includes("Alt+F9"), "pusta zakładka Filmy nie podpowiada skrótu");
await strona.click('[data-zakladka="klipy"]');
await strona.waitForTimeout(100);

// Tryb gry: zamiast wyboru ekranu lista działających gier
await strona.click("#wybor");
await strona.waitForSelector("#lista-wyboru [role=option]", { timeout: 3000 }).catch(() => {});
await zrzut("1-pasek-gry");
const gry = await strona.locator("#lista-wyboru [role=option]").allTextContents();
sprawdz(gry.some((t) => t.includes("Druga gra")), `lista na pasku nie pokazuje gier: ${JSON.stringify(gry)}`);
const druga = strona.locator("#lista-wyboru [role=option]", { hasText: "Druga gra" });
if (await druga.count()) await druga.click();
await strona.waitForTimeout(100);
sprawdz((await strona.evaluate(() => window.__wybranaGra?.proces)) === "druga.exe", "wybór gry z paska nie trafił do rdzenia");

// Ustawienia: wybór mikrofonu działa także przy wyłączonym „Mój mikrofon”
await strona.click('[data-zakladka="ustawienia"]');
await strona.waitForTimeout(300);
await zrzut("2-ustawienia");
const mik = strona.locator("#mikrofon-urzadzenie");
sprawdz(!(await mik.isDisabled()), "lista mikrofonów jest wyłączona");
sprawdz((await mik.locator("option").count()) === 3, "lista mikrofonów nie ma wszystkich urządzeń");
if (!(await mik.isDisabled())) await mik.selectOption("m2");
sprawdz(await strona.isChecked("#mikrofon"), "wybranie mikrofonu nie włączyło „Mój mikrofon”");
sprawdz((await strona.locator("#gra").count()) === 0, "pole „Gra” zostało w ustawieniach");

// „Mój głos”: dwa przyciski zamiast przełącznika
// „Edytor zaczyna od” zniknęło z ustawień
sprawdz(!/Edytor zaczyna od|Bez głosu/.test(await strona.textContent("#formularz")), "w ustawieniach zostało „Edytor zaczyna od”");

// Prosty widok (domyślny): same nazwy presetów, bez opisów, liczb i Zaawansowanych
sprawdz(!(await strona.isChecked("#dla-nerdow")), "„Dla nerdów” jest domyślnie włączone");
{
  const proste = await strona.evaluate(() => {
    const widac = (el) => el && el.offsetParent !== null && getComputedStyle(el).display !== "none";
    return {
      opisy: [...document.querySelectorAll("#formularz .ust .opis .pomoc:not(.uwaga):not(.sciezka)")].filter(widac).map((el) => el.id || el.textContent.slice(0, 30)),
      pola: ["fps", "bitrate", "fps-pulpitu", "bitrate-pulpitu", "enkoder", "metoda", "ram", "jakosc-filmu"].filter((id) => widac($(id))),
      liczby: [...document.querySelectorAll(".presety small")].filter(widac).length,
      ekran: widac($("monitor")),
      nazwy: [...document.querySelectorAll("#presety .segment:not([hidden]) span")].map((el) => el.textContent),
      filmy: [...document.querySelectorAll("#presety-filmu .segment span")].map((el) => el.textContent),
      folder: widac($("folder")),
    };
  });
  sprawdz(proste.opisy.length === 0, `w prostym widoku widać opisy: ${proste.opisy.join(" | ")}`);
  sprawdz(proste.pola.length === 0, `w prostym widoku widać pola dla nerdów: ${proste.pola.join(", ")}`);
  sprawdz(proste.liczby === 0, "w prostym widoku przyciski jakości pokazują klatki i Mb/s");
  sprawdz(!proste.ekran, "przy jednym monitorze widać wybór ekranu");
  sprawdz(proste.nazwy.join(",") === "Lekka,Standard,Najwyższa", `przyciski jakości gry: ${proste.nazwy}`);
  sprawdz(proste.filmy.join(",") === "Oryginał,Mniejszy,Najmniejszy", `przyciski rozmiaru filmów: ${proste.filmy}`);
  sprawdz(proste.folder, "w prostym widoku nie widać folderu klipów");
}
await zrzut("2b-ustawienia-proste");
// Rozmiar filmów przyciskiem: zmienia ukryte pole i daje niezapisaną zmianę
await strona.click('#presety-filmu .segment[data-format="av1"]');
sprawdz((await strona.inputValue("#format-filmu")) === "av1" && (await strona.locator('#presety-filmu .segment.wybrany[data-format="av1"]').count()) === 1, "przycisk rozmiaru filmów nie zmienia wyboru");
sprawdz((await strona.evaluate(() => zmienioneUstawienia())).includes("format_filmu"), "zmiana rozmiaru filmów nie jest niezapisaną zmianą");
await strona.click('#presety-filmu .segment[data-format="bez_zmian"]');
// Wartości spoza presetów: widać „Własne” zamiast pustego wyboru
await strona.evaluate(() => { $("fps").value = 45; $("fps").dispatchEvent(new Event("input", { bubbles: true })); });
sprawdz(await strona.isVisible("#presety .segment.wlasne"), "własne klatki/s nie pokazują „Własne”");
await strona.evaluate(() => { $("fps").value = 60; $("fps").dispatchEvent(new Event("input", { bubbles: true })); });
sprawdz(await strona.isHidden("#presety .segment.wlasne"), "„Własne” nie znika po powrocie do presetu");
// „Dla nerdów” działa od razu, bez paska zapisu, i jest pamiętane
const zmianyPrzed = await strona.evaluate(() => zmienioneUstawienia().join());
await strona.click('label[for="dla-nerdow"]');
sprawdz(await strona.isVisible("#fps") && await strona.isVisible("#enkoder") && await strona.isVisible("#ram"), "„Dla nerdów” nie pokazuje pól i Zaawansowanych");
sprawdz((await strona.evaluate(() => zmienioneUstawienia().join())) === zmianyPrzed, "„Dla nerdów” udaje niezapisaną zmianę ustawień");
sprawdz((await strona.evaluate(() => localStorage.getItem("clipeon.dlaNerdow"))) === "1", "„Dla nerdów” nie jest zapamiętane");

// „Zaawansowane” leżą zaraz pod Systemem (ta sama kolumna) i widać je w trybie „Dla nerdów”
sprawdz(await strona.isVisible("#enkoder"), "„Zaawansowane” nie są widoczne w trybie „Dla nerdów”");
await zrzut("3-zaawansowane");
sprawdz(await strona.evaluate(() => document.querySelector(".karta.zaawansowane").closest(".grupa") === document.getElementById("autostart").closest(".grupa")), "„Zaawansowane” nie są pod Systemem");

// ---------- Jakość nagrywania: karty Gra / Pulpit / Filmy ----------
// Klatki pulpitu widać od razu (w 1.4.3 były w zwiniętym wierszu i tester zmieniał klatki gry)
for (const id of ["fps", "bitrate", "fps-pulpitu", "bitrate-pulpitu", "monitor", "monitor-pulpitu", "presety-filmu"]) {
  sprawdz(await strona.isVisible(`#${id}`), `pole #${id} nie jest widoczne bez rozwijania`);
}
sprawdz((await strona.locator("details").count()) === 0, "w ustawieniach zostały zwijane wiersze");
// Trzy karty obok siebie, a każde pole leży w swojej karcie
const karty = await strona.evaluate(() => ["jakosc-gra", "jakosc-pulpit", "jakosc-filmy"].map((id) => Math.round($(id).getBoundingClientRect().top)));
sprawdz(new Set(karty).size === 1, `karty jakości nie leżą obok siebie: ${karty}`);
sprawdz(await strona.evaluate(() => $("fps-pulpitu").closest(".karta") === $("jakosc-pulpit") && $("fps").closest(".karta") === $("jakosc-gra") && $("format-filmu").closest(".karta") === $("jakosc-filmy")), "pola jakości są w złych kartach");
// Etykiety mówią, który tryb zmieniają
sprawdz((await strona.textContent('label:has(#fps-pulpitu)')).includes("pulpitu") && (await strona.textContent('label:has(#fps)')).includes("gry"), "etykiety klatek nie mówią, czy to gra, czy pulpit");
// Karta trybu, który nagrywa, jest podświetlona; plakietka idzie za przełącznikiem
sprawdz((await strona.isVisible("#jakosc-gra .plakietka")) && (await strona.isHidden("#jakosc-pulpit .plakietka")), "karta gry nie jest oznaczona jako nagrywająca");
await strona.evaluate(() => pokazStan({ ...stan, tryb: "pulpit" }));
sprawdz((await strona.isVisible("#jakosc-pulpit .plakietka")) && (await strona.isHidden("#jakosc-gra .plakietka")), "plakietka „Nagrywa teraz” nie przechodzi na pulpit");
sprawdz((await strona.textContent("#jakosc-filmu")).includes("Pulpit · 30 kl./s"), `karta filmów nie mówi, w jakiej jakości nagra film: „${await strona.textContent("#jakosc-filmu")}”`);
sprawdz((await strona.textContent("#status-tekst")) === "Nagrywa pulpit · 30 kl./s", `status nie mówi, ile klatek nagrywa pulpit: „${await strona.textContent("#status-tekst")}”`);
await zrzut("2a-jakosc-pulpit");
await strona.evaluate(() => pokazStan({ ...stan, tryb: "gra" }));
// Ekran 144 Hz: więcej klatek nic nie da
await strona.fill("#fps-pulpitu", "200");
sprawdz((await strona.textContent("#hz-pulpitu")).includes("144"), "brak uwagi, że ekran 144 Hz nie da 200 kl./s");
await strona.fill("#fps-pulpitu", "30");
sprawdz((await strona.textContent("#hz-pulpitu")) === "", "uwaga o odświeżaniu ekranu nie znika");

// ---------- Pasek zapisu i niezapisane zmiany ----------
// Gotowe ustawienie pulpitu zmienia tylko pulpit
await strona.click('#presety-pulpitu .segment:has-text("Płynna")');
sprawdz((await strona.inputValue("#fps-pulpitu")) === "60" && (await strona.inputValue("#fps")) === "60" && (await strona.inputValue("#bitrate")) === "20", "gotowe ustawienie pulpitu zmieniło grę");
sprawdz(await strona.isVisible("#pasek-zapisu"), "po zmianie nie widać paska zapisu");
sprawdz(await strona.evaluate(() => $("fps-pulpitu").closest("label").classList.contains("zmienione-pole")), "zmienione pole nie jest zaznaczone");
// Pasek leży na środku dołu okna, a przycisk jest duży
const pasek = await strona.locator("#pasek-zapisu").boundingBox();
const okno = strona.viewportSize();
sprawdz(Math.abs(pasek.x + pasek.width / 2 - okno.width / 2) < 4 && pasek.y + pasek.height > okno.height - 60, `pasek zapisu nie jest na środku dołu: ${JSON.stringify(pasek)}`);
sprawdz((await strona.locator("#zapisz-ustawienia").boundingBox()).height >= 40, "przycisk „Zapisz zmiany” jest mały");
await zrzut("2b-pasek-zapisu");
// Wyjście do innej zakładki: pytanie, „Wróć” zostawia w ustawieniach ze zmianami
await strona.click('[data-zakladka="klipy"]');
sprawdz(await strona.isVisible("#pytanie-zmiany"), "wyjście z ustawień bez zapisu nie pyta o zmiany");
sprawdz((await strona.textContent("#pytanie-lista")).includes("Pulpit: klatki/s"), `pytanie nie mówi, co zmieniono: „${await strona.textContent("#pytanie-lista")}”`);
await zrzut("2c-pytanie");
await strona.click("#pytanie-wroc");
sprawdz((await strona.isVisible("#widok-ustawienia")) && (await strona.inputValue("#fps-pulpitu")) === "60", "„Wróć do ustawień” nie zostawia zmian");
// Kliknięcie zakładki Ustawienia, gdy już je widać, nie kasuje zmian (mikrofon „sam się zmieniał”)
await strona.click('[data-zakladka="ustawienia"]');
await strona.waitForTimeout(100);
sprawdz((await strona.inputValue("#fps-pulpitu")) === "60", "ponowne kliknięcie „Ustawienia” skasowało niezapisane zmiany");
// „Nie zapisuj”: wychodzi, nic nie wysyła, a po powrocie są zapisane wartości
await strona.click('[data-zakladka="klipy"]');
await strona.click("#pytanie-odrzuc");
await strona.waitForTimeout(100);
sprawdz((await strona.isVisible("#widok-klipy")) && !(await strona.evaluate(() => window.__zapisane)), "„Nie zapisuj” nie wychodzi albo coś zapisało");
await strona.click('[data-zakladka="ustawienia"]');
await strona.waitForTimeout(300);
sprawdz((await strona.inputValue("#fps-pulpitu")) === "30" && (await strona.isHidden("#pasek-zapisu")), "po „Nie zapisuj” formularz nie wrócił do zapisanych wartości");
// Zamknięcie okna ze zmianami: pytanie, „Zapisz” zapisuje i dopiero wtedy zamyka
await strona.selectOption("#mikrofon-urzadzenie", "m2");
await strona.fill("#bitrate-pulpitu", "16");
const zamkniecie = strona.evaluate(() => {
  let zatrzymane = false;
  return window.__przyZamknieciu({ preventDefault: () => (zatrzymane = true) }).then(() => zatrzymane);
});
await strona.waitForTimeout(100);
sprawdz(await strona.isVisible("#pytanie-zmiany"), "zamknięcie okna ze zmianami nie pyta o zapis");
sprawdz(!(await strona.evaluate(() => window.__zniszczone)), "okno zamknęło się przed odpowiedzią");
await strona.click("#pytanie-zapisz");
sprawdz(await zamkniecie, "zamknięcie okna nie zostało wstrzymane");
await strona.waitForTimeout(100);
sprawdz((await strona.evaluate(() => [window.__zapisane?.cfg.bitrate_pulpitu_mbps, window.__zniszczone])).join() === "16,1", "„Zapisz” w pytaniu nie zapisał albo nie zamknął okna");
sprawdz(await strona.evaluate(() => $("pasek-zapisu").classList.contains("zapisano")), "po zapisie pasek nie mówi „Zapisano”");
// Bez zmian okno zamyka się bez pytania
sprawdz(!(await strona.evaluate(() => { let z = false; window.__przyZamknieciu({ preventDefault: () => (z = true) }); return z; })), "zamknięcie bez zmian jest wstrzymywane");

// Suwak sekund przyskakuje do równych wartości przy przeciąganiu myszką (strzałki dalej chodzą co 5 s)
await strona.locator("#bufor").scrollIntoViewIfNeeded();
const suwakR = await strona.locator("#bufor").boundingBox();
const naSuwaku = (s) => suwakR.x + 8 + ((suwakR.width - 16) * (s - 5)) / 595;
await strona.mouse.move(naSuwaku(30), suwakR.y + suwakR.height / 2);
await strona.mouse.down();
await strona.mouse.move(naSuwaku(64), suwakR.y + suwakR.height / 2, { steps: 4 });
sprawdz((await strona.inputValue("#bufor")) === "60" && (await strona.textContent("#bufor-wartosc")) === "1 min", `suwak nie przyskakuje do 1 min: ${await strona.inputValue("#bufor")} / „${await strona.textContent("#bufor-wartosc")}”`);
await strona.mouse.move(naSuwaku(75), suwakR.y + suwakR.height / 2, { steps: 4 });
sprawdz((await strona.inputValue("#bufor")) === "75", `suwak nie pozwala ustawić wartości między równymi: ${await strona.inputValue("#bufor")}`);
await strona.mouse.move(naSuwaku(31), suwakR.y + suwakR.height / 2, { steps: 4 });
await strona.mouse.up();
sprawdz((await strona.inputValue("#bufor")) === "30", `suwak nie wraca na 30 s: ${await strona.inputValue("#bufor")}`);
await strona.focus("#bufor");
await strona.keyboard.press("ArrowRight");
sprawdz((await strona.inputValue("#bufor")) === "35", `strzałka na suwaku: ${await strona.inputValue("#bufor")}`);
await strona.keyboard.press("ArrowLeft");
sprawdz((await strona.locator("#bufor-kreski i").count()) === 10, "pod suwakiem nie ma kresek co minutę");

// Zapis ustawień (z pytania przy zamykaniu okna) wysyła wszystkie pola z formularza
const zapisane = await strona.evaluate(() => window.__zapisane);
sprawdz(zapisane, "„Zapisz ustawienia” nic nie wysłał");
if (zapisane) {
  const c = zapisane.cfg;
  const oczekiwane = { mikrofon: true, mikrofon_urzadzenie: "m2", bufor_sekundy: 30, fps: 60, fps_pulpitu: 30, bitrate_pulpitu_mbps: 16, skrot: "Alt+F10", skrot_filmu: "Alt+F9", animowane_tlo: true, kolorowy_napis: true, sprzatanie_dni: 0 };
  for (const [k, v] of Object.entries(oczekiwane)) sprawdz(c[k] === v, `zapis ustawień: ${k} = ${JSON.stringify(c[k])}, oczekiwano ${JSON.stringify(v)}`);
}

// Suwak „Ile ostatnich sekund” nie może skakać: wiersz ma tę samą wysokość, a suwak to samo miejsce
for (const szerokosc of [1100, 900, 820]) {
  await strona.setViewportSize({ width: szerokosc, height: 800 });
  await strona.waitForTimeout(50);
  const pomiary = new Set();
  for (const s of [5, 60, 135, 200, 310, 600]) {
    const p = await strona.evaluate((s) => {
      const b = document.getElementById("bufor");
      b.value = s;
      b.dispatchEvent(new Event("input", { bubbles: true }));
      const r = b.getBoundingClientRect();
      return `${Math.round(r.top)}:${Math.round(r.left)}:${Math.round(r.width)}:${Math.round(b.closest(".ust").getBoundingClientRect().height)}`;
    }, s);
    pomiary.add(p);
  }
  sprawdz(pomiary.size === 1, `suwak „Ile ostatnich sekund” skacze przy oknie ${szerokosc} px: ${[...pomiary].join(" | ")}`);
}
await strona.setViewportSize({ width: 1100, height: 800 });

// Sposób przechwytywania: bez napisów o żółtej ramce, za to widać, którym sposobem Clipeon nagrywa teraz
sprawdz(!/żółt/i.test(await strona.textContent("#formularz")), "w ustawieniach został napis o żółtej ramce");
sprawdz((await strona.textContent("#metoda-teraz")).includes("Desktop Duplication"), `brak informacji o używanym sposobie przechwytywania: „${await strona.textContent("#metoda-teraz")}”`);
await strona.evaluate(() => pokazStan({ ...stan, droga: "potok", ramka: true }));
sprawdz((await strona.textContent("#metoda-teraz")).includes("Tryb zgodności"), "informacja o sposobie przechwytywania nie idzie za stanem");
await strona.evaluate(() => pokazStan({ ...stan, droga: "dda", ramka: false }));

// ---------- Ustawianie skrótu ----------
const wstrzymania = () => strona.evaluate(() => window.__wstrzymania || []);
await strona.click("#skrot");
sprawdz((await wstrzymania()).at(-1) === true, "kliknięcie pola skrótu nie wyłącza skrótów Clipeona");
// Wciskane klawisze widać od razu, na szaro
await strona.keyboard.down("Control");
await strona.keyboard.down("Shift");
sprawdz((await strona.textContent("#skrot")) === "Ctrl + Shift + …", `podgląd wciskanych klawiszy: „${await strona.textContent("#skrot")}”`);
const [szary, zwykly] = await strona.evaluate(() => [getComputedStyle($("skrot")).color, getComputedStyle($("skrot-filmu")).color]);
sprawdz(szary !== zwykly, "podgląd wciskanych klawiszy nie jest szary");
await zrzut("3a-skrot-podglad");
await strona.keyboard.up("Shift");
sprawdz((await strona.textContent("#skrot")) === "Ctrl + …", "puszczony klawisz zostaje w podglądzie skrótu");
await strona.keyboard.down("F8");
await strona.keyboard.up("F8");
await strona.keyboard.up("Control");
sprawdz((await strona.textContent("#skrot")) === "Ctrl+F8" && (await strona.evaluate(() => $("skrot").dataset.wartosc)) === "Ctrl+F8", `nowy skrót nie został przyjęty: „${await strona.textContent("#skrot")}”`);
sprawdz((await wstrzymania()).at(-1) === false, "po ustawieniu skrótu skróty Clipeona nie wracają");
// Esc anuluje i też przywraca skróty
await strona.click("#skrot");
await strona.keyboard.press("Escape");
sprawdz((await strona.textContent("#skrot")) === "Ctrl+F8" && (await wstrzymania()).at(-1) === false, "Esc nie anuluje ustawiania skrótu");
// Skrót filmu: nie może być taki sam jak skrót klipu, a Delete go usuwa
await strona.click("#skrot-filmu");
await strona.keyboard.press("Control+F8");
sprawdz((await strona.textContent("#skrot-filmu")).includes("zajęty"), "skrót filmu dał się ustawić taki sam jak skrót klipu");
await strona.keyboard.press("Delete");
sprawdz((await strona.textContent("#skrot-filmu")) === "Bez skrótu" && (await strona.evaluate(() => $("skrot-filmu").dataset.wartosc)) === "", "Delete nie usuwa skrótu filmu");
// Utrata fokusu okna w trakcie ustawiania: skróty wracają
await strona.click("#skrot-filmu");
await strona.evaluate(() => window.dispatchEvent(new Event("blur")));
sprawdz((await wstrzymania()).at(-1) === false && (await strona.locator(".pole-skrotu.nasluch").count()) === 0, "po przejściu do innego okna skróty zostają wyłączone");
await strona.click("#zapisz-ustawienia");
await strona.waitForTimeout(100);
const zeSkrotami = await strona.evaluate(() => window.__zapisane.cfg);
sprawdz(zeSkrotami.skrot === "Ctrl+F8" && zeSkrotami.skrot_filmu === "", `zapis skrótów: ${JSON.stringify([zeSkrotami.skrot, zeSkrotami.skrot_filmu])}`);
// Rozmiar filmów: wybór trafia do ustawień, a podpowiedź mówi, ile zajmie godzina nagrania
sprawdz((await strona.locator("#format-filmu option").count()) === 3 && (await strona.locator("#presety-filmu .segment").count()) === 3, "„Rozmiar filmów” nie ma formatów z rdzenia");
sprawdz((await strona.textContent("#format-filmu-uwaga")).includes("9.2 GB"), `podpowiedź rozmiaru filmu: „${await strona.textContent("#format-filmu-uwaga")}”`);
await strona.click('#presety-filmu .segment[data-format="hevc"]');
sprawdz((await strona.textContent("#format-filmu-uwaga")).includes("4.3 GB"), `podpowiedź rozmiaru filmu HEVC: „${await strona.textContent("#format-filmu-uwaga")}”`);
await strona.click("#zapisz-ustawienia");
await strona.waitForTimeout(100);
sprawdz((await strona.evaluate(() => window.__zapisane.cfg.format_filmu)) === "hevc", "wybrany rozmiar filmów nie trafia do ustawień");
// Zmniejszanie filmu w tle widać w bibliotece
await strona.evaluate(() => pokazStan({ ...stan, film_zmniejszanie: 42 }));
sprawdz((await strona.evaluate(() => !$("film-zmniejszanie").hidden && $("film-zmniejszanie").textContent.includes("42%"))), "postęp zmniejszania filmu nie jest pokazany");
await strona.evaluate(() => pokazStan({ ...stan, film_zmniejszanie: null }));
sprawdz(await strona.evaluate(() => $("film-zmniejszanie").hidden), "napis o zmniejszaniu filmu nie znika");

// Wyłączone „Animowane tło” chowa tło od razu po zapisie
await strona.click('label:has(#animowane-tlo)');
await strona.click("#zapisz-ustawienia");
await strona.waitForTimeout(100);
sprawdz(await strona.isHidden("#tlo"), "wyłączenie „Animowane tło” nie chowa tła");
await strona.click('label:has(#animowane-tlo)');
await strona.click("#zapisz-ustawienia");
await strona.waitForTimeout(100);
sprawdz(await strona.isVisible("#tlo"), "włączenie „Animowane tło” nie przywraca tła");

// ---------- Edytor ----------
await strona.click('[data-zakladka="klipy"]');
const klip = { sciezka: new URL("klip.webm", import.meta.url).href, nazwa: "klip.webm", rozmiar: 50e6, czas_ms: Date.now(), pulpit: false };
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForFunction(() => dlugosc > 19 && dzwiek.sciezki.length === 2, null, { timeout: 5000 }).catch(() => {});
const v = (w) => strona.evaluate(w);
sprawdz((await v(() => Math.round(dlugosc))) === 20, "podgląd nie wczytał klipu");
sprawdz((await v(() => dzwiek.sciezki.length)) === 2, "głośność na żywo: nie ma dwóch ścieżek (gra, mikrofon)");
sprawdz(await v(() => $("wideo").muted), "głośność na żywo: wideo nie jest wyciszone (dźwięk grałby podwójnie)");

// Ścieżki dźwięku pod osią czasu: Gra i Mikrofon z wykresem, bez starych suwaków i „Mój głos”
sprawdz((await strona.locator("#sciezki-dzwieku .sciezka").count()) === 2, "nie ma dwóch ścieżek dźwięku (Gra, Mikrofon)");
sprawdz((await strona.locator("#glos-wiersz, #mik-grupa, #z-mikrofonem").count()) === 0, "zostały stare opcje „Głośność” / „Mój głos”");
await strona.waitForFunction(() => sciezkiGlosu.every((s) => s.fala), null, { timeout: 3000 }).catch(() => {});
sprawdz(await v(() => sciezkiGlosu.every((s) => s.fala && s.fala.length === KUBELKI_FALI)), "ścieżki nie dostały wykresu głośności");
// Wykres jest narysowany: w cichej części klipu pusto, w głośnej są słupki
const narysowane = await v(() => {
  const c = sciezkiGlosu[0].plotno;
  const g = c.getContext("2d");
  const ile = (od, doo) => {
    const d = g.getImageData(Math.floor(c.width * od), 0, Math.max(1, Math.floor(c.width * (doo - od))), c.height).data;
    let n = 0;
    for (let i = 3; i < d.length; i += 4) if (d[i]) n++;
    return n;
  };
  return [ile(0.02, 0.2), ile(0.6, 0.95)];
});
sprawdz(narysowane[0] === 0 && narysowane[1] > 50, `wykres głośności nie pokazuje, gdzie jest dźwięk: ${narysowane}`);
// Wykres leży dokładnie pod osią czasu
let [osR, falaR] = [await strona.locator("#os").boundingBox(), await strona.locator(".fala").first().boundingBox()];
sprawdz(Math.abs(osR.x - falaR.x) < 1.5 && Math.abs(osR.width - falaR.width) < 1.5, `ścieżka dźwięku nie leży pod osią czasu: ${JSON.stringify([osR, falaR])}`);

// Edytor zawsze zaczyna z głosem (ustawienia „Edytor zaczyna od” już nie ma); mikrofon wycisza przycisk przy ścieżce
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 1 && !sciezkiGlosu[1].el.classList.contains("wyciszona")), "edytor nie zaczyna z włączonym mikrofonem");
await strona.locator(".sciezka .wycisz").nth(1).click();
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 0 && sciezkiGlosu[1].el.classList.contains("wyciszona")), "przycisk przy ścieżce nie wycisza mikrofonu");
sprawdz((await strona.textContent("#eksportuj")) === "Zapisz bez mojego głosu", "wyciszony mikrofon nie daje „Zapisz bez mojego głosu”");
await strona.locator(".sciezka .wycisz").nth(1).click();
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 1), "przycisk przy ścieżce nie włącza mikrofonu");

// Przeciągnięcie w dół = ciszej, i słychać to od razu. Linia rusza ze swojego miejsca (100%), nawet gdy
// złapano ścieżkę przy samej górze — dawniej skakała pod kursor, czyli tutaj na prawie 200%
// (odciszenie mikrofonu chowa wiersz zapisu, więc ścieżki mierzymy od nowa, gdy układ się uspokoi)
await strona.waitForTimeout(200);
falaR = await strona.locator(".fala").first().boundingBox();
const [zlapX, zlapY] = [falaR.x + falaR.width / 2, falaR.y + 4];
await strona.mouse.move(zlapX, zlapY);
await strona.mouse.down();
await strona.mouse.move(zlapX, zlapY + 3);
sprawdz((await v(() => sciezkiGlosu[0].glosnosc)) === 1, `złapanie ścieżki z dala od linii zmienia głośność skokiem: ${await v(() => sciezkiGlosu[0].glosnosc)}`);
// Ćwierć wysokości pola w dół = o 50% ciszej
await strona.mouse.move(zlapX, zlapY + 3 + falaR.height / 4, { steps: 4 });
await strona.mouse.up();
sprawdz((await v(() => sciezkiGlosu[0].glosnosc)) === 0.5, `przeciągnięcie linii głośności: ${await v(() => sciezkiGlosu[0].glosnosc)} zamiast 0.5`);
sprawdz((await strona.locator(".fala-procent").first().textContent()) === "50%", "linia głośności nie pokazuje procentu");

// Wykres nie rośnie razem z linią: głośność zmienia głównie jasność słupków, a wysokość tylko odrobinę
const wykresPrzy = (g) =>
  strona.evaluate((g) => {
    const s = sciezkiGlosu[0];
    ustawGlosnosc(s, g);
    const c = s.plotno;
    const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data;
    let [szczyt, jasnosc] = [c.height, 0];
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        const a = d[(y * c.width + x) * 4 + 3];
        if (a) {
          szczyt = Math.min(szczyt, y);
          jasnosc = Math.max(jasnosc, a);
        }
      }
    }
    return { wys: (c.height - szczyt) / c.height, jasnosc: jasnosc / 255 };
  }, g);
const [w0, w100, w200] = [await wykresPrzy(0), await wykresPrzy(1), await wykresPrzy(2)];
sprawdz(w0.wys > 0.4 && w200.wys < 0.8 && w200.wys - w0.wys < 0.2, `wykres dalej rośnie z głośnością: ${JSON.stringify([w0, w100, w200])}`);
sprawdz(w0.wys < w100.wys && w100.wys < w200.wys, `wykres nie rusza się ani odrobinę przy zmianie głośności: ${JSON.stringify([w0, w100, w200])}`);
sprawdz(w0.jasnosc < w100.jasnosc - 0.2 && w100.jasnosc < w200.jasnosc - 0.2, `głośność nie zmienia jasności wykresu: ${JSON.stringify([w0, w100, w200])}`);
// Etykieta z procentem nie przeskakuje: przy sąsiednich wartościach przesuwa się najwyżej o kilka pikseli
const etykietaY = async (g) => {
  await strona.evaluate((g) => ustawGlosnosc(sciezkiGlosu[0], g), g);
  return (await strona.locator(".fala-procent").first().boundingBox()).y;
};
let poprzedniaY = await etykietaY(0);
let najwiekszySkok = 0;
for (let g = 0.05; g <= 2.001; g += 0.05) {
  const y = await etykietaY(g);
  najwiekszySkok = Math.max(najwiekszySkok, Math.abs(y - poprzedniaY));
  poprzedniaY = y;
}
sprawdz(najwiekszySkok <= 1.5, `etykieta głośności przeskakuje o ${najwiekszySkok} px między sąsiednimi wartościami`);
const [etykieta0, etykieta200] = [await etykietaY(0), await etykietaY(2)];
sprawdz(etykieta200 >= falaR.y && etykieta0 + 18 <= falaR.y + falaR.height + 0.5, `etykieta głośności wychodzi poza ścieżkę: ${JSON.stringify([etykieta0, etykieta200, falaR])}`);
await v(() => ustawGlosnosc(sciezkiGlosu[0], 0.5));
// Wyciszenie mikrofonu przyciskiem przy ścieżce
await strona.locator(".sciezka .wycisz").nth(1).click();
await strona.waitForTimeout(200);
const wzmocnienia = await v(() => dzwiek.sciezki.map((s) => Math.round(s.g.gain.value * 100) / 100));
sprawdz(wzmocnienia[0] === 0.5 && wzmocnienia[1] === 0, `głośność na żywo nie działa: ${JSON.stringify(wzmocnienia)}`);
sprawdz((await strona.textContent("#eksportuj")) === "Zapisz z nową głośnością", "przycisk zapisu nie reaguje na zmianę głośności");
await zrzut("3b-sciezki");
// Strzałki na ścieżce, powrót do 100% dwuklikiem, odciszenie przywraca poprzednią głośność
await strona.locator(".fala").first().focus();
await strona.keyboard.press("ArrowUp");
sprawdz((await v(() => sciezkiGlosu[0].glosnosc)) === 0.55, "strzałka w górę na ścieżce nie podgłaśnia o 5%");
await strona.locator(".fala").first().dblclick();
sprawdz((await v(() => sciezkiGlosu[0].glosnosc)) === 1, "dwuklik nie przywraca 100%");
sprawdz((await strona.textContent("#eksportuj")) === "Zapisz bez mojego głosu", "po powrocie gry do 100% przycisk nie mówi „Zapisz bez mojego głosu”");
await strona.locator(".sciezka .wycisz").nth(1).click();
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 1), "odciszenie mikrofonu nie przywraca głośności");
// Kliknięcie w ścieżkę (bez przeciągania) przewija klip
await strona.mouse.click(falaR.x + falaR.width / 2, falaR.y + falaR.height / 2);
await strona.waitForTimeout(100);
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 10) < 0.3, "kliknięcie w ścieżkę dźwięku nie przewija klipu");
sprawdz(await v(() => sciezkiGlosu[0].glosnosc === 1), "samo kliknięcie w ścieżkę zmieniło głośność");

// Dźwięk idzie za obrazem
await v(() => $("wideo").play());
await strona.waitForTimeout(700);
const rozjazd = await v(() => Math.abs(dzwiek.sciezki[0].a.currentTime - $("wideo").currentTime));
sprawdz(rozjazd < 0.15, `dźwięk rozjeżdża się z obrazem o ${rozjazd.toFixed(2)} s`);
sprawdz(await v(() => !dzwiek.sciezki[0].a.paused), "ścieżka dźwięku nie gra razem z obrazem");
await v(() => $("wideo").pause());
await strona.waitForTimeout(100);
sprawdz(await v(() => dzwiek.sciezki.every((s) => s.a.paused)), "dźwięk gra dalej po pauzie");

// Strzałki: ±5 s, przecinek / kropka: jedna klatka
await v(() => ($("wideo").currentTime = 2));
await strona.waitForTimeout(100);
await strona.keyboard.press("ArrowRight");
await strona.waitForTimeout(100);
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 7) < 0.1, "strzałka w prawo nie przewija o 5 s");
await strona.keyboard.press("ArrowLeft");
await strona.waitForTimeout(100);
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 2) < 0.1, "strzałka w lewo nie cofa o 5 s");
const t0 = await v(() => $("wideo").currentTime);
await strona.keyboard.press(".");
await strona.waitForTimeout(100);
const t1 = await v(() => $("wideo").currentTime);
sprawdz(t1 - t0 > 0.005 && t1 - t0 < 0.03, `kropka nie przesuwa o jedną klatkę (${(t1 - t0).toFixed(3)} s)`);
await strona.keyboard.press(",");
await strona.waitForTimeout(100);
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - t0) < 0.02, "przecinek nie cofa o jedną klatkę");

// „Zachowaj oryginał”: domyślnie wyłączone — przycięta wersja zastępuje oryginał
await v(() => przesun("koniec", 12));
sprawdz(!(await strona.isHidden("#zachowaj-wiersz")), "nie widać „Zachowaj oryginał” przy zmianie");
sprawdz(!(await strona.isChecked("#zachowaj")), "„Zachowaj oryginał” jest domyślnie zaznaczone");
sprawdz((await strona.locator("#zastap").count()) === 0, "został stary przełącznik „Zastąp oryginał”");
await strona.click("#eksportuj");
await strona.waitForTimeout(200);
let zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(zl?.zastap === true && zl?.format === "mp4", `domyślny zapis nie zastępuje oryginału: ${JSON.stringify(zl)}`);
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForFunction(() => dlugosc > 19, null, { timeout: 5000 }).catch(() => {});
// Ten sam klip otwarty drugi raz: długość jest znana od razu, ale wczytanie pliku zeruje zaznaczenie — czekamy na nie
await strona.waitForTimeout(500);
await v(() => przesun("koniec", 12));
await strona.click("#zachowaj-wiersz");
await strona.click("#eksportuj");
await strona.waitForTimeout(200);
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(zl?.zastap === false, "„Zachowaj oryginał” nie zostawia oryginału");
// Przeciąganie zmienionego klipu (np. na Discorda): ta sama wersja zapisana wyżej idzie bez nowego eksportu…
const ileZlecen = () => v(() => (window.__zlecenia || []).length);
let przedPrzeciaganiem = await ileZlecen();
await v(() => plikDoPrzeciagniecia());
sprawdz((await ileZlecen()) === przedPrzeciaganiem, "przeciąganie zapisuje drugi raz wersję, która jest już w bibliotece");
// …a nowa wersja powstaje tylko w folderze do przeciągania, nie w bibliotece (majk: „automatycznie zapisuje mi go na dysk”)
await v(() => (ostatniEksport = null));
const wynikPrzed = await v(() => $("wynik").hidden);
await v(() => plikDoPrzeciagniecia());
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz((await ileZlecen()) === przedPrzeciaganiem + 1 && zl?.do_przeciagniecia === true && zl?.zastap === false, `przeciąganie nie prosi o plik tymczasowy: ${JSON.stringify(zl)}`);
sprawdz((await v(() => $("wynik").hidden)) === wynikPrzed, "przeciąganie pokazuje kartę zapisanego klipu");
await v(() => plikDoPrzeciagniecia());
sprawdz((await ileZlecen()) === przedPrzeciaganiem + 1, "każde przeciągnięcie tej samej wersji robi nowy plik");

// Kadrowanie: C włącza ramkę, uchwyt zmienia rozmiar, proporcje 9:16 działają
await strona.keyboard.press("c");
await strona.waitForTimeout(100);
sprawdz(await strona.isVisible("#kadr"), "C nie włącza kadrowania");
const przed = await v(() => ({ ...kadr }));
const u = await strona.locator('.kadr-uchwyt[data-u="se"]').boundingBox();
if (u) {
  await strona.mouse.move(u.x + u.width / 2, u.y + u.height / 2);
  await strona.mouse.down();
  await strona.mouse.move(u.x - 60, u.y - 40, { steps: 5 });
  await strona.mouse.up();
}
const po = await v(() => ({ ...kadr }));
sprawdz(po.szer < przed.szer && po.wys < przed.wys, `przeciągnięcie uchwytu nie zmniejsza kadru: ${JSON.stringify([przed, po])}`);
const srodek = await strona.locator("#kadr").boundingBox();
if (srodek) {
  await strona.mouse.move(srodek.x + srodek.width / 2, srodek.y + srodek.height / 2);
  await strona.mouse.down();
  await strona.mouse.move(srodek.x + srodek.width / 2 + 30, srodek.y + srodek.height / 2 + 10, { steps: 5 });
  await strona.mouse.up();
}
const przesuniety = await v(() => ({ ...kadr }));
sprawdz(przesuniety.x > po.x && Math.abs(przesuniety.szer - po.szer) < 0.5, "przeciągnięcie środka nie przesuwa kadru");
await strona.click('[data-proporcje="0.5625"]');
const pion = await v(() => kadr.szer / kadr.wys);
sprawdz(Math.abs(pion - 0.5625) < 0.01, `proporcje 9:16 nie działają (${pion.toFixed(3)})`);
await zrzut("4-kadr");
await strona.click("#eksportuj");
await strona.waitForTimeout(200);
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(zl?.kadr && zl.kadr.szer > 0 && zl.kadr.x + zl.kadr.szer <= 192 && zl.kadr.y + zl.kadr.wys <= 108, `zlecenie bez poprawnego kadru: ${JSON.stringify(zl?.kadr)}`);
await strona.click("#kadr-usun");

// GIF: najwyżej 15 s, krótszy dowolnie, zapętlony, bez dźwięku, nie zastępuje klipu
await v(() => przesun("koniec", 20) || przesun("start", 0));
await strona.click('[data-format="gif"]');
let z = await v(() => zakres);
sprawdz(z.koniec - z.start <= 15.001 && z.koniec - z.start > 14.9, `GIF nie przycina zaznaczenia do 15 s: ${JSON.stringify(z)}`);
await v(() => przesun("koniec", 20));
z = await v(() => zakres);
sprawdz(z.koniec - z.start <= 15.001, "uchwytem da się zaznaczyć GIF dłuższy niż 15 s");
await v(() => przesun("koniec", zakres.start + 4));
z = await v(() => zakres);
sprawdz(Math.abs(z.koniec - z.start - 4) < 0.01, "nie da się zrobić krótszego GIF-a");
sprawdz(await strona.isHidden("#sciezki-dzwieku"), "GIF pokazuje ścieżki dźwięku (GIF nie ma dźwięku)");
sprawdz(await strona.isHidden("#zachowaj-wiersz"), "GIF pokazuje „Zachowaj oryginał”");
const rozmiaryGif = await strona.locator("#rozmiary .segment").allTextContents();
sprawdz(rozmiaryGif.join("|") === "Bez limitu|Do 10 MB (Discord)", `GIF: zły wybór rozmiaru ${rozmiaryGif}`);
await strona.click('#rozmiary .segment:has-text("Do 10 MB")');
await strona.click("#eksportuj");
await strona.waitForTimeout(300);
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(zl?.format === "gif" && zl.zastap === false && zl.rozmiar_mb === 10 && zl.glosnosci.length === 0, `zlecenie GIF: ${JSON.stringify(zl)}`);
sprawdz(await strona.isHidden("#wynik-otworz"), "„Otwórz” przy GIF-ie (podgląd go nie odtworzy)");
await zrzut("5-gif");

// Pełny ekran: przyciemnienie pod sterowaniem
await v(() => $("okno-odtwarzacza").classList.add("pelny"));
const fade = await v(() => getComputedStyle($("okno-odtwarzacza"), "::after").backgroundImage);
sprawdz(fade.includes("gradient"), "brak przyciemnienia pod sterowaniem w pełnym ekranie");
// W pełnym ekranie kadrowanie znika (wraca po wyjściu), tak samo ścieżki dźwięku
await v(() => {
  formatZapisu = "mp4";
  kadr = kadrNaSrodku();
  kadrowanie = true;
  ulozKadr();
  odswiezEdytor();
});
sprawdz((await strona.isHidden("#kadr")) && (await strona.isHidden("#kadruj")) && (await strona.isHidden("#sciezki-dzwieku")), "w pełnym ekranie widać kadrowanie albo ścieżki dźwięku");
await zrzut("6-pelny");
await v(() => $("okno-odtwarzacza").classList.remove("pelny"));
sprawdz((await strona.isVisible("#kadr")) && (await strona.isVisible("#sciezki-dzwieku")), "po wyjściu z pełnego ekranu kadr albo ścieżki nie wracają");
await strona.click("#kadr-usun");

// Licznik klatek
await strona.keyboard.press("Control+Shift+D");
await strona.waitForTimeout(1200);
sprawdz(!(await strona.isHidden("#licznik-klatek")), "Ctrl+Shift+D nie pokazuje licznika klatek");
await strona.keyboard.press("Control+Shift+D");

// Zamknięcie podglądu zatrzymuje dźwięk i zdejmuje wyciszenie
await v(() => zamknijOdtwarzacz());
sprawdz(await v(() => dzwiek.sciezki.length === 0 && !$("wideo").muted), "zamknięcie podglądu nie sprząta dźwięku na żywo");

// Kopiuj: plik klipu idzie do schowka (Ctrl+V w Discordzie)
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForTimeout(300);
await strona.click("#odtw-kopiuj");
await strona.waitForTimeout(100);
sprawdz((await v(() => window.__skopiowane)) === klip.sciezka, "„Kopiuj” nie kopiuje klipu do schowka");

// Klip z dźwiękiem gry osobno: Gra / Discord i reszta / Mikrofon, każdą wycisza się osobno
await v(() => zamknijOdtwarzacz());
await v(() => (window.__info = { sciezki_audio: 4, nazwy_sciezek: ["Wszystko razem", "Gra", "Discord i reszta", "Mikrofon"] }));
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForFunction(() => dzwiek.sciezki.length === 3, null, { timeout: 5000 }).catch(() => {});
const nazwy = await strona.locator("#sciezki-dzwieku .sciezka-opis span").allTextContents();
sprawdz(nazwy.join("|") === "Gra|Discord i reszta|Mikrofon", `złe ścieżki przy trzech źródłach: ${nazwy}`);
sprawdz((await v(() => dzwiek.sciezki.length)) === 3, "głośność na żywo: nie ma trzech ścieżek");
await strona.locator(".sciezka .wycisz").nth(1).click();
sprawdz((await strona.textContent("#eksportuj")) === "Zapisz bez Discorda i reszty", `wyciszony Discord: przycisk „${await strona.textContent("#eksportuj")}”`);
await strona.click("#eksportuj");
await strona.waitForTimeout(200);
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(JSON.stringify(zl?.glosnosci) === "[1,0,1]", `zlecenie bez Discorda: ${JSON.stringify(zl?.glosnosci)}`);
// Starszy plik bez nazw ścieżek dostaje domyślne
await v(() => zamknijOdtwarzacz());
await v(() => (window.__info = { sciezki_audio: 4, nazwy_sciezek: [] }));
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForTimeout(300);
const domyslne = await strona.locator("#sciezki-dzwieku .sciezka-opis span").allTextContents();
sprawdz(domyslne.join("|") === "Gra|Discord i reszta|Mikrofon", `domyślne nazwy ścieżek: ${domyslne}`);
await v(() => zamknijOdtwarzacz());

// Znaczniki filmu: kreski na osi, kliknięcie skacze, [ i ] do poprzedniego / następnego
await v(() => (window.__info = { znaczniki: [5, 12.5] }));
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForFunction(() => document.querySelectorAll("#os .os-znacznik").length === 2, null, { timeout: 3000 }).catch(() => {});
sprawdz((await strona.locator("#os .os-znacznik").count()) === 2, "nie ma dwóch znaczników na osi czasu");
await strona.locator("#os .os-znacznik").nth(1).click();
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 12.5) < 0.05, "kliknięcie znacznika nie skacze do niego");
await strona.locator("#wideo").focus().catch(() => {});
await v(() => document.activeElement.blur());
await strona.keyboard.press("[");
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 5) < 0.05, "[ nie skacze do poprzedniego znacznika");
await strona.keyboard.press("]");
sprawdz(Math.abs((await v(() => $("wideo").currentTime)) - 12.5) < 0.05, "] nie skacze do następnego znacznika");
await zrzut("7-znaczniki");
await v(() => zamknijOdtwarzacz());
await v(() => (window.__info = {}));
// Klip bez znaczników nie ma kresek (stare kreski znikają)
await strona.evaluate((k) => odtworz(k), klip);
await strona.waitForTimeout(300);
sprawdz((await strona.locator("#os .os-znacznik").count()) === 0, "kreski znaczników zostały po poprzednim filmie");
await v(() => zamknijOdtwarzacz());
sprawdz((await v(() => $("skrot-znacznika").dataset.wartosc)) === "Alt+F8", "w ustawieniach nie ma domyślnego skrótu znacznika Alt+F8");


// Pasek górny mieści się przy każdej szerokości, także z widocznym przyciskiem „Aktualizuj do X”
for (const aktualizacja of [true, false]) {
  await strona.evaluate((a) => pokazAktualizacje(a ? { wersja: "10.11.12" } : null), aktualizacja);
  for (const szerokosc of [1680, 1600, 1366, 1200, 1100, 1024, 980, 900, 860, 780, 640]) {
    await strona.setViewportSize({ width: szerokosc, height: 800 });
    await strona.waitForTimeout(30);
    const wynik = await strona.evaluate(() => {
      const g = document.querySelector(".gora");
      const poza = [...g.querySelectorAll("button, .status, .tryb, .marka")].filter((el) => {
        if (el.hidden || el.offsetParent === null) return false;
        const r = el.getBoundingClientRect();
        return r.right > window.innerWidth + 1 || r.left < -1;
      });
      return { przelew: g.scrollWidth - g.clientWidth, poza: poza.map((el) => el.id || el.className) };
    });
    const opis = aktualizacja ? "z przyciskiem aktualizacji" : "bez przycisku aktualizacji";
    sprawdz(wynik.przelew <= 1, `pasek górny nie mieści się (${opis}, ${szerokosc} px): wystaje ${wynik.przelew} px`);
    sprawdz(wynik.poza.length === 0, `elementy paska poza oknem (${opis}, ${szerokosc} px): ${wynik.poza.join(", ")}`);
  }
}
await strona.evaluate(() => pokazAktualizacje(null));
await strona.setViewportSize({ width: 1100, height: 800 });

await przegladarka.close();
if (bledy.length) {
  console.error("Test okna nie przeszedł:\n- " + bledy.join("\n- "));
  process.exit(1);
}
console.log("Test okna: OK");
