// Integração da ferramenta Texto e das guias de 9-slice com o canvas. Aqui só se desenham guias;
// os pixels do texto são sempre rasterizados pelo Rust (`draw_text`).
import { useEditor } from "../../core/store";
import type { Point, Sprite } from "../../core/types";
import type { TextDraft } from "./state";

/** Clique com a ferramenta Texto: abre a caixa (ou a move, mantendo o texto digitado). */
export function startTextDraft(p: Point, clientX: number, clientY: number) {
  const s = useEditor.getState();
  if (s.spriteId === undefined) return;
  const prev = s.textDraft;
  s.set({
    textDraft: { at: p, screen: [clientX, clientY], sprite: s.spriteId, layer: s.layerId, frame: s.frameId, text: prev?.text ?? "", box: undefined },
  });
}

/** Desenha a caixa medida do texto em edição e as linhas do 9-slice. */
export function drawTextOverlays(
  g: CanvasRenderingContext2D,
  sprite: Sprite,
  ox: number,
  oy: number,
  z: number,
  s: { textDraft?: TextDraft; nineSliceGuides: boolean; tool: string },
) {
  const d = s.textDraft;
  if (d && s.tool === "text" && d.sprite === sprite.id) {
    g.save();
    g.setLineDash([4, 3]);
    g.lineWidth = 1;
    g.strokeStyle = "#5fe3ff";
    if (d.box && d.box.w > 0) g.strokeRect(ox + d.box.x * z + 0.5, oy + d.box.y * z + 0.5, d.box.w * z - 1, d.box.h * z - 1);
    g.setLineDash([]);
    g.strokeStyle = "#ffffff";
    g.strokeRect(ox + d.at[0] * z + 0.5, oy + d.at[1] * z + 0.5, z - 1, z - 1);
    g.restore();
  }
  const m = sprite.extras?.texto?.nine_slice;
  if (s.nineSliceGuides && m) {
    const W = sprite.width * z;
    const H = sprite.height * z;
    g.save();
    g.setLineDash([6, 4]);
    g.lineWidth = 1;
    g.strokeStyle = "rgba(255,64,200,0.95)";
    g.beginPath();
    for (const x of [m.left, sprite.width - m.right]) {
      g.moveTo(ox + x * z + 0.5, oy);
      g.lineTo(ox + x * z + 0.5, oy + H);
    }
    for (const y of [m.top, sprite.height - m.bottom]) {
      g.moveTo(ox, oy + y * z + 0.5);
      g.lineTo(ox + W, oy + y * z + 0.5);
    }
    g.stroke();
    g.restore();
  }
}
