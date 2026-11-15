// Cenário em parallax (320x180, entardecer): 7 camadas que se emendam na horizontal, para rolar sem fim.
// A luz de borda (shade) usa wrap: true, senão a última coluna acende e vira uma linha em cada emenda.
// Cada camada é uma layer do sprite "cenario"; a exportação gera um PNG por camada.
// Uso (na pasta examples/): node demo-h-parallax/gerar.mjs > demo-h-parallax/cenario.json
//   PROJECT=<id> node ...  redesenha as camadas de um projeto já aberto.

const W = 320;
const H = 180;
const R = Math.round;
const TAU = Math.PI * 2;

// Paleta de entardecer: o que está longe fica mais claro e mais roxo (perspectiva atmosférica).
const C = {
  ceu: ["#1b1b3a", "#2e2459", "#4b2c6e", "#73306f", "#a33d6b", "#d4566a", "#ee7f5d", "#f8aa5f", "#fdd27b"],
  estrela: "#fff4d6",
  estrela2: "#b9a6d9",
  sol: "#fff1b8",
  solAro: "#ffe08f",
  nuvem: "#c4637a",
  nuvemLuz: "#f39b88",
  nuvemSombra: "#8a426f",
  longe: "#7a4f88",
  longeNevoa: "#9a6496",
  longeLuz: "#bf7a9c",
  perto: "#4a3768",
  pertoNevoa: "#5d4277",
  pertoLuz: "#8a5a8c",
  pertoVinco: "#3d2d58",
  florLonge: "#2c2a4f",
  florLongeLuz: "#4f4673",
  florPerto: "#1d1d38",
  florPertoLuz: "#3a3358",
  grama: "#2f5a3e",
  gramaLuz: "#4b7a4a",
  gramaBrilho: "#79a85a",
  terra: "#4a3326",
  terraClara: "#5e4130",
  terraEscura: "#33231b",
  pedra: "#6d6a80",
  pedraLuz: "#8a87a0",
  florAmarela: "#f2c94c",
  florRosa: "#e07a9a",
};
const PAL = [...new Set(Object.values(C).flat())];

// Gerador pseudoaleatório com semente: o cenário sai sempre igual.
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Relevo periódico: só senos com número inteiro de ciclos na largura, então y(0) == y(W).
function perfil(base, termos) {
  return (x) => base - termos.reduce((s, [amp, ciclos, fase, crista]) => {
    const v = Math.sin((TAU * ciclos * x) / W + fase);
    return s + amp * (crista ? 1 - Math.abs(v) * 2 + 1 : v) * (crista ? 0.5 : 1);
  }, 0);
}

function silhueta(y) {
  const pts = [];
  for (let x = 0; x <= W; x += 2) pts.push([x, R(y(x))]);
  return [...pts, [W, H], [0, H]];
}

// Repete uma forma deslocada de -W e +W, para que o que sai de um lado entre pelo outro.
const emVolta = (fn) => [-W, 0, W].flatMap((dx) => fn(dx));

// Pinheiro em camadas: copa em "andares" serrilhados e tronco curto.
function pinheiro(cx, base, h, w) {
  const andares = 4;
  const topo = base - h;
  const dir = [];
  for (let i = 1; i <= andares; i++) {
    const ty = topo + (h - 3) * (i / andares);
    const tw = (w / 2) * (0.35 + 0.65 * (i / andares));
    dir.push([cx + tw, ty], [cx + tw * 0.45, ty - 1]);
  }
  dir.pop();
  const esq = dir.map(([x, y]) => [2 * cx - x, y]).reverse();
  return [[cx, topo], ...dir, [cx + 1, base], [cx - 1, base], ...esq];
}

function camadaCeu(frame) {
  const L = (op) => ({ layer: "ceu", frame, ...op });
  const r = rng(7);
  const ops = [
    L({ op: "gradient", colors: C.ceu, shape: { type: "linear", angle: 90 }, dither: "bayer4", rect: { x: 0, y: 0, w: W, h: 140 } }),
    L({ op: "rect", x: 0, y: 140, w: W, h: 40, color: C.ceu.at(-1), fill: true }),
  ];
  const estrelas = [];
  for (let i = 0; i < 46; i++) {
    const x = Math.floor(r() * W);
    const y = Math.floor(r() * r() * 52);
    estrelas.push([x, y, r() < 0.3 ? C.estrela : C.estrela2]);
  }
  ops.push(L({ op: "pixels", pixels: estrelas }));
  // Sol baixo, atrás das montanhas, com um aro de brilho.
  ops.push(L({ op: "circle", center: [232, 118], radius: 17, color: C.solAro, fill: true }));
  ops.push(L({ op: "circle", center: [232, 118], radius: 14, color: C.sol, fill: true }));
  return ops;
}

