// Sobreposições das ferramentas no canvas: eixos de simetria, contorno da seleção por máscara,
// laço em andamento e o quadrado do pincel sob o cursor. Só desenho; nada aqui edita pixels.
import type { useEditor } from "../../core/store";
import type { Point, Sprite } from "../../core/types";
import { BRUSH_TOOLS, symmetryAxes } from "./brush";
import { maskEdges } from "./selection";

type State = ReturnType<typeof useEditor.getState>;

export type OverlayExtras = {
  /** Deslocamento do arraste da ferramenta Mover (a seleção acompanha). */
  move?: Point;
  /** Pontos do laço sendo desenhado. */
  lasso?: Point[];
  /** Pixel sob o cursor (sem arraste). */
  hover?: Point;
};

function dashed(g: CanvasRenderingContext2D, path: () => void) {
  g.lineWidth = 1;
  g.setLineDash([4, 3]);
  g.strokeStyle = "#ffffff";
  g.beginPath();
  path();
  g.stroke();
  g.strokeStyle = "#000000";
  g.lineDashOffset = 3.5;
  g.beginPath();
  path();
  g.stroke();
  g.setLineDash([]);
  g.lineDashOffset = 0;
}

export function drawToolsOverlay(g: CanvasRenderingContext2D, s: State, sprite: Sprite, ox: number, oy: number, z: number, extra: OverlayExtras) {
  const W = sprite.width * z;
  const H = sprite.height * z;
  // Symmetry axes of the brushes.
  if (BRUSH_TOOLS.includes(s.tool) && (s.symmetry.x || s.symmetry.y)) {
    const axes = symmetryAxes(s.symmetry, sprite.width, sprite.height);
    g.save();
    g.setLineDash([8, 4]);
    g.lineWidth = 1.5;
    g.strokeStyle = "rgba(255,79,216,0.95)";
    g.beginPath();
    if (axes.x !== undefined) {
      const x = Math.round(ox + (axes.x + 0.5) * z) + 0.5;
      g.moveTo(x, oy - 6);
      g.lineTo(x, oy + H + 6);
    }
    if (axes.y !== undefined) {
      const y = Math.round(oy + (axes.y + 0.5) * z) + 0.5;
      g.moveTo(ox - 6, y);
      g.lineTo(ox + W + 6, y);
    }
    g.stroke();
    g.restore();
  }
  // Mask selection outline (the bounding box is not drawn when there is a mask).
  if (s.selectionMask) {
    const segs = maskEdges(s.selectionMask);
    const [mx, my] = extra.move ?? [0, 0];
    dashed(g, () => {
      for (let i = 0; i < segs.length; i += 4) {
        g.moveTo(ox + (segs[i] + mx) * z + 0.5, oy + (segs[i + 1] + my) * z + 0.5);
        g.lineTo(ox + (segs[i + 2] + mx) * z + 0.5, oy + (segs[i + 3] + my) * z + 0.5);
      }
    });
  }
  // Lasso being drawn.
  const lasso = extra.lasso;
  if (lasso && lasso.length > 0) {
    const at = (p: Point): Point => [ox + (p[0] + 0.5) * z, oy + (p[1] + 0.5) * z];
    dashed(g, () => {
      lasso.forEach((p, i) => (i ? g.lineTo(...at(p)) : g.moveTo(...at(p))));
      g.lineTo(...at(lasso[0]));
    });
  }
  // Brush square under the cursor for the new tools (pencil and eraser already have theirs).
  const hover = extra.hover;
  if (hover && ["dither", "shade", "wand", "lasso"].includes(s.tool)) {
    const b = s.tool === "dither" || s.tool === "shade" ? s.brush : 1;
    const lo = -Math.floor((b - 1) / 2);
    g.strokeStyle = "rgba(255,255,255,0.9)";
    g.lineWidth = 1;
    g.strokeRect(ox + (hover[0] + lo) * z + 0.5, oy + (hover[1] + lo) * z + 0.5, b * z - 1, b * z - 1);
  }
}
