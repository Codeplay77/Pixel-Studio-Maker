// Tarefas de exportação: exportações salvas no projeto que rodam sempre que ele é salvo
// (e em lote com `psm export`). Criar, rodar e apagar são comandos (export_job_set,
// export_jobs_run, export_job_delete); aqui só a interface.
import { ListChecks, Play, Trash2, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import * as api from "../../core/api";
import { useEditor } from "../../core/store";
import type { ProjectExtras } from "../../core/types";

type Job = NonNullable<NonNullable<ProjectExtras["godot"]>["export_jobs"]>[number];

const COMMAND_LABELS: Record<string, string> = {
  export_png: "PNG",
  export_spritesheet: "Sprite sheet",
  export_frames: "PNGs",
  export_gif: "GIF",
  export_godot_metadata: "Godot SpriteFrames",
  export_godot_scene: "Cena Godot",
};

function jobsOf(): Job[] {
  return useEditor.getState().doc?.model.extras?.godot?.export_jobs ?? [];
}

function parentDir(path?: string | null): string | undefined {
  if (!path) return undefined;
  const p = path.replace(/\\/g, "/");
  const i = p.lastIndexOf("/");
  return i > 0 ? p.slice(0, i) : undefined;
}

/** `path` relativo à pasta `dir` quando está dentro dela (assim o projeto pode mudar de lugar). */
function relativeTo(dir: string, path: string): string {
  const p = path.replace(/\\/g, "/");
  const d = dir.replace(/\\/g, "/").replace(/\/$/, "");
  return p.toLowerCase().startsWith(`${d.toLowerCase()}/`) ? p.slice(d.length + 1) : path;
}

function fileName(path: unknown): string {
  return typeof path === "string" ? (path.replace(/\\/g, "/").split("/").pop() ?? path) : "";
}

/**
 * Chamado pelo diálogo Exportar ao montar a chamada: com "Exportar sempre que salvar" marcado,
 * grava o mesmo comando e argumentos como tarefa `on_save` (caminhos relativos ao projeto
 * quando ficam dentro da pasta dele). Não acrescenta nada ao mapa de chamadas.
 */
export function rememberExportJob(get: () => [string, Record<string, unknown>] | undefined): Record<string, never> {
  if (!useEditor.getState().exportOnSave) return {};
  // One job per click: the box starts unchecked next time.
  useEditor.getState().set({ exportOnSave: false });
  queueMicrotask(() => {
    const call = get();
    if (!call) return;
    const [command, args] = call;
    const st = useEditor.getState();
    const dir = parentDir(st.doc?.path);
    const stored: Record<string, unknown> = { ...args };
    for (const key of ["path", "directory"]) {
      const v = stored[key];
      if (dir && typeof v === "string") stored[key] = relativeTo(dir, v);
    }
    const name = `${COMMAND_LABELS[command] ?? command} ${fileName(args.path ?? args.directory)}`.trim();
    void st
      .run("export_job_set", { name, command, args: stored, on_save: true })
      .then(() => st.toast(dir ? `"${name}" vai exportar sempre que o projeto for salvo` : `"${name}" vai exportar ao salvar (salve o projeto para usar caminhos relativos)`))
      .catch(() => {});
  });
  return {};
}

/** Opções do diálogo Exportar: exportar ao salvar e acesso às tarefas. */
export function ExportOnSaveOption({ kind }: { kind: string }) {
  const s = useEditor();
  const count = s.doc?.model.extras?.godot?.export_jobs?.length ?? 0;
  return (
    <>
      {kind === "godot_scene" && (
        <p className="py-1 text-[12px] leading-5 text-faint">
          Grava a cena .tscn, o SpriteFrames .tres e a sheet .png com o mesmo nome. Salvando dentro de um projeto Godot, os caminhos res:// são ajustados sozinhos.
        </p>
      )}
      <label className="flex items-center justify-between gap-3 py-1 text-muted">
        <span>Exportar sempre que salvar</span>
        <input type="checkbox" checked={s.exportOnSave} onChange={(e) => s.set({ exportOnSave: e.target.checked })} />
      </label>
      <div className="flex justify-end py-1">
        <button className="btn h-7 text-[12px]" onClick={() => s.set({ dialog: "exportJobs" })}>
          <ListChecks size={14} /> Tarefas de exportação{count ? ` (${count})` : ""}…
        </button>
      </div>
    </>
  );
}

function argsSummary(job: Job): string {
  const a = job.args ?? {};
  return String(a.path ?? a.directory ?? "");
}

/** Lista as tarefas do projeto: rodar, ligar/desligar ao salvar, apagar. */
export function ExportJobsDialog() {
  const s = useEditor();
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const [baseDir, setBaseDir] = useState<string>();
  const jobs = s.doc?.model.extras?.godot?.export_jobs ?? [];
  const close = () => s.set({ dialog: null });
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);
  useEffect(() => {
    void api.call("export_job_list", {}, s.projectId).then((r) => setBaseDir(r.data.base_dir)).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jobs.length]);
  const runJobs = (names: string[]) =>
    void s
      .run("export_jobs_run", names.length ? { jobs: names } : {})
      .then((r) => {
        const ran = r.data.ran ?? 0;
        const failed = r.data.failed ?? 0;
        if (!failed) s.toast(ran === 1 ? "Exportação concluída" : `${ran} exportações concluídas`);
      })
      .catch(() => {});
  return (
    <dialog ref={ref} className="editor-dialog" aria-labelledby={titleId} onCancel={(e) => { e.preventDefault(); close(); }}>
      <div className="mb-4 flex items-center justify-between">
        <h2 id={titleId} className="text-lg font-semibold tracking-tight">Tarefas de exportação</h2>
        <button className="btn-icon" aria-label="Fechar diálogo" onClick={close}><X size={18} /></button>
      </div>
      <p className="mb-3 leading-5 text-muted">
        Exportações guardadas no projeto. As marcadas rodam sempre que você salva; todas podem rodar em lote com <code className="font-mono text-[12px]">psm export</code>.
      </p>
      <ul className="flex flex-col gap-1">
        {jobs.map((j) => (
          <li key={j.name} className="flex items-center gap-2 rounded-md bg-shell px-2 py-1.5">
            <div className="min-w-0 flex-1">
              <div className="truncate">{j.name}</div>
              <div className="truncate font-mono text-[11px] text-faint" title={argsSummary(j)}>{COMMAND_LABELS[j.command] ?? j.command} · {argsSummary(j)}</div>
            </div>
            <label className="flex shrink-0 items-center gap-1 text-[11px] text-muted" title="Rodar sempre que o projeto for salvo">
              <input type="checkbox" checked={!!j.on_save} onChange={(e) => void s.run("export_job_set", { name: j.name, on_save: e.target.checked }).catch(() => {})} /> ao salvar
            </label>
            <button className="btn-icon h-7 w-7" title="Rodar agora" onClick={() => runJobs([j.name])}><Play size={14} /></button>
            <button className="btn-icon h-7 w-7" title="Apagar a tarefa" onClick={() => void s.run("export_job_delete", { name: j.name }).catch(() => {})}><Trash2 size={14} /></button>
          </li>
        ))}
        {jobs.length === 0 && <p className="text-faint">Nenhuma tarefa ainda. No diálogo Exportar, marque "Exportar sempre que salvar".</p>}
      </ul>
      {baseDir && <p className="mt-3 text-[11px] break-all text-faint">Caminhos relativos partem de: {baseDir}</p>}
      <div className="mt-4 flex justify-end gap-2">
        <button className="btn" onClick={() => s.set({ dialog: "export" })}>Voltar a Exportar</button>
        <button className="btn btn-primary" disabled={jobs.length === 0} onClick={() => runJobs(jobsOf().map((j) => j.name))}>
          <Play size={14} /> Rodar todas
        </button>
      </div>
    </dialog>
  );
}
