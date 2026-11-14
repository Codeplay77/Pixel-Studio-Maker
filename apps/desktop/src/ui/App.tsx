import { listen } from "@tauri-apps/api/event";
import { Bot, FilePlus2, FolderOpen } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import * as api from "../core/api";
import { useKey } from "../core/hooks";
import { timelineFrames, useEditor } from "../core/store";
import type { RuntimeEvent } from "../core/types";
import { BrandMark } from "./Brand";
import { CanvasView } from "./CanvasView";
import { ExportDialog, ImportDialog, NewProjectDialog } from "./Dialogs";
import { McpPanel } from "./McpPanel";
import { RightPanel } from "./RightPanel";
import { Timeline } from "./Timeline";
import { selectionActions, Toolbar } from "./Toolbar";
import { openProjectDialog, saveProject, TopBar } from "./TopBar";

function Welcome() {
  const s = useEditor();
  const [recover, setRecover] = useRecoverable();
  return (
    <div className="flex flex-1 items-center justify-center">
      <div className="flex w-[460px] flex-col items-center gap-5 text-center">
        <BrandMark size={72} />
        <div>
          <h1 className="bg-linear-to-r from-accent to-accent-2 bg-clip-text text-3xl font-bold tracking-tight text-transparent">Pixel Studio Maker</h1>
          <p className="mt-1 text-muted">Pixel art e sprite sheets, à mão ou pelo Claude.</p>
        </div>
        <div className="flex gap-2">
          <button className="btn btn-primary h-10 px-4" onClick={() => s.set({ dialog: "new" })} disabled={!s.connected}>
            <FilePlus2 size={17} /> Novo projeto
          </button>
          <button className="btn h-10 bg-raised px-4" onClick={() => void openProjectDialog()} disabled={!s.connected}>
            <FolderOpen size={17} /> Abrir
          </button>
        </div>
        {recover.length > 0 && (
          <div className="w-full rounded-lg bg-panel p-3 text-left">
            <span className="panel-title">Recuperar trabalho não salvo</span>
            {recover.map((r) => (
              <button key={r.autosave} className="btn mt-1 w-full justify-between" onClick={() => void s.run("project_open", { path: r.autosave, recover: true }).then(() => setRecover([]))}>
                <span>{r.name ?? "sem nome"}</span>
                <span className="text-[11px] text-faint">{r.saved_at}</span>
              </button>
            ))}
          </div>
        )}
        <button className="btn text-accent-2" onClick={() => s.set({ mcpOpen: true })}>
          <Bot size={16} /> Conectar o Claude Code
        </button>
        {!s.connected && <p className="text-[12px] text-warn">Iniciando o runtime…</p>}
      </div>
    </div>
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
    <div className="pointer-events-none fixed right-4 bottom-40 z-50 flex flex-col items-end gap-2">
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
    const ctrl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    if (ctrl && k === "z" && !e.shiftKey) return e.preventDefault(), void st.run("history_undo", { steps: 1 }).catch(() => {});
    if (ctrl && (k === "y" || (k === "z" && e.shiftKey))) return e.preventDefault(), void st.run("history_redo", { steps: 1 }).catch(() => {});
    if (ctrl && k === "s") return e.preventDefault(), void saveProject(e.shiftKey);
    if (ctrl && k === "o") return e.preventDefault(), void openProjectDialog();
    if (ctrl && k === "n") return e.preventDefault(), st.set({ dialog: "new" });
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
    if (k === "delete" || k === "backspace") return selectionActions().remove();
    if (k === "escape") return st.set({ selection: undefined, mcpOpen: false, dialog: null });
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
      <Toasts />
    </div>
  );
}
