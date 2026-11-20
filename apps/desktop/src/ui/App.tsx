import { listen } from "@tauri-apps/api/event";
import { ArrowRight, Bot, FilePlus2, FolderOpen, Grid3x3, Keyboard, Sparkles } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import * as api from "../core/api";
import { useKey } from "../core/hooks";
import { timelineFrames, useEditor } from "../core/store";
import type { RuntimeEvent } from "../core/types";
import { BrandMark } from "./Brand";
import { CanvasView } from "./CanvasView";
import { ExportDialog, ImportDialog, NewProjectDialog } from "./Dialogs";
import { McpPanel } from "./McpPanel";
import { PixelateDialog } from "./PixelateDialog";
import { RightPanel } from "./RightPanel";
import { Timeline } from "./Timeline";
import { selectionActions, ToolOptions, Toolbar } from "./Toolbar";
import { openProjectDialog, requestCloseProject, saveProject, TopBar } from "./TopBar";
import { AutotileDialog, TileMapDialog } from "./tiles/TileDialogs";
import { ExportJobsDialog } from "./godot/ExportJobs";
import { FERRAMENTAS_KEYS } from "./tools/ToolsUi";
import { TextBox } from "./texto/TextBox";

function Welcome() {
  const s = useEditor();
  const [recover, setRecover] = useRecoverable();
  return (
    <main className="welcome-page">
      <div className="welcome-content">
        <div className="welcome-heading">
          <span className="eyebrow"><span className="status-dot bg-accent" /> UM PIXEL. INFINITAS POSSIBILIDADES.</span>
          <h1>Pequenos pixels.<br /><span>Grandes ideias.</span></h1>
          <p>Um espaço para desenhar, animar e dar vida aos seus sprites.<br />Comece do zero ou continue de onde a inspiração parou.</p>
        </div>
        <div className="start-grid">
          <button className="start-card start-card-primary" disabled={!s.connected} onClick={() => s.set({ dialog: "new" })}>
            <span className="start-icon"><FilePlus2 size={23} /></span><span className="start-card-title">Criar um projeto <ArrowRight size={18} /></span><span className="start-card-description">Uma tela em branco para a sua próxima ideia.</span><kbd>Ctrl + N</kbd>
          </button>
          <button className="start-card" disabled={!s.connected} onClick={() => void openProjectDialog()}>
            <span className="start-icon"><FolderOpen size={23} /></span><span className="start-card-title">Abrir projeto <ArrowRight size={18} /></span><span className="start-card-description">Continue seus desenhos e animações.</span><kbd>Ctrl + O</kbd>
          </button>
          <button className="start-card" disabled={!s.connected} onClick={() => s.set({ dialog: "pixelate" })}>
            <span className="start-icon"><Grid3x3 size={23} /></span><span className="start-card-title">Converter imagem <ArrowRight size={18} /></span><span className="start-card-description">Transforme uma foto ou ilustração em pixel art.</span><span className="file-types">PNG · JPG</span>
          </button>
        </div>
        {recover.length > 0 && <section className="recovery-card">
          <h2 className="panel-title">Recuperar trabalho não salvo</h2>
          {recover.map((r) => <button key={r.autosave} className="btn mt-1 w-full justify-between" onClick={() => void s.run("project_open", { path: r.autosave, recover: true }).then(() => setRecover([]))}>
            <span>{r.name ?? "Sem nome"}</span><span className="text-xs text-muted">{r.saved_at}</span>
          </button>)}
        </section>}
        <button className="assistant-card" onClick={() => s.set({ mcpOpen: true })}>
          <span className="assistant-icon"><Bot size={22} /></span>
          <span><strong>Crie também com o seu assistente</strong><small>Conecte o Claude Code e acompanhe cada pixel no editor.</small></span>
          <span className="assistant-link">Configurar MCP <ArrowRight size={16} /></span>
        </button>
        <div className="welcome-tips"><Keyboard size={16} /><span>Seu próximo desenho começa com um atalho.</span><span><kbd>B</kbd> Lápis</span><span><kbd>E</kbd> Borracha</span><span><kbd>Espaço</kbd> Mover tela</span></div>
        {!s.connected && <p className="connection-notice" role="status">{api.isTauri() ? (s.runtimeError ? "Não foi possível conectar ao motor de edição. Consulte o painel MCP." : "Conectando ao motor de edição…") : "Prévia da interface. Abra o aplicativo desktop para criar e editar projetos."}</p>}
      </div>
      <footer className="welcome-footer"><span className="flex items-center gap-2"><BrandMark size={20} /> Pixel Studio Maker</span><span className="flex items-center gap-2"><Sparkles size={13} /> Feito para criar, pixel por pixel.</span></footer>
    </main>
  );
}

type Recoverable = { autosave: string; name?: string; saved_at?: string };
function useRecoverable(): [Recoverable[], (r: Recoverable[]) => void] {
  const connected = useEditor((s) => s.connected);
  const [list, setList] = useState<Recoverable[]>([]);
  useEffect(() => {
    if (!connected) return;
    void api
      .call("project_list")
      .then((r) => setList(r.data.recoverable ?? []))
      .catch(() => {});
  }, [connected]);
  return [list, setList];
}

function Toasts() {
  const toasts = useEditor((s) => s.toasts);
  return (
    <div role="status" aria-live="polite" className="pointer-events-none fixed right-4 bottom-40 z-50 flex flex-col items-end gap-2">
      {toasts.map((t) => (
        <div key={t.id} className={`max-w-md rounded-lg px-3 py-2 shadow-xl ring-1 ${t.kind === "error" ? "bg-[#3a1620] text-danger ring-danger/40" : "bg-raised text-ink ring-line"}`}>
          {t.text}
        </div>
      ))}
    </div>
  );
}

