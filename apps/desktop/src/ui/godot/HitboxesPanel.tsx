// Painel "Hitboxes": caixas de jogo (hitbox, hurtbox, colisão) com um retângulo por frame.
// Toda alteração é um comando (hitbox_create, hitbox_update, hitbox_set); o painel só mostra.
import { Copy, Eraser, Eye, EyeOff, Plus, Swords, Trash2 } from "lucide-react";
import { useState } from "react";
import { currentAnimation, currentSprite, useEditor } from "../../core/store";
import { hitboxColor, hitboxesOf, KIND_LABELS, rectAt, type Hitbox, type HitboxKind } from "./hitboxCanvas";

const KINDS = Object.keys(KIND_LABELS) as HitboxKind[];

function HitboxRow({ h }: { h: Hitbox }) {
  const s = useEditor();
  const sprite = currentSprite(s)!;
  const anim = currentAnimation(s);
  const r = rectAt(h, s.frameId);
  const t = { sprite: sprite.id, hitbox: h.id };
  const active = s.tool === "hitbox" && s.hitboxTarget === h.name;
  const hidden = s.hiddenHitboxes.includes(h.id);
  const track = anim ? (anim.tracks.find((x) => (x.direction ?? null) === (s.direction ?? null)) ?? anim.tracks[0]) : undefined;
  return (
    <li className={`flex flex-col gap-1 rounded-md px-2 py-1.5 ${active ? "bg-raised ring-1 ring-accent" : "bg-panel"}`}>
      <div className="flex items-center gap-2">
        <label className="relative block h-5 w-5 shrink-0 cursor-pointer overflow-hidden rounded ring-1 ring-line" title="Cor da caixa">
          <span className="absolute inset-0" style={{ background: hitboxColor(h) }} />
          <input type="color" aria-label={`Cor de ${h.name}`} className="absolute inset-0 opacity-0" value={hitboxColor(h)} onChange={(e) => void s.run("hitbox_update", { ...t, color: e.target.value })} />
        </label>
        <button className="min-w-0 flex-1 truncate text-left" title="Desenhar esta caixa no canvas (H)" onClick={() => s.set({ tool: "hitbox", hitboxTarget: h.name, showHitboxes: true })}>
          {h.name}
        </button>
        <span className="font-mono text-[11px] text-muted" title="Retângulo neste frame">{r ? `${r.x},${r.y} ${r.w}×${r.h}` : "desligada"}</span>
      </div>
      <div className="flex items-center gap-0.5">
        <select aria-label={`Tipo de ${h.name}`} className="mr-auto py-0.5 text-[11px]" value={h.kind} onChange={(e) => void s.run("hitbox_update", { ...t, kind: e.target.value })}>
          {KINDS.map((k) => <option key={k} value={k}>{KIND_LABELS[k]}</option>)}
        </select>
        <button className={`btn-icon h-6 w-6 ${active ? "text-accent" : ""}`} title="Desenhar no canvas: arraste o retângulo neste frame" onClick={() => s.set({ tool: "hitbox", hitboxTarget: h.name, showHitboxes: true })}>
          <Swords size={13} />
        </button>
        <button
          className="btn-icon h-6 w-6"
          title={track ? `Copiar este retângulo para todos os frames de "${anim!.name}"${track.direction ? ` (${track.direction})` : ""}` : "Escolha uma animação na linha do tempo para copiar para a trilha"}
          disabled={!r || !track}
          onClick={() => void s.run("hitbox_set", { ...t, rect: r, animation: anim!.id, direction: track?.direction ?? undefined })}
        >
          <Copy size={13} />
        </button>
        <button className="btn-icon h-6 w-6" title="Desligar neste frame" disabled={!r || s.frameId === undefined} onClick={() => void s.run("hitbox_set", { ...t, clear: true, frame: s.frameId })}>
          <Eraser size={13} />
        </button>
        <button className="btn-icon h-6 w-6" title={hidden ? "Mostrar no canvas" : "Ocultar no canvas"} aria-pressed={hidden} onClick={() => s.set({ hiddenHitboxes: hidden ? s.hiddenHitboxes.filter((x) => x !== h.id) : [...s.hiddenHitboxes, h.id] })}>
          {hidden ? <EyeOff size={13} /> : <Eye size={13} />}
        </button>
        <button className="btn-icon h-6 w-6" title="Apagar a caixa" onClick={() => void s.run("hitbox_update", { ...t, delete: true })}>
          <Trash2 size={13} />
        </button>
      </div>
    </li>
  );
}

