// Język okna. Polskie teksty zostają w kodzie (HTML i app.js), a po angielsku podmienia je ten plik
// na podstawie słownika z en.js (ten sam plik czyta rdzeń dla dymków i menu przy zegarze).
//
// Jak to działa:
// - Słownik jest zwykłym skryptem okna (en.js → window.SLOWNIK_EN), ładowanym przed tym plikiem.
//   Nic nie jest pobierane w trakcie działania: żadnego fetch ani pytania rdzenia o słownik.
// - Obserwator zmian tłumaczy każdy tekst i podpis (title, aria-label…) zaraz po tym, jak app.js go
//   wpisze, więc app.js nie musi owijać napisów.
// - Dla każdego przetłumaczonego tekstu pamiętamy polski oryginał, więc powrót na polski działa od razu,
//   bez przeładowania okna.
// - Język ustawia app.js: przy starcie z ustawień, po zapisie z formularza. Do tego czasu okno bierze
//   język zapamiętany w przeglądarce okna (żeby nie mignęło po polsku).
(function () {
  // Błędy okna trafiają do dziennika Clipeona (clipeon.log) — inaczej nikt by ich nie zobaczył
  let zgloszone = 0;
  function zglos(opis) {
    console.error(opis);
    if (zgloszone++ >= 20) return;
    try {
      window.__TAURI__?.core.invoke("blad_okna", { opis: String(opis).slice(0, 500) }).catch(() => {});
    } catch {}
  }
  window.addEventListener("error", (e) => zglos(`${e.message} (${(e.filename || "").split("/").pop()}:${e.lineno})`));
  window.addEventListener("unhandledrejection", (e) => zglos(`nieobsłużony błąd: ${e.reason?.message || e.reason}`));

  const ATRYBUTY = ["title", "aria-label", "placeholder", "data-pusty", "alt"];
  const PAMIEC = "clipeon.jezyk";
  const dane = window.SLOWNIK_EN || { teksty: {}, wzory: [] };
  const slownik = dane.teksty || {};
  const wzory = [];
  for (const [w, z] of dane.wzory || []) {
    try {
      wzory.push([new RegExp(w, "u"), z]);
    } catch (e) {
      zglos(`zły wzór tłumaczenia ${w}: ${e.message}`);
    }
  }

  const J = { kod: "pl", en: false, locale: "pl-PL" };
  // Tekst → { pl: oryginał, en: to, co wpisaliśmy }; element → Map(atrybut → { pl, en })
  const teksty = new WeakMap();
  const atrybuty = new WeakMap();
  let obserwator = null;

  const czytaj = () => {
    try {
      return localStorage.getItem(PAMIEC);
    } catch {
      return null;
    }
  };
  const pamietaj = (kod) => {
    try {
      localStorage.setItem(PAMIEC, kod);
    } catch {}
  };

  // Polski tekst po angielsku (bez zmian, gdy nie ma go w słowniku). Odstępy na brzegach zostają.
  function przetlumacz(tekst, glebokosc = 0) {
    if (typeof tekst !== "string") return tekst;
    const n = tekst.replace(/\s+/g, " ").trim();
    if (!n) return tekst;
    let wynik = slownik[n];
    if (wynik === undefined && glebokosc < 3) {
      for (const [wzor, zamiana] of wzory) {
        const m = n.match(wzor);
        if (m) {
          wynik = zamiana.replace(/\$(\d)/g, (_, i) => przetlumacz(m[Number(i)] ?? "", glebokosc + 1));
          break;
        }
      }
    }
    if (wynik === undefined) return tekst;
    return tekst.match(/^\s*/)[0] + wynik + tekst.match(/\s*$/)[0];
  }

  // Tekst w bieżącym języku (dla app.js, np. w confirm)
  const t = (tekst) => (J.en ? przetlumacz(tekst) : tekst);

  // Docelowa wartość tekstu: polski oryginał (to, co wpisał app.js) albo jego tłumaczenie
  function cel(zapis, teraz) {
    // Wartość różna od naszego tłumaczenia = app.js wpisał nowy (polski) tekst
    const pl = zapis && teraz === zapis.en ? zapis.pl : teraz;
    return { pl, en: J.en ? przetlumacz(pl) : pl };
  }

  function wezel(n, zDziecmi = true) {
    if (n.nodeType === Node.TEXT_NODE) {
      const r = n.parentNode && n.parentNode.nodeName;
      if (r === "SCRIPT" || r === "STYLE" || r === "TEXTAREA") return;
      const c = cel(teksty.get(n), n.nodeValue);
      if (c.en !== c.pl) teksty.set(n, c);
      else teksty.delete(n);
      if (n.nodeValue !== c.en) n.nodeValue = c.en;
    } else if (n.nodeType === Node.ELEMENT_NODE) {
      let mapa = atrybuty.get(n);
      for (const a of ATRYBUTY) {
        const v = n.getAttribute(a);
        if (v === null) continue;
        const c = cel(mapa && mapa.get(a), v);
        if (c.en !== c.pl) {
          if (!mapa) atrybuty.set(n, (mapa = new Map()));
          mapa.set(a, c);
        } else if (mapa) mapa.delete(a);
        if (v !== c.en) n.setAttribute(a, c.en);
      }
      if (zDziecmi) for (const d of n.childNodes) wezel(d);
    }
  }

  function obserwuj() {
    if (!obserwator) {
      obserwator = new MutationObserver((zmiany) => {
        for (const z of zmiany) {
          if (z.type === "characterData") wezel(z.target);
          else if (z.type === "attributes") wezel(z.target, false);
          else for (const d of z.addedNodes) wezel(d);
        }
      });
    }
    obserwator.observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATRYBUTY });
  }

  // Ustawia język okna: "pl" albo "en". Działa od razu w obie strony, bez przeładowania.
  function ustaw(kod) {
    kod = kod === "en" ? "en" : "pl";
    pamietaj(kod);
    if (kod === J.kod) return;
    try {
      J.kod = kod;
      J.en = kod === "en";
      J.locale = J.en ? "en-GB" : "pl-PL";
      document.documentElement.lang = kod;
      if (J.en) obserwuj();
      else obserwator?.disconnect();
      wezel(document.documentElement);
      document.dispatchEvent(new CustomEvent("zmiana-jezyka", { detail: kod }));
    } catch (e) {
      zglos(`nie udało się zmienić języka na ${kod}: ${e.message}`);
    }
  }

  // Potwierdzenia (usuwanie klipów) też po angielsku
  const potwierdz = window.confirm.bind(window);
  window.confirm = (tekst) => potwierdz(t(tekst));

  J.t = t;
  J.ustaw = ustaw;
  J.slowa = Object.keys(slownik).length;
  window.Jezyk = J;

  if (!window.SLOWNIK_EN) zglos("brak słownika en.js — okno zostaje po polsku");
  // Język z poprzedniego uruchomienia od razu (okno nie mignie po polsku); app.js zaraz ustawi ten z ustawień
  const zapamietany = czytaj();
  if (zapamietany === "en") ustaw("en");
})();
