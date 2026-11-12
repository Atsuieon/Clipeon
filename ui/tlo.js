// Tło okna: siatka znaków ASCII. Większość okna jest czarna; tu i tam leżą nieregularne plamy napisu
// „CLIPEON”, które na brzegach rzedną: litery → plusy → dwukropki → rozproszone kropki → nic.
// Plamy bardzo powoli zmieniają kształt i miejsce.
// Ruch myszki zostawia smugę gęstych znaków: im szybszy ruch, tym jaśniejsza. Smuga nie idzie nigdzie
// (nic nie leci do góry) — strzępi się na brzegach i powoli gaśnie tam, gdzie powstała. Myszka, która
// stoi, niczego nie rozjaśnia, więc efekt wokół niej sam znika.
// Nad przyciskiem, polem ustawień albo klipem smuga nie powstaje — tam ma być czytelnie.
// Rysowanie stoi, gdy okno jest w tle, schowane albo otwarty jest edytor (zero pracy podczas gry).
// Wtedy też <html> dostaje klasę „tlo-stoi”, która zatrzymuje zorzę na napisie „Clipeon” (style.css).
// Tła nie gasi systemowe „bez animacji” (prefers-reduced-motion): ma własny wyłącznik w Ustawieniach.
window.Tlo = (() => {
  const plotno = document.getElementById("tlo");
  const ctx = plotno.getContext("2d");
  const NAPIS = "CLIPEON ";
  const LEKKIE = "/*=<>?+-~";
  const GESTE = "$%&@#S";
  const CZCIONKA = 'bold 10px Consolas, "Cascadia Mono", "Courier New", monospace';
  const WIERSZ = 12; // wysokość komórki w px
  const KLATKA_MS = 1000 / 60; // gdy widać smugę
  const SPOKOJ_MS = 125; // gdy rusza się samo tło (jest tak wolne, że 8 klatek na sekundę wystarcza)
  // Jasność znaków: kropki i dwukropki, plusy, napis, smuga, środek smugi
  const POZIOMY = [0.11, 0.15, 0.24, 0.45, 0.7];

  // Plamy tła: rozmiar w px, prędkość dryfu w px/s, progi pola szumu (0–1)
  const PLAMA_X = 330;
  const PLAMA_Y = 170;
  const DRYF = 2.2;
  // Pole szumu (0–1) → jasność tła. Poniżej pierwszego progu jest czarno (ponad połowa okna), potem
  // szeroki pas rozproszonych kropek, węższe pasy „:” i „+”, a od 0,68 napis (nigdy symbole smugi).
  const TLO_POLE = [0.54, 0.615, 0.65, 0.68, 0.74];
  const TLO_JASNOSC = [0.012, 0.06, 0.1, 0.15, 0.23];
  const TLO_NAPIS = 0.14; // jaka część okna ma być napisem (reszta progów idzie za tym)

  // Smuga za myszką: promień w px (odchylenie krzywej dzwonowej), czas gaśnięcia, czułość na prędkość
  const PROMIEN_X = 50;
  const PROMIEN_Y = 35;
  const GASNIE_MS = 1600; // po tylu ms smuga ma ok. 1/3 jasności
  const ROZJASNIA = 0.4; // jaką część drogi do jasności ruchu robi jedno dotknięcie
  const KROK = 9; // co ile px ruchu przykładamy pędzel
  const WOLNO = 70; // px/s — wolniejszy ruch nie zostawia nic
  const SZYBKO = 1100; // px/s — od tej prędkości smuga ma pełną jasność
  // Nad tym smuga nie powstaje
  const BEZ_POSWIATY =
    "button, a, input, select, textarea, label, summary, .karta, .menu, .nakladka, .belka, .baner, .pasek-zaznaczenia, .dymek, .gora";

  // Szum: losowa krata wygładzana między węzłami. Losuje się przy starcie, więc tło jest za każdym razem inne.
  const KRATA = 64;
  const wezly = new Float32Array(KRATA * KRATA).map(() => Math.random());
  function szum(x, y) {
    const [x0, y0] = [Math.floor(x), Math.floor(y)];
    let [fx, fy] = [x - x0, y - y0];
    fx = fx * fx * (3 - 2 * fx);
    fy = fy * fy * (3 - 2 * fy);
    const [xa, xb] = [x0 & (KRATA - 1), (x0 + 1) & (KRATA - 1)];
    const [ya, yb] = [(y0 & (KRATA - 1)) * KRATA, ((y0 + 1) & (KRATA - 1)) * KRATA];
    const gora = wezly[ya + xa] + (wezly[ya + xb] - wezly[ya + xa]) * fx;
    const dol = wezly[yb + xa] + (wezly[yb + xb] - wezly[yb + xa]) * fx;
    return gora + (dol - gora) * fy;
  }
  // Tani, powtarzalny „los” dla komórki (stałe znaki bez Math.random w każdej klatce)
  const los = (x, y, t) => {
    const n = Math.sin(x * 12.9898 + y * 78.233 + t * 37.719) * 43758.5453;
    return n - Math.floor(n);
  };

  let wlaczone = true;
  let kolumna = 6.05; // szerokość znaku (mierzona po ustawieniu czcionki)
  let kolumny = 0;
  let wiersze = 0;
  let plamy = new Float32Array(0); // jasność tła w każdej komórce
  let strzepy = new Float32Array(0); // jak mocno komórka „wygryza” brzeg smugi (stałe, 0–1)
  let ziarna = new Float32Array(0); // stały los komórki (0–1)
  let swiatlo = new Float32Array(0); // jasność smugi w każdej komórce (0–1)
  let pedzel = { rx: 0, ry: 0, sila: new Float32Array(0) }; // kształt smugi w komórkach
  let mysz = null; // ostatnie położenie myszki { x, y, t } albo null, gdy smuga nie ma powstawać
  let predkosc = 0; // wygładzona prędkość myszki w px/s
  let swieci = false; // czy gdziekolwiek widać jeszcze smugę
  let klatka = 0; // numer zamówionej klatki animacji (0 = animacja stoi)
  let ostatnia = 0;
  let czas = 0;
  let czasPlam = -1e9; // kiedy ostatnio przeliczono plamy tła
  let poprawka = null; // przesunięcie pola szumu, żeby napisu było zawsze mniej więcej tyle samo
  let doPoprawki = 0; // za ile przeliczeń plam poprawka liczy się od nowa
  const slupki = new Uint32Array(200);

  function dopasuj() {
    const skala = window.devicePixelRatio || 1;
    const [w, h] = [window.innerWidth, window.innerHeight];
    plotno.width = Math.round(w * skala);
    plotno.height = Math.round(h * skala);
    ctx.setTransform(skala, 0, 0, skala, 0, 0);
    ctx.font = CZCIONKA;
    ctx.textBaseline = "top";
    kolumna = ctx.measureText("M".repeat(40)).width / 40 || 6.05;
    kolumny = Math.ceil(w / kolumna) + 1;
    wiersze = Math.ceil(h / WIERSZ) + 1;
    const ile = kolumny * wiersze;
    [plamy, strzepy, ziarna, swiatlo] = [0, 0, 0, 0].map(() => new Float32Array(ile));
    for (let y = 0; y < wiersze; y++) {
      for (let x = 0; x < kolumny; x++) {
        const i = y * kolumny + x;
        ziarna[i] = los(x, y, 0);
        // Strzępy: większe „kęsy” (ok. 45 px) i drobne ziarno, żeby brzeg smugi był poszarpany jak dym
        strzepy[i] = 0.7 * szum(17 + (x * kolumna) / 46, 31 + (y * WIERSZ) / 38) + 0.3 * ziarna[i];
      }
    }
    // Pędzel: krzywa dzwonowa policzona raz (w komórkach), potem tylko przykładana wzdłuż ruchu myszki
    const [sx, sy] = [PROMIEN_X / kolumna, PROMIEN_Y / WIERSZ];
    const [rx, ry] = [Math.ceil(sx * 2.4), Math.ceil(sy * 2.4)];
    const sila = new Float32Array((2 * rx + 1) * (2 * ry + 1));
    for (let dy = -ry; dy <= ry; dy++) {
      for (let dx = -rx; dx <= rx; dx++) {
        sila[(dy + ry) * (2 * rx + 1) + dx + rx] = Math.exp(-0.5 * ((dx / sx) ** 2 + (dy / sy) ** 2));
      }
    }
    pedzel = { rx, ry, sila };
    czasPlam = -1e9;
    poprawka = null;
    doPoprawki = 0;
  }

  function jasnoscTla(pole) {
    if (pole <= TLO_POLE[0]) return 0;
    for (let i = 1; i < TLO_POLE.length; i++) {
      if (pole < TLO_POLE[i]) {
        const u = (pole - TLO_POLE[i - 1]) / (TLO_POLE[i] - TLO_POLE[i - 1]);
        return TLO_JASNOSC[i - 1] + (TLO_JASNOSC[i] - TLO_JASNOSC[i - 1]) * u;
      }
    }
    return TLO_JASNOSC[TLO_JASNOSC.length - 1];
  }

  // Plamy tła: dwie warstwy szumu, które bardzo powoli suną w różne strony, więc plamy zmieniają kształt
  function przeliczPlamy() {
    const t = czas / 1000;
    const [ax, ay] = [(t * DRYF) / PLAMA_X, (t * DRYF * 0.4) / PLAMA_Y];
    const [bx, by] = [(-t * DRYF * 0.7) / (PLAMA_X * 0.42), (t * DRYF * 0.9) / (PLAMA_Y * 0.42)];
    const mierz = doPoprawki-- <= 0;
    if (mierz) slupki.fill(0);
    const przesun = poprawka || 0;
    for (let y = 0; y < wiersze; y++) {
      const py = y * WIERSZ;
      for (let x = 0; x < kolumny; x++) {
        const px = x * kolumna;
        const pole = 0.64 * szum(px / PLAMA_X + ax, py / PLAMA_Y + ay) + 0.36 * szum(5.3 + px / (PLAMA_X * 0.42) + bx, 9.1 + py / (PLAMA_Y * 0.42) + by);
        if (mierz) slupki[Math.min(199, Math.floor(pole * 200))]++;
        plamy[y * kolumny + x] = jasnoscTla(pole - przesun);
      }
    }
    czasPlam = czas;
    if (!mierz) return;
    // Losowy szum raz daje dużo jasnych miejsc, raz mało. Szukamy wartości, powyżej której leży TLO_NAPIS okna,
    // i przesuwamy pole tak, żeby wypadła na progu napisu. Poprawka idzie do celu powoli (plamy nie skaczą).
    doPoprawki = 24;
    let [ile, k] = [0, 199];
    while (k > 0 && ile + slupki[k] < kolumny * wiersze * TLO_NAPIS) ile += slupki[k--];
    const cel = (k + 1) / 200 - TLO_POLE[3];
    if (poprawka === null) {
      poprawka = cel;
      przeliczPlamy();
      doPoprawki = 24;
    } else {
      poprawka += (cel - poprawka) * 0.2;
    }
  }

  // Rozjaśnia smugę wokół punktu okna do jasności „moc” (zależy od prędkości myszki)
  function dotknij(px, py, moc) {
    const [mx, my] = [Math.round(px / kolumna), Math.round(py / WIERSZ)];
    const { rx, ry, sila } = pedzel;
    for (let dy = -ry; dy <= ry; dy++) {
      const y = my + dy;
      if (y < 0 || y >= wiersze) continue;
      for (let dx = -rx; dx <= rx; dx++) {
        const x = mx + dx;
        if (x < 0 || x >= kolumny) continue;
        const cel = sila[(dy + ry) * (2 * rx + 1) + dx + rx] * moc;
        const i = y * kolumny + x;
        if (cel > swiatlo[i]) swiatlo[i] += (cel - swiatlo[i]) * ROZJASNIA;
      }
    }
    swieci = true;
  }

  // Smuga gaśnie w miejscu (nic się nie przesuwa)
  function zgas(ms) {
    const zostaje = Math.exp(-ms / GASNIE_MS);
    let najwiecej = 0;
    for (let i = 0; i < swiatlo.length; i++) {
      const s = (swiatlo[i] *= zostaje);
      if (s > najwiecej) najwiecej = s;
    }
    if (najwiecej < 0.02) {
      swiatlo.fill(0);
      swieci = false;
    }
  }

  function rysuj() {
    const t = czas / 1000;
    if (czas - czasPlam >= SPOKOJ_MS) przeliczPlamy();
    ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
    const linie = POZIOMY.map(() => new Array(kolumny));
    for (let y = 0; y < wiersze; y++) {
      for (const l of linie) l.fill(" ");
      for (let x = 0; x < kolumny; x++) {
        const i = y * kolumny + x;
        const ziarno = ziarna[i];
        let jasnosc = plamy[i];
        const s = swiatlo[i];
        if (s > 0.02) {
          // Środek smugi zostaje pełny, a słabszy brzeg jest wygryzany nierówno (strzępy)
          const smuga = s - strzepy[i] * 0.36 * (1 - s);
          if (smuga > jasnosc) jasnosc = smuga;
        }
        // Im jaśniej, tym gęstszy znak: nic, rozproszone kropki, : + napis, lekkie symbole, gęste symbole
        if (jasnosc < 0.012) continue;
        let poziom;
        let znak;
        if (jasnosc < 0.06) {
          // Kropki rzedną ku czerni: im ciemniej, tym mniej komórek ją dostaje
          if (ziarno > (jasnosc - 0.012) / 0.048) continue;
          poziom = 0;
          znak = ".";
        } else if (jasnosc < 0.1) {
          poziom = 0;
          znak = ":";
        } else if (jasnosc < 0.15) {
          poziom = 1;
          znak = "+";
        } else if (jasnosc < 0.32) {
          poziom = 2;
          znak = NAPIS[(x + y * 3) % NAPIS.length];
        } else {
          // Każda komórka co jakiś czas (we własnym rytmie) zmienia symbol — smuga lekko mieni się, nie miga
          const l = los(x, y, Math.floor(t * 0.35 + ziarno * 7));
          if (jasnosc < 0.5) {
            poziom = 3;
            znak = l < 0.4 ? LEKKIE[Math.floor(l * 97) % LEKKIE.length] : NAPIS[(x + y * 3) % NAPIS.length];
          } else {
            poziom = jasnosc < 0.72 ? 3 : 4;
            znak = l < 0.3 ? NAPIS[(x + y * 3) % NAPIS.length] : l < 0.6 ? LEKKIE[Math.floor(l * 97) % LEKKIE.length] : GESTE[Math.floor(l * 89) % GESTE.length];
          }
        }
        linie[poziom][x] = znak;
      }
      // Jeden napis na wiersz i poziom jasności (czcionka o stałej szerokości trzyma kolumny)
      for (let p = 0; p < POZIOMY.length; p++) {
        const tekst = linie[p].join("");
        if (tekst.trim() === "") continue;
        ctx.fillStyle = `rgba(255, 255, 255, ${POZIOMY[p]})`;
        ctx.fillText(tekst, 0, y * WIERSZ);
      }
    }
  }

  function petla(teraz) {
    klatka = 0;
    if (!maDzialac()) return;
    klatka = requestAnimationFrame(petla);
    if (teraz - ostatnia < (swieci ? KLATKA_MS : SPOKOJ_MS) - 2) return;
    // Po przerwie (okno było w tle) czas nie skacze do przodu
    const ms = Math.min(teraz - ostatnia, 150);
    czas += ms;
    ostatnia = teraz;
    if (swieci) zgas(ms);
    rysuj();
  }

  function maDzialac() {
    const edytor = document.getElementById("odtwarzacz");
    return wlaczone && !document.hidden && document.hasFocus() && !(edytor && !edytor.hidden);
  }

  // Rusza albo zatrzymuje animację według stanu okna; zatrzymana zostawia spokojny, nieruchomy obraz
  function odswiez() {
    plotno.hidden = !wlaczone;
    document.documentElement.classList.toggle("tlo-stoi", !maDzialac());
    if (!wlaczone) return;
    if (maDzialac()) {
      if (!klatka) {
        ostatnia = performance.now();
        klatka = requestAnimationFrame(petla);
      }
    } else if (!klatka) {
      swiatlo.fill(0);
      swieci = false;
      mysz = null;
      rysuj();
    }
  }

  document.addEventListener("pointermove", (e) => {
    if (e.target.closest?.(BEZ_POSWIATY)) {
      mysz = null;
      return;
    }
    const nowa = { x: e.clientX, y: e.clientY, t: e.timeStamp };
    if (mysz && klatka) {
      const droga = Math.hypot(nowa.x - mysz.x, nowa.y - mysz.y);
      const ms = Math.max(4, nowa.t - mysz.t);
      // Po dłuższym postoju prędkość liczy się od nowa; w ruchu jest wygładzana (zdarzenia przychodzą nierówno)
      const chwilowa = (droga / ms) * 1000;
      predkosc = ms > 120 ? chwilowa : predkosc * 0.6 + chwilowa * 0.4;
      const moc = Math.min(1, Math.max(0, (predkosc - WOLNO) / (SZYBKO - WOLNO))) ** 0.55;
      if (moc > 0.02) {
        // Pędzel co kilka px drogi: szybki ruch zostawia ciągłą smugę, a nie pojedyncze plamy
        const kroki = Math.min(120, Math.max(1, Math.round(droga / KROK)));
        for (let i = 1; i <= kroki; i++) {
          const u = i / kroki;
          dotknij(mysz.x + (nowa.x - mysz.x) * u, mysz.y + (nowa.y - mysz.y) * u, moc);
        }
      }
    } else {
      predkosc = 0;
    }
    mysz = nowa;
  });
  document.addEventListener("pointerleave", () => (mysz = null));
  document.documentElement.addEventListener("mouseleave", () => (mysz = null));
  window.addEventListener("resize", () => {
    dopasuj();
    if (wlaczone && !klatka) rysuj();
  });
  for (const zdarzenie of ["focus", "blur"]) window.addEventListener(zdarzenie, () => setTimeout(odswiez));
  document.addEventListener("visibilitychange", odswiez);

  dopasuj();
  odswiez();

  return {
    /** Włącza albo wyłącza tło (ustawienie „Animowane tło”). */
    wlacz(tak) {
      wlaczone = !!tak;
      odswiez();
    },
    /** Sprawdza od nowa, czy animacja ma działać (np. po otwarciu albo zamknięciu edytora). */
    odswiez,
  };
})();
