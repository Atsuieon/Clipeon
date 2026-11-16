// Test prawdziwego Clipeona na Windows (GitHub Actions, windows-latest): prawdziwy rdzeń w Rust,
// prawdziwe okno WebView2, prawdziwy config w %APPDATA%\Clipeon. Playwright podłącza się do okna
// przez port debugowania WebView2 (zmienna CLIPEON_PORT_TESTU=9222, którą Clipeon dokleja do argumentów okna).
//
// Sprawdza zmianę języka tak, jak robi to użytkownik: Ustawienia → Język → Zapisz zmiany,
// w obie strony, potem zamknięcie programu i ponowne uruchomienie.
//
// Uruchomienie: node testy/prawdziwe-okno.mjs target\release\clipeon.exe
import { chromium } from "playwright";
import { spawn, execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";

const EXE = resolve(process.argv[2] || "target/release/clipeon.exe");
const PORT = 9222;
const APPDATA = process.env.APPDATA;
const CONFIG = join(APPDATA, "Clipeon", "config.toml");
const LOG = join(APPDATA, "Clipeon", "logs", "clipeon.log");
const ZRZUTY = resolve("zrzuty-prawdziwe");
mkdirSync(ZRZUTY, { recursive: true });

const bledy = [];
const sprawdz = (warunek, opis) => {
  console.log(`${warunek ? "OK  " : "BŁĄD"} ${opis}`);
  if (!warunek) bledy.push(opis);
};
const czekaj = (ms) => new Promise((r) => setTimeout(r, ms));

function uruchom() {
  spawn(EXE, [], { detached: true, stdio: "ignore" }).unref();
}

function zamknij() {
  try {
    execFileSync("taskkill", ["/IM", basename(EXE), "/F"], { stdio: "ignore" });
  } catch {}
}

// Start programu i otwarcie okna (drugie uruchomienie = „pokaż okno” w działającym Clipeonie)
async function otworzOkno(etap) {
  uruchom();
  await czekaj(6000);
  uruchom();
  const koniec = Date.now() + 90_000;
  let ostatni = "";
  while (Date.now() < koniec) {
    try {
      const przegladarka = await chromium.connectOverCDP(`http://127.0.0.1:${PORT}`);
      for (const k of przegladarka.contexts()) {
        for (const s of k.pages()) {
          if (s.url().includes("index.html")) {
            s.on("console", (m) => m.type() === "error" && console.log(`  [konsola ${etap}] ${m.text()}`));
            s.on("pageerror", (e) => bledy.push(`${etap}: błąd skryptu: ${e.message}`));
            // Okno gotowe: app.js wczytał ustawienia z rdzenia
            await s.waitForFunction(() => document.querySelector("#jezyk")?.value, null, { timeout: 30_000 });
            await czekaj(1000);
            return { przegladarka, s };
          }
          ostatni = s.url();
        }
      }
    } catch (e) {
      ostatni = e.message.split("\n")[0];
    }
    await czekaj(1000);
  }
  console.log("--- clipeon.log ---");
  console.log(existsSync(LOG) ? readFileSync(LOG, "utf8").split("\n").slice(-40).join("\n") : "(brak)");
  try {
    console.log("--- procesy ---");
    console.log(
      execFileSync("powershell", ["-NoProfile", "-Command", "Get-CimInstance Win32_Process | Where-Object { $_.Name -match 'clipeon|webview' } | ForEach-Object { $_.Name + ' ' + $_.CommandLine.Substring(0, [Math]::Min(300, $_.CommandLine.Length)) }"], { encoding: "utf8" }),
    );
  } catch {}
  throw new Error(`${etap}: nie udało się podłączyć do okna Clipeona (ostatnio: ${ostatni})`);
}

const tekst = (s, sel) => s.evaluate((q) => document.querySelector(q)?.textContent.trim() ?? null, sel);
const POLSKIE = /[ąćęłńóśźżĄĆĘŁŃÓŚŹŻ]/;

async function stan(s, etap) {
  const st = await s.evaluate(() => ({
    lang: document.documentElement.lang,
    jezyk: window.Jezyk?.kod,
    slowa: window.Jezyk?.slowa,
    slownik: !!window.SLOWNIK_EN,
    zakladki: [...document.querySelectorAll("[data-zakladka]")].map((z) => z.textContent.trim()).join(" | "),
  }));
  console.log(`  [${etap}] ${JSON.stringify(st)}`);
  await s.screenshot({ path: join(ZRZUTY, `${etap}.png`) }).catch(() => {});
  return st;
}

async function zapiszJezyk(s, kod, etap) {
  await s.click('[data-zakladka="ustawienia"]');
  await czekaj(300);
  await s.selectOption("#jezyk", kod);
  await s.click("#zapisz-ustawienia");
  await s
    .waitForFunction(() => /zapisano|blad/.test(document.querySelector("#pasek-zapisu")?.className || ""), null, { timeout: 30_000 })
    .catch(() => {});
  const komunikat = await tekst(s, "#komunikat");
  console.log(`  [${etap}] komunikat po zapisie: ${komunikat}`);
  sprawdz(!(await s.evaluate(() => document.querySelector("#pasek-zapisu")?.classList.contains("blad"))), `${etap}: zapis bez błędu`);
  await czekaj(500);
  return komunikat;
}

const czytajConfig = () => (existsSync(CONFIG) ? readFileSync(CONFIG, "utf8") : "");
const jezykWConfigu = () => czytajConfig().match(/^jezyk\s*=\s*"(\w+)"/m)?.[1];

zamknij();
sprawdz(existsSync(EXE), `jest program ${EXE}`);

// 1. Pierwsze uruchomienie (czysty config) — język z Windows (na GitHubie Windows jest po angielsku)
let { przegladarka, s } = await otworzOkno("start");
let st = await stan(s, "1-start");
console.log(`  config: jezyk = ${jezykWConfigu()}`);
sprawdz(st.slownik && st.slowa > 200, "okno ma słownik angielski");
sprawdz(jezykWConfigu() === st.lang, `język okna (${st.lang}) zgadza się z configiem (${jezykWConfigu()})`);

// 2. Polski z ustawień
await zapiszJezyk(s, "pl", "2-pl");
st = await stan(s, "2-pl");
sprawdz(st.lang === "pl" && st.zakladki.includes("Ustawienia"), "po zapisie „Polski” okno jest po polsku");
sprawdz(jezykWConfigu() === "pl", `config ma jezyk = "pl" (jest: ${jezykWConfigu()})`);

// 3. Angielski z ustawień — to zgłaszał użytkownik
const komunikat = await zapiszJezyk(s, "en", "3-en");
st = await stan(s, "3-en");
sprawdz(st.lang === "en", "po zapisie „English” okno ma lang=en");
sprawdz(st.zakladki.includes("Settings") && st.zakladki.includes("Clips"), `zakładki po angielsku (${st.zakladki})`);
sprawdz(/^Saved\./.test(komunikat || ""), `komunikat po zapisie po angielsku (${komunikat})`);
sprawdz(jezykWConfigu() === "en", `config ma jezyk = "en" (jest: ${jezykWConfigu()})`);
for (const z of ["ustawienia", "klipy", "filmy"]) {
  await s.click(`[data-zakladka="${z}"]`);
  await czekaj(400);
  const widoczne = await s.evaluate(() => document.body.innerText);
  const polskie = widoczne.split("\n").filter((l) => POLSKIE.test(l));
  sprawdz(polskie.length === 0, `zakładka ${z} po angielsku${polskie.length ? `: ${polskie.slice(0, 8).join(" | ")}` : ""}`);
  await s.screenshot({ path: join(ZRZUTY, `3-en-${z}.png`) }).catch(() => {});
}

// 4. Zamknięcie i ponowne uruchomienie: angielski z configu (pamięć okna celowo ustawiona na polski)
await s.evaluate(() => localStorage.setItem("clipeon.jezyk", "pl"));
await przegladarka.close().catch(() => {});
zamknij();
await czekaj(2000);
({ przegladarka, s } = await otworzOkno("restart"));
st = await stan(s, "4-restart");
sprawdz(st.lang === "en" && st.zakladki.includes("Settings"), "po ponownym uruchomieniu okno jest po angielsku");

// 5. I z powrotem na polski, bez restartu
await zapiszJezyk(s, "pl", "5-pl");
st = await stan(s, "5-pl");
sprawdz(st.lang === "pl" && st.zakladki.includes("Ustawienia"), "powrót na polski działa od razu");
await przegladarka.close().catch(() => {});
zamknij();

// 6. Dziennik: żadnych błędów okna
const dziennik = existsSync(LOG) ? readFileSync(LOG, "utf8") : "";
const bledyOkna = dziennik.split("\n").filter((l) => l.includes("Okno:"));
sprawdz(bledyOkna.length === 0, `brak błędów okna w clipeon.log${bledyOkna.length ? `: ${bledyOkna.join(" | ")}` : ""}`);
console.log("--- clipeon.log (koniec) ---");
console.log(dziennik.split("\n").slice(-40).join("\n"));
console.log("--- config.toml ---");
console.log(czytajConfig());

if (bledy.length) {
  console.error(`\n${bledy.length} błędów:\n- ${bledy.join("\n- ")}`);
  process.exit(1);
}
console.log("\nJęzyk w prawdziwym Clipeonie działa.");