function camadaNuvens(frame) {
  const L = (op) => ({ layer: "nuvens", frame, ...op });
  const r = rng(11);
  const ops = [];
  const nuvens = [
    [18, 62, 74, 9], [104, 40, 52, 7], [166, 70, 96, 11], [250, 48, 62, 8], [292, 78, 48, 6], [60, 88, 40, 5], [212, 92, 56, 6],
  ];
  for (const [x, y, w, h] of nuvens) {
    ops.push(...emVolta((dx) => {
      const partes = [L({ op: "ellipse", x: x + dx, y, w, h, color: C.nuvem, fill: true })];
      // Bolhas por cima para quebrar a forma.
      for (let k = 0; k < 3; k++) {
        const bw = R(w * (0.3 + r() * 0.25));
        const bx = R(x + dx + (w - bw) * (0.15 + 0.7 * r()));
        partes.push(L({ op: "ellipse", x: bx, y: y - R(h * 0.45), w: bw, h: R(h * 0.9), color: C.nuvem, fill: true }));
      }
      return partes;
    }));
  }
  // Iluminadas por baixo (o sol está baixo) e mais escuras em cima.
  ops.push(L({ op: "shade", light: "bottom", highlight: C.nuvemLuz, shadow: C.nuvemSombra, highlight_depth: 1, shadow_depth: 1, wrap: true }));
  return ops;
}

function camadaMontanhasLonge(frame) {
  const L = (op) => ({ layer: "montanhas_longe", frame, ...op });
  const y = perfil(122, [[13, 1, 0.5], [9, 3, 1.3], [7, 5, 0.2, true], [2.5, 11, 2.1]]);
  return [
    L({ op: "polygon", points: silhueta(y), color: C.longe }),
    // Pé das montanhas sumindo na névoa.
    L({ op: "gradient", colors: [C.longe, C.longe, C.longeNevoa], shape: { type: "linear", angle: 90 }, dither: "bayer4", only_opaque: true, rect: { x: 0, y: 84, w: W, h: H - 84 } }),
    L({ op: "shade", light: "top_right", highlight: C.longeLuz, highlight_depth: 1, wrap: true }),
  ];
}

function camadaMontanhasPerto(frame) {
  const L = (op) => ({ layer: "montanhas_perto", frame, ...op });
  const y = perfil(134, [[11, 2, 0.9, true], [7, 4, 2.0], [4, 7, 0.4], [2, 13, 1.1]]);
  const ops = [
    L({ op: "polygon", points: silhueta(y), color: C.perto }),
    L({ op: "gradient", colors: [C.perto, C.perto, C.pertoNevoa], shape: { type: "linear", angle: 90 }, dither: "bayer4", only_opaque: true, rect: { x: 0, y: 104, w: W, h: H - 104 } }),
  ];
  // Vincos descendo dos picos (lado da sombra) para dar volume.
  for (let x = 2; x < W; x += 2) {
    const a = y(x - 2), b = y(x), c = y(x + 2);
    if (b < a && b <= c && b < 132) {
      const yy = R(b);
      ops.push(...emVolta((dx) => [L({ op: "line", from: [x + dx, yy + 2], to: [x + dx - 9, yy + 16], color: C.pertoVinco })]));
    }
  }
  ops.push(L({ op: "shade", light: "top_right", highlight: C.pertoLuz, highlight_depth: 1, wrap: true }));
  return ops;
}

function floresta(layer, frame, seed, base, hMin, hMax, passo, cor, luz, falhas = 0) {
  const L = (op) => ({ layer, frame, ...op });
  const r = rng(seed);
  const ops = [L({ op: "rect", x: 0, y: base - 2, w: W, h: H - base + 2, color: cor, fill: true })];
  for (let x = 0; x < W; x += passo * (0.7 + r() * 0.6)) {
    const pula = r() < falhas;
    const h = hMin + r() * (hMax - hMin);
    const w = h * (0.42 + r() * 0.12);
    const cx = R(x);
    const b = base + R(r() * 3);
    if (pula) continue;
    ops.push(...emVolta((dx) => [L({ op: "polygon", points: pinheiro(cx + dx, b, R(h), w).map(([px, py]) => [R(px), R(py)]), color: cor })]));
  }
  // Luz de borda do lado do sol (direita).
  ops.push(L({ op: "shade", light: "top_right", highlight: luz, highlight_depth: 1, wrap: true }));
  return ops;
}

