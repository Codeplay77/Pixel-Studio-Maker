// Demonstração C, etapa 2: a partir da matriz lida pelo MCP (pixel_read_region do slime importado),
// cria 6 frames de idle editando linhas e colunas (achatar, esticar, piscar) e gera o roteiro MCP.
// Uso (na pasta examples/): node demo-c-importar/gerar-idle.mjs > demo-c-importar/2-idle.json
import { readFileSync } from "node:fs";

const log = readFileSync(new URL("./saida/1-importar.log", import.meta.url), "utf8");
const line = log.split("\n").find((l) => l.includes("pixel_read_region ok"));
const read = JSON.parse(line.slice(line.indexOf("{")));
const { rows, legend } = read.data;
const SHADOW = Object.keys(legend).find((k) => legend[k].length === 9 && k !== "."); // a cor com alfa
const EYE = Object.keys(legend).find((k) => legend[k] === "#10161a");
const SHINE = Object.keys(legend).find((k) => legend[k] === "#f2fff0");
const BODY = Object.keys(legend).find((k) => legend[k] === "#4cbb52");

const isBody = (r) => [...r].some((c) => c !== "." && c !== SHADOW);
const top = rows.findIndex(isBody);
const bottom = rows.length - 1 - [...rows].reverse().findIndex(isBody);
const cut = bottom - 4; // linha do corpo logo abaixo da boca
const W = rows[0].length;
const CENTER = 16;

function recenter(row, extra) {
  // `row` tem W+extra colunas (extra pode ser negativo): volta a W mantendo o centro.
  if (extra > 0) return row.slice(Math.floor(extra / 2), Math.floor(extra / 2) + W);
  const pad = -extra;
  return ".".repeat(Math.floor(pad / 2)) + row + ".".repeat(Math.ceil(pad / 2));
}

function squash(src, k) {
  const out = [...src];
  out.splice(cut, k);
  for (let i = 0; i < k; i++) out.unshift(".".repeat(W));
  return out.map((r, y) => (y >= top && isBody(r)) || r.includes(SHADOW) ? recenter(r.slice(0, CENTER) + r[CENTER].repeat(k) + r.slice(CENTER), k) : r);
}

function stretch(src, k) {
  const out = [...src];
  for (let i = 0; i < k; i++) out.splice(cut, 0, src[cut]);
  out.splice(0, k);
  return out.map((r) => (isBody(r) ? recenter(r.slice(0, CENTER - k) + r.slice(CENTER), -k) : r));
}

function blink(src) {
  const eyeRows = src.map((r, y) => (r.includes(EYE) ? y : -1)).filter((y) => y >= 0);
  const [first, last] = [eyeRows[0], eyeRows[eyeRows.length - 1]];
  return src.map((r, y) => (y === first || y === last ? r.replaceAll(EYE, BODY).replaceAll(SHINE, BODY) : r));
}

const FRAMES = [
  { rows: rows, ms: 220, nota: "repouso (o sprite importado, intacto)" },
  { rows: squash(rows, 1), ms: 110, nota: "começa a achatar" },
  { rows: squash(rows, 2), ms: 160, nota: "achatado: mais largo e mais baixo" },
  { rows: squash(rows, 1), ms: 110, nota: "volta" },
  { rows: blink(rows), ms: 140, nota: "pisca" },
  { rows: stretch(rows, 1), ms: 120, nota: "estica: mais alto e mais estreito" },
];

const writeLegend = { ...Object.fromEntries(Object.entries(legend).filter(([k]) => k !== ".")), x: "clear" };
const colors = Object.entries(legend).filter(([k]) => k !== ".").map(([, v]) => v);
const steps = [
  { tool: "project_open", args: { path: "demo-c-importar/slime.psmproj" } },
  { tool: "palette_create", args: { name: "Slime (do PNG)", colors } },
  { tool: "region_create", args: { name: "corpo", part: "torso", layer: "slime", from_frame: 3 } },
  { tool: "anchor_create", args: { name: "chao", kind: "ground", position: [16, bottom] } },
  { tool: "animation_create", args: { name: "idle", frames: 6, copy_from: 3, loop_mode: "loop" } },
  { tool: "animation_update", args: { animation: "idle", cycle: { ground_y: bottom, notes: "Respiração: achata (mais largo), volta, pisca, estica. A base fica sempre na mesma linha." } } },
];
FRAMES.forEach((f, i) => {
  steps.push({
    tool: "draw_batch",
    args: {
      label: `idle ${i}: ${f.nota}`,
      frame: { animation: "idle", index: i },
      layer: "slime",
      ops: [{ op: "matrix", at: [0, 0], legend: writeLegend, rows: f.rows.map((r) => r.replaceAll(".", "x")) }],
    },
  });
});
steps.push(
  { tool: "animation_set_timing", args: { animation: "idle", durations: FRAMES.map((f) => f.ms) } },
  { tool: "animation_preview", args: { animation: "idle", scale: 6 }, save_as: "c2-idle-strip" },
  { tool: "animation_preview", args: { animation: "idle", mode: "gif", scale: 6 }, save_as: "c3-idle" },
  { tool: "frame_compare", args: { a: 3, b: { animation: "idle", index: 2 }, scale: 6 }, save_as: "c4-original-vs-achatado" },
  { tool: "project_validate", args: {} },
  { tool: "project_save", args: {} },
  { tool: "export_spritesheet", args: { path: "demo-c-importar/slime_idle.png", selection: { animations: ["idle"] }, sheet: { layout: "horizontal" } } },
  { tool: "export_gif", args: { path: "demo-c-importar/slime_idle.gif", animation: "idle", scale: 4 } },
);
console.log(JSON.stringify(steps, null, 1));
