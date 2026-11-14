// Jacaré nadando na superfície (1 ação, 1 direção), pixel art 64x64. Desenho original no estilo
// de uma referência do autor: vista de lado, cabeça fora d'água, corpo descendo na diagonal,
// o que está abaixo da linha d'água fica esverdeado-escuro.
// Uso (na pasta examples/): node demo-g-jacare/gerar.mjs > demo-g-jacare/jacare.json
//   PROJECT=<id> node ...  redesenha os frames de um projeto já aberto.

const PAL = [
  "#111a0e", // 0 contorno
  "#283b1c", // 1 verde sombra
  "#3e5a28", // 2 verde
  "#5a7b34", // 3 verde claro
  "#86a24a", // 4 brilho
  "#d2b25a", // 5 mandíbula creme
  "#9c8038", // 6 creme na sombra
  "#a9a457", // 7 ventre
  "#ec8b1c", // 8 olho
  "#130b05", // 9 pupila
  "#f3eedb", // 10 dentes
  "#08191c", // 11 contorno submerso
  "#1b3832", // 12 sombra submersa
  "#2a5040", // 13 verde submerso
  "#3d6a4f", // 14 claro submerso
  "#5a8660", // 15 brilho submerso
  "#6f8c62", // 16 ventre submerso
  "#8adcf2", // 17 água
  "#f4fcff", // 18 espuma
  "#3a8fb0", // 19 água funda
];
// Cor seca -> cor debaixo d'água.
const WET = { 0: 11, 1: 12, 2: 13, 3: 14, 4: 15, 5: 16, 6: 13, 7: 16, 8: 14, 9: 11, 10: 15 };

const WATER = 36;
const N = 8;
const R = Math.round;
const rad = (d) => (d * Math.PI) / 180;
const pt = (x, y) => [R(x), R(y)];

// Silhueta fixa da cabeça e do tronco (y sem o balanço).
const TOP = [[1, 28], [2, 26], [4, 25], [6, 26], [10, 25], [14, 24], [17, 22], [19, 20], [23, 20], [25, 21], [28, 23], [31, 24], [35, 26], [40, 28], [45, 31], [49, 34], [52, 37]];
const BOTTOM = [[48, 42], [44, 43], [38, 42], [32, 40], [27, 38], [24, 35], [20, 33], [12, 31], [5, 30], [2, 30], [1, 29]];
const MOUTH = [[2, 28], [8, 29], [14, 29], [19, 30], [22, 29], [24, 28]];
// Coluna da cauda: x, y, meia largura.
const TAIL = [[49, 38, 4], [52, 43, 3.2], [55, 48, 2.5], [57, 53, 1.8], [59, 57, 1.2], [60, 61, 0.5]];

const edgeY = (edge, x) => {
  const e = [...edge].sort((a, b) => a[0] - b[0]);
  for (let i = 0; i < e.length - 1; i++) {
    const [x0, y0] = e[i], [x1, y1] = e[i + 1];
    if (x >= x0 && x <= x1) return y0 + ((y1 - y0) * (x - x0)) / Math.max(1, x1 - x0);
  }
  return e[e.length - 1][1];
};

// Quadrilátero de um segmento com meias larguras w0 -> w1.
function quad(p0, p1, w0, w1) {
  const dx = p1[0] - p0[0], dy = p1[1] - p0[1];
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len, ny = dx / len;
  return [pt(p0[0] + nx * w0, p0[1] + ny * w0), pt(p1[0] + nx * w1, p1[1] + ny * w1), pt(p1[0] - nx * w1, p1[1] - ny * w1), pt(p0[0] - nx * w0, p0[1] - ny * w0)];
}

function tail(phi, b) {
  // Onda lateral que desce da base até a ponta (aparece como balanço em x).
  return TAIL.map(([x, y, w], i) => {
    const u = i / (TAIL.length - 1);
    return [x + 2.6 * u * Math.sin(phi - 1.4 * Math.PI * u), y + b, w];
  });
}