export function HitboxesPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const [name, setName] = useState("");
  const [kind, setKind] = useState<HitboxKind>("hitbox");
  if (!sprite) return <p className="text-faint">Abra ou crie um projeto.</p>;
  const boxes = hitboxesOf(sprite);
  const create = (fromSelection: boolean) => {
    const n = name.trim();
    const rect = fromSelection ? s.selection : undefined;
    void s
      .run("hitbox_create", { sprite: sprite.id, name: n, kind, ...(rect && s.frameId !== undefined ? { rect, frame: s.frameId } : {}) })
      .then(() => {
        s.set({ tool: "hitbox", hitboxTarget: n, showHitboxes: true });
        setName("");
      })
      .catch(() => {});
  };
  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="panel-title">Hitboxes</span>
      </div>
      <label className="mb-2 flex items-center gap-2 text-muted">
        <input type="checkbox" checked={s.showHitboxes} onChange={(e) => s.set({ showHitboxes: e.target.checked })} />
        Mostrar no canvas
      </label>
      <ul className="mb-3 flex flex-col gap-1">
        {boxes.map((h) => <HitboxRow key={h.id} h={h} />)}
        {boxes.length === 0 && (
          <p className="text-faint">Nenhuma caixa ainda. Hitboxes marcam onde um golpe acerta, hurtboxes onde o personagem recebe dano. Cada frame tem o seu retângulo; frame sem retângulo = caixa desligada. A cena do Godot leva tudo isso.</p>
        )}
      </ul>
      <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
        <span className="panel-title">Nova caixa</span>
        <input placeholder="nome (ex.: golpe, corpo)" value={name} onChange={(e) => setName(e.target.value)} />
        <select value={kind} onChange={(e) => setKind(e.target.value as HitboxKind)}>
          {KINDS.map((k) => <option key={k} value={k}>{KIND_LABELS[k]}</option>)}
        </select>
        <div className="flex gap-1">
          <button className="btn h-7 text-[12px]" disabled={!name.trim()} onClick={() => create(false)}>
            <Plus size={14} /> Criar e desenhar
          </button>
          <button className="btn h-7 text-[12px]" disabled={!name.trim() || !s.selection} title="Usa a seleção como retângulo neste frame" onClick={() => create(true)}>
            Da seleção
          </button>
        </div>
      </div>
    </div>
  );
}

/** Dica da ferramenta Hitbox na barra de opções, com a escolha da caixa. */
export function HitboxToolHint() {
  const s = useEditor();
  const boxes = hitboxesOf(currentSprite(s));
  if (boxes.length === 0) {
    return (
      <span className="text-muted">
        Crie uma caixa no painel{" "}
        <button className="text-accent underline" onClick={() => s.set({ panel: "hitboxes" })}>Hitboxes</button>
      </span>
    );
  }
  return (
    <span className="flex items-center gap-2 text-muted">
      Caixa
      <select aria-label="Caixa da ferramenta Hitbox" value={s.hitboxTarget ?? ""} onChange={(e) => s.set({ hitboxTarget: e.target.value || undefined })}>
        <option value="">escolha…</option>
        {boxes.map((h) => <option key={h.id} value={h.name}>{h.name}</option>)}
      </select>
      <span>Arraste no canvas para definir o retângulo neste frame</span>
    </span>
  );
}
