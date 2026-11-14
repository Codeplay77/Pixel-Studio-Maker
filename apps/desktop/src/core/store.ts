// Estado da interface. O estado do projeto (modelo, pixels, histórico) vive no runtime; aqui fica
// só o que é da tela: seleção de layer/frame, ferramenta, cores, zoom, e uma cópia do modelo
// recebida do runtime para desenhar painéis.
import { create } from "zustand";
import * as api from "./api";
import type { Animation, ClientSummary, OpRecord, ProjectState, ProjectSummary, Rect, RuntimeEvent, Sprite } from "./types";

export type Tool = "pencil" | "eraser" | "fill" | "line" | "rect" | "ellipse" | "select" | "eyedropper" | "move" | "anchor";

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
  panel: "layers" | "palette" | "props" | "regions" | "anchors" | "inspect";
  mcpOpen: boolean;
  dialog?: "new" | "export" | "import" | null;
  lastAgentActivity?: string;
  /** Anchor que a ferramenta "anchor" posiciona ao clicar no canvas. */
  anchorTarget?: string;

  set: (p: Partial<EditorState>) => void;
  toast: (text: string, kind?: Toast["kind"]) => void;
  refresh: () => Promise<void>;
  openProject: (id: string) => Promise<void>;
  onEvent: (e: RuntimeEvent) => void;
  run: (name: string, args?: Record<string, unknown>) => Promise<any>;
};

let toastId = 0;

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
      const sprite = doc.model.sprites.find((x) => x.id === s.spriteId) ?? doc.model.sprites[0];
      const patch: Partial<EditorState> = { doc, epoch: s.epoch + 1 };
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
    set({ projectId: id, spriteId: undefined, layerId: undefined, frameId: undefined, animationId: undefined, direction: undefined, selection: undefined });
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
        void get().refresh();
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
