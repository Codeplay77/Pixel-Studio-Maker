// Hitboxes no canvas: desenho das caixas do frame atual e o arraste da ferramenta "hitbox".
// O retângulo é gravado pelo comando `hitbox_set` (Rust); aqui só se desenha e se envia.
import { currentSprite, useEditor } from "../../core/store";
import type { Point, Rect, Sprite } from "../../core/types";

export type Hitbox = NonNullable<NonNullable<NonNullable<Sprite["extras"]>["godot"]>["hitboxes"]>[number];
export type HitboxKind = Hitbox["kind"];

/** Cores padrão por tipo (as mesmas do motor). */
export const KIND_COLORS: Record<HitboxKind, string> = {
  hitbox: "#ff3b30",
  hurtbox: "#34c759",
  collision: "#0a84ff",
  custom: "#ffcc00",
};

export const KIND_LABELS: Record<HitboxKind, string> = {
  hitbox: "Hitbox (ataque)",
  hurtbox: "Hurtbox (corpo)",
  collision: "Colisão",
  custom: "Outra",
};

export function hitboxesOf(sprite?: Sprite): Hitbox[] {
  return sprite?.extras?.godot?.hitboxes ?? [];
}

export function hitboxColor(h: Hitbox): string {
  return h.color?.slice(0, 7) ?? KIND_COLORS[h.kind];
}

export function rectAt(h: Hitbox, frame?: number): Rect | undefined {
  return frame === undefined ? undefined : h.frames?.[String(frame)];
}

function normRect(a: Point, b: Point): Rect {
  const x = Math.min(a[0], b[0]);
  const y = Math.min(a[1], b[1]);
  return { x, y, w: Math.abs(a[0] - b[0]) + 1, h: Math.abs(a[1] - b[1]) + 1 };
}

type View = { frameId?: number; showHitboxes: boolean; hiddenHitboxes: number[]; hitboxTarget?: string; tool: string };

/** Caixas do frame atual (preenchimento leve, contorno e nome) e o retângulo sendo arrastado. */
export function drawHitboxes(g: CanvasRenderingContext2D, sprite: Sprite, s: View, ox: number, oy: number, z: number, drag?: { from: Point; to: Point }) {
  const boxes = hitboxesOf(sprite);
  if ((s.showHitboxes || s.tool === "hitbox") && s.frameId !== undefined) {
    for (const h of boxes) {
      const r = rectAt(h, s.frameId);
      if (!r || s.hiddenHitboxes.includes(h.id)) continue;
      const color = hitboxColor(h);
      const active = s.tool === "hitbox" && s.hitboxTarget === h.name;
      g.globalAlpha = 0.16;
      g.fillStyle = color;
      g.fillRect(ox + r.x * z, oy + r.y * z, r.w * z, r.h * z);
      g.globalAlpha = 1;
      g.strokeStyle = color;
      g.lineWidth = active ? 3 : 2;
      g.strokeRect(ox + r.x * z + 1, oy + r.y * z + 1, r.w * z - 2, r.h * z - 2);
      g.font = "11px Segoe UI";
      g.fillStyle = "#000";
      g.fillText(h.name, ox + r.x * z + 4, oy + r.y * z - 3);
      g.fillStyle = color;
      g.fillText(h.name, ox + r.x * z + 3, oy + r.y * z - 4);
    }
  }
  if (drag) {
    const target = boxes.find((h) => h.name === s.hitboxTarget);
    const r = normRect(drag.from, drag.to);
    g.setLineDash([6, 4]);
    g.lineWidth = 2;
    g.strokeStyle = target ? hitboxColor(target) : "#ffffff";
    g.strokeRect(ox + r.x * z + 1, oy + r.y * z + 1, r.w * z - 2, r.h * z - 2);
    g.setLineDash([]);
  }
}

/** Fim do arraste: grava o retângulo (recortado ao canvas) na caixa escolhida, no frame atual. */
export async function commitHitboxDrag(from: Point, to: Point) {
  const s = useEditor.getState();
  const sprite = currentSprite(s);
  if (!sprite || !s.hitboxTarget || s.frameId === undefined) return;
  const r = normRect(from, to);
  const x = Math.max(0, r.x);
  const y = Math.max(0, r.y);
  const w = Math.min(sprite.width, r.x + r.w) - x;
  const h = Math.min(sprite.height, r.y + r.h) - y;
  if (w <= 0 || h <= 0) return;
  await s.run("hitbox_set", { sprite: sprite.id, hitbox: s.hitboxTarget, rect: { x, y, w, h }, frame: s.frameId }).catch(() => {});
}
