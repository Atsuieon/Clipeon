// Tło okna: siatka znaków ASCII. Większość okna jest czarna; tu i tam leżą nieregularne plamy napisu
// „CLIPEON”, które na brzegach rzedną: litery → plusy → dwukropki → rozproszone kropki → nic.
// Plamy bardzo powoli zmieniają kształt i miejsce.
// Ruch myszki miesza „płyn” pod znakami: myszka wlewa barwnik (jasne, gęste znaki) i popycha płyn
// w stronę ruchu. Barwnik płynie z prądem, zwija się w wiry i powoli rozmywa — jak dym albo tusz w wodzie.
// Fizyka to „Stable Fluids” Josa Stama (adwekcja, wiry, ciśnienie), liczona na zgrubnej siatce na procesorze.
// Myszka, która stoi, niczego nie dolewa, więc efekt wokół niej sam znika.
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

  // Płyn pod znakami
  const OCZKO = 8; // bok komórki symulacji w px
  const PROMIEN = 20; // px — promień pędzla przy szybkim ruchu (odchylenie krzywej dzwonowej)
  const PROMIEN_WOLNO = 5; // px — przy bardzo wolnym ruchu: maleńka plamka
  const WOLNO = 15; // px/s — tak wolny ruch prawie nic nie wlewa
  const SZYBKO = 1000; // px/s — od tej prędkości pędzel ma pełny rozmiar i wlewa najwięcej
  const KROK = 6; // co ile px ruchu przykładamy pędzel
  const PCHA = 0.65; // do jakiej części prędkości myszki rozpędza się płyn pod pędzlem
  const CHWYTA = 0.35; // jak szybko płyn pod pędzlem łapie tę prędkość (za każde ~16 ms ruchu)
  const WLEWA = 0.55; // ile barwnika wlewa pędzel za każde ~16 ms ruchu (1 = pełna jasność)
  const WIRY = 22; // siła zawijania (vorticity confinement)
  const ZAPAS = 4; // o ile komórek obszar liczenia wystaje poza miejsca, gdzie coś płynie
  const PRZEBIEGI = 20; // przebiegi Jacobiego dla ciśnienia
  const BARWNIK_GASNIE = 0.6; // na sekundę (barwnik / (1 + x·dt)) — po ~1 s zostaje ok. 55%
  const PRAD_GASNIE = 0.9; // na sekundę dla prędkości — płyn płynie jeszcze chwilę siłą rozpędu
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
  let ziarna = new Float32Array(0); // stały los komórki (0–1)
  let mysz = null; // ostatnie położenie myszki { x, y, t } albo null, gdy smuga nie ma powstawać
  let swieci = false; // czy płyn jeszcze się rusza albo widać barwnik
  // Płyn: siatka gx × gy komórek po OCZKO px z ramką jednej komórki dookoła okna.
  // u, v — prędkość w komórkach na sekundę; barwnik 0–~1,5.
  let gx = 0;
  let gy = 0;
  let u = new Float32Array(0);
  let v = new Float32Array(0);
  let barwnik = new Float32Array(0);
  let [u2, v2, barwnik2, cisnienie, cisnienie2, rozbieznosc, wir] = Array(7).fill(new Float32Array(0));
  let predkosc = 0; // wygładzona prędkość myszki w px/s
  let wlewy = []; // ruchy myszki od ostatniej klatki: { x, y, dx, dy, chwyt, ile } w komórkach płynu
  // Płyn liczymy tylko w prostokącie wokół miejsc, gdzie coś płynie (reszta okna stoi i ma zera)
  let [ox0, ox1, oy0, oy1] = [1, 0, 1, 0];
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
    [plamy, ziarna] = [0, 0].map(() => new Float32Array(ile));
    for (let y = 0; y < wiersze; y++) for (let x = 0; x < kolumny; x++) ziarna[y * kolumny + x] = los(x, y, 0);
    gx = Math.ceil(w / OCZKO) + 2;
    gy = Math.ceil(h / OCZKO) + 2;
    [u, v, barwnik, u2, v2, barwnik2, cisnienie, cisnienie2, rozbieznosc, wir] = Array.from({ length: 10 }, () => new Float32Array(gx * gy));
    wlewy = [];
    swieci = false;
    [ox0, ox1, oy0, oy1] = [1, 0, 1, 0];
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

  // Pędzel (krzywa dzwonowa o promieniu „sigma” komórek) w punkcie (x, y) komórek płynu: rozpędza płyn do
  // prędkości (dx, dy) (część „chwyt”) i wlewa barwnik. Płyn dostaje najwyżej prędkość myszki, więc barwnik
  // nie ucieka przed kursorem, tylko ciągnie się za nim.
  function wlej(x, y, dx, dy, chwyt, ile, sigma) {
    const [cx, cy] = [Math.round(x), Math.round(y)];
    const r = Math.ceil(sigma * 2.5);
    const dzielnik = -0.5 / (sigma * sigma);
    const pusty = ox1 < ox0;
    ox0 = Math.max(1, Math.min(pusty ? Infinity : ox0, cx - r - ZAPAS));
    ox1 = Math.min(gx - 2, Math.max(pusty ? -Infinity : ox1, cx + r + ZAPAS));
    oy0 = Math.max(1, Math.min(pusty ? Infinity : oy0, cy - r - ZAPAS));
    oy1 = Math.min(gy - 2, Math.max(pusty ? -Infinity : oy1, cy + r + ZAPAS));
    for (let oy = -r; oy <= r; oy++) {
      const yy = cy + oy;
      if (yy < 1 || yy >= gy - 1) continue;
      for (let ox = -r; ox <= r; ox++) {
        const xx = cx + ox;
        if (xx < 1 || xx >= gx - 1) continue;
        const [ex, ey] = [xx - x, yy - y];
        const g = Math.exp((ex * ex + ey * ey) * dzielnik);
        if (g < 0.01) continue;
        const i = yy * gx + xx;
        const k = g * chwyt;
        u[i] += (dx - u[i]) * k;
        v[i] += (dy - v[i]) * k;
        barwnik[i] = Math.min(1.5, barwnik[i] + g * ile);
      }
    }
  }

  // Wartość pola w dowolnym punkcie siatki (interpolacja dwuliniowa, przycięta do okna)
  function probka(pole, x, y) {
    x = Math.min(gx - 1.001, Math.max(0, x));
    y = Math.min(gy - 1.001, Math.max(0, y));
    const [x0, y0] = [x | 0, y | 0];
    const [fx, fy] = [x - x0, y - y0];
    const i = y0 * gx + x0;
    const gora = pole[i] + (pole[i + 1] - pole[i]) * fx;
    const dol = pole[i + gx] + (pole[i + gx + 1] - pole[i + gx]) * fx;
    return gora + (dol - gora) * fy;
  }

  // Brzegi okna to ściany: płyn się od nich odbija, a nie wylewa
  function sciany(pu, pv) {
    for (let x = 0; x < gx; x++) {
      const [g, d] = [x, (gy - 1) * gx + x];
      pu[g] = pu[g + gx];
      pv[g] = -pv[g + gx];
      pu[d] = pu[d - gx];
      pv[d] = -pv[d - gx];
    }
    for (let y = 0; y < gy; y++) {
      const [l, p] = [y * gx, y * gx + gx - 1];
      pu[l] = -pu[l + 1];
      pv[l] = pv[l + 1];
      pu[p] = -pu[p - 1];
      pv[p] = pv[p - 1];
    }
  }

  // Jeden krok płynu (dt w sekundach): wlewy myszki, wiry, ciśnienie (płyn się nie ściska), przenoszenie
  function krokPlynu(dt) {
    for (const w of wlewy) wlej(w.x, w.y, w.dx, w.dy, w.chwyt, w.ile, w.sigma);
    wlewy = [];
    if (ox1 < ox0) return;
    // Wiry: mierzymy, jak płyn się kręci, i podkręcamy go mocniej tam, gdzie już się kręci — stąd zawijasy
    for (let y = oy0; y <= oy1; y++) {
      for (let x = ox0; x <= ox1; x++) {
        const i = y * gx + x;
        wir[i] = 0.5 * (v[i + 1] - v[i - 1] - (u[i + gx] - u[i - gx]));
      }
    }
    for (let y = Math.max(2, oy0); y <= Math.min(gy - 3, oy1); y++) {
      for (let x = Math.max(2, ox0); x <= Math.min(gx - 3, ox1); x++) {
        const i = y * gx + x;
        const nx = 0.5 * (Math.abs(wir[i + 1]) - Math.abs(wir[i - 1]));
        const ny = 0.5 * (Math.abs(wir[i + gx]) - Math.abs(wir[i - gx]));
        const dl = Math.hypot(nx, ny) + 1e-5;
        const k = (WIRY * wir[i] * dt) / dl;
        u[i] += ny * k;
        v[i] -= nx * k;
      }
    }
    sciany(u, v);
    // Ciśnienie: usuwa ściskanie i rozrzedzanie, więc płyn omija się i zawija zamiast znikać
    for (let y = oy0; y <= oy1; y++) {
      for (let x = ox0; x <= ox1; x++) {
        const i = y * gx + x;
        rozbieznosc[i] = 0.5 * (u[i + 1] - u[i - 1] + v[i + gx] - v[i - gx]);
      }
    }
    // Poza obszarem ciśnienie jest zerem (tam nic nie płynie)
    cisnienie.fill(0);
    cisnienie2.fill(0);
    for (let n = 0; n < PRZEBIEGI; n++) {
      for (let y = oy0; y <= oy1; y++) {
        for (let x = ox0; x <= ox1; x++) {
          const i = y * gx + x;
          cisnienie2[i] = 0.25 * (cisnienie[i - 1] + cisnienie[i + 1] + cisnienie[i - gx] + cisnienie[i + gx] - rozbieznosc[i]);
        }
      }
      for (let x = 0; x < gx; x++) {
        cisnienie2[x] = cisnienie2[x + gx];
        cisnienie2[(gy - 1) * gx + x] = cisnienie2[(gy - 2) * gx + x];
      }
      for (let y = 0; y < gy; y++) {
        cisnienie2[y * gx] = cisnienie2[y * gx + 1];
        cisnienie2[y * gx + gx - 1] = cisnienie2[y * gx + gx - 2];
      }
      [cisnienie, cisnienie2] = [cisnienie2, cisnienie];
    }
    for (let y = oy0; y <= oy1; y++) {
      for (let x = ox0; x <= ox1; x++) {
        const i = y * gx + x;
        u[i] -= 0.5 * (cisnienie[i + 1] - cisnienie[i - 1]);
        v[i] -= 0.5 * (cisnienie[i + gx] - cisnienie[i - gx]);
      }
    }
    sciany(u, v);
    // Przenoszenie: każda komórka bierze to, co przypłynęło do niej z miejsca „pod prąd”
    const [zostajePrad, zostajeBarwnik] = [1 / (1 + PRAD_GASNIE * dt), 1 / (1 + BARWNIK_GASNIE * dt)];
    let najwiecej = 0;
    let [nx0, nx1, ny0, ny1] = [gx, -1, gy, -1]; // gdzie po tym kroku jeszcze coś płynie
    for (let y = oy0; y <= oy1; y++) {
      for (let x = ox0; x <= ox1; x++) {
        const i = y * gx + x;
        const [sx, sy] = [x - dt * u[i], y - dt * v[i]];
        const pu = (u2[i] = probka(u, sx, sy) * zostajePrad);
        const pv = (v2[i] = probka(v, sx, sy) * zostajePrad);
        const b = (barwnik2[i] = probka(barwnik, sx, sy) * zostajeBarwnik);
        if (b > najwiecej) najwiecej = b;
        if (b > 0.01 || Math.abs(pu) + Math.abs(pv) > 0.3) {
          if (x < nx0) nx0 = x;
          if (x > nx1) nx1 = x;
          if (y < ny0) ny0 = y;
          if (y > ny1) ny1 = y;
        }
      }
    }
    [u, u2] = [u2, u];
    [v, v2] = [v2, v];
    [barwnik, barwnik2] = [barwnik2, barwnik];
    sciany(u, v);
    // Obszar idzie za płynem: rośnie, gdy płyn się rozlewa, i kurczy się, gdy gaśnie
    [ox0, ox1, oy0, oy1] = [Math.max(1, nx0 - ZAPAS), Math.min(gx - 2, nx1 + ZAPAS), Math.max(1, ny0 - ZAPAS), Math.min(gy - 2, ny1 + ZAPAS)];
    // Gdy barwnika już prawie nie widać, płyn się zatrzymuje i tło wraca do 8 klatek na sekundę
    if (najwiecej < 0.03) {
      for (const pole of [u, v, barwnik, u2, v2, barwnik2]) pole.fill(0);
      [ox0, ox1, oy0, oy1] = [1, 0, 1, 0];
      swieci = false;
    }
  }

  function rysuj() {
    const t = czas / 1000;
    if (czas - czasPlam >= SPOKOJ_MS) przeliczPlamy();
    ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
    const linie = POZIOMY.map(() => new Array(kolumny));
    // Znaki nad obszarem płynu (tylko tam może być barwnik)
    const [kx0, kx1] = [Math.floor(((ox0 - 1) * OCZKO) / kolumna) - 1, Math.ceil(((ox1 - 1) * OCZKO) / kolumna) + 1];
    const [ky0, ky1] = [Math.floor(((oy0 - 1) * OCZKO) / WIERSZ) - 1, Math.ceil(((oy1 - 1) * OCZKO) / WIERSZ) + 1];
    for (let y = 0; y < wiersze; y++) {
      for (const l of linie) l.fill(" ");
      for (let x = 0; x < kolumny; x++) {
        const i = y * kolumny + x;
        const ziarno = ziarna[i];
        let jasnosc = plamy[i];
        if (swieci && x >= kx0 && x <= kx1 && y >= ky0 && y <= ky1) {
          // Barwnik płynu pod środkiem znaku; drobne ziarno strzępi rzadki brzeg jak dym
          const b = probka(barwnik, 1 + ((x + 0.5) * kolumna) / OCZKO, 1 + ((y + 0.5) * WIERSZ) / OCZKO);
          if (b > 0.03) {
            const smuga = Math.min(1, b) * 0.85 - ziarno * 0.12 * (1 - Math.min(1, b));
            if (smuga > jasnosc) jasnosc = smuga;
          }
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
    // Płyn liczymy w krokach najwyżej ~1/30 s, żeby szybki ruch nie „przeskakiwał” komórek
    if (swieci) for (let zostalo = ms / 1000; zostalo > 1e-4; zostalo -= 1 / 30) krokPlynu(Math.min(zostalo, 1 / 30));
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
      for (const pole of [u, v, barwnik, u2, v2, barwnik2]) pole.fill(0);
      [ox0, ox1, oy0, oy1] = [1, 0, 1, 0];
      wlewy = [];
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
    if (!mysz) predkosc = 0;
    if (mysz && klatka) {
      const [dx, dy] = [nowa.x - mysz.x, nowa.y - mysz.y];
      const droga = Math.hypot(dx, dy);
      if (droga > 0.5) {
        // Ile „ruchu” niesie to zdarzenie (w jednostkach ~16 ms): po dłuższym postoju nie więcej niż 2
        const porcja = Math.min(2, Math.max(0.25, (nowa.t - mysz.t) / 16.7));
        // Prędkość myszki w komórkach płynu na sekundę
        const ms = Math.max(4, nowa.t - mysz.t);
        const [px, py] = [(dx / OCZKO / ms) * 1000, (dy / OCZKO / ms) * 1000];
        // Wolny ruch = maleńka, słaba plamka; szybki = szeroka, gęsta struga. Prędkość wygładzona,
        // bo zdarzenia przychodzą nierówno; po dłuższym postoju liczy się od nowa.
        const chwilowa = (droga / ms) * 1000;
        predkosc = ms > 120 ? chwilowa : predkosc * 0.6 + chwilowa * 0.4;
        const moc = Math.min(1, Math.max(0, (predkosc - WOLNO) / (SZYBKO - WOLNO))) ** 0.6;
        const sigma = (PROMIEN_WOLNO + (PROMIEN - PROMIEN_WOLNO) * moc) / OCZKO;
        // Pędzel co kilka px drogi: szybki ruch zostawia ciągłą strugę, a nie pojedyncze plamy
        const kroki = Math.min(80, Math.max(1, Math.round(droga / KROK)));
        for (let i = 1; i <= kroki; i++) {
          const t = i / kroki;
          wlewy.push({
            x: 1 + (mysz.x + dx * t) / OCZKO,
            y: 1 + (mysz.y + dy * t) / OCZKO,
            dx: px * PCHA,
            dy: py * PCHA,
            chwyt: 1 - (1 - Math.min(0.9, CHWYTA * porcja)) ** (1 / kroki),
            ile: (WLEWA * porcja * moc ** 1.5) / kroki,
            sigma,
          });
        }
        swieci = true;
      }
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
