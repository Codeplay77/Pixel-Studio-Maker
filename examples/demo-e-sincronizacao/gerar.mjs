// Demonstração E (parte do agente): com o editor aberto, muda a cor da armadura, aumenta o escudo
// e corrige os três primeiros frames da caminhada E. Tudo aparece no editor sem recarregar.
// Uso (na pasta examples/): node demo-e-sincronizacao/gerar.mjs > demo-e-sincronizacao/agente.json
//                           psm mcp-run demo-e-sincronizacao/agente.json --images demo-e-sincronizacao/saida

const ARMOR = [
  ["#1d3f7a", "#1e5136"],
  ["#2f6ad0", "#2f9a5a"],
  ["#6fa8f0", "#6fd08c"],
  ["#c5e2ff", "#c8f5d2"],
];
// Deslocamento do escudo e balanço do corpo em cada frame da caminhada E (ver demo-b).
const FRAMES = [
  { dx: -1, b: 0 },
  { dx: -1, b: 1 },
  { dx: 0, b: -1 },
];

const steps = [{ tool: "project_open", args: { path: "demo-a-guerreiro/guerreiro.psmproj" } }];
steps.push({ tool: "transaction_begin", args: { label: "armadura verde" } });
for (const [from, to] of ARMOR) steps.push({ tool: "palette_replace", args: { from, to } });
steps.push({ tool: "transaction_commit", args: {} });
steps.push({ tool: "frame_preview", args: { frame: { animation: "walk", direction: "E", index: 0 }, scale: 6 }, save_as: "e1-armadura-verde" });

FRAMES.forEach(({ dx, b }, i) => {
  const frame = { animation: "walk", direction: "E", index: i };
  const P = (x, y) => [x + dx, y + b];
  const S = (op) => ({ layer: "escudo", frame, ...op });
  steps.push({
    tool: "draw_batch",
    args: {
      label: `escudo maior no frame ${i}`,
      ops: [
        S({ op: "clear" }),
        S({ op: "rect", x: 24 + dx, y: 34 + b, w: 4, h: 5, color: 1, fill: true }),
        S({ op: "rect", x: 25 + dx, y: 38 + b, w: 4, h: 3, color: 6, fill: true }),
        S({ op: "pixel", x: 25 + dx, y: 38 + b, color: 7 }),
        S({ op: "polygon", points: [P(28, 29), P(43, 29), P(43, 41), P(35, 54), P(28, 41)], color: 6 }),
        S({ op: "polygon", points: [P(29, 30), P(42, 30), P(42, 41), P(35, 52), P(29, 41)], color: 2 }),
        S({ op: "line", from: P(41, 31), to: P(41, 41), color: 1 }),
        S({ op: "line", from: P(40, 42), to: P(36, 49), color: 1 }),
        S({ op: "line", from: P(30, 31), to: P(30, 41), color: 3 }),
        S({ op: "line", from: P(30, 42), to: P(34, 49), color: 3 }),
        S({ op: "line", from: P(35, 33), to: P(35, 47), color: 13 }),
        S({ op: "line", from: P(31, 38), to: P(39, 38), color: 13 }),
        S({ op: "line", from: P(28, 29), to: P(42, 29), color: 7 }),
        S({ op: "line", from: P(28, 30), to: P(28, 40), color: 7 }),
        S({ op: "line", from: P(43, 30), to: P(43, 41), color: 5 }),
        S({ op: "line", from: P(42, 42), to: P(36, 52), color: 5 }),
        S({ op: "outline", color: 0 }),
        // Correção: a mão da espada ficaria escondida atrás do escudo maior.
        { layer: "espada", frame, op: "transform", transform: { kind: "move", dx: 2, dy: 0 } },
      ],
    },
  });
});
steps.push(
  { tool: "frame_compare", args: { a: { animation: "walk", direction: "E", index: 3 }, b: { animation: "walk", direction: "E", index: 2 }, scale: 4 }, save_as: "e2-escudo-maior" },
  { tool: "project_save", args: {} },
);
console.log(JSON.stringify(steps, null, 1));
