import { open, save } from "@tauri-apps/plugin-dialog";
import { Bot, ChevronRight, Download, FilePlus2, FolderOpen, Grid3x3, Redo2, Save, SaveAll, Undo2, Upload, X } from "lucide-react";
import { useState } from "react";
import * as api from "../core/api";
import { useEditor } from "../core/store";
import type { ProjectSummary } from "../core/types";
import { BrandMark } from "./Brand";

const PROJECT_FILTER = [{ name: "Projeto Pixel Studio Maker", extensions: ["psmproj"] }];

/** Fecha um projeto aberto (de qualquer aba), salvando ou descartando antes se pedido. */
async function closeProjectNow(p: ProjectSummary, mode: "close" | "save" | "discard"): Promise<boolean> {
  const s = useEditor.getState();
  try {
    if (mode === "save") {
      const path = p.path ?? (await save({ defaultPath: `${p.name}.psmproj`, filters: PROJECT_FILTER }));
      if (!path) return false;
      await api.call("project_save", { path }, p.id);
    }
    await api.call("project_close", mode === "discard" ? { discard: true } : {}, p.id);
    return true;
  } catch (e) {
    s.toast(api.errorMessage(e), "error");
    return false;
  }
}

/** Pede para fechar uma aba: fecha direto se está tudo salvo; senão pergunta o que fazer. */
export function requestCloseProject(p: ProjectSummary) {
  const s = useEditor.getState();
  if (p.transaction_owner) return s.toast(`${p.transaction_owner} está editando este projeto agora; espere terminar para fechar.`, "error");
  if (!p.dirty) return void closeProjectNow(p, "close");
  s.set({ closingProject: p.id });
}

function CloseProjectDialog({ p }: { p: ProjectSummary }) {
  const s = useEditor();
  const [busy, setBusy] = useState(false);
  const cancel = () => s.set({ closingProject: undefined });
  const act = async (mode: "save" | "discard") => {
    setBusy(true);
    const ok = await closeProjectNow(p, mode);
    setBusy(false);
    if (ok) cancel();
  };
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55" onClick={cancel} onKeyDown={(e) => e.key === "Escape" && cancel()}>
      <div role="dialog" aria-modal="true" aria-labelledby="close-project-title" className="w-[440px] rounded-xl bg-panel p-5 shadow-2xl ring-1 ring-line" onClick={(e) => e.stopPropagation()}>
        <h2 id="close-project-title" className="mb-2 text-[15px] font-semibold">Fechar “{p.name}”?</h2>
        <p className="mb-5 text-muted">{p.path ? "Há alterações que ainda não foram salvas." : "Este projeto ainda não foi salvo em nenhum arquivo."}</p>
        <div className="flex justify-end gap-2">
          <button className="btn" disabled={busy} onClick={cancel}>Cancelar</button>
          <button className="btn text-danger" disabled={busy} onClick={() => void act("discard")}>Fechar sem salvar</button>
          <button className="btn btn-primary" autoFocus disabled={busy} onClick={() => void act("save")}>{p.path ? "Salvar e fechar" : "Salvar como… e fechar"}</button>
        </div>
      </div>
    </div>
  );
}

export async function openProjectDialog() {
  const s = useEditor.getState();
  const path = await open({ multiple: false, filters: [{ name: "Projeto Pixel Studio Maker", extensions: ["psmproj"] }] });
  if (typeof path === "string") await s.run("project_open", { path }).catch(() => {});
}

export async function saveProject(asNew = false) {
  const s = useEditor.getState();
  if (!s.projectId) return;
  let path: string | null | undefined = asNew ? undefined : s.doc?.path;
  if (!path) {
    path = await save({ defaultPath: `${s.doc?.model.name ?? "projeto"}.psmproj`, filters: [{ name: "Projeto Pixel Studio Maker", extensions: ["psmproj"] }] });
    if (!path) return;
  }
  await s.run("project_save", { path }).then(() => s.toast("Projeto salvo")).catch(() => {});
}

