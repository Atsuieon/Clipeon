// Tło okna: siatka znaków ASCII. Przez okno płyną powolne fale napisu „CLIPEON”, a za myszką idzie
// „płomyk” gęstszych znaków, który unosi się i gaśnie. Płomyk znika, gdy mysz jest nad przyciskiem,
// polem ustawień albo klipem — tam ma być czytelnie.
// Rysowanie stoi, gdy okno jest w tle, schowane albo otwarty jest edytor (zero pracy podczas gry).
// Wtedy też <html> dostaje klasę „tlo-stoi”, która zatrzymuje zorzę na napisie „Clipeon” (style.css).
// Tła nie gasi systemowe „bez animacji” (prefers-reduced-motion): ma własny wyłącznik w Ustawieniach.
window.Tlo = (() => {
  const plotno = document.getElementById("tlo");
  const ctx = plotno.getContext("2d");
  const NAPIS = "CLIPEON ";
  const SYMBOLE = "$#%&@*/<>=+?";
  const CZCIONKA = 'bold 10px Consolas, "Cascadia Mono", "Courier New", monospace';
  const WIERSZ = 12; // wysokość komórki w px
  const KLATKA_MS = 1000 / 30;
  // Jasność znaków: kropki tła, brzeg fali, fala, płomyk, środek płomyka
  const POZIOMY = [0.09, 0.15, 0.24, 0.45, 0.7];
  // Nad tym płomyk nie idzie za myszką
  const BEZ_PLOMYKA =
    "button, a, input, select, textarea, label, summary, .karta, .menu, .nakladka, .belka, .baner, .pasek-zaznaczenia, .dymek, .gora";

  let wlaczone = true;
  let kolumna = 6.05; // szerokość znaku (mierzona po ustawieniu czcionki)
  let kolumny = 0;
  let wiersze = 0;
  let cieplo = new Float32Array(0);
  let nowe = new Float32Array(0);
  let mysz = null; // { x, y } w px okna albo null, gdy płomyk ma zgasnąć
  let ruch = 0; // jak szybko mysz się ostatnio ruszała (0–1)
  let klatka = 0; // numer zamówionej klatki animacji (0 = animacja stoi)
  let numer = 0; // która to klatka z kolei (ziarno poszarpanego brzegu płomyka)
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
    cieplo = new Float32Array(kolumny * wiersze);
    nowe = new Float32Array(kolumny * wiersze);
  }

  // Tani, powtarzalny „los” dla komórki (migotanie znaków bez Math.random w każdej klatce)
  const los = (x, y, t) => {
    const n = Math.sin(x * 12.9898 + y * 78.233 + t * 37.719) * 43758.5453;
    return n - Math.floor(n);
  };

  // Płomyk: ciepło wędruje w górę, rozlewa się na boki i stygnie
  function krokPlomyka() {
    const k = kolumny;
    for (let y = 0; y < wiersze; y++) {
      const dol = Math.min(y + 1, wiersze - 1) * k;
      const tu = y * k;
      for (let x = 0; x < k; x++) {
        const l = x > 0 ? x - 1 : x;
        const p = x < k - 1 ? x + 1 : x;
        const v = (cieplo[dol + l] + cieplo[dol + x] * 2 + cieplo[dol + p] + cieplo[tu + x] * 3) / 7;
        nowe[tu + x] = v * 0.962 - 0.003;
      }
    }
    [cieplo, nowe] = [nowe, cieplo];
    if (!mysz) return;
    // Źródło ciepła pod myszką: większe, gdy mysz się rusza
    const mx = Math.floor(mysz.x / kolumna);
    const my = Math.floor(mysz.y / WIERSZ);
    const r = 5 + Math.round(ruch * 6);
    for (let dy = -2; dy <= 2; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        const [x, y] = [mx + dx, my + dy];
        if (x < 0 || y < 0 || x >= kolumny || y >= wiersze) continue;
        // Poszarpany brzeg: każda komórka dostaje trochę inną porcję ciepła
        const sila = (1 - Math.abs(dx) / (r + 1)) * (1 - Math.abs(dy) / 3.2) * (0.6 + ruch * 0.4);
        const i = y * kolumny + x;
        cieplo[i] = Math.max(cieplo[i], sila * (0.55 + los(x, y, numer) * 0.45));
      }
    }
    ruch *= 0.9;
  }

  function rysuj() {
    const t = czas / 1000;
    ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
    const linie = POZIOMY.map(() => new Array(kolumny));
    const migot = Math.floor(t * 9);
    for (let y = 0; y < wiersze; y++) {
      for (const l of linie) l.fill(" ");
      // Fale: dwa powolne, krzyżujące się grzbiety, lekko pofalowane w pionie
      const fy = Math.sin(y * 0.19 + t * 0.21) * 1.6;
      const gy = Math.cos(y * 0.11 - t * 0.17);
      for (let x = 0; x < kolumny; x++) {
        const fala = Math.sin(x * 0.043 + fy + t * 0.33) * gy + Math.sin((x * 0.5 + y) * 0.052 - t * 0.27) * 0.8;
        // 0 poza falą … 1 na grzbiecie
        const otoczenie = Math.min(1, Math.max(0, (fala - 0.75) / 0.85));
        const zar = cieplo[y * kolumny + x];
        let poziom;
        let znak;
        if (zar > 0.42) {
          // Środek płomyka: gęste symbole
          poziom = zar > 0.62 ? 4 : 3;
          znak = SYMBOLE[Math.floor(los(x, y, migot) * SYMBOLE.length)];
        } else if (zar > 0.13) {
          // Płomyk: napis przetykany symbolami
          poziom = zar > 0.27 ? 3 : 2;
          const l = los(x, y, migot);
          znak = l < 0.3 ? SYMBOLE[Math.floor(l * 40) % SYMBOLE.length] : NAPIS[(x + y * 3) % NAPIS.length];
        } else if (otoczenie > 0.45) {
          poziom = 2;
          znak = NAPIS[(x + y * 3) % NAPIS.length];
        } else if (otoczenie > 0.1 || zar > 0.05) {
          poziom = 1;
          znak = otoczenie > 0.28 ? "+" : ":";
        } else {
          poziom = 0;
          znak = ".";
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
    czas += Math.min(teraz - ostatnia, 100);
    ostatnia = teraz;
    numer++;
    krokPlomyka();
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
      cieplo.fill(0);
      mysz = null;
      rysuj();
    }
  }

  document.addEventListener("pointermove", (e) => {
    if (e.target.closest?.(BEZ_PLOMYKA)) {
      mysz = null;
      return;
    }
    if (mysz) ruch = Math.min(1, ruch + Math.hypot(e.clientX - mysz.x, e.clientY - mysz.y) / 60);
    mysz = { x: e.clientX, y: e.clientY };
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