function camadaChao(frame) {
  const L = (op) => ({ layer: "chao", frame, ...op });
  const r = rng(23);
  const topo = (x) => 164 + R(Math.sin((TAU * 4 * x) / W) * 0.6 + Math.sin((TAU * 9 * x) / W + 1) * 0.6);
  const ops = [
    L({ op: "rect", x: 0, y: 166, w: W, h: 14, color: C.terra, fill: true }),
    L({ op: "gradient", colors: [C.terraClara, C.terra, C.terraEscura], shape: { type: "linear", angle: 90 }, dither: "bayer4", only_opaque: true, rect: { x: 0, y: 168, w: W, h: 12 } }),
  ];
  // Faixa de grama com borda irregular e a luz no alto.
  const grama = [];
  for (let x = 0; x < W; x++) {
    const t = topo(x);
    for (let y = t; y <= 168; y++) grama.push([x, y, y === t ? C.gramaLuz : C.grama]);
    // Grama pendurada sobre a terra.
    if (r() < 0.35) grama.push([x, 169, C.grama]);
    if (r() < 0.12) grama.push([x, 170, C.grama]);
  }
  ops.push(L({ op: "pixels", pixels: grama }));
  // Tufos, flores e pedras (com volta nas bordas).
  const enfeites = [];
  for (let i = 0; i < 70; i++) {
    const x = Math.floor(r() * W);
    const t = topo(x);
    const alto = 1 + Math.floor(r() * 3);
    for (let k = 1; k <= alto; k++) enfeites.push([x, t - k, k === alto ? C.gramaBrilho : C.gramaLuz]);
  }
  for (let i = 0; i < 9; i++) {
    const x = Math.floor(r() * W);
    enfeites.push([x, topo(x) - 1, r() < 0.5 ? C.florAmarela : C.florRosa]);
  }
  ops.push(L({ op: "pixels", pixels: enfeites }));
  for (let i = 0; i < 7; i++) {
    const x = Math.floor(r() * W);
    const y = 171 + Math.floor(r() * 6);
    const w = 3 + Math.floor(r() * 4);
    ops.push(...emVolta((dx) => [
      L({ op: "ellipse", x: x + dx, y, w, h: 3, color: C.pedra, fill: true }),
      L({ op: "line", from: [x + dx + 1, y], to: [x + dx + w - 2, y], color: C.pedraLuz }),
    ]));
  }
  return ops;
}

// Ordem de baixo para cima, com a velocidade de cada camada no parallax (1 = anda junto com o chão).
export const CAMADAS = [
  ["ceu", 0.0, camadaCeu],
  ["nuvens", 0.1, camadaNuvens],
  ["montanhas_longe", 0.2, camadaMontanhasLonge],
  ["montanhas_perto", 0.35, camadaMontanhasPerto],
  ["floresta_longe", 0.55, (f) => floresta("floresta_longe", f, 31, 152, 12, 26, 6, C.florLonge, C.florLongeLuz, 0.1)],
  ["floresta_perto", 0.75, (f) => floresta("floresta_perto", f, 37, 168, 22, 48, 19, C.florPerto, C.florPertoLuz, 0.3)],
  ["chao", 1.0, camadaChao],
];

if (import.meta.url === `file://${process.argv[1].replace(/\\/g, "/")}` || process.argv[1]?.endsWith("gerar.mjs")) {
  const PROJECT = process.env.PROJECT;
  const steps = PROJECT
    ? []
    : [
        { tool: "project_create", args: { name: "Cenário parallax", width: W, height: H, sprite_name: "cenario" } },
        { tool: "palette_create", args: { name: "Entardecer", colors: PAL } },
        { tool: "layer_properties", args: { layer: "Layer 1", name: CAMADAS[0][0] } },
        ...CAMADAS.slice(1).map(([name]) => ({ tool: "layer_create", args: { name } })),
      ];
  const frame = undefined;
  for (const [name, , fn] of CAMADAS) {
    steps.push({ tool: "draw_batch", args: { label: `camada ${name}`, ops: [{ layer: name, op: "clear" }, ...fn(frame)] } });
  }
  steps.push({ tool: "frame_preview", args: { scale: 4 }, save_as: "p1-cenario" });
  for (const [i, [name]] of CAMADAS.entries()) {
    steps.push({ tool: "export_png", args: { path: `demo-h-parallax/godot/camadas/${String(i + 1).padStart(2, "0")}_${name}.png`, layers: [name] } });
  }
  steps.push({ tool: "export_png", args: { path: "demo-h-parallax/cenario.png" } });
  steps.push({ tool: "export_png", args: { path: "demo-h-parallax/cenario_x3.png", scale: 3 } });
  steps.push({ tool: "project_save", args: { path: "demo-h-parallax/cenario_parallax.psmproj" } });
  if (PROJECT) for (const s of steps) s.args = { project: PROJECT, ...s.args };
  console.log(JSON.stringify(steps, null, 1));
}
