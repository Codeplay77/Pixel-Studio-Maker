// Pincéis (lápis, borracha, dithering, sombreado): todos usam a op SDC `stroke`.
// Durante o arraste, o traço é desenhado ao vivo dentro de uma transação; ao soltar, a transação é
// desfeita e o traço inteiro vai como uma única op `stroke` (um passo no histórico).
import * as api from "../../core/api";
import { useEditor, type Tool } from "../../core/store";
import type { Point } from "../../core/types";
import type { SymmetryState } from "./types";

export type StrokeDrag = { kind: "stroke"; points: Point[]; label: string; base: Record<string, unknown>; redrawing?: boolean; sent?: number };

type Target = () => Record<string, unknown>;
type Enqueue = (fn: () => Promise<unknown>) => unknown;

/** Ferramentas que pintam com `stroke` (e usam espessura e simetria). */
export const BRUSH_TOOLS: Tool[] = ["pencil", "eraser", "dither", "shade"];

const LABELS: Partial<Record<Tool, string>> = { pencil: "Lápis", eraser: "Borracha", dither: "Dithering", shade: "Sombreado" };

/** Argumento `symmetry` da op, ou undefined sem eixos ligados. */
export function symmetryArg(sym: SymmetryState): Record<string, number | boolean> | undefined {
  if (!sym.x && !sym.y) return undefined;
  const out: Record<string, number | boolean> = {};
  if (sym.x) out.x = sym.ax ?? true;
  if (sym.y) out.y = sym.ay ?? true;
  return out;
}

/** Posição dos eixos de simetria em pixels (centro do sprite quando não definida). */
export function symmetryAxes(sym: SymmetryState, width: number, height: number): { x?: number; y?: number } {
  return { x: sym.x ? (sym.ax ?? (width - 1) / 2) : undefined, y: sym.y ? (sym.ay ?? (height - 1) / 2) : undefined };
}

/** A op `stroke` sem os pontos, conforme a ferramenta, o botão do mouse e as opções. */
export function strokeBase(tool: Tool, button: number): Record<string, unknown> {
  const s = useEditor.getState();
  const main = button === 2 ? s.secondary : s.primary;
  const other = button === 2 ? s.primary : s.secondary;
  const base: Record<string, unknown> = { op: "stroke", thickness: s.brush, wrap: s.tileMode };
  const symmetry = symmetryArg(s.symmetry);
  if (symmetry) base.symmetry = symmetry;
  switch (tool) {
    case "eraser":
      base.color = "clear";
      break;
    case "dither":
      Object.assign(base, { mode: "dither", color: main, pattern: s.ditherPattern, density: s.ditherDensity });
      if (s.ditherFill) base.secondary = other;
      break;
    case "shade":
      // Ramp = the palette in use (the Rust default); left lightens, right darkens.
      Object.assign(base, { mode: "shade", step: button === 2 ? -1 : 1 });
      break;
    default:
      base.color = main;
  }
  return base;
}

export function startStroke(tool: Tool, p: Point, button: number, enqueue: Enqueue, target: Target, pid?: string): StrokeDrag {
  const d: StrokeDrag = { kind: "stroke", points: [p], label: LABELS[tool] ?? "Traço", base: strokeBase(tool, button) };
  enqueue(() => api.call("transaction_begin", { label: d.label }, pid));
  enqueue(() => api.call("draw_batch", { ...target(), ops: [{ ...d.base, points: [p] }] }, pid));
  return d;
}

export function extendStroke(d: StrokeDrag, p: Point, enqueue: Enqueue, target: Target, pid?: string) {
  const last = d.points[d.points.length - 1];
  if (last[0] === p[0] && last[1] === p[1]) return;
  d.points.push(p);
  // Coalesce pointer events while the runtime is busy, preserving every sampled point.
  if (d.redrawing) return;
  d.redrawing = true;
  enqueue(async () => {
    d.redrawing = false;
    if (d.base.mode === "shade") {
      // Shading cannot be applied twice to the shared endpoints of segments.
      await api.call("transaction_rollback", {}, pid);
      await api.call("transaction_begin", { label: d.label }, pid);
      await api.call("draw_batch", { ...target(), ops: [{ ...d.base, points: [...d.points] }] }, pid);
    } else {
      const end = d.points.length;
      const points = d.points.slice(d.sent ?? 0, end);
      d.sent = end - 1;
      await api.call("draw_batch", { ...target(), ops: [{ ...d.base, points }] }, pid);
    }
  });
}

/** Troca o traço ao vivo por uma única op `stroke` com todos os pontos (um passo de desfazer). */
export function finishStroke(d: StrokeDrag, enqueue: Enqueue, target: Target, pid?: string) {
  const points = d.points;
  enqueue(async () => {
    await api.call("transaction_rollback", {}, pid);
    await api.call("draw_batch", { ...target(), label: d.label, ops: [{ ...d.base, points, pixel_perfect: d.base.thickness === 1 }] }, pid);
  });
}