export function TopBar() {
  const s = useEditor();
  const has = !!s.projectId;
  const agents = s.clients.filter((c) => c.kind === "mcp");
  const closing = s.projects.find((p) => p.id === s.closingProject);
  return (
    <header className="app-header">
      <div className="main-header">
        <div className="brand-lockup">
          <BrandMark size={34} />
          <div><strong>Pixel Studio <span>Maker</span></strong><small>SEU ESTÚDIO DE PIXEL ART</small></div>
        </div>
        <div className="header-divider" />
        <nav className="flex items-center gap-1" aria-label="Arquivo">
          <button className="btn" title="Novo projeto (Ctrl+N)" disabled={!s.connected} onClick={() => s.set({ dialog: "new" })}><FilePlus2 size={16} /> Novo</button>
          <button className="btn" title="Abrir projeto (Ctrl+O)" disabled={!s.connected} onClick={() => void openProjectDialog()}><FolderOpen size={16} /> Abrir</button>
          <button className="btn" title="Salvar (Ctrl+S)" disabled={!has} onClick={() => void saveProject()}><Save size={16} /> Salvar</button>
          {has && <button className="btn-icon" title="Salvar como (Ctrl+Shift+S)" onClick={() => void saveProject(true)}><SaveAll size={16} /></button>}
        </nav>
        <div className="ml-auto flex items-center gap-2">
          <button className={`btn connection-button ${s.mcpOpen ? "bg-raised" : ""}`} aria-expanded={s.mcpOpen} title="Conexão e atividade dos agentes" onClick={() => s.set({ mcpOpen: !s.mcpOpen })}>
            <Bot size={16} /><span>{agents.length ? `${agents.length} agente${agents.length > 1 ? "s" : ""}` : "Assistente MCP"}</span>
            <span className={`status-dot ${s.connected ? "bg-ok" : "bg-warn"}`} />
          </button>
          {has && <button className="btn btn-primary" onClick={() => s.set({ dialog: "export" })}><Download size={16} /> Exportar</button>}
        </div>
      </div>
      {has && <div className="document-bar">
        <div className="flex shrink-0 items-center gap-1 border-r border-line pr-3">
          <button className="btn-icon" title={s.doc?.undo.length ? `Desfazer: ${s.doc.undo[s.doc.undo.length - 1]}` : "Desfazer (Ctrl+Z)"} disabled={!s.doc?.undo.length} onClick={() => void s.run("history_undo", { steps: 1 })}><Undo2 size={16} /></button>
          <button className="btn-icon" title="Refazer (Ctrl+Y)" disabled={!s.doc?.redo.length} onClick={() => void s.run("history_redo", { steps: 1 })}><Redo2 size={16} /></button>
        </div>
        <nav className="project-tabs" aria-label="Projetos abertos">
          {s.projects.map((p) => <div key={p.id} className={`project-tab ${p.id === s.projectId ? "is-active" : ""}`} onAuxClick={(e) => { if (e.button === 1) { e.preventDefault(); requestCloseProject(p); } }}>
            <button className="project-tab-main" onClick={() => void s.openProject(p.id)} aria-current={p.id === s.projectId ? "page" : undefined} title={p.path ?? "Projeto ainda não salvo"}>
              <Grid3x3 size={14} /><span className="truncate">{p.name}</span>
              {p.dirty && <span className="status-dot bg-warn" aria-label="Alterações não salvas" />}
              {p.transaction_owner && <span className="chip text-accent">{p.transaction_owner} editando</span>}
            </button>
            <button className="tab-close" title="Fechar (Ctrl+W ou clique do meio)" aria-label={`Fechar ${p.name}`} onClick={() => requestCloseProject(p)}><X size={13} /></button>
          </div>)}
        </nav>
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <button className="btn text-muted" onClick={() => s.set({ dialog: "import" })}><Upload size={14} /> Importar</button>
          <button className="btn text-muted" disabled={!s.connected} onClick={() => s.set({ dialog: "pixelate" })}><Grid3x3 size={14} /> Converter imagem <ChevronRight size={12} /></button>
        </div>
      </div>}
      {closing && <CloseProjectDialog p={closing} />}
    </header>
  );
}
