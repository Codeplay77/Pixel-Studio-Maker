// Demonstração D: as 8 direções da caminhada do guerreiro.
// Desenha S, N, SE e NE do zero (mesma paleta e proporções da vista E), e monta W, SW e NW a partir
// dos espelhos de E, SE e NE com o equipamento refeito (o escudo continua no braço esquerdo).
// Uso (na pasta examples/): node demo-d-direcoes/gerar.mjs > demo-d-direcoes/direcoes.json
//                           psm mcp-run demo-d-direcoes/direcoes.json --images demo-d-direcoes/saida

const LEG = { // fases da caminhada vista de frente/costas: quanto cada pé sobe e o balanço do corpo
  liftL: [0, 1, 2, 1, 0, 0, 0, 0],
  liftR: [0, 0, 0, 0, 0, 1, 2, 1],
  bob: [0, 1, -1, 0, 0, 1, -1, 0],
  swing: [-1, -1, 0, 1, 1, 1, 0, -1], // balanço lateral dos braços/equipamento
  stride: [1, 1, 0, -1, -1, -1, 0, 1], // passada diagonal nas vistas 3/4
};

const HELMET = {
  S: [
    "......ooooo......", "....ooLLLLLoo....", "...oLHHLLLLLBo...", "..oLHHLLLLLLBBo..", "..oLHLLLLLLLBBo..",
    ".oLLLLLLLLLLBBDo.", ".oBLLLLLLLLLBBDo.", ".oBBBBBBBBBBBBDo.", "oDBoooooSooooBDDo", "oDBoKKKKSKKKKoBDo",
    "oDBoKeKKSKKeKoBDo", "oDBoKKKKSKKKKoBDo", "oDBokKKKKKKKkoBDo", ".oDBokkKKKkkoBDo.", ".oDBBoooooooBBDo.",
    "..oDBBBBBBBBBDo..", "...ooDDDDDDDoo...", ".....ooooooo.....",
  ],
  N: [
    "......ooooo......", "....ooLLLLLoo....", "...oLHHLLLLLBo...", "..oLHHLLLLLLBBo..", "..oLHLLLLLLLBBo..",
    ".oLLLLLLLLLLBBDo.", ".oBLLLLLLLLLBBDo.", ".oBBBBBBBBBBBBDo.", "oDBBBBBBBBBBBBBDo", "oDBBBBBBBBBBBBBDo",
    "oDBBBBBBDBBBBBBDo", "oDBBBBBBDBBBBBBDo", "oDDBBBBBDBBBBBDDo", ".oDDBBBBDBBBBDDo.", ".oDDDBBBDBBBDDDo.",
    "..oDDDDDDDDDDDo..", "...ooDDDDDDDoo...", ".....ooooooo.....",
  ],
  SE: [
    "......ooooo......", "....ooLLLLLoo....", "...oLHHLLLLLLo...", "..oLHHLLLLLLLBo..", "..oLHLLLLLLLLBo..",
    ".oLLLLLLLLLLLBBo.", ".oBLLLLLLLLLLBBo.", ".oBBBBBBBBBBBBBo.", "oDBBBBoooooSoooBo", "oDBBBoKKKKKSKKKo.",
    "oDBBBoKKeKKSKeKo.", "oDBBBoKKKKKSKKKKo", "oDDBBokKKKKKKKKo.", ".oDDBBokkKKKKko..", ".oDDDBBoooooooo..",
    "..oDDDBBBBBBBo...", "...ooDDDDDDoo....", ".....oooooo......",
  ],
  NE: [
    "......ooooo......", "....ooLLLLLoo....", "...oLHHLLLLLBo...", "..oLHHLLLLLLBBo..", "..oLHLLLLLLLBBo..",
    ".oLLLLLLLLLLBBDo.", ".oBLLLLLLLLLBBDo.", ".oBBBBBBBBBBBBDo.", "oDBBBBBBBBBBBBDDo", "oDBBBBBBBBBBBBBoo",
    "oDBBBBDBBBBBBBBoo", "oDBBBBDBBBBBBBBDo", "oDDBBBDBBBBBBBDDo", ".oDDBBDBBBBBBDDo.", ".oDDDBDBBBBBDDDo.",
    "..oDDDDDDDDDDDo..", "...ooDDDDDDDoo...", ".....ooooooo.....",
  ],
};
const HELMET_LEGEND = { o: 0, D: 1, B: 2, L: 3, H: 4, s: 5, S: 6, K: 9, k: 8, e: 0 };

