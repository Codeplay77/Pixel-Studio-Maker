// Seleção por máscara: varinha, laço, Shift/Alt para somar/subtrair, e as ações da seleção
// (copiar, recortar, colar, apagar, mover, espelhar, girar). Toda máscara é calculada no Rust
// (select_color, select_polygon, draw_transform); aqui só se guarda e se mostra o resultado.
import { currentSprite, useEditor } from "../../core/store";
import type { Point, Rect } from "../../core/types";
import type { SelectionMask, SelectMode } from "./types";

type Target = Record<string, unknown>;
type Modifiers = { shiftKey: boolean; altKey: boolean };

// The mask belongs to the selection object it was set with. Anything else that replaces
// `selection` (marquee, Ctrl+A, Esc, opening a project) drops the mask automatically.
useEditor.subscribe((st) => {
  if (st.selectionMask && st.selection !== st.selectionMask.bounds) useEditor.setState({ selectionMask: undefined });
});

/** Shift = somar, Alt = subtrair, Shift+Alt = interseção. */
export function selectMode(e: Modifiers): SelectMode {
  if (e.shiftKey && e.altKey) return "intersect";
  if (e.shiftKey) return "add";
  if (e.altKey) return "subtract";
  return "replace";
}

/** A seleção atual como máscara (um retângulo vira linhas de `#`), para combinar no Rust. */
export function currentBase(): SelectionMask | undefined {
  const st = useEditor.getState();
  if (st.selectionMask) return st.selectionMask;
  const r = st.selection;
  if (!r) return undefined;
  return { bounds: r, rows: Array.from({ length: r.h }, () => "#".repeat(r.w)) };
}

/** Guarda a máscara que o Rust devolveu (`bounds` null = seleção vazia). */
export function applyMask(data: { bounds?: Rect | null; rows?: string[] } | undefined) {
  if (!data?.bounds || !data.rows?.length) {
    useEditor.getState().set({ selection: undefined, selectionMask: undefined });
    return;
  }
  const mask: SelectionMask = { bounds: data.bounds, rows: data.rows };
  useEditor.getState().set({ selectionMask: mask, selection: mask.bounds });
}

function inside(p: Point): boolean {
  const sp = currentSprite(useEditor.getState());
  return !!sp && p[0] >= 0 && p[1] >= 0 && p[0] < sp.width && p[1] < sp.height;
}

function combineArgs(mode: SelectMode) {
  return mode === "replace" ? { mode } : { mode, base: currentBase() };
}

/** Varinha mágica no pixel `p`. */
export function wandAt(p: Point, e: Modifiers, target: Target) {
  const st = useEditor.getState();
  const mode = selectMode(e);
  if (!inside(p)) {
    if (mode === "replace") st.set({ selection: undefined, selectionMask: undefined });
    return;
  }
  void st
    .run("select_color", { ...target, x: p[0], y: p[1], contiguous: st.wandContiguous, tolerance: st.wandTolerance, composite: st.wandComposite, ...combineArgs(mode) })
    .then((r) => applyMask(r.data))
    .catch(() => {});
}

/** Fecha o laço desenhado. Um clique sem arraste limpa a seleção (no modo substituir). */
export function lassoFinish(points: Point[], mode: SelectMode) {
  const st = useEditor.getState();
  if (points.length < 3) {
    if (mode === "replace") st.set({ selection: undefined, selectionMask: undefined });
    return;
  }
  void st.run("select_polygon", { sprite: st.spriteId, points, ...combineArgs(mode) }).then((r) => applyMask(r.data)).catch(() => {});
}

/** Retângulo de seleção com Shift/Alt: combina com a seleção atual no Rust. */
export function marqueeCombine(r: Rect, mode: SelectMode) {
  const st = useEditor.getState();
  const corners: Point[] = [
    [r.x, r.y],
    [r.x + r.w - 1, r.y],
    [r.x + r.w - 1, r.y + r.h - 1],
    [r.x, r.y + r.h - 1],
  ];
  void st.run("select_polygon", { sprite: st.spriteId, points: corners, ...combineArgs(mode) }).then((res) => applyMask(res.data)).catch(() => {});
}

