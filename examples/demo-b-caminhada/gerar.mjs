// Gera o roteiro MCP da caminhada (Demonstração B). As poses abaixo são as decisões artísticas;
// o script só as expande em operações SDC (draw_batch) para o `psm mcp-run`.
// Uso (na pasta examples/): node demo-b-caminhada/gerar.mjs > demo-b-caminhada/caminhada.json
//                           psm mcp-run demo-b-caminhada/caminhada.json --images demo-b-caminhada/saida
const NEAR = { layer: "perna_frente", base: 2, hl: 3, sh: 1, steel: 6, steelHi: 7, boot: 11, bootHi: 17, sole: 10 };
const FAR = { layer: "perna_tras", base: 1, hl: 2, sh: 1, steel: 5, steelHi: 6, boot: 10, bootHi: 11, sole: 0 };

// [quadril, joelho, tornozelo] por frame; `ergue` = pé fora do chão inclinado.
const POSES = [
  { bob: 0, near: [[25, 45], [27, 50], [29, 55]], far: [[23, 45], [21, 50], [19, 55]], shield: -1, sword: 1, nota: "contato: perna da frente esticada à frente" },
  { bob: 1, near: [[25, 46], [27, 50], [27, 55]], far: [[23, 46], [21, 51], [18, 53], "ergue"], shield: -1, sword: 1, nota: "descida: peso na perna da frente" },
  { bob: -1, near: [[25, 44], [25, 49], [25, 55]], far: [[23, 44], [26, 48], [23, 52]], shield: 0, sword: 0, nota: "passagem: perna de trás cruzando com o joelho alto" },
  { bob: 0, near: [[25, 45], [24, 50], [22, 55]], far: [[23, 45], [26, 49], [28, 53]], shield: 1, sword: -1, nota: "subida: perna de trás alcançando a frente" },
  { bob: 0, near: [[25, 45], [23, 50], [21, 55]], far: [[23, 45], [25, 50], [27, 55]], shield: 1, sword: -1, nota: "contato oposto" },
  { bob: 1, near: [[25, 46], [23, 51], [20, 53], "ergue"], far: [[23, 46], [25, 50], [25, 55]], shield: 1, sword: -1, nota: "descida oposta" },
  { bob: -1, near: [[25, 44], [28, 48], [25, 52]], far: [[23, 44], [23, 49], [23, 55]], shield: 0, sword: 0, nota: "passagem oposta" },
  { bob: 0, near: [[25, 45], [28, 49], [30, 53]], far: [[23, 45], [22, 50], [20, 55]], shield: -1, sword: 1, nota: "subida oposta" },
];

const off = ([x, y], dx, dy = 0) => [x + dx, y + dy];

function leg(c, [H, K, A, mode], frame) {
  const L = (op) => ({ layer: c.layer, frame, ...op });
  const ops = [
    L({ op: "clear" }),
    L({ op: "line", from: H, to: K, color: c.base, thickness: 5 }),
    L({ op: "line", from: K, to: A, color: c.base, thickness: 5 }),
    L({ op: "line", from: off(H, -2), to: off(K, -2), color: c.hl }),
    L({ op: "line", from: off(K, -2), to: off(A, -2), color: c.hl }),
    L({ op: "line", from: off(H, 2), to: off(K, 2), color: c.sh }),
    L({ op: "line", from: off(K, 2), to: off(A, 2), color: c.sh }),
    L({ op: "rect", x: K[0] - 1, y: K[1] - 1, w: 3, h: 2, color: c.steel, fill: true }),
    L({ op: "pixel", x: K[0] - 1, y: K[1] - 1, color: c.steelHi }),
  ];
  const [ax, ay] = A;
  if (mode === "ergue") {
    // Calcanhar levantado: a bota inclina com a ponta para baixo.
    ops.push(L({ op: "polygon", points: [[ax - 3, ay - 1], [ax + 1, ay], [ax + 4, ay + 3], [ax + 3, ay + 4], [ax - 1, ay + 3], [ax - 3, ay + 1]], color: c.boot }));
    ops.push(L({ op: "line", from: [ax - 3, ay - 1], to: [ax, ay], color: c.bootHi }));
  } else {
    ops.push(L({ op: "polygon", points: [[ax - 3, ay], [ax + 2, ay], [ax + 5, ay + 2], [ax + 5, ay + 4], [ax - 3, ay + 4]], color: c.boot }));
    ops.push(L({ op: "line", from: [ax - 3, ay], to: [ax + 1, ay], color: c.bootHi }));
    ops.push(L({ op: "line", from: [ax - 3, ay + 4], to: [ax + 5, ay + 4], color: c.sole }));
  }
  ops.push(L({ op: "outline", color: 0 }));
  return ops;
}

