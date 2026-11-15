// Tipos dos dados de tiles (espelham crates/sprite-drawing/src/tiles.rs) e contas de grade.
// Só leitura e desenho: toda alteração passa pelos comandos tile_* do runtime.
import type { Point, Rect, Sprite } from "./types";

export type TileCoord = [number, number];
export type TerrainMode = "match_corners_and_sides" | "match_corners" | "match_sides";
export type PeeringBit =
  | "right_side"
  | "bottom_right_corner"
  | "bottom_side"
  | "bottom_left_corner"
  | "left_side"
  | "top_left_corner"
  | "top_side"
  | "top_right_corner";

export type Terrain = { name: string; color: string };
export type TerrainSet = { name: string; mode: TerrainMode; terrains: Terrain[] };
export type TileTerrain = { tile: TileCoord; set: number; terrain?: number; peering?: Partial<Record<PeeringBit, number>> };
export type TileCollision = { tile: TileCoord; polygons: Point[][] };
export type TileMap = { width: number; height: number; cells: (TileCoord | null)[] };

export type TileData = {
  tile_width: number;
  tile_height: number;
  margin?: [number, number];
  spacing?: [number, number];
  collisions?: TileCollision[];
  terrain_sets?: TerrainSet[];
  terrains?: TileTerrain[];
  preview_map?: TileMap;
};

export const TERRAIN_MODES: { id: TerrainMode; label: string }[] = [
  { id: "match_corners_and_sides", label: "Cantos e lados (blob 47)" },
  { id: "match_corners", label: "Só cantos" },
  { id: "match_sides", label: "Só lados" },
];

/** Bits em volta do centro, numa grade 3x3 (null = o centro). */
export const BIT_GRID: (PeeringBit | null)[][] = [
  ["top_left_corner", "top_side", "top_right_corner"],
  ["left_side", null, "right_side"],
  ["bottom_left_corner", "bottom_side", "bottom_right_corner"],
];

export function bitAllowed(mode: TerrainMode, bit: PeeringBit): boolean {
  const corner = bit.endsWith("_corner");
  return mode === "match_corners_and_sides" || (mode === "match_corners" ? corner : !corner);
}

export function tileData(sprite?: Sprite): TileData | undefined {
  return sprite?.extras?.tiles ?? undefined;
}

/** Colunas e linhas da grade (mesma regra do Rust e do atlas do Godot). */
export function gridSize(sprite: Sprite, d: TileData): { columns: number; rows: number } {
  const [mx, my] = d.margin ?? [0, 0];
  const [sx, sy] = d.spacing ?? [0, 0];
  const fit = (size: number, margin: number, tile: number, spacing: number) => (tile <= 0 || size < margin + tile ? 0 : 1 + Math.floor((size - margin - tile) / (tile + spacing)));
  return { columns: fit(sprite.width, mx, d.tile_width, sx), rows: fit(sprite.height, my, d.tile_height, sy) };
}

export function tileRect(d: TileData, [c, r]: TileCoord): Rect {
  const [mx, my] = d.margin ?? [0, 0];
  const [sx, sy] = d.spacing ?? [0, 0];
  return { x: mx + c * (d.tile_width + sx), y: my + r * (d.tile_height + sy), w: d.tile_width, h: d.tile_height };
}

/** Tile sob um pixel do sprite (ou undefined fora da grade, na margem ou no espaçamento). */
export function tileAt(sprite: Sprite, d: TileData, x: number, y: number): TileCoord | undefined {
  const { columns, rows } = gridSize(sprite, d);
  const [mx, my] = d.margin ?? [0, 0];
  const [sx, sy] = d.spacing ?? [0, 0];
  const c = Math.floor((x - mx) / (d.tile_width + sx));
  const r = Math.floor((y - my) / (d.tile_height + sy));
  if (c < 0 || r < 0 || c >= columns || r >= rows) return undefined;
  const rect = tileRect(d, [c, r]);
  if (x >= rect.x + rect.w || y >= rect.y + rect.h) return undefined;
  return [c, r];
}

export function sameTile(a?: TileCoord, b?: TileCoord): boolean {
  return !!a && !!b && a[0] === b[0] && a[1] === b[1];
}

export function collisionOf(d: TileData, t: TileCoord): TileCollision | undefined {
  return d.collisions?.find((c) => sameTile(c.tile, t));
}

export function terrainOf(d: TileData, t: TileCoord): TileTerrain | undefined {
  return d.terrains?.find((c) => sameTile(c.tile, t));
}