function views(view, i) {
  const frame = { animation: "walk", direction: view, index: i };
  const bob = LEG.bob[i];
  const sw = LEG.swing[i];
  const three = view === "SE" || view === "NE";
  const back = view === "N" || view === "NE";
  const st = three ? LEG.stride[i] : 0;
  const cx = three ? 33 : 32; // centro do corpo
  const ops = [];
  const L = (layer, op) => ops.push({ layer, frame, ...op });
  for (const layer of ["sombra", "perna_tras", "escudo_tras", "espada", "perna_frente", "corpo", "pluma", "cabeca", "escudo", "espada_frente"]) L(layer, { op: "clear" });

  L("sombra", { op: "ellipse", x: cx - 13, y: 57, w: 26, h: 4, color: 16, fill: true });

  // Pernas: perna_frente = esquerda da tela, perna_tras = direita (nas 3/4, a de trás é mais escura).
  const leg = (layer, x0, lift, dx, dark) => {
    const hip = 45 + bob, ankle = 55 - lift * 2, knee = 49 - lift + Math.min(0, bob);
    const base = dark ? 1 : 2, hl = dark ? 2 : 3;
    L(layer, { op: "rect", x: x0 + dx, y: hip, w: 5, h: ankle - hip, color: base, fill: true });
    L(layer, { op: "line", from: [x0 + dx, hip], to: [x0 + dx, ankle - 1], color: hl });
    L(layer, { op: "line", from: [x0 + dx + 4, hip], to: [x0 + dx + 4, ankle - 1], color: 1 });
    L(layer, { op: "rect", x: x0 + dx + 1, y: knee, w: 3, h: 2, color: dark ? 5 : 6, fill: true });
    L(layer, { op: "pixel", x: x0 + dx + 1, y: knee, color: dark ? 6 : 7 });
    if (back) {
      L(layer, { op: "rect", x: x0 + dx - 1, y: ankle, w: 7, h: 4, color: dark ? 10 : 11, fill: true });
      L(layer, { op: "line", from: [x0 + dx, ankle], to: [x0 + dx + 4, ankle], color: 10 });
    } else {
      L(layer, { op: "polygon", points: [[x0 + dx - 1, ankle], [x0 + dx + 5, ankle], [x0 + dx + 6, ankle + 2], [x0 + dx + 6, ankle + 3], [x0 + dx - 1, ankle + 3]], color: dark ? 10 : 11 });
      L(layer, { op: "line", from: [x0 + dx, ankle], to: [x0 + dx + 4, ankle], color: dark ? 11 : 17 });
    }
    L(layer, { op: "outline", color: 0 });
  };
  leg("perna_frente", cx - 6, LEG.liftL[i], st, false);
  leg("perna_tras", cx + 2, LEG.liftR[i], -st, three);

  // Corpo: tronco, cinto, saiote, ombreiras.
  const y = (v) => v + bob;
  const tx = cx - 6;
  L("corpo", { op: "rect", x: tx, y: y(28), w: 13, h: 13, color: 2, fill: true });
  L("corpo", { op: "line", from: [tx, y(29)], to: [tx, y(40)], color: 3 });
  L("corpo", { op: "line", from: [tx + 12, y(29)], to: [tx + 12, y(40)], color: 1 });
  if (back) L("corpo", { op: "line", from: [cx, y(29)], to: [cx, y(40)], color: 1 });
  else L("corpo", { op: "rect", x: tx + 2, y: y(30), w: 2, h: 2, color: 4, fill: true });
  L("corpo", { op: "rect", x: tx - 1, y: y(41), w: 15, h: 3, color: 11, fill: true });
  L("corpo", { op: "line", from: [tx - 1, y(41)], to: [tx + 13, y(41)], color: 17 });
  if (!back) L("corpo", { op: "rect", x: cx - 1, y: y(41), w: 3, h: 3, color: 13, fill: true });
  L("corpo", { op: "polygon", points: [[tx - 2, y(44)], [tx + 14, y(44)], [tx + 15, y(48)], [tx - 3, y(48)]], color: 2 });
  L("corpo", { op: "line", from: [tx - 2, y(46)], to: [tx + 14, y(46)], color: 1 });
  L("corpo", { op: "line", from: [tx - 2, y(44)], to: [tx + 13, y(44)], color: 3 });
  for (const [px, hl] of [[tx - 5, true], [tx + 10, false]]) {
    L("corpo", { op: "ellipse", x: px, y: y(26), w: 8, h: 8, color: 2, fill: true });
    L("corpo", { op: "ellipse", x: px + 1, y: y(27), w: 4, h: 3, color: hl ? 3 : 2, fill: true });
    L("corpo", { op: "line", from: [px + 1, y(33)], to: [px + 6, y(33)], color: 1 });
  }
  L("corpo", { op: "outline", color: 0 });

  // Capacete (matriz desenhada à mão) e penacho.
  L("cabeca", { op: "matrix", at: [cx - 8, y(11)], legend: HELMET_LEGEND, rows: HELMET[view] });
  const px = cx + (three ? -2 : 0);
  L("pluma", { op: "line", from: [px, y(10)], to: [px, y(5)], color: 15, thickness: 3 });
  L("pluma", { op: "curve", points: [[px, y(4)], [px - 3, y(2)], [px - 7, y(4)]], color: 15, thickness: 2 });
  L("pluma", { op: "line", from: [px + 1, y(6)], to: [px + 1, y(9)], color: 14 });
  L("pluma", { op: "rect", x: px - 1, y: y(10), w: 3, h: 1, color: 13, fill: true });
  L("pluma", { op: "outline", color: 0 });

  // Escudo (braço esquerdo do guerreiro) e espada (braço direito).
  const kite = (layer, x0, w, top, face) => {
    const right = x0 + w - 1, mid = x0 + Math.floor(w / 2);
    L(layer, { op: "polygon", points: [[x0, top], [right, top], [right, top + 10], [mid, top + 20], [x0, top + 10]], color: 6 });
    L(layer, { op: "polygon", points: [[x0 + 1, top + 1], [right - 1, top + 1], [right - 1, top + 10], [mid, top + 18], [x0 + 1, top + 10]], color: face ? 2 : 11 });
    if (face) {
      L(layer, { op: "line", from: [right - 2, top + 2], to: [right - 2, top + 10], color: 1 });
      L(layer, { op: "line", from: [x0 + 2, top + 2], to: [x0 + 2, top + 10], color: 3 });
      L(layer, { op: "line", from: [mid, top + 3], to: [mid, top + 15], color: 13 });
      L(layer, { op: "line", from: [mid - 3, top + 7], to: [mid + 3, top + 7], color: 13 });
    } else {
      L(layer, { op: "line", from: [x0 + 1, top + 5], to: [right - 1, top + 5], color: 10 });
      L(layer, { op: "line", from: [x0 + 1, top + 11], to: [right - 1, top + 11], color: 10 });
    }
    L(layer, { op: "line", from: [x0, top], to: [right - 1, top], color: 7 });
    L(layer, { op: "outline", color: 0 });
  };
  const sword = (layer, hx, hy, tipDx) => {
    const top = hy - 24;
    L(layer, { op: "line", from: [hx, hy - 2], to: [hx + tipDx, top], color: 7 });
    L(layer, { op: "line", from: [hx + 1, hy - 2], to: [hx + 1 + tipDx, top], color: 6 });
    L(layer, { op: "pixel", x: hx + tipDx, y: top - 1, color: 7 });
    L(layer, { op: "line", from: [hx - 3, hy - 1], to: [hx + 4, hy - 1], color: 13 });
    L(layer, { op: "rect", x: hx - 1, y: hy, w: 4, h: 3, color: 6, fill: true });
    L(layer, { op: "pixel", x: hx - 1, y: hy, color: 7 });
    L(layer, { op: "rect", x: hx - 1, y: hy - 7, w: 4, h: 6, color: 1, fill: true });
    L(layer, { op: "outline", color: 0 });
  };
  if (back) {
    kite("escudo", cx - 18 + sw, 11, y(30), false);
    sword("espada", cx + 9 - sw, y(39), view === "NE" ? 6 : 0);
  } else {
    kite("escudo", cx + 6 + sw, view === "SE" ? 11 : 12, y(30), true);
    sword("espada", cx - 10 - sw, y(39), view === "SE" ? -4 : 0);
  }
  return { tool: "draw_batch", args: { label: `walk ${view} ${i}`, ops } };
}

