// Estado da interface das ferramentas de texto (fontes e 9-slice). Só preferências de tela:
// medir, desenhar, definir fontes e renderizar o 9-slice são comandos do runtime (Rust).
import { create } from "zustand";
import type { Point, Rect, Sprite } from "../../core/types";

export type TextAlign = "left" | "center" | "right";
export type SliceMode = "stretch" | "tile" | "tile_fit";

/** Caixa flutuante da ferramenta Texto, aberta ao clicar no canvas. */
export type TextDraft = {
  /** Pixel do sprite onde o texto começa. */
  at: Point;
  /** Posição do clique na tela, para colocar a caixa. */
  screen: [number, number];
  sprite: number;
  layer?: number;
  frame?: number;
  text: string;
  /** Caixa medida pelo Rust (`text_measure`), em pixels do sprite. */
  box?: Rect;
};

export const BUILTIN_FONTS = ["pixel-5x7", "mini-3x5"] as const;

export const SLICE_MODES: { id: SliceMode; label: string }[] = [
  { id: "stretch", label: "Esticar" },
  { id: "tile", label: "Repetir" },
  { id: "tile_fit", label: "Repetir encaixando" },
];

type TextUi = {
  /** Fonte da ferramenta Texto: embutida ou nome de um sprite-fonte. */
  font: string;
  scale: number;
  align: TextAlign;
  /** Fontes de sprite: manter as cores desenhadas em vez da cor principal. */
  keepColors: boolean;
  /** Prévia do 9-slice no painel. */
  edges: SliceMode;
  center: SliceMode;
  /** Modos do NinePatchRect na exportação para o Godot. */
  axisH: SliceMode;
  axisV: SliceMode;
  set: (p: Partial<TextUi>) => void;
};

export const useTextUi = create<TextUi>((set) => ({
  font: "pixel-5x7",
  scale: 1,
  align: "left",
  keepColors: false,
  edges: "stretch",
  center: "stretch",
  axisH: "stretch",
  axisV: "stretch",
  set: (p) => set(p),
}));

/** Sprites do projeto definidos como fonte. */
export function spriteFonts(sprites: Sprite[]): Sprite[] {
  return sprites.filter((sp) => !!sp.extras?.texto?.font);
}

export function isBuiltinFont(name: string): boolean {
  return (BUILTIN_FONTS as readonly string[]).includes(name);
}

/** Argumentos dos eixos do NinePatchRect para `export_nine_slice`. */
export function nineSliceAxes() {
  const u = useTextUi.getState();
  return { axis_stretch_horizontal: u.axisH, axis_stretch_vertical: u.axisV };
}

/** Imagem devolvida por um comando, como URL de dados. */
export function imageUrl(result: { images?: { mime: string; base64: string }[] } | undefined): string | undefined {
  const img = result?.images?.[0];
  return img ? `data:${img.mime};base64,${img.base64}` : undefined;
}
