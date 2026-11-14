// Gaivota voando (1 ação, 1 direção), estilo pixel art realista.
// As decisões estão nas tabelas: ângulos da asa por frame, atraso da ponta, balanço do corpo.
// Uso (na pasta examples/): node demo-f-gaivota/gerar.mjs > demo-f-gaivota/gaivota.json
//                           psm mcp-run demo-f-gaivota/gaivota.json --images demo-f-gaivota/saida

const PAL = [
  "#3b4352", // 0 contorno
  "#ffffff", // 1 branco
  "#e4eaf0", // 2 branco na sombra
  "#c5d0da", // 3 branco em sombra profunda
  "#b8c4cf", // 4 cinza-manto claro
  "#9aa8b6", // 5 cinza-manto
  "#7b8896", // 6 cinza-manto escuro
  "#24272c", // 7 ponta preta
  "#3d4249", // 8 ponta preta vista por baixo
  "#f2f5f7", // 9 espelho branco da ponta
  "#f1c232", // 10 bico
  "#c48f17", // 11 bico na sombra
  "#d63a2a", // 12 mancha vermelha do bico
  "#191717", // 13 olho
  "#f3e6b0", // 14 íris clara
  "#e3a69c", // 15 pés
  "#d0d8e0", // 16 face de baixo da asa
];

// Ângulo do braço da asa (graus, + para cima) e da ponta (atrasa no movimento); balanço do corpo.
const FRAMES = [
  { th: 72, tip: 78, bob: 1, nota: "asas no alto" },
  { th: 50, tip: 66, bob: 0, nota: "começa a descer; a ponta fica para trás" },
  { th: 15, tip: 36, bob: -1, nota: "descida rápida" },
  { th: -22, tip: 0, bob: -2, nota: "abaixo do corpo; o corpo sobe" },
  { th: -50, tip: -36, bob: -2, nota: "fim da descida" },
  { th: -34, tip: -56, bob: -1, nota: "começa a subir; a ponta fica para baixo" },
  { th: 4, tip: -22, bob: 0, nota: "subida, asa quase de lado" },
  { th: 44, tip: 22, bob: 1, nota: "subindo" },
];

// Descida geral para a asa no alto caber no canvas (no frame 0 a ponta chega a y = -3 sem isso).
const DY = 5;
const rad = (d) => (d * Math.PI) / 180;
const R = Math.round;

function wing(layer, frame, S, f, near) {
  const s1 = Math.sin(rad(f.th));
  const s2 = Math.sin(rad(f.tip));
  const sy = S[1] + f.bob;
  const L1 = 15, L2 = 18;
  const W = [S[0], R(sy - L1 * s1)];
  const T = [R(S[0] - 10 - 6 * Math.abs(s2)), R(W[1] - L2 * s2)];
  const Rle = [S[0] + 5, sy], Rte = [S[0] - 7, sy + 2];
  const Wle = [W[0] + 3, W[1]], Wte = [W[0] - 6, W[1] + (f.th >= 0 ? 2 : -2)];
  // Asa de perto mostra a face de cima ao descer; a de longe, ao subir.
  const top = near ? f.th < 0 : f.th > 0;
  const arm = top ? (near ? 5 : 6) : near ? 16 : 3;
  const L = (op) => ({ layer, frame, ...op });
  const mid = (a, b, k) => [R(a[0] + (b[0] - a[0]) * k), R(a[1] + (b[1] - a[1]) * k)];
  const ops = [
    L({ op: "polygon", points: [Rle, Wle, Wte, Rte], color: arm }),
    L({ op: "polygon", points: [Wle, T, Wte], color: top ? 7 : 8 }),
    // Base das primárias: cinza antes do preto.
    L({ op: "polygon", points: [Wle, mid(Wle, T, 0.35), mid(Wte, T, 0.35), Wte], color: top ? 6 : 16 }),
    L({ op: "line", from: Rle, to: Wle, color: top ? 4 : 1 }),
    L({ op: "line", from: Rte, to: Wte, color: top ? 1 : 3 }),
    // Fileira das coberteiras: uma linha de penas paralela ao bordo de ataque.
    L({ op: "line", from: mid(Rle, Rte, 0.45), to: mid(Wle, Wte, 0.45), color: top ? 4 : 3 }),
  ];
  if (top) {
    const m = mid(T, mid(Wle, Wte, 0.5), 0.22);
    ops.push(L({ op: "pixels", pixels: [[m[0], m[1], 9], [m[0] + 1, m[1], 9]] }));
  }
  ops.push(L({ op: "outline", color: 0 }));
  return ops;
}

