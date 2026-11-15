// Tipos das ferramentas de desenho (pincéis, simetria, seleção por máscara).
import type { Rect } from "../../core/types";

/** Máscara de seleção como o Rust devolve (select_color, select_polygon, draw_transform): `#` = selecionado. */
export type SelectionMask = { bounds: Rect; rows: string[] };

/** Como uma seleção nova combina com a atual. */
export type SelectMode = "replace" | "add" | "subtract" | "intersect";

export type DitherPatternName = "bayer2" | "bayer4" | "bayer8" | "checker";

/** Simetria dos pincéis: eixos ligados e posição (undefined = centro do sprite; .5 = entre dois pixels). */
export type SymmetryState = { x: boolean; y: boolean; ax?: number; ay?: number };

/** Pixels copiados com selection_read, prontos para colar com a op `matrix`. */
export type ClipboardPixels = { rect: Rect; data: Record<string, unknown>; mask?: SelectionMask };
