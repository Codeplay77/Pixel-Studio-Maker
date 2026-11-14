import { open, save } from "@tauri-apps/plugin-dialog";
import { Bot, Download, FilePlus2, FolderOpen, Redo2, Save, SaveAll, Settings, Undo2, Upload } from "lucide-react";
import { useEditor } from "../core/store";
import { BrandMark } from "./Brand";

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
  const agentClients = s.clients.filter((c) => c.kind === "mcp");
  return (
    <header className="flex h-11 shrink-0 items-center gap-1 border-b border-line bg-shell px-2">
      <div className="mr-2 flex items-center gap-2 pl-1">
        <BrandMark />
        <span className="font-semibold tracking-tight">Pixel Studio Maker</span>
      </div>
      <button className="btn-icon" title="Novo projeto (Ctrl+N)" onClick={() => s.set({ dialog: "new" })}>
        <FilePlus2 size={17} />
      </button>
      <button className="btn-icon" title="Abrir (Ctrl+O)" onClick={() => void openProjectDialog()}>
        <FolderOpen size={17} />
      </button>
      <button className="btn-icon" title="Salvar (Ctrl+S)" disabled={!has} onClick={() => void saveProject()}>
        <Save size={17} />
      </button>
      <button className="btn-icon" title="Salvar como" disabled={!has} onClick={() => void saveProject(true)}>
        <SaveAll size={17} />
      </button>
      <div className="mx-1 h-5 w-px bg-line" />
      <button className="btn-icon" title="Importar PNG/GIF" disabled={!has} onClick={() => s.set({ dialog: "import" })}>
        <Upload size={17} />
      </button>
      <button className="btn-icon" title="Exportar" disabled={!has} onClick={() => s.set({ dialog: "export" })}>
        <Download size={17} />
      </button>
      <div className="mx-1 h-5 w-px bg-line" />
      <button className="btn-icon" title={s.doc?.undo.length ? `Desfazer: ${s.doc.undo[s.doc.undo.length - 1]}` : "Desfazer (Ctrl+Z)"} disabled={!s.doc?.undo.length} onClick={() => void s.run("history_undo", { steps: 1 })}>
        <Undo2 size={17} />
      </button>
      <button className="btn-icon" title={s.doc?.redo.length ? `Refazer: ${s.doc.redo[0]}` : "Refazer (Ctrl+Y)"} disabled={!s.doc?.redo.length} onClick={() => void s.run("history_redo", { steps: 1 })}>
        <Redo2 size={17} />
      </button>

      <div className="ml-3 flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {s.projects.map((p) => (
          <button
            key={p.id}
            onClick={() => void s.openProject(p.id)}
            className={`flex h-7 max-w-52 shrink-0 items-center gap-1.5 truncate rounded-md px-2.5 text-[12px] ${p.id === s.projectId ? "bg-raised text-ink" : "text-muted hover:bg-hover"}`}
            title={p.path ?? "não salvo"}
          >
            <span className="truncate">{p.name}</span>
            {p.dirty && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-warn" title="alterações não salvas" />}
            {p.transaction_owner && <span className="chip bg-accent-2/15 text-accent-2">{p.transaction_owner} editando</span>}
          </button>
        ))}
      </div>

      {s.lastAgentActivity && <span className="hidden max-w-64 truncate text-[11px] text-accent-2 xl:block" title="Última ação de um agente">{s.lastAgentActivity}</span>}
      <button
        className={`btn h-7 text-[12px] ${s.connected ? "" : "text-danger"}`}
        title="Painel MCP"
        onClick={() => s.set({ mcpOpen: !s.mcpOpen })}
      >
        <span className={`h-2 w-2 rounded-full ${s.connected ? (agentClients.length ? "bg-ok shadow-[0_0_8px] shadow-ok" : "bg-faint") : "bg-danger"}`} />
        <Bot size={16} />
        {s.connected ? (agentClients.length ? `MCP: ${agentClients.length} agente${agentClients.length > 1 ? "s" : ""}` : "MCP pronto") : "Runtime offline"}
      </button>
      <button className="btn-icon" title="Configurações do MCP" onClick={() => s.set({ mcpOpen: true })}>
        <Settings size={17} />
      </button>
    </header>
  );
}
