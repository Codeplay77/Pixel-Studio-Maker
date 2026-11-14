import { Bot, Check, Copy, Monitor, Terminal, X } from "lucide-react";
import { useEffect, useState } from "react";
import * as api from "../core/api";
import { useEditor } from "../core/store";
import type { RuntimeInfo } from "../core/types";

function CopyBox({ text, label }: { text: string; label: string }) {
  const [done, setDone] = useState(false);
  return (
    <div className="relative">
      <span className="panel-title">{label}</span>
      <pre className="mt-1 max-h-40 overflow-auto rounded-md bg-canvas p-2 pr-9 font-mono text-[11px] whitespace-pre-wrap text-muted select-text">{text}</pre>
      <button
        className="btn-icon absolute top-6 right-1 h-7 w-7"
        title="Copiar"
        onClick={() => {
          void navigator.clipboard.writeText(text);
          setDone(true);
          setTimeout(() => setDone(false), 1500);
        }}
      >
        {done ? <Check size={14} className="text-ok" /> : <Copy size={14} />}
      </button>
    </div>
  );
}

export function McpPanel() {
  const s = useEditor();
  const [info, setInfo] = useState<RuntimeInfo>();
  useEffect(() => {
    if (s.mcpOpen) void api.runtimeInfo().then(setInfo).catch(() => {});
  }, [s.mcpOpen, s.connected]);
  if (!s.mcpOpen) return null;
  const project = s.projects.find((p) => p.id === s.projectId);
  return (
    <div className="absolute top-16 right-0 bottom-0 z-30 flex w-[380px] flex-col border-l border-line bg-panel shadow-2xl">
      <div className="flex h-11 shrink-0 items-center justify-between border-b border-line px-3">
        <span className="flex items-center gap-2 font-semibold">
          <Bot size={17} className="text-accent-2" /> Claude e MCP
        </span>
        <button className="btn-icon" aria-label="Fechar painel MCP" onClick={() => s.set({ mcpOpen: false })}>
          <X size={16} />
        </button>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-3">
        <div className="flex flex-col gap-1 rounded-md bg-shell p-2">
          <span className="panel-title">Estado</span>
          <span className={s.connected ? "text-ok" : "text-danger"}>{s.connected ? "Runtime conectado" : "Runtime desconectado (tentando de novo)"}</span>
          {info && <span className="font-mono text-[11px] text-faint select-text">{info.endpoint}</span>}
          {project && (
            <span className="text-muted">
              Projeto atual: <b className="text-ink">{project.name}</b> · revisão {project.revision}
              {project.transaction_owner && <span className="text-accent-2"> · {project.transaction_owner} em transação</span>}
            </span>
          )}
          {info && <span className="text-muted">{info.tools} ferramentas disponíveis para o agente</span>}
        </div>
        <div className="flex flex-col gap-1 rounded-md bg-shell p-2">
          <span className="panel-title">Conectados</span>
          {s.clients.map((c) => (
            <span key={c.id} className="flex items-center gap-2 text-muted">
              {c.kind === "mcp" ? <Bot size={14} className="text-accent-2" /> : c.kind === "desktop" ? <Monitor size={14} /> : <Terminal size={14} />}
              {c.name} <span className="text-faint">({c.kind})</span>
            </span>
          ))}
          {!s.clients.some((c) => c.kind === "mcp") && <span className="text-[12px] text-faint">Nenhum agente conectado. Configure o Claude Code abaixo e peça para ele desenhar.</span>}
        </div>
        {info && (
          <div className="flex flex-col gap-2 rounded-md bg-shell p-2">
            <span className="panel-title">Configurar no Claude Code</span>
            <p className="text-[12px] text-muted">Rode no terminal, na pasta do seu projeto:</p>
            <CopyBox label="comando" text={info.claude_code_command} />
            <p className="text-[12px] text-muted">Ou adicione ao arquivo .mcp.json do projeto:</p>
            <CopyBox label=".mcp.json" text={JSON.stringify(info.claude_code_config, null, 2)} />
            {!info.psm_exists && <p className="text-[12px] text-warn">Não achei o executável psm em {info.psm}. Compile com "cargo build -p sprite-cli".</p>}
          </div>
        )}
        <div className="flex min-h-0 flex-col gap-1 rounded-md bg-shell p-2">
          <span className="panel-title">Operações recentes</span>
          {s.ops.length === 0 && <span className="text-[12px] text-faint">Nada ainda.</span>}
          {s.ops.slice(0, 80).map((o, i) => (
            <div key={i} className="flex items-start gap-2 text-[12px]">
              <span className={`mt-1 h-1.5 w-1.5 shrink-0 rounded-full ${o.ok ? (o.client_kind === "mcp" ? "bg-accent-2" : "bg-faint") : "bg-danger"}`} />
              <div className="min-w-0 flex-1">
                <span className={o.client_kind === "mcp" ? "text-accent-2" : "text-muted"}>{o.client}</span> <span className="font-mono">{o.command}</span>
                <div className={`truncate text-[11px] ${o.ok ? "text-faint" : "text-danger"}`} title={o.summary}>
                  {o.summary} · {o.millis}ms
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
