import { Check, X } from "lucide-react";
import { useEffect, useRef } from "react";
import * as api from "../../core/api";
import { useEditor } from "../../core/store";
import { isBuiltinFont, useTextUi } from "./state";

/** Argumentos de fonte, escala e alinhamento comuns a `text_measure` e `draw_text`. */
function textArgs(text: string, at: [number, number]) {
  const u = useTextUi.getState();
  return { text, at, font: u.font, scale: u.scale, align: u.align };
}

/**
 * Caixa flutuante da ferramenta Texto: o texto é medido no Rust enquanto se digita (o canvas
 * mostra a caixa medida) e Enter escreve com `draw_text` na cor principal.
 */
export function TextBox() {
  const draft = useEditor((s) => s.textDraft);
  const tool = useEditor((s) => s.tool);
  const spriteId = useEditor((s) => s.spriteId);
  const projectId = useEditor((s) => s.projectId);
  const set = useEditor((s) => s.set);
  const ui = useTextUi();
  const ref = useRef<HTMLTextAreaElement>(null);

  // Leaving the tool (or the sprite) closes the box.
  useEffect(() => {
    if (draft && (tool !== "text" || draft.sprite !== spriteId)) set({ textDraft: undefined });
  }, [tool, spriteId, draft, set]);

  useEffect(() => {
    ref.current?.focus();
  }, [draft?.at[0], draft?.at[1]]);

  // Measure (debounced) whenever the text or its options change.
  const text = draft?.text ?? "";
  const ax = draft?.at[0];
  const ay = draft?.at[1];
  useEffect(() => {
    if (ax === undefined || ay === undefined || !projectId) return;
    if (!text) {
      const d = useEditor.getState().textDraft;
      if (d?.box) set({ textDraft: { ...d, box: undefined } });
      return;
    }
    let alive = true;
    const t = setTimeout(() => {
      api
        .call("text_measure", textArgs(text, [ax, ay]), projectId)
        .then((r) => {
          const d = useEditor.getState().textDraft;
          if (alive && d && d.text === text) set({ textDraft: { ...d, box: r.data.box } });
        })
        .catch(() => {});
    }, 90);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [text, ax, ay, ui.font, ui.scale, ui.align, projectId, set]);

  if (!draft || tool !== "text") return null;

  const close = () => set({ textDraft: undefined });
  const commit = async () => {
    const st = useEditor.getState();
    const d = st.textDraft;
    if (!d || !d.text.trim()) return close();
    const args: Record<string, unknown> = { sprite: d.sprite, layer: d.layer, frame: d.frame, ...textArgs(d.text, d.at) };
    // Sprite fonts may keep their drawn colors; built-in fonts always use the primary color.
    if (isBuiltinFont(ui.font) || !ui.keepColors) args.color = st.primary;
    await st.run("draw_text", args).then(close).catch(() => {});
  };

  const left = Math.min(draft.screen[0] + 14, window.innerWidth - 300);
  const top = Math.min(draft.screen[1] + 14, window.innerHeight - 170);
  return (
    <div className="fixed z-40 w-[280px] rounded-xl border border-line bg-panel p-2 shadow-2xl" style={{ left, top }} role="dialog" aria-label="Escrever texto">
      <textarea
        ref={ref}
        rows={3}
        className="w-full resize-none font-mono text-[13px]"
        placeholder="Digite o texto…"
        aria-label="Texto"
        value={draft.text}
        onChange={(e) => set({ textDraft: { ...draft, text: e.target.value } })}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          } else if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            void commit();
          }
        }}
      />
      <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-muted">
        <span>
          {ui.font} · {ui.scale}x · {draft.box ? `${draft.box.w}×${draft.box.h} px` : "—"}
        </span>
        <span className="flex gap-1">
          <button className="btn-icon h-7 w-7" title="Cancelar (Esc)" onClick={close}>
            <X size={14} />
          </button>
          <button className="btn-icon h-7 w-7 text-accent" title="Escrever (Enter)" disabled={!draft.text.trim()} onClick={() => void commit()}>
            <Check size={14} />
          </button>
        </span>
      </div>
      <p className="mt-0.5 text-[10px] text-muted">Enter escreve · Shift+Enter quebra a linha · Esc cancela</p>
    </div>
  );
}
