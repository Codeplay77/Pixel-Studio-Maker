// Desenho do modo tile e da grade de tiles no canvas principal. Só desenha: os dados vêm do runtime.
import type { Sprite } from "../../core/types";
import { gridSize, tileData, tileRect, type TileCoord } from "../../core/tiles";

/** Cópias do frame nas 8 posições em volta (antes do frame principal), um pouco apagadas. */
export function drawTileRepeat(g: CanvasRenderingContext2D, frame: HTMLCanvasElement, ox: number, oy: number, w: number, h: number) {
  g.save();
  for (let dy = -1; dy <= 1; dy++) {
    for (let dx = -1; dx <= 1; dx++) {
      if (!dx && !dy) continue;
      const x = ox + dx * w;
      const y = oy + dy * h;
      g.globalAlpha = 1;
      g.fillStyle = "#2b3033";
      g.fillRect(x, y, w, h);
      g.globalAlpha = 0.5;
      g.drawImage(frame, x, y, w, h);
    }
  }
  g.restore();
  g.strokeStyle = "rgba(95,227,255,0.35)";
  g.lineWidth = 1;
  g.setLineDash([4, 4]);
  g.strokeRect(ox - w + 0.5, oy - h + 0.5, w * 3 - 1, h * 3 - 1);
  g.setLineDash([]);
}

/** Grade de tiles, colisões e o tile escolhido, sobre o canvas (sprites com grade). */
export function drawTileOverlay(g: CanvasRenderingContext2D, sprite: Sprite, ox: number, oy: number, z: number, showGrid: boolean, selected?: TileCoord) {
  const d = tileData(sprite);
  if (!d || !showGrid) return;
  const { columns, rows } = gridSize(sprite, d);
  g.save();
  g.lineWidth = 1;
  g.strokeStyle = "rgba(0,229,255,0.75)";
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < columns; c++) {
      const t = tileRect(d, [c, r]);
      g.strokeRect(ox + t.x * z + 0.5, oy + t.y * z + 0.5, t.w * z - 1, t.h * z - 1);
    }
  }
  for (const col of d.collisions ?? []) {
    const t = tileRect(d, col.tile);
    for (const poly of col.polygons) {
      g.beginPath();
      poly.forEach(([x, y], i) => (i ? g.lineTo(ox + (t.x + x) * z, oy + (t.y + y) * z) : g.moveTo(ox + (t.x + x) * z, oy + (t.y + y) * z)));
      g.closePath();
      g.fillStyle = "rgba(255,64,129,0.22)";
      g.fill();
      g.strokeStyle = "rgba(255,64,129,0.95)";
      g.stroke();
    }
  }
  if (selected && selected[0] < columns && selected[1] < rows) {
    const t = tileRect(d, selected);
    g.strokeStyle = "#ffd600";
    g.lineWidth = 2;
    g.strokeRect(ox + t.x * z + 1, oy + t.y * z + 1, t.w * z - 2, t.h * z - 2);
  }
  g.restore();
}
