import { useEditor } from "../../core/store";
import { BUILTIN_FONTS, isBuiltinFont, spriteFonts, useTextUi, type TextAlign } from "./state";

/** Seletor de fonte: as embutidas e os sprites definidos como fonte. */
export function FontSelect({ value, onChange, className = "" }: { value: string; onChange: (v: string) => void; className?: string }) {
  const sprites = useEditor((s) => s.doc?.model.sprites ?? []);
  const fonts = spriteFonts(sprites);
  return (
    <select aria-label="Fonte" className={className} value={value} onChange={(e) => onChange(e.target.value)}>
      <optgroup label="Embutidas">
        {BUILTIN_FONTS.map((f) => (
          <option key={f} value={f}>
            {f}
          </option>
        ))}
      </optgroup>
      {fonts.length > 0 && (
        <optgroup label="Fontes do projeto">
          {fonts.map((sp) => (
            <option key={sp.id} value={sp.name}>
              {sp.name}
            </option>
          ))}
        </optgroup>
      )}
    </select>
  );
}

/** Opções da ferramenta Texto, na barra acima do canvas. */
export function TextToolOptions() {
  const ui = useTextUi();
  return (
    <>
      <label className="flex items-center gap-2 text-muted">
        Fonte <FontSelect value={ui.font} onChange={(font) => ui.set({ font })} />
      </label>
      <label className="flex items-center gap-2 text-muted">
        Escala
        <select aria-label="Escala do texto" value={ui.scale} onChange={(e) => ui.set({ scale: Number(e.target.value) })}>
          {[1, 2, 3, 4, 6, 8].map((v) => (
            <option key={v} value={v}>
              {v}x
            </option>
          ))}
        </select>
      </label>
      <label className="flex items-center gap-2 text-muted">
        Alinhar
        <select aria-label="Alinhamento do texto" value={ui.align} onChange={(e) => ui.set({ align: e.target.value as TextAlign })}>
          <option value="left">Esquerda</option>
          <option value="center">Centro</option>
          <option value="right">Direita</option>
        </select>
      </label>
      {!isBuiltinFont(ui.font) && (
        <label className="flex items-center gap-2 text-muted">
          <input type="checkbox" checked={ui.keepColors} onChange={(e) => ui.set({ keepColors: e.target.checked })} /> Cores da fonte
        </label>
      )}
      <span className="text-muted">Clique no canvas para escrever</span>
    </>
  );
}
