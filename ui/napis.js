// Napis „Clipeon” w rogu okna złożony z cząsteczek (od 1.4.10).
// Na start (i za każdym razem, gdy okno wraca z ukrycia) kropki zlatują się z rozsypki w litery.
// Kursor rozpycha je na boki, a kliknięcie w napis rozpryskuje je jak po uderzeniu; potem wracają sprężyną.
// Kropki, które są daleko od swojego miejsca, jaśnieją do bieli i dostają poświatę.
// Pomysł z efektu ParticleText z React Bits; kod napisany od zera (React Bits ma licencję MIT + Commons Clause).
// Wydajność: kilkaset kropek, rysowanie tylko gdy coś się rusza. Gdy wszystko stoi, zostaje delikatne
// drganie co 1/15 s, ale tylko przy aktywnym oknie: w tle, schowane albo przy otwartym edytorze nic nie liczy.
// Nie zależy od systemowego „bez animacji” (prefers-reduced-motion): wyłącznik jest w Ustawieniach.
window.Napis = (() => {
  const marka = document.querySelector(".marka");
  const plotno = document.createElement("canvas");
  plotno.className = "marka-plotno";
  plotno.setAttribute("aria-hidden", "true");
  document.body.appendChild(plotno);
  const ctx = plotno.getContext("2d");

  const ZAPAS_X = 90; // o ile px płótno wystaje poza napis (miejsce na rozsypkę i rozprysk)
  const ZAPAS_Y = 34;
  const GESTOSC = 1.5; // co ile px liter stoi kropka
  const KROPKA = 1.0; // średnica kropki w px
  const PROG = 140; // od jakiego krycia piksela litery stoi kropka (wyżej = cieńsze litery, otwarte „e” i „o”)
  const ROZSYPKA = 70; // px — jak daleko kropki leżą przed zlotem
  const ZLOT_MS = 1400; // ile leci jedna kropka
  const OPOZNIENIE_MS = 450; // rozrzut startu kropek
  const ODPYCHA = 8; // px — o ile kursor odsuwa kropki tuż przy sobie
  const ZASIEG = 36; // px — od jakiej odległości kursor zaczyna działać
  const ROZPRYSK = 340; // siła kliknięcia (px/s na starcie)
  const ROZPRYSK_ZASIEG = 60; // px
  const SPREZYNA = 26; // powrót po rozprysku
  const TLUMIENIE = 8;
  const DRGANIE = 0.35; // px — spokojne falowanie kropek w spoczynku
  const SPOKOJ_MS = 1000 / 15;
  const ODCIENIE = 8; // ile stopni koloru (kropki grupowane, żeby rzadko zmieniać fillStyle)
  const ZAR = 22; // px od miejsca, przy których kropka świeci pełnym kolorem ruchu
  // Kolor spoczynku i kolor kropek w ruchu: szare kropki rozjaśniają się do bieli (bez kolorów, decyzja brosky'ego;
  // pomarańczowy akcent z oryginału, [255, 90, 54], odrzucony 2026-11-15)
  const kolor = { baza: [225, 225, 225], ruch: [255, 255, 255], poswiata: "rgba(255,255,255,0.9)" };
  const paleta = Array.from({ length: ODCIENIE }, (_, i) => {
    const t = i / (ODCIENIE - 1);
    const [r, g, b] = kolor.baza.map((c, k) => Math.round(c + (kolor.ruch[k] - c) * t));
    return `rgb(${r},${g},${b})`;
  });

  let kropki = [];
  let szer = 0;
  let wys = 0;
  let dpr = 1;
  let wlaczone = true;
  let klatka = 0;
  let czekaj = 0;
  let ostatnia = 0;
  let zlotOd = -1; // performance.now() początku zlotu, -1 = bez zlotu
  let rozprysk = false;
  let doZlotu = true; // przy następnym ułożeniu kropki mają się zlecieć
  let ksztalt = ""; // co było ostatnio ułożone (napis, czcionka, rozmiar)
  const mysz = { jest: false, x: 0, y: 0, gx: 0, gy: 0 };

  const ease = (t) => 1 - (1 - t) ** 3;

  function maDzialac() {
    const edytor = document.getElementById("odtwarzacz");
    return wlaczone && !document.hidden && document.hasFocus() && !(edytor && !edytor.hidden);
  }

  // Układa płótno nad napisem i wyznacza miejsca kropek z liter narysowanych tą samą czcionką
  function uloz() {
    const r = marka.getBoundingClientRect();
    if (!wlaczone || r.width < 1 || r.height < 1) {
      plotno.hidden = true;
      ksztalt = "";
      return false;
    }
    plotno.hidden = false;
    const styl = getComputedStyle(marka);
    const czcionka = `${styl.fontWeight} ${styl.fontSize} ${styl.fontFamily}`;
    const nowyDpr = Math.min(window.devicePixelRatio || 1, 2);
    const klucz = [marka.textContent, czcionka, r.width, r.height, nowyDpr].join("|");
    plotno.style.left = `${r.left - ZAPAS_X}px`;
    plotno.style.top = `${r.top - ZAPAS_Y}px`;
    if (klucz === ksztalt) return true;
    ksztalt = klucz;
    dpr = nowyDpr;
    szer = Math.ceil(r.width + 2 * ZAPAS_X);
    wys = Math.ceil(r.height + 2 * ZAPAS_Y);
    plotno.style.width = `${szer}px`;
    plotno.style.height = `${wys}px`;
    plotno.width = Math.round(szer * dpr);
    plotno.height = Math.round(wys * dpr);

    // Litery w pamięci, w pikselach ekranu (ostrzej przy skalowaniu 125–200%)
    const tekst = marka.textContent.trim();
    const pomoc = document.createElement("canvas");
    pomoc.width = plotno.width;
    pomoc.height = plotno.height;
    const p = pomoc.getContext("2d", { willReadFrequently: true });
    p.scale(dpr, dpr);
    p.font = czcionka;
    p.textBaseline = "alphabetic";
    const m = p.measureText(tekst);
    const gora = m.actualBoundingBoxAscent;
    const dol = m.actualBoundingBoxDescent;
    // W pionie jak w HTML: środek pudełka liter na środku linii napisu
    const y = ZAPAS_Y + r.height / 2 + (gora - dol) / 2;
    p.fillStyle = "#fff";
    p.fillText(tekst, ZAPAS_X + (m.actualBoundingBoxLeft || 0), y);
    const dane = p.getImageData(0, 0, pomoc.width, pomoc.height).data;

    const stare = kropki;
    kropki = [];
    const krok = GESTOSC * dpr;
    let i = 0;
    for (let py = krok / 2; py < pomoc.height; py += krok) {
      for (let px = krok / 2; px < pomoc.width; px += krok) {
        const a = dane[(Math.floor(py) * pomoc.width + Math.floor(px)) * 4 + 3];
        if (a < PROG) continue;
        // Stały pseudolosowy „charakter” kropki: kierunek rozsypki, opóźnienie, głębia
        const los = (Math.sin(i * 12.9898 + 78.233) * 43758.5453) % 1;
        const los2 = (Math.sin(i * 39.3468 + 11.135) * 24634.6345) % 1;
        const [s, g] = [Math.abs(los), Math.abs(los2)];
        const tx = px / dpr;
        const ty = py / dpr;
        const stara = stare[i];
        kropki.push({
          tx, ty,
          x: stara ? stara.x : tx,
          y: stara ? stara.y : ty,
          sx: tx, sy: ty, // start zlotu
          ox: 0, oy: 0, vx: 0, vy: 0, // odchylenie po rozprysku
          s, g,
          rozmiar: KROPKA * (0.8 + 0.4 * (a / 255)) * (0.85 + 0.3 * g),
        });
        i++;
      }
    }
    if (doZlotu) zlot();
    return true;
  }

  // Rozsypuje kropki wokół napisu; przez ZLOT_MS (+ opóźnienie) wracają na swoje miejsca
  function zlot() {
    doZlotu = false;
    for (const k of kropki) {
      const kat = k.s * Math.PI * 2;
      const dal = ROZSYPKA * (0.35 + 0.8 * k.g);
      k.sx = k.tx + Math.cos(kat) * dal * 1.4;
      k.sy = k.ty + Math.sin(kat) * dal * 0.55;
      k.x = k.sx;
      k.y = k.sy;
      k.ox = k.oy = k.vx = k.vy = 0;
    }
    rozprysk = false;
    zlotOd = performance.now();
  }

  function rozpryskuj(x, y) {
    for (const k of kropki) {
      const [dx, dy] = [k.x - x, k.y - y];
      const d = Math.hypot(dx, dy);
      if (d > ROZPRYSK_ZASIEG) continue;
      const sila = ROZPRYSK * (0.25 + 0.75 * (1 - d / ROZPRYSK_ZASIEG) ** 2) * (0.7 + 0.6 * k.g);
      const kat = Math.atan2(dy, dx) + (k.s - 0.5) * 0.9;
      k.vx += Math.cos(kat) * sila;
      k.vy += Math.sin(kat) * sila;
    }
    rozprysk = true;
    budz();
  }

  // Jeden krok: liczy położenia i rysuje. Zwraca, czy coś się jeszcze rusza.
  function krok(teraz) {
    const dt = ostatnia ? Math.min(0.05, (teraz - ostatnia) / 1000) : 1 / 60;
    ostatnia = teraz;
    const klatki = dt * 60;
    const nadaza = 1 - 0.75 ** klatki; // kropka dogania swoje miejsce (miękko, bez skoków)
    mysz.gx += (mysz.x - mysz.gx) * (1 - 0.8 ** klatki);
    mysz.gy += (mysz.y - mysz.gy) * (1 - 0.8 ** klatki);
    const odpychaj = mysz.jest && mysz.x > -ZASIEG && mysz.x < szer + ZASIEG && mysz.y > -ZASIEG && mysz.y < wys + ZASIEG;
    const czas = teraz / 1000;
    const zbierajacy = zlotOd >= 0;
    let ruch = false;
    let zlotTrwa = false;
    let drga = false;
    const grupy = Array.from({ length: ODCIENIE }, () => []);

    for (const k of kropki) {
      let x = k.tx;
      let y = k.ty;
      let widac = 1;
      if (zbierajacy) {
        const t = Math.min(1, Math.max(0, (teraz - zlotOd - k.s * OPOZNIENIE_MS) / ZLOT_MS));
        if (t < 1) zlotTrwa = true;
        const e = ease(t);
        x = k.sx + (k.tx - k.sx) * e;
        y = k.sy + (k.ty - k.sy) * e;
        widac = 0.25 + 0.75 * e;
      } else if (DRGANIE > 0) {
        x += Math.sin(czas * 1.3 + k.s * 20) * DRGANIE * k.g;
        y += Math.cos(czas * 1.1 + k.g * 20) * DRGANIE * k.g;
        drga = true;
      }
      if (rozprysk) {
        k.vx += (-SPREZYNA * k.ox - TLUMIENIE * k.vx) * dt;
        k.vy += (-SPREZYNA * k.oy - TLUMIENIE * k.vy) * dt;
        k.ox += k.vx * dt;
        k.oy += k.vy * dt;
        if (Math.abs(k.ox) + Math.abs(k.oy) > 0.05 || Math.abs(k.vx) + Math.abs(k.vy) > 1) ruch = true;
        else k.ox = k.oy = k.vx = k.vy = 0;
        x += k.ox;
        y += k.oy;
      }
      if (odpychaj) {
        const [dx, dy] = [x - mysz.gx, y - mysz.gy];
        const d = Math.hypot(dx, dy);
        if (d > 0.001 && d < ZASIEG) {
          const f = (1 - d / ZASIEG) ** 2 * ODPYCHA;
          x += (dx / d) * f;
          y += (dy / d) * f;
        }
      }
      const [mx, my] = [(x - k.x) * nadaza, (y - k.y) * nadaza];
      k.x += mx;
      k.y += my;
      if (Math.abs(mx) + Math.abs(my) > 0.02) ruch = true;
      const daleko = Math.hypot(k.x - k.tx, k.y - k.ty);
      const odcien = Math.round(Math.min(1, Math.max(0, (daleko - 1.5) / ZAR)) * (ODCIENIE - 1));
      k.widac = widac;
      grupy[odcien].push(k);
    }
    if (zbierajacy && !zlotTrwa) zlotOd = -1;
    if (rozprysk && !ruch) rozprysk = false;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, szer, wys);
    for (let o = 0; o < ODCIENIE; o++) {
      const grupa = grupy[o];
      if (!grupa.length) continue;
      // Kropki w ruchu dostają poświatę (drogą, więc tylko dla nich i tylko gdy jest ich mało)
      if (o >= ODCIENIE / 2) {
        ctx.shadowColor = kolor.poswiata;
        ctx.shadowBlur = 4 * dpr;
      } else ctx.shadowBlur = 0;
      ctx.fillStyle = paleta[o];
      // Przezroczystość z grubsza (podczas zlotu kropki jaśnieją): 4 poziomy
      for (let poziom = 1; poziom <= 4; poziom++) {
        ctx.globalAlpha = poziom / 4;
        ctx.beginPath();
        let cos = false;
        for (const k of grupa) {
          if (Math.ceil(k.widac * 4) !== poziom) continue;
          const r = k.rozmiar;
          ctx.rect(k.x - r / 2, k.y - r / 2, r, r);
          cos = true;
        }
        if (cos) ctx.fill();
      }
    }
    ctx.globalAlpha = 1;
    ctx.shadowBlur = 0;
    return { szybko: ruch || zlotTrwa || rozprysk || odpychaj, drga };
  }

  function petla(teraz) {
    klatka = 0;
    if (!maDzialac() || !uloz()) return;
    const { szybko, drga } = krok(teraz);
    if (szybko) klatka = requestAnimationFrame(petla);
    else if (drga) czekaj = setTimeout(() => ((czekaj = 0), (klatka = requestAnimationFrame(petla))), SPOKOJ_MS);
  }

  // Rusza animację, jeśli stoi (np. po ruchu myszki albo kliknięciu)
  function budz() {
    if (klatka || !maDzialac()) return;
    if (czekaj) clearTimeout(czekaj), (czekaj = 0);
    ostatnia = 0;
    klatka = requestAnimationFrame(petla);
  }

  // Sprawdza stan okna: w tle wszystko staje i zostaje ułożony, nieruchomy napis
  function odswiez() {
    marka.classList.toggle("czasteczki", wlaczone);
    if (!uloz()) return;
    if (maDzialac()) return budz();
    if (klatka) cancelAnimationFrame(klatka), (klatka = 0);
    if (czekaj) clearTimeout(czekaj), (czekaj = 0);
    zlotOd = -1;
    rozprysk = false;
    mysz.jest = false;
    for (const k of kropki) [k.x, k.y, k.ox, k.oy, k.vx, k.vy] = [k.tx, k.ty, 0, 0, 0, 0];
    ostatnia = 0;
    krok(performance.now());
  }

  document.addEventListener("pointermove", (e) => {
    if (!wlaczone || plotno.hidden) return;
    const r = plotno.getBoundingClientRect();
    const [x, y] = [e.clientX - r.left, e.clientY - r.top];
    if (!mysz.jest) [mysz.gx, mysz.gy] = [x, y];
    const blisko = x > -ZASIEG && x < szer + ZASIEG && y > -ZASIEG && y < wys + ZASIEG;
    [mysz.x, mysz.y, mysz.jest] = [x, y, true];
    if (blisko) budz();
  });
  document.documentElement.addEventListener("mouseleave", () => {
    mysz.jest = false;
    budz();
  });
  marka.addEventListener("pointerdown", (e) => {
    if (!wlaczone || plotno.hidden) return;
    const r = plotno.getBoundingClientRect();
    rozpryskuj(e.clientX - r.left, e.clientY - r.top);
  });
  new ResizeObserver(() => odswiez()).observe(marka);
  window.addEventListener("resize", () => odswiez());
  for (const zdarzenie of ["focus", "blur"]) window.addEventListener(zdarzenie, () => setTimeout(odswiez));
  document.addEventListener("visibilitychange", () => {
    // Okno wraca z zasobnika: kropki zlatują się od nowa
    if (!document.hidden) doZlotu = true;
    odswiez();
    if (!document.hidden && doZlotu && kropki.length) zlot(), (doZlotu = false), budz();
  });
  // Kształt liter bierzemy dopiero po załadowaniu czcionki (inaczej kropki ułożyłyby się zastępczą)
  (document.fonts ? document.fonts.ready : Promise.resolve()).then(odswiez, odswiez);

  return {
    /** Włącza albo wyłącza napis z cząsteczek (ustawienie „Napis z cząsteczek”); wyłączony = zwykły biały napis. */
    wlacz(tak) {
      const bylo = wlaczone;
      wlaczone = !!tak;
      if (wlaczone && !bylo) doZlotu = true;
      odswiez();
    },
    /** Sprawdza od nowa, czy animacja ma działać (np. po otwarciu albo zamknięciu edytora). */
    odswiez,
    /** Zlot od nowa (do podglądu i testów). */
    zlot() {
      if (kropki.length) zlot(), budz();
    },
  };
})();
