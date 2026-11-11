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
    dzwiek_zapisu: true, powiadomienie_zapisu: true, sprzatanie_dni: 0, sprzatanie_gb: 0, glos_w_edytorze: true,
    sprawdzaj_aktualizacje: true, monitor: "", monitor_id: "", tryb: "gra", monitor_pulpitu: "",
    monitor_pulpitu_id: "", fps_pulpitu: 30, bitrate_pulpitu_mbps: 12, metoda_przechwytywania: "auto",
    mikrofon_urzadzenie: "", gra: "", gra_nazwa: "", skrot_filmu: "Alt+F9", animowane_tlo: true,
    format_filmu: "bez_zmian",
  };
  const stan = {
    nagrywa: true, pauza: false, bledy: [], wznawianie: null, tryb: "gra", ekran: "Ekran 1", monitor: "",
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
    info_klipu: () => ({ dlugosc: 20, sciezki_audio: 3, fps: 60, szerokosc: 192, wysokosc: 108, webp: true }),
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
      getCurrentWindow: () => ({ minimize() {}, toggleMaximize() {}, close() {}, isMaximized: async () => false, onResized() {} }),
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

// ---------- Tło ze znaków i poświata wokół myszki ----------
// Jasne znaki (przezroczystość ≥ 0,4) rysuje tylko poświata — liczymy je wokół podanego punktu
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
// Nad pustym miejscem poświata idzie za myszką…
for (let i = 0; i < 12; i++) {
  await strona.mouse.move(500 + i * 6, 640 + (i % 3) * 4);
  await strona.waitForTimeout(40);
}
await strona.waitForTimeout(400); // poświata rozjaśnia się powoli
sprawdz((await jasneZnaki(560, 640)) > 20, "poświata nie idzie za myszką nad pustym tłem");
await zrzut("0a-tlo-poswiata");
// …a nad przyciskiem gaśnie
const przyciskFolder = await strona.locator("#otworz-folder").boundingBox();
await strona.mouse.move(przyciskFolder.x + 10, przyciskFolder.y + 10);
await strona.waitForTimeout(4000);
sprawdz((await jasneZnaki(560, 640)) === 0 && (await jasneZnaki(przyciskFolder.x + 10, przyciskFolder.y + 10)) === 0, "poświata nie gaśnie, gdy mysz jest nad przyciskiem");
// Napis „Clipeon” ma zorzę (kolorowe tło przycięte do liter)
const marka = await strona.evaluate(() => {
  const s = getComputedStyle(document.querySelector(".marka"));
  return [s.backgroundClip || s.webkitBackgroundClip, s.animationName, s.backgroundImage.includes("radial-gradient")];
});
sprawdz(marka[0].startsWith("text") && marka[1] === "zorza" && marka[2], `napis „Clipeon” nie ma zorzy: ${JSON.stringify(marka)}`);

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
const dwojka = strona.locator('[data-pole="glos-w-edytorze"]');
sprawdz((await dwojka.locator(".wybrany").textContent()) === "Z moim głosem", "„Edytor zaczyna od” nie pokazuje zapisanego wyboru");
await dwojka.locator('[data-wartosc="nie"]').click();
sprawdz(!(await strona.isChecked("#glos-w-edytorze")), "przycisk „Bez głosu” nie zmienia ustawienia");

// „Zaawansowane” to karta jak inne grupy i da się ją rozwinąć
await strona.click(".zaawansowane > summary");
sprawdz(await strona.isVisible("#enkoder"), "„Zaawansowane” się nie rozwija");
await zrzut("3-zaawansowane");
sprawdz((await strona.locator(".karta .zaawansowane").count()) === 1, "„Zaawansowane” nie są w karcie");

// Zapis ustawień wysyła wszystkie pola z formularza
await strona.click("#zapisz-ustawienia");
await strona.waitForTimeout(100);
const zapisane = await strona.evaluate(() => window.__zapisane);
sprawdz(zapisane, "„Zapisz ustawienia” nic nie wysłał");
if (zapisane) {
  const c = zapisane.cfg;
  const oczekiwane = { mikrofon: true, mikrofon_urzadzenie: "m2", glos_w_edytorze: false, bufor_sekundy: 30, fps: 60, skrot: "Alt+F10", skrot_filmu: "Alt+F9", animowane_tlo: true, sprzatanie_dni: 0 };
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
sprawdz(!/żółt/i.test(await strona.textContent(".zaawansowane")), "w ustawieniach został napis o żółtej ramce");
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
sprawdz((await strona.locator("#format-filmu option").count()) === 3, "lista „Rozmiar filmów” nie ma formatów z rdzenia");
sprawdz((await strona.textContent("#format-filmu-uwaga")).includes("9.2 GB"), `podpowiedź rozmiaru filmu: „${await strona.textContent("#format-filmu-uwaga")}”`);
await strona.selectOption("#format-filmu", "hevc");
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

// Ustawienie „Edytor zaczyna od: Bez głosu” (zapisane wyżej w teście ustawień) = mikrofon od razu wyciszony
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 0 && sciezkiGlosu[1].el.classList.contains("wyciszona")), "„Bez głosu” w ustawieniach nie wycisza ścieżki mikrofonu na starcie");
sprawdz((await strona.textContent("#eksportuj")) === "Zapisz bez mojego głosu", "wyciszony mikrofon nie daje „Zapisz bez mojego głosu”");
await strona.locator(".sciezka .wycisz").nth(1).click();
sprawdz(await v(() => sciezkiGlosu[1].glosnosc === 1), "przycisk przy ścieżce nie włącza mikrofonu");

// Przeciągnięcie linii głośności w dół = ciszej, i słychać to od razu
// (odciszenie mikrofonu chowa wiersz zapisu, więc ścieżki mierzymy od nowa, gdy układ się uspokoi)
await strona.waitForTimeout(200);
falaR = await strona.locator(".fala").first().boundingBox();
await strona.mouse.move(falaR.x + falaR.width / 2, falaR.y + falaR.height / 2);
await strona.mouse.down();
await strona.mouse.move(falaR.x + falaR.width / 2, falaR.y + falaR.height * 0.75, { steps: 4 });
await strona.mouse.up();
sprawdz((await v(() => sciezkiGlosu[0].glosnosc)) === 0.5, `przeciągnięcie linii głośności: ${await v(() => sciezkiGlosu[0].glosnosc)} zamiast 0.5`);
sprawdz((await strona.locator(".fala-procent").first().textContent()) === "50%", "linia głośności nie pokazuje procentu");
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
await v(() => przesun("koniec", 12));
await strona.click("#zachowaj-wiersz");
await strona.click("#eksportuj");
await strona.waitForTimeout(200);
zl = await v(() => window.__zlecenia?.at(-1));
sprawdz(zl?.zastap === false, "„Zachowaj oryginał” nie zostawia oryginału");

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

await przegladarka.close();
if (bledy.length) {
  console.error("Test okna nie przeszedł:\n- " + bledy.join("\n- "));
  process.exit(1);
}
console.log("Test okna: OK");