/** Move os pixels selecionados (ou a layer inteira, sem seleção) e a seleção junto. */
export function moveSelection(target: Target, dx: number, dy: number) {
  const st = useEditor.getState();
  const transform = { kind: "move", dx, dy };
  if (st.selectionMask) {
    void st.run("draw_transform", { ...target, transform, mask: st.selectionMask }).then((r) => applyMask(r.data.selection)).catch(() => {});
    return;
  }
  void st.run("draw_transform", { ...target, transform, rect: st.selection ?? undefined }).catch(() => {});
  if (st.selection) st.set({ selection: { ...st.selection, x: st.selection.x + dx, y: st.selection.y + dy } });
}

/** Ações da seleção, respeitando a máscara quando houver (só os pixels selecionados). */
export function maskSelectionActions() {
  const s = useEditor.getState();
  const t = { sprite: s.spriteId, layer: s.layerId, frame: s.frameId };
  const area = s.selectionMask ? { mask: s.selectionMask } : s.selection ? { rect: s.selection } : undefined;
  const copy = async (): Promise<boolean> => {
    if (!area || s.spriteId === undefined || s.layerId === undefined || s.frameId === undefined) return false;
    const r = await s.run("selection_read", { ...t, ...area }).catch(() => undefined);
    if (!r) return false;
    // An empty selection comes back without `rect` (the runtime warning is already shown).
    const rect: Rect | undefined = r.data?.rect ?? undefined;
    if (!rect) return false;
    const data = r.data.pixels ? { pixels: r.data.pixels, palette: r.data.palette } : { rows: r.data.rows, legend: r.data.legend };
    const mask: SelectionMask | undefined = s.selectionMask && r.data.selection?.bounds ? { bounds: r.data.selection.bounds, rows: r.data.selection.rows } : undefined;
    s.set({ clipboard: { sprite: s.spriteId, layer: s.layerId, frame: s.frameId, rect }, clipboardPixels: { rect, data, mask } });
    s.toast("Seleção copiada");
    return true;
  };
  const remove = () => {
    if (area) void s.run("pixel_clear", { ...t, ...area }).catch(() => {});
  };
  return {
    remove,
    copy: () => void copy(),
    cut: () => void copy().then((ok) => ok && remove()),
    paste: () => {
      const c = s.clipboardPixels;
      if (!c) return;
      const at: Point = s.selection ? [s.selection.x, s.selection.y] : [c.rect.x, c.rect.y];
      void s.run("draw_batch", { ...t, label: "Colar", ops: [{ op: "matrix", at, ...c.data }] }).catch(() => {});
      if (c.mask) applyMask({ bounds: { ...c.mask.bounds, x: at[0], y: at[1] }, rows: c.mask.rows });
      else s.set({ selection: { x: at[0], y: at[1], w: c.rect.w, h: c.rect.h } });
    },
    transform: (transform: Record<string, unknown>) => {
      if (s.selectionMask) {
        void s.run("draw_transform", { ...t, transform, mask: s.selectionMask }).then((r) => applyMask(r.data.selection)).catch(() => {});
        return;
      }
      void s.run("draw_transform", { ...t, transform, rect: s.selection ?? undefined }).catch(() => {});
    },
  };
}

// ---- drawing helpers (display only) -------------------------------------------------------

const edgeCache = new WeakMap<SelectionMask, number[]>();

/** Bordas da máscara (segmentos entre pixel selecionado e não selecionado), em pixels. */
export function maskEdges(mask: SelectionMask): number[] {
  const cached = edgeCache.get(mask);
  if (cached) return cached;
  const { x: bx, y: by } = mask.bounds;
  const on = (x: number, y: number) => {
    const row = mask.rows[y];
    if (!row || x < 0 || x >= row.length) return false;
    const ch = row[x];
    return ch !== "." && ch !== " ";
  };
  const segs: number[] = [];
  mask.rows.forEach((row, y) => {
    for (let x = 0; x < row.length; x++) {
      if (!on(x, y)) continue;
      const X = bx + x;
      const Y = by + y;
      if (!on(x, y - 1)) segs.push(X, Y, X + 1, Y);
      if (!on(x, y + 1)) segs.push(X, Y + 1, X + 1, Y + 1);
      if (!on(x - 1, y)) segs.push(X, Y, X, Y + 1);
      if (!on(x + 1, y)) segs.push(X + 1, Y, X + 1, Y + 1);
    }
  });
  edgeCache.set(mask, segs);
  return segs;
}
