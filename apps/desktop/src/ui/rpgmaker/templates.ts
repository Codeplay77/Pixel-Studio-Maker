// Modelos de arquivo do RPG Maker para a interface. Quem cria as regiões, valida o tamanho e
// grava os dados no sprite é o comando rpgmaker_template (Rust, o mesmo do MCP); aqui só ficam os
// nomes e o tamanho de cada modelo, para criar o projeto já no tamanho certo.

export type RpgEngine = "mz" | "mv" | "vxace";

export type RpgKind =
  | "tileset_a1"
  | "tileset_a2"
  | "tileset_a3"
  | "tileset_a4"
  | "tileset_a5"
  | "tileset_b"
  | "tileset_c"
  | "tileset_d"
  | "tileset_e"
  | "character"
  | "face"
  | "sv_battler"
  | "window";

/** Espelha `RpgMakerInfo` (crates/sprite-animation/src/rpgmaker.rs). */
export type RpgMakerInfo = { kind: RpgKind; engine: RpgEngine; tile: number; cell?: [number, number]; single?: boolean };

export const RPG_ENGINES: { id: RpgEngine; label: string }[] = [
  { id: "mz", label: "RPG Maker MZ" },
  { id: "mv", label: "RPG Maker MV" },
  { id: "vxace", label: "RPG Maker VX Ace" },
];

export const RPG_KINDS: { id: RpgKind; label: string }[] = [
  { id: "tileset_a1", label: "Tileset A1 (água e cascatas)" },
  { id: "tileset_a2", label: "Tileset A2 (chão)" },
  { id: "tileset_a3", label: "Tileset A3 (telhados e paredes)" },
  { id: "tileset_a4", label: "Tileset A4 (paredes)" },
  { id: "tileset_a5", label: "Tileset A5 (tiles comuns)" },
  { id: "tileset_b", label: "Tileset B (objetos)" },
  { id: "tileset_c", label: "Tileset C (objetos)" },
  { id: "tileset_d", label: "Tileset D (objetos)" },
  { id: "tileset_e", label: "Tileset E (objetos)" },
  { id: "character", label: "Personagens (characters)" },
  { id: "face", label: "Faces" },
  { id: "sv_battler", label: "Battler lateral (sv_actors)" },
  { id: "window", label: "Janela (Window.png)" },
];

export const tileOf = (engine: RpgEngine) => (engine === "vxace" ? 32 : 48);

/** Tamanho do arquivo em px, o mesmo que rpgmaker_template exige; `null` se o motor não tem o arquivo. */
export function templateSize(kind: RpgKind, engine: RpgEngine, single = false): [number, number] | null {
  const t = tileOf(engine);
  switch (kind) {
    case "tileset_a1":
    case "tileset_a2":
      return [16 * t, 12 * t];
    case "tileset_a3":
      return [16 * t, 8 * t];
    case "tileset_a4":
      return [16 * t, 15 * t];
    case "tileset_a5":
      return [8 * t, 16 * t];
    case "tileset_b":
    case "tileset_c":
    case "tileset_d":
    case "tileset_e":
      return [16 * t, 16 * t];
    case "character":
      return single ? [3 * t, 4 * t] : [12 * t, 8 * t];
    case "face":
      return [12 * t, 6 * t];
    case "sv_battler":
      return engine === "vxace" ? null : [576, 384];
    case "window":
      return [4 * t, 4 * t];
  }
}

/** Movimentos do battler lateral, na ordem do RPG Maker: [chave, nome em português]. */
export const SV_MOTIONS: [string, string][] = [
  ["walk", "andar"],
  ["wait", "esperar"],
  ["chant", "entoar"],
  ["guard", "defender"],
  ["damage", "dano"],
  ["evade", "esquivar"],
  ["thrust", "estocar"],
  ["swing", "brandir"],
  ["missile", "disparar"],
  ["skill", "habilidade"],
  ["spell", "magia"],
  ["item", "item"],
  ["escape", "fugir"],
  ["victory", "vitória"],
  ["dying", "agonizando"],
  ["abnormal", "estado anormal"],
  ["sleep", "dormir"],
  ["dead", "morto"],
];

/** Escolha de modelo no diálogo de projeto novo (`kind` vazio = em branco). */
export type RpgTemplateChoice = { kind: RpgKind | ""; engine: RpgEngine; single: boolean; textColors: boolean };

export const BLANK_TEMPLATE: RpgTemplateChoice = { kind: "", engine: "mz", single: false, textColors: true };

export function choiceSize(c: RpgTemplateChoice): [number, number] | null {
  return c.kind ? templateSize(c.kind, c.engine, c.single) : null;
}