const ref = (i) => ({ animation: "walk", direction: "E", index: i });
const steps = [
  { tool: "project_open", args: { path: "demo-a-guerreiro/guerreiro.psmproj" } },
  { tool: "animation_create", args: { name: "walk", directions: ["E"], frames: 8, copy_from: 3, duration_ms: 100, loop_mode: "loop", tags: ["locomocao"] } },
  { tool: "animation_update", args: { animation: "walk", cycle: { ground_y: 59, notes: "Caminhada no lugar, 8 frames, 10 fps. Contatos nos frames 0 e 4, passagens em 2 e 6. Corpo sobe na passagem e desce após o contato; escudo e espada balançam em oposição às pernas." } } },
];

POSES.forEach((p, i) => {
  const frame = ref(i);
  const upper = ["corpo", "cabeca", "pluma"].map((layer) => ({ layer, frame, op: "transform", transform: { kind: "move", dx: 0, dy: p.bob } }));
  steps.push({
    tool: "draw_batch",
    args: {
      label: `walk ${i}: ${p.nota}`,
      ops: [
        ...leg(FAR, p.far, frame),
        ...leg(NEAR, p.near, frame),
        ...upper,
        { layer: "escudo", frame, op: "transform", transform: { kind: "move", dx: p.shield, dy: p.bob } },
        { layer: "espada", frame, op: "transform", transform: { kind: "move", dx: p.sword, dy: p.bob } },
      ],
    },
  });
  // Os pés no chão viram anchors por frame (ajudam a conferir o deslizamento).
  const foot = (pose) => [pose[2][0] + 1, pose[3] ? pose[2][1] + 3 : 59];
  steps.push({ tool: "anchor_update", args: { anchor: "pe_frente", positions: [{ frame, position: foot(p.near) }] } });
  steps.push({ tool: "anchor_update", args: { anchor: "pe_tras", positions: [{ frame, position: foot(p.far) }] } });
  steps.push({ tool: "anchor_update", args: { anchor: "quadril", positions: [{ frame, position: [24, 44 + p.bob] }] } });
});

steps.push(
  { tool: "pose_create", args: { name: "walk_contato", kind: "contact", animation: "walk", direction: "E", frame: ref(0), ground_y: 59, notes: "Perna da frente à frente, calcanhar no chão; braço do escudo para trás." } },
  { tool: "pose_create", args: { name: "walk_passagem", kind: "passing", animation: "walk", direction: "E", frame: ref(2), ground_y: 59, notes: "Pernas cruzando; corpo no ponto mais alto." } },
  { tool: "pose_create", args: { name: "walk_contato_oposto", kind: "contact", animation: "walk", direction: "E", frame: ref(4), ground_y: 59 } },
  { tool: "pose_create", args: { name: "walk_passagem_oposta", kind: "passing", animation: "walk", direction: "E", frame: ref(6), ground_y: 59 } },
  { tool: "animation_preview", args: { animation: "walk", direction: "E", scale: 4 }, save_as: "b1-walk-strip" },
  { tool: "animation_preview", args: { animation: "walk", direction: "E", mode: "gif", scale: 4 }, save_as: "b2-walk" },
  { tool: "frame_compare", args: { a: ref(0), b: ref(2), scale: 4 }, save_as: "b3-compare-0-2" },
  { tool: "project_validate", args: {} },
  { tool: "project_save", args: {} },
  { tool: "export_spritesheet", args: { path: "demo-b-caminhada/guerreiro_walk_E.png", selection: { animations: ["walk"] }, sheet: { layout: "horizontal" } } },
  { tool: "export_gif", args: { path: "demo-b-caminhada/guerreiro_walk_E.gif", animation: "walk", direction: "E", scale: 4 } },
);

console.log(JSON.stringify(steps, null, 1));