function body(frame, b) {
  const L = (op) => ({ layer: "corpo", frame, ...op });
  const y = (v) => v + b + DY;
  return [
    // Cauda curta em cunha.
    L({ op: "polygon", points: [[18, y(33)], [28, y(31)], [29, y(37)], [19, y(36)]], color: 1 }),
    L({ op: "line", from: [18, y(33)], to: [19, y(36)], color: 2 }),
    L({ op: "ellipse", x: 26, y: y(29), w: 30, h: 12, color: 1, fill: true }),
    L({ op: "polygon", points: [[46, y(30)], [53, y(26)], [57, y(36)], [50, y(40)]], color: 1 }),
    L({ op: "ellipse", x: 51, y: y(24), w: 12, h: 11, color: 1, fill: true }),
    // Sombreado do branco: claro em cima, azulado embaixo (com dithering).
    L({ op: "gradient", colors: [1, 1, 2, 3], shape: { type: "linear", angle: 90 }, dither: "bayer4", only_opaque: true, rect: { x: 10, y: y(22), w: 56, h: 21 } }),
    L({ op: "ellipse", x: 29, y: y(29), w: 22, h: 4, color: 5, fill: true }),
    L({ op: "line", from: [31, y(29)], to: [47, y(29)], color: 4 }),
    // Bico, mancha vermelha, olho e pés recolhidos.
    L({ op: "polygon", points: [[62, y(29)], [71, y(30)], [72, y(31)], [69, y(32)], [62, y(33)]], color: 10 }),
    L({ op: "line", from: [62, y(31)], to: [66, y(31)], color: 11 }),
    L({ op: "line", from: [63, y(32)], to: [68, y(32)], color: 11 }),
    L({ op: "pixel", x: 67, y: y(32), color: 12 }),
    L({ op: "pixel", x: 58, y: y(28), color: 13 }),
    L({ op: "pixel", x: 57, y: y(28), color: 14 }),
    L({ op: "pixels", pixels: [[31, y(41)], [32, y(41)]], color: 15 }),
    L({ op: "outline", color: 0 }),
  ];
}

const PROJECT = process.env.PROJECT;
const steps = PROJECT ? [] : [
  { tool: "project_create", args: { name: "Gaivota", width: 80, height: 64, sprite_name: "gaivota" } },
  { tool: "palette_create", args: { name: "Gaivota realista", colors: PAL } },
  { tool: "layer_properties", args: { layer: "Layer 1", name: "asa_tras" } },
  { tool: "layer_create", args: { name: "corpo" } },
  { tool: "layer_create", args: { name: "asa_frente" } },
  { tool: "animation_create", args: { name: "voar", directions: ["E"], frames: 8, duration_ms: 80, loop_mode: "loop" } },
  { tool: "animation_update", args: { animation: "voar", cycle: { notes: "Batida de asas em 8 frames, 12,5 fps. Descida rápida (frames 1-4) e subida mais lenta; a ponta da asa atrasa; o corpo sobe quando as asas empurram para baixo." } } },
];
FRAMES.forEach((f, i) => {
  const frame = { animation: "voar", direction: "E", index: i };
  steps.push({
    tool: "draw_batch",
    args: {
      label: `voar ${i}: ${f.nota}`,
      ops: [
        { layer: "asa_tras", frame, op: "clear" },
        { layer: "corpo", frame, op: "clear" },
        { layer: "asa_frente", frame, op: "clear" },
        ...wing("asa_tras", frame, [45, 29 + DY], f, false),
        ...body(frame, f.bob),
        ...wing("asa_frente", frame, [41, 31 + DY], f, true),
      ],
    },
  });
});
steps.push(
  { tool: "animation_preview", args: { animation: "voar", direction: "E", scale: 3 }, save_as: "f1-voar-strip" },
  { tool: "animation_preview", args: { animation: "voar", direction: "E", mode: "gif", scale: 4 }, save_as: "f2-voar" },
);
steps.push({ tool: "project_save", args: { path: "demo-f-gaivota/gaivota.psmproj" } });
if (PROJECT) for (const s of steps) s.args = { project: PROJECT, ...s.args };
console.log(JSON.stringify(steps, null, 1));