const steps = [
  { tool: "project_open", args: { path: "demo-a-guerreiro/guerreiro.psmproj" } },
  { tool: "snapshot_create", args: { name: "antes das 8 direções" } },
  { tool: "layer_create", args: { name: "escudo_tras", above: "perna_tras" } },
  { tool: "layer_create", args: { name: "espada_frente" } },
];
for (const view of ["S", "N", "SE", "NE"]) {
  steps.push({ tool: "animation_set_direction", args: { animation: "walk", direction: view, create: 8, reference: view === "S" || view === "N" ? "E" : "E" } });
  for (let i = 0; i < 8; i++) steps.push(views(view, i));
}
// Espelhos opcionais como ponto de partida, com o equipamento refeito.
const fix = [
  // [direção nova, espelho de, fonte do equipamento (direção, espelhada?), deslocamento do escudo]
  ["W", "E", "W", -4],
  ["SW", "SE", "SE", -2],
  ["NW", "NE", "NE", 2],
];
for (const [dir, from, equipFrom, shift] of fix) {
  steps.push({ tool: "animation_set_direction", args: { animation: "walk", direction: dir, copy_from: { direction: from, mirror: true }, reference: from } });
  for (let i = 0; i < 8; i++) {
    const frame = { animation: "walk", direction: dir, index: i };
    const src = { animation: "walk", direction: equipFrom, index: i };
    const full = { x: 0, y: 0, w: 64, h: 64 };
    steps.push({
      tool: "draw_batch",
      args: {
        label: `walk ${dir} ${i}: escudo atrás do corpo, espada na frente`,
        ops: [
          { layer: "escudo_tras", frame, op: "copy", from: { layer: "escudo", frame: src }, rect: full, to: [shift, 0] },
          { layer: "espada_frente", frame, op: "copy", from: { layer: "espada", frame: src }, rect: full, to: [0, 0] },
          { layer: "escudo", frame, op: "clear" },
          { layer: "espada", frame, op: "clear" },
        ],
      },
    });
  }
}
const order = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
for (const d of order) steps.push({ tool: "animation_preview", args: { animation: "walk", direction: d, scale: 3 }, save_as: `d-walk-${d}` });
steps.push(
  { tool: "animation_info", args: { animation: "walk", metrics: false } },
  { tool: "project_validate", args: {} },
  { tool: "project_save", args: {} },
  { tool: "export_godot_metadata", args: { path: "demo-d-direcoes/godot/guerreiro.tres", selection: { animations: ["walk"] } } },
  { tool: "export_spritesheet", args: { path: "demo-d-direcoes/guerreiro_walk_8dir.png", selection: { animations: ["walk"] }, sheet: { layout: "rows", spacing: 1 } } },
);
console.log(JSON.stringify(steps, null, 1));