function body(frame, phi, b) {
  const L = (op) => ({ layer: "corpo", frame, ...op });
  const y = (v) => v + b;
  const sh = (pts) => pts.map(([px, py, ...c]) => [px, y(py), ...c]);
  const sp = tail(phi, b);
  const left = [], right = [];
  for (let i = 0; i < sp.length; i++) {
    const a = sp[Math.max(0, i - 1)], c = sp[Math.min(sp.length - 1, i + 1)];
    const dx = c[0] - a[0], dy = c[1] - a[1], len = Math.hypot(dx, dy);
    const nx = -dy / len, ny = dx / len, w = sp[i][2];
    left.push(pt(sp[i][0] + nx * w, sp[i][1] + ny * w));
    right.push(pt(sp[i][0] - nx * w, sp[i][1] - ny * w));
  }
  const ops = [
    L({ op: "polygon", points: [...sh(TOP), ...sh(BOTTOM)], color: 2 }),
    L({ op: "polygon", points: [...left, ...right.reverse()], color: 2 }),
    // Volume: luz de cima.
    L({ op: "shade", light: "top", highlight: 3, shadow: 1, highlight_depth: 2, shadow_depth: 2 }),
    L({ op: "shade", light: "top", highlight: 4, highlight_depth: 1, shadow: 0, shadow_depth: 0, rect: { x: 0, y: 0, w: 30, h: 64 } }),
  ];
  // Escamas do dorso (silhueta serrilhada) e da crista da cauda.
  const bumps = [];
  for (let x = 28; x <= 50; x += 3) {
    const t = R(edgeY(TOP, x));
    bumps.push([x, y(t) - 1, 1], [x + 1, y(t) - 1, 3]);
  }
  for (let i = 0; i < sp.length - 2; i++) {
    for (const k of [0.3, 0.8]) {
      const a = sp[i], c = sp[i + 1];
      const cx = a[0] + (c[0] - a[0]) * k, cy = a[1] + (c[1] - a[1]) * k, w = a[2] + (c[2] - a[2]) * k;
      const dx = c[0] - a[0], dy = c[1] - a[1], len = Math.hypot(dx, dy);
      bumps.push([R(cx + (dy / len) * (w + 0.8)), R(cy - (dx / len) * (w + 0.8)), 1]);
    }
  }
  ops.push(L({ op: "pixels", pixels: bumps }));
  // Osteodermos: três fileiras de placas paralelas ao dorso (a de baixo fica no flanco submerso).
  const plates = [];
  for (let x = 27; x <= 48; x += 3) {
    const t = R(edgeY(TOP, x));
    plates.push([x, y(t + 2), 3], [x, y(t + 3), 1], [x + 1, y(t + 5), 3], [x + 1, y(t + 6), 1], [x + 2, y(t + 8), 3], [x + 2, y(t + 9), 1]);
  }
  ops.push(L({ op: "pixels", pixels: plates }));
  // Ventre creme com divisões.
  const belly = [];
  for (let x = 26; x <= 46; x++) {
    const bt = R(edgeY(BOTTOM, x));
    belly.push([x, y(bt) - 1, x % 3 === 0 ? 6 : 7], [x, y(bt) - 2, x % 3 === 0 ? 1 : 2]);
  }
  ops.push(L({ op: "pixels", pixels: belly }));
  // Mandíbula creme e fina, boca com dentes brancos sobre a linha escura, narina.
  ops.push(
    L({ op: "polygon", points: sh([[2, 29], ...MOUTH.slice(1, -1), [22, 30], [21, 33], [12, 31], [5, 30]]), color: 5 }),
    L({ op: "polyline", points: sh([[3, 30], [12, 31], [21, 33]]), color: 6 }),
    L({ op: "polyline", points: sh(MOUTH), color: 0 }),
    L({ op: "pixels", pixels: sh([[4, 28], [7, 29], [7, 30], [10, 29], [13, 29], [16, 30], [16, 31], [19, 30]]), color: 10 }),
    L({ op: "pixel", x: 3, y: y(26), color: 0 }),
    L({ op: "pixel", x: 4, y: y(25), color: 4 }),
    // Olho: pálpebra clara em cima, íris laranja, pupila em fenda, pele escura atrás.
    L({ op: "pixels", pixels: sh([[20, 20, 4], [21, 20, 4], [22, 20, 3], [20, 21, 8], [21, 21, 9], [22, 21, 8], [21, 22, 1], [24, 22, 1], [25, 23, 1]]) }),
  );
  ops.push(L({ op: "outline", color: 0 }));
  return ops;
}

