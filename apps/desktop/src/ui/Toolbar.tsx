import { ArrowLeftRight, Brush, PictureInPicture2, Ruler, Circle, Copy, Crosshair, Eraser, FlipHorizontal2, FlipVertical2, Grid3x3, Hand, Minus, MousePointer2, PaintBucket, Pipette, RotateCw, Scissors, Square, SquareDashed, ClipboardPaste, Trash2 } from "lucide-react";
import type { ReactNode } from "react";
import { currentSprite, useEditor, type Tool } from "../core/store";

export const TOOLS: { id: Tool; icon: ReactNode; label: string; key: string }[] = [
  { id: "pencil", icon: <Brush size={17} />, label: "Lápis", key: "B" },
  { id: "eraser", icon: <Eraser size={17} />, label: "Borracha", key: "E" },
  { id: "fill", icon: <PaintBucket size={17} />, label: "Balde (Shift: global)", key: "G" },
  { id: "line", icon: <Minus size={17} />, label: "Linha", key: "L" },
  { id: "rect", icon: <Square size={17} />, label: "Retângulo", key: "R" },
  { id: "ellipse", icon: <Circle size={17} />, label: "Elipse", key: "O" },
  { id: "select", icon: <SquareDashed size={17} />, label: "Seleção", key: "M" },
  { id: "move", icon: <Hand size={17} />, label: "Mover (layer ou seleção)", key: "V" },
  { id: "eyedropper", icon: <Pipette size={17} />, label: "Conta-gotas", key: "I" },
  { id: "anchor", icon: <Crosshair size={17} />, label: "Posicionar anchor", key: "A" },
];

export function selectionActions() {
  const s = useEditor.getState();
  const t = { sprite: s.spriteId, layer: s.layerId, frame: s.frameId };
  return {
    remove: () => s.selection && void s.run("pixel_clear", { ...t, rect: s.selection }),
    copy: () => {
      if (!s.selection || s.spriteId === undefined || s.layerId === undefined || s.frameId === undefined) return;
      s.set({ clipboard: { sprite: s.spriteId, layer: s.layerId, frame: s.frameId, rect: s.selection } });
      s.toast("Seleção copiada");
    },
    cut: () => {
      const a = selectionActions();
      a.copy();
      a.remove();
    },
    paste: () => {
      const c = s.clipboard;
      if (!c) return;
      const at = s.selection ? [s.selection.x, s.selection.y] : [c.rect.x, c.rect.y];
      void s.run("draw_batch", { ...t, label: "Colar", ops: [{ op: "copy", from: { layer: c.layer, frame: c.frame }, rect: c.rect, to: at, mode: "skip_transparent" }] });
      s.set({ selection: { x: at[0], y: at[1], w: c.rect.w, h: c.rect.h } });
    },
    transform: (transform: Record<string, unknown>) => void s.run("draw_transform", { ...t, transform, rect: s.selection ?? undefined }),
  };
}

function Swatch({ color, title, onChange }: { color: string; title: string; onChange: (c: string) => void }) {
  return (
    <label className="checker relative block h-7 w-7 cursor-pointer overflow-hidden rounded-md ring-1 ring-line" title={title}>
      <span className="absolute inset-0" style={{ background: color }} />
      <input type="color" aria-label={title} className="absolute inset-0 opacity-0" value={color.slice(0, 7)} onChange={(e) => onChange(e.target.value)} />
    </label>
  );
}

