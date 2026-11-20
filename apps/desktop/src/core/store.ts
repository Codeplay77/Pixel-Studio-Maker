// Estado da interface. O estado do projeto (modelo, pixels, histórico) vive no runtime; aqui fica
// só o que é da tela: seleção de layer/frame, ferramenta, cores, zoom, e uma cópia do modelo
// recebida do runtime para desenhar painéis.
import { create } from "zustand";
import * as api from "./api";
import type { Animation, ClientSummary, OpRecord, ProjectState, ProjectSummary, Rect, RuntimeEvent, Sprite } from "./types";
import type { ClipboardPixels, DitherPatternName, SelectionMask, SymmetryState } from "../ui/tools/types";
import type { TextDraft } from "../ui/texto/state";

export type Tool =
  | "pencil"
  | "eraser"
  | "fill"
  | "line"
  | "rect"
  | "ellipse"
  | "select"
  | "eyedropper"
  | "move"
  | "anchor"
  | "dither"
  | "shade"
  | "wand"
  | "lasso"
  | "text"
  | "hitbox";

export type Toast = { id: number; kind: "info" | "error"; text: string };

type Clipboard = { sprite: number; layer: number; frame: number; rect: Rect };

type EditorState = {
  connected: boolean;
  runtimeError?: string;
  projects: ProjectSummary[];
  clients: ClientSummary[];
  ops: OpRecord[];
  projectId?: string;
  doc?: ProjectState;
  spriteId?: number;
  layerId?: number;
  frameId?: number;
  animationId?: number;
  direction?: string | null;
  tool: Tool;
  primary: string;
  secondary: string;
  brush: number;
  filledShapes: boolean;
  zoom: number;
  grid: boolean;
  guides: boolean;
  showPreview: boolean;
  onion: { enabled: boolean; before: number; after: number };
  showRegions: boolean;
  showAnchors: boolean;
  showReferences: boolean;
  playing: boolean;
  selection?: Rect;
  clipboard?: Clipboard;
  /** Versão por frame: muda quando os pixels mudam, para recarregar a imagem. */
  frameEpoch: Record<number, number>;
  epoch: number;
  toasts: Toast[];
  panel:
    | "layers"
    | "palette"
    | "props"
    | "regions"
    | "anchors"
    | "inspect"
    | "tiles"
    | "hitboxes"
    | "text";
  mcpOpen: boolean;
  dialog?:
    | "new"
    | "export"
    | "import"
    | "pixelate"
    | "autotile"
    | "tilemap"
    | "exportJobs"
    | null;
  lastAgentActivity?: string;
  /** Anchor que a ferramenta "anchor" posiciona ao clicar no canvas. */
  anchorTarget?: string;
  /** Projeto cuja aba pediu para fechar com alterações não salvas (abre a confirmação). */
  closingProject?: string;

  /** Modo tile: o canvas mostra o sprite repetido em volta e os traços dão a volta nas bordas. */
  tileMode: boolean;
  /** Grade de tiles e colisões por cima do canvas (sprites com grade). */
  showTileGrid: boolean;
  /** Tile [coluna, linha] escolhido no painel Tiles. */
  tileSelected: [number, number];
  /** Caixa (nome) que a ferramenta "hitbox" posiciona ao arrastar no canvas. */
  hitboxTarget?: string;
  /** Mostrar as hitboxes do frame atual no canvas. */
  showHitboxes: boolean;
  /** Caixas ocultas no canvas (ids). */
  hiddenHitboxes: number[];
  /** No diálogo Exportar: também gravar a exportação como tarefa que roda ao salvar. */
  exportOnSave: boolean;
  /** Seleção por máscara (varinha, laço, Shift/Alt), calculada no Rust. `selection` é a caixa dela: o mesmo objeto que `selectionMask.bounds`. */
  selectionMask?: SelectionMask;
  /** Pixels copiados (selection_read); colar escreve exatamente estes pixels. */
  clipboardPixels?: ClipboardPixels;
  /** Simetria do lápis, da borracha e dos pincéis de dithering e sombreado. */
  symmetry: SymmetryState;
  ditherPattern: DitherPatternName;
  /** Densidade da cor no pincel de dithering (0 a 1). */
  ditherDensity: number;
  /** Pincel de dithering pinta a outra cor onde o padrão não pinta. */
  ditherFill: boolean;
  wandTolerance: number;
  wandContiguous: boolean;
  /** Varinha amostra a imagem composta (todas as layers visíveis). */
  wandComposite: boolean;
  /** Caixa aberta da ferramenta Texto (posição, texto e caixa medida pelo Rust). */
  textDraft?: TextDraft;
  /** Mostrar as linhas-guia do 9-slice no canvas. */
  nineSliceGuides: boolean;

  set: (p: Partial<EditorState>) => void;
  toast: (text: string, kind?: Toast["kind"]) => void;
  refresh: () => Promise<void>;
  openProject: (id: string) => Promise<void>;
  onEvent: (e: RuntimeEvent) => void;
  run: (name: string, args?: Record<string, unknown>) => Promise<any>;
};

let toastId = 0;
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
function scheduleRefresh() {
  clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => void useEditor.getState().refresh(), 100);
}