// Perna com dois segmentos e dedos; ângulos em graus a partir da vertical (+ para trás).
function limb(layer, frame, root, a1, a2, l1, l2, w, color, toes) {
  const L = (op) => ({ layer, frame, ...op });
  const e = [root[0] + l1 * Math.sin(rad(a1)), root[1] + l1 * Math.cos(rad(a1))];
  const h = [e[0] + l2 * Math.sin(rad(a2)), e[1] + l2 * Math.cos(rad(a2))];
  const ops = [
    L({ op: "polygon", points: quad(root, e, w, w * 0.75), color }),
    L({ op: "polygon", points: quad(e, h, w * 0.65, w * 0.45), color }),
    L({ op: "circle", center: pt(...e), radius: Math.max(1, R(w * 0.6)), color, fill: true }),
  ];
  // Pé espalmado: dedos em leque.
  for (const s of [-35, 0, 35].slice(0, toes)) {
    const a = rad(a2 + s);
    ops.push(L({ op: "line", from: pt(...h), to: pt(h[0] + 3 * Math.sin(a), h[1] + 3 * Math.cos(a)), color }));
  }
  return ops;
}

// Remada: para trás com a perna esticada, volta dobrada.
function stroke(ph, base, amp) {
  const a1 = base + amp * Math.sin(ph);
  return [a1, a1 + 35 - 25 * Math.cos(ph)];
}

function legs(frame, phi, b) {
  const far = [], near = [];
  const [fa, fb] = stroke(phi + Math.PI, 15, 40);
  const [ha, hb] = stroke(phi, 10, 35);
  far.push(...limb("pernas_tras", frame, [31, 37 + b], fa, fb, 4, 4, 1.6, 1, 3));
  far.push(...limb("pernas_tras", frame, [48, 40 + b], ha, hb, 4, 5, 1.8, 1, 3));
  const [na, nb] = stroke(phi, 15, 45);
  const [ka, kb] = stroke(phi + Math.PI, 10, 35);
  near.push(...limb("pernas_frente", frame, [27, 37 + b], na, nb, 5, 5, 2, 3, 3));
  near.push(...limb("pernas_frente", frame, [44, 41 + b], ka, kb, 5, 5, 2.2, 3, 3));
  near.push({ layer: "pernas_frente", frame, op: "shade", light: "top", highlight: 4, shadow: 2, highlight_depth: 1, shadow_depth: 1 });
  far.push({ layer: "pernas_tras", frame, op: "outline", color: 0 });
  // Contorno verde-escuro: separa a perna do corpo sem riscar o flanco de preto.
  near.push({ layer: "pernas_frente", frame, op: "outline", color: 1 });
  return [...far, ...near];
}

function water(frame, t) {
  const L = (op) => ({ layer: "agua", frame, ...op });
  // Superfície tracejada que corre para trás (o jacaré nada para a esquerda).
  const line = [];
  for (let x = 0; x < 64; x++) {
    const k = (((x - 2 * t) % 16) + 16) % 16;
    if (k < 11) line.push([x, WATER, k < 3 ? 18 : 17]);
  }
  const ops = [L({ op: "pixels", pixels: line })];
  // Espuma onde o peito corta a água e onde o dorso afunda.
  const bow = t % 2 === 0 ? [[15, 35], [18, 35], [22, 35]] : [[16, 35], [20, 35], [23, 35]];
  ops.push(L({ op: "line", from: [14, WATER], to: [26, WATER], color: 18 }));
  ops.push(L({ op: "pixels", pixels: bow, color: 17 }));
  ops.push(L({ op: "line", from: [48, WATER], to: [55, WATER], color: 18 }));
  ops.push(L({ op: "pixels", pixels: t % 2 === 0 ? [[50, 35], [54, 35]] : [[51, 35], [53, 35]], color: 17 }));
  // Marolas logo abaixo da superfície.
  ops.push(L({ op: "pixels", pixels: [[(t * 2) % 12, WATER + 1], [(t * 2 + 6) % 12, WATER + 1], [56 + ((t * 2) % 8), WATER + 1]], color: 19 }));
  return ops;
}

