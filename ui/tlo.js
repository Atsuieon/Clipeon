// Tło okna: siatka znaków ASCII. Przez okno płyną powolne fale napisu „CLIPEON”, a wokół myszki znaki
// gęstnieją w miękką poświatę: kropki przechodzą w dwukropki, plusy, litery i w środku w gęste symbole.
// Poświata idzie za myszką, a tam, gdzie mysz była, spokojnie gaśnie w miejscu (zostaje ślad ruchu).
// Nad przyciskiem, polem ustawień albo klipem poświata nie idzie za myszką — tam ma być czytelnie.
// Rysowanie stoi, gdy okno jest w tle, schowane albo otwarty jest edytor (zero pracy podczas gry).
// Wtedy też <html> dostaje klasę „tlo-stoi”, która zatrzymuje zorzę na napisie „Clipeon” (style.css).
// Tła nie gasi systemowe „bez animacji” (prefers-reduced-motion): ma własny wyłącznik w Ustawieniach.
window.Tlo = (() => {
  const plotno = document.getElementById("tlo");
  const ctx = plotno.getContext("2d");
  const NAPIS = "CLIPEON ";
  const LEKKIE = "/*=<>?+";
  const GESTE = "$%&@#S";
  const CZCIONKA = 'bold 10px Consolas, "Cascadia Mono", "Courier New", monospace';
  const WIERSZ = 12; // wysokość komórki w px
  const KLATKA_MS = 1000 / 60;
  // Jasność znaków: kropki tła, brzeg, napis, poświata, środek poświaty
  const POZIOMY = [0.09, 0.15, 0.24, 0.45, 0.7];
  // Poświata: promień w px (odchylenie krzywej dzwonowej), czas gaśnięcia i rozjaśniania
  const PROMIEN_X = 62;
  const PROMIEN_Y = 44;
  const GASNIE_MS = 650; // po tylu ms ślad ma ok. 1/3 jasności
  const ROZJASNIA = 0.3; // jaką część drogi do pełnej jasności robi jedno dotknięcie
  // Nad tym poświata nie idzie za myszką
  const BEZ_POSWIATY =
    "button, a, input, select, textarea, label, summary, .karta, .menu, .nakladka, .belka, .baner, .pasek-zaznaczenia, .dymek, .gora";

  let wlaczone = true;
  let kolumna = 6.05; // szerokość znaku (mierzona po ustawieniu czcionki)
  let kolumny = 0;
  let wiersze = 0;
  let swiatlo = new Float32Array(0); // jasność poświaty w każdej komórce (0–1)
  let pedzel = { rx: 0, ry: 0, sila: new Float32Array(0) }; // kształt poświaty w komórkach
  let mysz = null; // { x, y } w px okna albo null, gdy poświata nie ma iść za myszką
  let klatka = 0; // numer zamówionej klatki animacji (0 = animacja stoi)
  let ostatnia = 0;
  let czas = 0;

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
    swiatlo = new Float32Array(kolumny * wiersze);
    // Pędzel: krzywa dzwonowa policzona raz (w komórkach), potem tylko przykładana pod myszką
    const [sx, sy] = [PROMIEN_X / kolumna, PROMIEN_Y / WIERSZ];
    const [rx, ry] = [Math.ceil(sx * 2.4), Math.ceil(sy * 2.4)];
    const sila = new Float32Array((2 * rx + 1) * (2 * ry + 1));
    for (let dy = -ry; dy <= ry; dy++) {
      for (let dx = -rx; dx <= rx; dx++) {
        sila[(dy + ry) * (2 * rx + 1) + dx + rx] = Math.exp(-0.5 * ((dx / sx) ** 2 + (dy / sy) ** 2));
      }
    }
    pedzel = { rx, ry, sila };
  }

  // Tani, powtarzalny „los” dla komórki (stałe znaki bez Math.random w każdej klatce)
  const los = (x, y, t) => {
    const n = Math.sin(x * 12.9898 + y * 78.233 + t * 37.719) * 43758.5453;
    return n - Math.floor(n);
  };

  // Rozjaśnia poświatę wokół punktu okna. Jasność dochodzi do celu stopniowo, więc nic nie wyskakuje nagle.
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
  }

  // Jedna klatka poświaty: wszystko gaśnie w miejscu (nic się nie przesuwa), a pod myszką się odnawia
  function krokPoswiaty(ms) {
    const zostaje = Math.exp(-ms / GASNIE_MS);
    for (let i = 0; i < swiatlo.length; i++) swiatlo[i] *= zostaje;
    if (mysz) dotknij(mysz.x, mysz.y, 1);
  }

  function rysuj() {
    const t = czas / 1000;
    ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
    const linie = POZIOMY.map(() => new Array(kolumny));
    for (let y = 0; y < wiersze; y++) {
      for (const l of linie) l.fill(" ");
      // Fale: dwa powolne, krzyżujące się grzbiety, lekko pofalowane w pionie
      const fy = Math.sin(y * 0.19 + t * 0.21) * 1.6;
      const gy = Math.cos(y * 0.11 - t * 0.17);
      for (let x = 0; x < kolumny; x++) {
        const fala = Math.sin(x * 0.043 + fy + t * 0.33) * gy + Math.sin((x * 0.5 + y) * 0.052 - t * 0.27) * 0.8;
        // Fala daje najwyżej tyle jasności, żeby na grzbiecie pokazał się napis
        let jasnosc = Math.min(1, Math.max(0, (fala - 0.75) / 0.85)) * 0.31;
        const ziarno = los(x, y, 0);
        const s = swiatlo[y * kolumny + x];
        if (s > 0.02) {
          // Brzeg poświaty faluje powoli i jest lekko ziarnisty, więc nie wygląda jak koło od cyrkla
          const brzeg = 0.5 + 0.25 * Math.sin(x * 0.19 + y * 0.11 + t * 0.8) + 0.25 * Math.sin(y * 0.41 - x * 0.07 - t * 0.6);
          jasnosc = Math.max(jasnosc, s * (0.62 + 0.38 * brzeg) * (0.86 + 0.28 * ziarno));
        }
        // Im jaśniej, tym gęstszy znak: . : + napis, lekkie symbole, gęste symbole
        let poziom;
        let znak;
        if (jasnosc < 0.04) {
          poziom = 0;
          znak = ".";
        } else if (jasnosc < 0.15) {
          poziom = 1;
          znak = jasnosc < 0.09 ? ":" : "+";
        } else if (jasnosc < 0.32) {
          poziom = 2;
          znak = NAPIS[(x + y * 3) % NAPIS.length];
        } else {
          // Każda komórka co jakiś czas (we własnym rytmie) zmienia symbol — poświata lekko mieni się, nie miga
          const zmiana = Math.floor(t * 1.2 + ziarno * 7);
          const l = los(x, y, zmiana);
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
    if (teraz - ostatnia < KLATKA_MS - 2) return;
    // Po przerwie (okno było w tle) czas nie skacze do przodu
    const ms = Math.min(teraz - ostatnia, 100);
    czas += ms;
    ostatnia = teraz;
    krokPoswiaty(ms);
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
      mysz = null;
      rysuj();
    }
  }

  document.addEventListener("pointermove", (e) => {
    if (e.target.closest?.(BEZ_POSWIATY)) {
      mysz = null;
      return;
    }
    const nowa = { x: e.clientX, y: e.clientY };
    // Szybki ruch: poświata zostawia ciągły ślad, a nie pojedyncze plamy (dotknięcia co kilkanaście px)
    if (mysz && klatka) {
      const droga = Math.hypot(nowa.x - mysz.x, nowa.y - mysz.y);
      const kroki = Math.min(80, Math.floor(droga / 14));
      for (let i = 1; i <= kroki; i++) {
        const u = i / (kroki + 1);
        dotknij(mysz.x + (nowa.x - mysz.x) * u, mysz.y + (nowa.y - mysz.y) * u, 1);
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
