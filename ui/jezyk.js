// Język okna: polskie teksty zostają w kodzie, a przy języku angielskim tłumaczy je ten plik.
// Słownik i wzory są w en.json (ten sam plik czyta rdzeń dla dymków i menu przy zegarze).
// Tłumaczenie działa na gotowym oknie: obserwator zmian podmienia teksty i podpisy (title, aria-label…)
// zaraz po tym, jak skrypt okna je wpisze — dzięki temu app.js nie musi owijać każdego napisu.
(function () {
  const ATRYBUTY = ["title", "aria-label", "placeholder", "data-pusty", "alt"];
  const PAMIEC = "clipeon.jezyk";
  const J = { en: false, locale: "pl-PL", slownik: null, wzory: [] };
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

  // Tekst po angielsku (albo bez zmian). Odstępy na początku i końcu zostają.
  function t(tekst, glebokosc = 0) {
    if (!J.en || !J.slownik || typeof tekst !== "string") return tekst;
    const n = tekst.replace(/\s+/g, " ").trim();
    if (!n) return tekst;
    let wynik = J.slownik[n];
    if (wynik === undefined && glebokosc < 3) {
      for (const [wzor, zamiana] of J.wzory) {
        const m = n.match(wzor);
        if (m) {
          wynik = zamiana.replace(/\$(\d)/g, (_, i) => t(m[Number(i)] ?? "", glebokosc + 1));
          break;
        }
      }
    }
    if (wynik === undefined) return tekst;
    return tekst.match(/^\s*/)[0] + wynik + tekst.match(/\s*$/)[0];
  }

  function wezel(n, zDziecmi = true) {
    if (n.nodeType === Node.TEXT_NODE) {
      const r = n.parentNode && n.parentNode.nodeName;
      if (r === "SCRIPT" || r === "STYLE" || r === "TEXTAREA") return;
      const nowy = t(n.nodeValue);
      if (nowy !== n.nodeValue) n.nodeValue = nowy;
    } else if (n.nodeType === Node.ELEMENT_NODE) {
      for (const a of ATRYBUTY) {
        const v = n.getAttribute(a);
        if (v) {
          const nowy = t(v);
          if (nowy !== v) n.setAttribute(a, nowy);
        }
      }
      if (zDziecmi) for (const d of n.childNodes) wezel(d);
    }
  }

  function wlacz() {
    document.documentElement.lang = "en";
    wezel(document.documentElement);
    if (obserwator) return;
    obserwator = new MutationObserver((zmiany) => {
      for (const z of zmiany) {
        if (z.type === "characterData") wezel(z.target);
        else if (z.type === "attributes") wezel(z.target, false);
        else for (const d of z.addedNodes) wezel(d);
      }
    });
    obserwator.observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATRYBUTY });
  }

  async function wczytajSlownik() {
    if (J.slownik) return;
    // Przez rdzeń, nie fetch("en.json"): polityka bezpieczeństwa okna blokuje fetch plików okna
    const d = JSON.parse(await window.__TAURI__.core.invoke("tlumaczenie"));
    J.wzory = d.wzory.map(([w, z]) => [new RegExp(w, "u"), z]);
    J.slownik = d.teksty;
  }

  // Ustawia język ("pl" albo "en"). Powrót z angielskiego na polski przeładowuje okno
  // (oryginalnych polskich tekstów już w nim nie ma).
  async function ustaw(kod) {
    const en = kod === "en";
    pamietaj(en ? "en" : "pl");
    if (en === J.en) return;
    if (!en) {
      location.reload();
      return;
    }
    try {
      await wczytajSlownik();
    } catch (e) {
      console.error("Nie udało się wczytać tłumaczenia:", e);
      return;
    }
    J.en = true;
    J.locale = "en-GB";
    wlacz();
  }

  // Potwierdzenia (usuwanie klipów) też po angielsku
  const potwierdz = window.confirm.bind(window);
  window.confirm = (tekst) => potwierdz(t(tekst));

  // Pytamy rdzeń o język z ustawień. Zanim odpowie, okno ostatnio było
  // po angielsku — chowamy je na chwilę, żeby nie mignęło po polsku.
  async function sprawdz() {
    let kod = czytaj() || "pl";
    try {
      kod = await window.__TAURI__.core.invoke("jezyk");
    } catch {}
    await ustaw(kod);
  }

  J.t = t;
  J.ustaw = ustaw;
  J.sprawdz = sprawdz;
  window.Jezyk = J;

  if (czytaj() === "en") {
    document.documentElement.style.visibility = "hidden";
    const pokaz = () => (document.documentElement.style.visibility = "");
    setTimeout(pokaz, 1500);
    ustaw("en").finally(() => sprawdz().finally(pokaz));
  } else {
    sprawdz();
  }
})();