export function App() {
  const s = useEditor();

  useEffect(() => {
    if (!api.isTauri()) return;
    const un1 = listen<RuntimeEvent>("runtime-event", (e) => useEditor.getState().onEvent(e.payload));
    const un2 = listen<{ connected: boolean; error?: string }>("runtime-status", (e) => {
      const st = useEditor.getState();
      st.set({ connected: e.payload.connected, runtimeError: e.payload.error });
      if (e.payload.connected) {
        void api.call("project_list").then((r) => {
          const open = r.data.open ?? [];
          st.set({ projects: open });
          if (open.length && !useEditor.getState().projectId) void st.openProject(open[open.length - 1].id);
          else if (useEditor.getState().projectId) void st.refresh();
        });
        void api.call("editor_status").then((r) => st.set({ clients: r.data.clients ?? [], ops: (r.data.recent_operations ?? []) }));
      }
    });
    // In case the connection came up before the listeners.
    void api.runtimeInfo().then((i) => i.connected && useEditor.getState().set({ connected: true }));
    void api.call("project_list").then((r) => {
      const open = r.data.open ?? [];
      useEditor.getState().set({ projects: open, connected: true });
      if (open.length) void useEditor.getState().openProject(open[open.length - 1].id);
    }).catch(() => {});
    return () => {
      void un1.then((f) => f());
      void un2.then((f) => f());
    };
  }, []);

  const onKey = useCallback((e: KeyboardEvent) => {
    const st = useEditor.getState();
    if (e.defaultPrevented || document.querySelector("dialog[open]")) return;
    if (st.dialog) {
      if (e.key === "Escape") st.set({ dialog: null });
      return;
    }
    if ((e.key === "Enter" || e.key === " ") && (e.target as HTMLElement)?.closest("button")) return;
    const ctrl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (ctrl && k === "z" && !e.shiftKey) return e.preventDefault(), void st.run("history_undo", { steps: 1 }).catch(() => {});
    if (ctrl && (k === "y" || (k === "z" && e.shiftKey))) return e.preventDefault(), void st.run("history_redo", { steps: 1 }).catch(() => {});
    if (ctrl && k === "s") return e.preventDefault(), void saveProject(e.shiftKey);
    if (ctrl && k === "o") return e.preventDefault(), void openProjectDialog();
    if (ctrl && k === "n") return e.preventDefault(), st.set({ dialog: "new" });
    if (ctrl && k === "w") {
      const p = st.projects.find((x) => x.id === st.projectId);
      return e.preventDefault(), p && requestCloseProject(p);
    }
    if (ctrl && k === "c") return selectionActions().copy();
    if (ctrl && k === "x") return selectionActions().cut();
    if (ctrl && k === "v") return selectionActions().paste();
    if (ctrl && k === "a") {
      const sp = st.doc?.model.sprites.find((x) => x.id === st.spriteId);
      return e.preventDefault(), sp && st.set({ selection: { x: 0, y: 0, w: sp.width, h: sp.height } });
    }
    if (ctrl) return;
    const tools: Record<string, Parameters<typeof st.set>[0]["tool"]> = { b: "pencil", e: "eraser", g: "fill", l: "line", r: "rect", o: "ellipse", m: "select", v: "move", i: "eyedropper", a: "anchor" };
    if (tools[k]) return st.set({ tool: tools[k] });
    if (FERRAMENTAS_KEYS[k]) return st.set({ tool: FERRAMENTAS_KEYS[k] });
    if (k === "t") return st.set({ tool: "text" });
    if (k === "h") return st.set({ tool: "hitbox" });
    if (k === "delete" || k === "backspace") return selectionActions().remove();
    if (k === "escape") return st.set({ selection: undefined, selectionMask: undefined, mcpOpen: false, dialog: null });
    if (k === "x") return st.set({ primary: st.secondary, secondary: st.primary });
    if (k === "[") return st.set({ brush: Math.max(1, st.brush - 1) });
    if (k === "]") return st.set({ brush: Math.min(8, st.brush + 1) });
    if (k === "#") return st.set({ grid: !st.grid });
    if (k === "enter") return st.set({ playing: !st.playing });
    if (k === "," || k === ".") {
      const frames = timelineFrames(st);
      if (!frames.length) return;
      const i = Math.max(0, frames.indexOf(st.frameId ?? frames[0]));
      st.set({ frameId: frames[(i + (k === "," ? -1 : 1) + frames.length) % frames.length], playing: false });
    }
  }, []);
  useKey(onKey);

  return (
    <div className="relative flex h-full flex-col">
      <TopBar />
      {s.projectId && s.doc ? (
        <div className="flex min-h-0 flex-1">
          <Toolbar />
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <ToolOptions />
            <CanvasView />
            <Timeline />
          </div>
          <RightPanel />
        </div>
      ) : (
        <Welcome />
      )}
      <McpPanel />
      {s.dialog === "new" && <NewProjectDialog />}
      {s.dialog === "import" && <ImportDialog />}
      {s.dialog === "export" && <ExportDialog />}
      {s.dialog === "pixelate" && <PixelateDialog />}
      {s.dialog === "autotile" && <AutotileDialog />}
      {s.dialog === "tilemap" && <TileMapDialog />}
      {s.dialog === "exportJobs" && <ExportJobsDialog />}
      <TextBox />
      <Toasts />
    </div>
  );
}