function submerge(layer, frame) {
  return Object.entries(WET).map(([from, to]) => ({ layer, frame, op: "replace_color", from: Number(from), to, rect: { x: 0, y: WATER + 1, w: 64, h: 63 - WATER } }));
}

const BOB = [0, 0, 1, 1, 1, 0, 0, 0];
const LAYERS = ["pernas_tras", "corpo", "pernas_frente", "agua"];
const PROJECT = process.env.PROJECT;
const steps = PROJECT
  ? []
  : [
      { tool: "project_create", args: { name: "Jacaré", width: 64, height: 64, sprite_name: "jacare" } },
      { tool: "palette_create", args: { name: "Jacaré no rio", colors: PAL } },
      { tool: "layer_properties", args: { layer: "Layer 1", name: "pernas_tras" } },
      { tool: "layer_create", args: { name: "corpo" } },
      { tool: "layer_create", args: { name: "pernas_frente" } },
      { tool: "layer_create", args: { name: "agua" } },
      { tool: "animation_create", args: { name: "nadar", directions: ["W"], frames: N, duration_ms: 100, loop_mode: "loop" } },
      { tool: "animation_update", args: { animation: "nadar", cycle: { ground_y: WATER, notes: "Nado na superfície (linha d'água em y=36). Pernas em pares diagonais: remam para trás esticadas e voltam dobradas. A cauda ondula com a onda descendo até a ponta; a superfície corre para trás." } } },
    ];
for (let t = 0; t < N; t++) {
  const frame = { animation: "nadar", direction: "W", index: t };
  const phi = (t / N) * Math.PI * 2;
  steps.push({
    tool: "draw_batch",
    args: {
      label: `nadar ${t}`,
      ops: [
        ...LAYERS.map((layer) => ({ layer, frame, op: "clear" })),
        ...body(frame, phi, BOB[t]),
        ...legs(frame, phi, BOB[t]),
        ...["pernas_tras", "corpo", "pernas_frente"].flatMap((l) => submerge(l, frame)),
        // Reflexos da superfície sobre o flanco submerso, correndo para trás.
        { layer: "corpo", frame, op: "pattern", rows: ["aa..............", "................", "........a.......", "................"], legend: { a: 15 }, offset: [2 * t, 0], only_opaque: true, rect: { x: 26, y: WATER + 2, w: 26, h: 4 } },
        ...water(frame, t),
      ],
    },
  });
}
steps.push(
  { tool: "animation_preview", args: { animation: "nadar", direction: "W", scale: 3 }, save_as: "g1-nadar-strip" },
  { tool: "frame_preview", args: { frame: { animation: "nadar", direction: "W", index: 0 }, scale: 8 }, save_as: "g2-frame0" },
  { tool: "frame_preview", args: { frame: { animation: "nadar", direction: "W", index: 4 }, scale: 8 }, save_as: "g3-frame4" },
  { tool: "project_validate", args: {} },
  { tool: "export_gif", args: { path: "demo-g-jacare/jacare_nadar.gif", animation: "nadar", direction: "W", scale: 4 } },
  { tool: "export_spritesheet", args: { path: "demo-g-jacare/jacare_nadar.png", selection: { animations: ["nadar"] }, sheet: { layout: "horizontal" } } },
  { tool: "project_save", args: { path: "demo-g-jacare/jacare.psmproj" } },
);
if (PROJECT) for (const s of steps) s.args = { project: PROJECT, ...s.args };
console.log(JSON.stringify(steps, null, 1));