export function Toolbar() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const a = selectionActions();
  return (
    <aside className="tool-rail" aria-label="Ferramentas de desenho">
      {TOOLS.map((t) => (
        <button key={t.id} className={`btn-icon ${s.tool === t.id ? "btn-icon-active" : ""}`} aria-label={`${t.label} (${t.key})`} aria-pressed={s.tool === t.id} title={`${t.label} (${t.key})`} onClick={() => s.set({ tool: t.id })} disabled={!sprite}>
          {t.icon}<span className="tool-key" aria-hidden="true">{t.key}</span>
        </button>
      ))}
      <div className="my-1 h-px w-7 bg-line" />
      <button className="btn-icon" title="Espelhar na horizontal (layer ou seleção)" disabled={!sprite} onClick={() => a.transform({ kind: "flip_h" })}>
        <FlipHorizontal2 size={17} />
      </button>
      <button className="btn-icon" title="Espelhar na vertical" disabled={!sprite} onClick={() => a.transform({ kind: "flip_v" })}>
        <FlipVertical2 size={17} />
      </button>
      <button className="btn-icon" title="Girar 90°" disabled={!sprite} onClick={() => a.transform({ kind: "rotate", quarter_turns: 1 })}>
        <RotateCw size={17} />
      </button>
      {s.selection && (
        <>
          <button className="btn-icon" title="Copiar (Ctrl+C)" onClick={a.copy}>
            <Copy size={17} />
          </button>
          <button className="btn-icon" title="Recortar (Ctrl+X)" onClick={a.cut}>
            <Scissors size={17} />
          </button>
          <button className="btn-icon" title="Apagar seleção (Delete)" onClick={a.remove}>
            <Trash2 size={17} />
          </button>
          <button className="btn-icon" title="Limpar seleção (Esc)" onClick={() => s.set({ selection: undefined })}>
            <MousePointer2 size={17} />
          </button>
        </>
      )}
      {s.clipboard && (
        <button className="btn-icon" title="Colar (Ctrl+V)" onClick={a.paste}>
          <ClipboardPaste size={17} />
        </button>
      )}
      <div className="my-1 h-px w-7 bg-line" />
      <button className={`btn-icon ${s.guides ? "text-accent-2" : ""}`} title="Guias: centro e linha do chão" onClick={() => s.set({ guides: !s.guides })}>
        <Ruler size={17} />
      </button>
      <div className="mt-auto flex flex-col items-center gap-1.5 pt-2">
        <div className="relative h-11 w-11">
          <div className="absolute right-0 bottom-0">
            <Swatch color={s.secondary} title="Cor secundária (botão direito)" onChange={(c) => s.set({ secondary: c })} />
          </div>
          <div className="absolute top-0 left-0">
            <Swatch color={s.primary} title="Cor principal" onChange={(c) => s.set({ primary: c })} />
          </div>
        </div>
        <button className="btn-icon h-6" title="Trocar cores (X)" onClick={() => s.set({ primary: s.secondary, secondary: s.primary })}>
          <ArrowLeftRight size={14} />
        </button>

      </div>
    </aside>
  );
}


/** Keep settings beside the canvas, where their effect is visible. */
export function ToolOptions() {
  const s = useEditor();
  const tool = TOOLS.find((t) => t.id === s.tool)!;
  const brushTool = ["pencil", "eraser", "line"].includes(s.tool);
  return <div className="tool-options" aria-label="Opções da ferramenta">
    <span className="flex min-w-24 items-center gap-2 font-medium text-ink">{tool.icon}{tool.label.split(" (")[0]}</span>
    <span className="h-5 w-px bg-line" />
    {brushTool && <label className="flex items-center gap-2 text-muted">Espessura <select aria-label="Espessura do pincel" value={s.brush} onChange={(e) => s.set({ brush: Number(e.target.value) })}>{Array.from({ length: 8 }, (_, i) => <option key={i} value={i + 1}>{i + 1} px</option>)}</select></label>}
    {["rect", "ellipse"].includes(s.tool) && <label className="flex items-center gap-2 text-muted"><input type="checkbox" checked={s.filledShapes} onChange={(e) => s.set({ filledShapes: e.target.checked })} /> Preencher forma</label>}
    {s.tool === "fill" && <span className="text-muted">Shift + clique para substituir a cor em toda a camada</span>}
    {s.tool === "select" && <span className="text-muted">Arraste para selecionar · Esc para limpar</span>}
    {s.tool === "move" && <span className="text-muted">Arraste a camada ou a seleção</span>}
    {s.tool === "eyedropper" && <span className="text-muted">Clique no canvas para capturar uma cor</span>}
    {s.tool === "anchor" && <span className="text-muted">{s.anchorTarget ? `Ponto: ${s.anchorTarget}` : "Escolha um ponto no painel Pontos"}</span>}
    <div className="ml-auto flex items-center gap-1">
      <button className={`btn h-8 ${s.grid ? "text-accent bg-accent/10" : "text-muted"}`} aria-pressed={s.grid} title="Mostrar grade (#)" onClick={() => s.set({ grid: !s.grid })}><Grid3x3 size={14} /> Grade</button>
      <button className={`btn h-8 ${s.showPreview ? "text-accent bg-accent/10" : "text-muted"}`} aria-pressed={s.showPreview} onClick={() => s.set({ showPreview: !s.showPreview })}><PictureInPicture2 size={14} /> Prévia</button>
    </div>
  </div>;
}
