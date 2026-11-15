// Entradas da barra de ferramentas e opções dos pincéis de dithering e sombreado, da simetria,
// da varinha e do laço.
import { Contrast, FlipHorizontal2, FlipVertical2, Grid2x2, Lasso, WandSparkles } from "lucide-react";
import type { ReactNode } from "react";
import { currentPalette, currentSprite, useEditor, type Tool } from "../../core/store";
import { BRUSH_TOOLS, symmetryAxes } from "./brush";
import type { DitherPatternName } from "./types";

export const FERRAMENTAS_TOOLS: { id: Tool; icon: ReactNode; label: string; key: string }[] = [
  { id: "dither", icon: <Grid2x2 size={17} />, label: "Pincel de dithering (direito: cor secundária)", key: "D" },
  { id: "shade", icon: <Contrast size={17} />, label: "Pincel de sombreado (esquerdo clareia, direito escurece)", key: "U" },
  { id: "wand", icon: <WandSparkles size={17} />, label: "Varinha mágica (Shift: somar, Alt: subtrair)", key: "W" },
  { id: "lasso", icon: <Lasso size={17} />, label: "Laço (Shift: somar, Alt: subtrair)", key: "Q" },
];

/** Teclas de atalho das ferramentas novas. */
export const FERRAMENTAS_KEYS: Record<string, Tool> = { d: "dither", u: "shade", w: "wand", q: "lasso" };

const PATTERNS: { id: DitherPatternName; label: string }[] = [
  { id: "bayer2", label: "Bayer 2×2" },
  { id: "bayer4", label: "Bayer 4×4" },
  { id: "bayer8", label: "Bayer 8×8" },
  { id: "checker", label: "Xadrez" },
];

/** Snaps an axis position to whole or half pixels inside the sprite. */
function snapAxis(v: number, size: number): number {
  return Math.min(size - 1, Math.max(0, Math.round(v * 2) / 2));
}

function SymmetryControls() {
  const s = useEditor();
  const sprite = currentSprite(s);
  if (!sprite) return null;
  const sym = s.symmetry;
  const axes = symmetryAxes(sym, sprite.width, sprite.height);
  const toggle = (axis: "x" | "y") => s.set({ symmetry: { ...sym, [axis]: !sym[axis] } });
  const setAxis = (axis: "ax" | "ay", raw: string, size: number) => {
    const v = Number(raw);
    if (raw !== "" && Number.isFinite(v)) s.set({ symmetry: { ...sym, [axis]: snapAxis(v, size) } });
  };
  const on = (b: boolean) => (b ? "text-accent bg-accent/10" : "text-muted");
  return <span className="flex items-center gap-1.5 text-muted">
    Simetria
    <button className={`btn h-8 ${on(sym.x)}`} aria-pressed={sym.x} title="Espelhar esquerda↔direita (eixo vertical)" onClick={() => toggle("x")}><FlipHorizontal2 size={14} /> X</button>
    {sym.x && <input type="number" step={0.5} min={0} max={sprite.width - 1} className="w-16" aria-label="Posição do eixo vertical (x)" title="Coluna do eixo; .5 fica entre dois pixels" value={axes.x} onChange={(e) => setAxis("ax", e.target.value, sprite.width)} />}
    <button className={`btn h-8 ${on(sym.y)}`} aria-pressed={sym.y} title="Espelhar cima↔baixo (eixo horizontal)" onClick={() => toggle("y")}><FlipVertical2 size={14} /> Y</button>
    {sym.y && <input type="number" step={0.5} min={0} max={sprite.height - 1} className="w-16" aria-label="Posição do eixo horizontal (y)" title="Linha do eixo; .5 fica entre dois pixels" value={axes.y} onChange={(e) => setAxis("ay", e.target.value, sprite.height)} />}
    {(sym.x || sym.y) && (sym.ax !== undefined || sym.ay !== undefined) && <button className="btn h-8 text-muted" title="Voltar os eixos para o centro do sprite" onClick={() => s.set({ symmetry: { ...sym, ax: undefined, ay: undefined } })}>Centro</button>}
  </span>;
}

/** Opções das ferramentas desta área, na barra acima do canvas. */
export function FerramentasOptions() {
  const s = useEditor();
  const palette = currentPalette(s);
  const brush = BRUSH_TOOLS.includes(s.tool);
  return <>
    {s.tool === "dither" && <>
      <label className="flex items-center gap-2 text-muted">Padrão <select aria-label="Padrão de dithering" value={s.ditherPattern} onChange={(e) => s.set({ ditherPattern: e.target.value as DitherPatternName })}>{PATTERNS.map((p) => <option key={p.id} value={p.id}>{p.label}</option>)}</select></label>
      <label className="flex items-center gap-2 text-muted">Densidade <input type="range" min={0} max={1} step={1 / 16} aria-label="Densidade do dithering" value={s.ditherDensity} onChange={(e) => s.set({ ditherDensity: Number(e.target.value) })} /><span className="w-9 font-mono text-[11px]">{Math.round(s.ditherDensity * 100)}%</span></label>
      <label className="flex items-center gap-2 text-muted" title="Onde o padrão não pinta a cor, pinta a outra cor (com o botão direito, a principal)"><input type="checkbox" checked={s.ditherFill} onChange={(e) => s.set({ ditherFill: e.target.checked })} /> Outra cor no resto</label>
    </>}
    {s.tool === "shade" && <span className="text-muted" title="A rampa é a paleta atual, na ordem (da mais escura para a mais clara). Cores fora dela não mudam.">Rampa: paleta {palette ? `“${palette.name}” (${palette.colors.length} cores)` : "atual"} · esquerdo clareia, direito escurece</span>}
    {brush && <SymmetryControls />}
    {brush && s.tileMode && <span className="text-muted">Modo tile: o traço dá a volta nas bordas</span>}
    {s.tool === "wand" && <>
      <label className="flex items-center gap-2 text-muted">Tolerância <input type="number" min={0} max={255} className="w-16" aria-label="Tolerância da varinha" value={s.wandTolerance} onChange={(e) => s.set({ wandTolerance: Math.max(0, Math.min(255, Math.round(Number(e.target.value) || 0))) })} /></label>
      <label className="flex items-center gap-2 text-muted"><input type="checkbox" checked={s.wandContiguous} onChange={(e) => s.set({ wandContiguous: e.target.checked })} /> Contígua</label>
      <label className="flex items-center gap-2 text-muted" title="Amostrar a imagem composta (todas as layers visíveis) em vez da layer atual"><input type="checkbox" checked={s.wandComposite} onChange={(e) => s.set({ wandComposite: e.target.checked })} /> Todas as layers</label>
      <span className="text-muted">Shift: somar · Alt: subtrair</span>
    </>}
    {s.tool === "lasso" && <span className="text-muted">Arraste em volta da área · Shift: somar · Alt: subtrair · Esc para limpar</span>}
  </>;
}
