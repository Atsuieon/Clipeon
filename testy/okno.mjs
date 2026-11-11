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
const sprawdz = (warunek, opis) => {
  if (!warunek) bledy.push(opis);
};

// 1. Każdy $("id") z app.js istnieje w index.html (tak zepsuło się okno w 1.3.2)
const js = readFileSync(ui + "app.js", "utf8");
const html = readFileSync(ui + "index.html", "utf8");
for (const id of new Set([...js.matchAll(/\$\("([\w-]+)"\)/g)].map((m) => m[1]))) {
  sprawdz(html.includes(`id="${id}"`), `app.js odwołuje się do #${id}, którego nie ma w index.html`);
}

// 2. Okno w przeglądarce z udawanym rdzeniem
function udawanyTauri() {
  const cfg = {
    bufor_sekundy: 30, fps: 60, bitrate_mbps: 20, enkoder: "auto", dzwiek_systemowy: true, mikrofon: false,
    folder_klipow: "C:\\Klipy", skrot: "Alt+F10", ikona_przy_zegarze: true, ikona_ustawiona: true,
    dzwiek_zapisu: true, powiadomienie_zapisu: true, sprzatanie_dni: 0, sprzatanie_gb: 0, glos_w_edytorze: true,
    sprawdzaj_aktualizacje: true, monitor: "", monitor_id: "", tryb: "gra", monitor_pulpitu: "",
    monitor_pulpitu_id: "", fps_pulpitu: 30, bitrate_pulpitu_mbps: 12, metoda_przechwytywania: "auto",
    mikrofon_urzadzenie: "", gra: "", gra_nazwa: "",
  };
  const stan = {
    nagrywa: true, pauza: false, bledy: [], wznawianie: null, tryb: "gra", ekran: "Ekran 1", monitor: "",
    droga: "dda", gra: "Gra testowa", wybrana_gra: "", gra_proces: "", czeka_na_gre: false,
  };
  const odpowiedzi = {
    stan: () => stan,
    ustawienia: () => ({ cfg: structuredClone(cfg), autostart: false, enkodery: [["auto", "Automatycznie"]], wersja: "test", repozytorium: "" }),
    mikrofony: () => [
      { id: "m1", nazwa: "Mikrofon (USB)", domyslny: true },
      { id: "m2", nazwa: "Mikrofon (Zestaw)", domyslny: false },
    ],
    gry: () => [{ nazwa: "Gra testowa", proces: "gra.exe" }, { nazwa: "Druga gra", proces: "druga.exe" }],
    monitory: () => [{ id: "\\\\.\\DISPLAY1", numer: 1, glowny: true, nazwa: "Monitor", opis: "Ekran 1", szerokosc: 1920, wysokosc: 1080, hz: 144, hdr: false }],
    klipy: () => [],
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
const strona = await przegladarka.newPage({ viewport: { width: 1100, height: 800 } });
strona.on("pageerror", (e) => bledy.push(`błąd skryptu: ${e.message}`));
strona.on("console", (m) => m.type() === "error" && bledy.push(`konsola: ${m.text()}`));
await strona.addInitScript(udawanyTauri);
await strona.goto(pathToFileURL(ui + "index.html").href);
await strona.waitForTimeout(500);

sprawdz((await strona.textContent("#status-tekst")) !== "Łączenie…", "status został na „Łączenie…”");
for (const id of ["okno-min", "okno-max", "okno-zamknij"]) {
  sprawdz((await strona.locator(`#${id} svg`).count()) === 1, `przycisk #${id} nie ma ikony`);
}

// Tryb gry: zamiast wyboru ekranu lista działających gier
await strona.click("#ekran");
const gry = await strona.locator("#lista-ekranow [role=option]").allTextContents();
sprawdz(gry.some((t) => t.includes("Druga gra")), `lista na pasku nie pokazuje gier: ${JSON.stringify(gry)}`);
const druga = strona.locator("#lista-ekranow [role=option]", { hasText: "Druga gra" });
if (await druga.count()) await druga.click();
await strona.waitForTimeout(100);
sprawdz((await strona.evaluate(() => window.__wybranaGra?.proces)) === "druga.exe", "wybór gry z paska nie trafił do rdzenia");

// Ustawienia: wybór mikrofonu działa także przy wyłączonym „Mój mikrofon”
await strona.click('[data-zakladka="ustawienia"]');
await strona.waitForTimeout(300);
const mik = strona.locator("#mikrofon-urzadzenie");
sprawdz(!(await mik.isDisabled()), "lista mikrofonów jest wyłączona");
sprawdz((await mik.locator("option").count()) === 3, "lista mikrofonów nie ma wszystkich urządzeń");
if (!(await mik.isDisabled())) await mik.selectOption("m2");
sprawdz(await strona.isChecked("#mikrofon"), "wybranie mikrofonu nie włączyło „Mój mikrofon”");
sprawdz((await strona.locator("#gra").count()) === 0, "pole „Gra” zostało w ustawieniach");

await przegladarka.close();
if (bledy.length) {
  console.error("Test okna nie przeszedł:\n- " + bledy.join("\n- "));
  process.exit(1);
}
console.log("Test okna: OK");