export const useEditor = create<EditorState>((set, get) => ({
  connected: false,
  projects: [],
  clients: [],
  ops: [],
  tool: "pencil",
  primary: "#2f6ad0",
  secondary: "#00000000",
  brush: 1,
  filledShapes: false,
  zoom: 8,
  grid: true,
  guides: false,
  showPreview: true,
  onion: { enabled: false, before: 1, after: 1 },
  showRegions: false,
  showAnchors: false,
  showReferences: true,
  playing: false,
  frameEpoch: {},
  epoch: 0,
  toasts: [],
  panel: "layers",
  mcpOpen: false,

  tileMode: false,
  showTileGrid: true,
  tileSelected: [0, 0],
  showHitboxes: true,
  hiddenHitboxes: [],
  exportOnSave: false,
  symmetry: { x: false, y: false },
  ditherPattern: "bayer4",
  ditherDensity: 0.5,
  ditherFill: false,
  wandTolerance: 0,
  wandContiguous: true,
  wandComposite: false,
  nineSliceGuides: false,

  set: (p) => set(p),

  toast: (text, kind = "info") => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => set({ toasts: get().toasts.filter((t) => t.id !== id) }), kind === "error" ? 6000 : 3000);
  },

  refresh: async () => {
    const { projectId } = get();
    if (!projectId) return;
    try {
      const doc = await api.getState(projectId);
      const s = get();
      if (s.projectId !== projectId) return;
      const sprite = doc.model.sprites.find((x) => x.id === s.spriteId) ?? doc.model.sprites[0];
      const patch: Partial<EditorState> = { doc };
      if (sprite) {
        patch.spriteId = sprite.id;
        if (!sprite.layers.some((l) => l.id === s.layerId)) {
          const top = [...sprite.layers].reverse().find((l) => !l.reference) ?? sprite.layers[sprite.layers.length - 1];
          patch.layerId = top?.id;
        }
        if (!sprite.frames.some((f) => f.id === s.frameId)) patch.frameId = sprite.frames[0]?.id;
        if (s.animationId !== undefined && !sprite.animations.some((a) => a.id === s.animationId)) {
          patch.animationId = undefined;
          patch.direction = undefined;
        }
      }
      set(patch);
    } catch (e) {
      if (api.errorCode(e) === "NO_PROJECT") set({ projectId: undefined, doc: undefined });
      else get().toast(api.errorMessage(e), "error");
    }
  },

  openProject: async (id) => {
    await api.selectProject(id).catch(() => {});
    set({ projectId: id, spriteId: undefined, layerId: undefined, frameId: undefined, animationId: undefined, direction: undefined, selection: undefined, selectionMask: undefined });
    await get().refresh();
  },

  onEvent: (e) => {
    const s = get();
    switch (e.event) {
      case "changed": {
        if (e.project !== s.projectId) return;
        const frames = { ...s.frameEpoch };
        let structural = false;
        for (const c of e.changes) {
          if (c.kind === "structure" || c.frame === undefined) structural = true;
          else frames[c.frame] = (frames[c.frame] ?? 0) + 1;
        }
        const agent = e.source !== "Editor";
        set({ frameEpoch: frames, epoch: s.epoch + (structural ? 1 : 0), lastAgentActivity: agent ? `${e.source}: ${e.command}` : s.lastAgentActivity });
        scheduleRefresh();
        return;
      }
      case "operation":
        set({ ops: [{ ...e }, ...s.ops].slice(0, 200) });
        return;
      case "projects": {
        set({ projects: e.projects });
        if (!s.projectId && e.projects.length > 0) void get().openProject(e.projects[e.projects.length - 1].id);
        if (s.projectId && !e.projects.some((p) => p.id === s.projectId)) set({ projectId: undefined, doc: undefined });
        if (s.projectId) void get().refresh();
        return;
      }
      case "clients":
        set({ clients: e.clients });
        return;
      case "resync":
        set({ epoch: s.epoch + 1 });
        void get().refresh();
        return;
    }
  },

  run: async (name, args = {}) => {
    const s = get();
    try {
      const r = await api.call(name, args, s.projectId);
      if (r.project && r.project !== s.projectId) await get().openProject(r.project);
      for (const w of r.warnings ?? []) if (w !== "nada mudou") get().toast(w);
      return r;
    } catch (e) {
      get().toast(api.errorMessage(e), "error");
      throw e;
    }
  },
}));

export function currentSprite(s: { doc?: ProjectState; spriteId?: number }): Sprite | undefined {
  return s.doc?.model.sprites.find((x) => x.id === s.spriteId) ?? s.doc?.model.sprites[0];
}

export function currentAnimation(s: { doc?: ProjectState; spriteId?: number; animationId?: number }): Animation | undefined {
  return currentSprite(s)?.animations.find((a) => a.id === s.animationId);
}

/** Frames que a linha do tempo mostra: a trilha escolhida, ou todos os frames do sprite. */
export function timelineFrames(s: { doc?: ProjectState; spriteId?: number; animationId?: number; direction?: string | null }): number[] {
  const sprite = currentSprite(s);
  if (!sprite) return [];
  const anim = currentAnimation(s);
  if (!anim) return sprite.frames.map((f) => f.id);
  const track = anim.tracks.find((t) => (t.direction ?? null) === (s.direction ?? null)) ?? anim.tracks[0];
  return track?.frames ?? [];
}

/** Paleta em uso: a do sprite, senão a ativa do projeto. */
export function currentPalette(s: { doc?: ProjectState; spriteId?: number }) {
  const sprite = currentSprite(s);
  const m = s.doc?.model;
  const name = sprite?.palette ?? m?.active_palette;
  return m?.palettes.find((p) => p.name === name) ?? m?.palettes[0];
}
