// Opções do diálogo Exportar para "Variantes de paleta" e "Normal map (Godot)". O estado fica num
// store próprio; o diálogo só monta a chamada com variantExportArgs/normalMapArgs.
import { create } from "zustand";
import { useEditor } from "../../core/store";

type SheetLayout = "rows" | "horizontal" | "vertical" | "grid" | "packed";

type PaletasExportState = {
  /** Variantes escolhidas (vazio = todas). */
  variants: string[];
  format: "spritesheet" | "png" | "gif";
  includeOriginal: boolean;
  layout: SheetLayout;
  spacing: number;
  trim: boolean;
  mode: "bevel" | "luminance" | "mix";
  bevel: number;
  strength: number;
  flipY: boolean;
  /** Normal map no layout de uma sprite sheet (para casar com a sheet exportada). */
  normalSheet: boolean;
  set: (p: Partial<PaletasExportState>) => void;
};

export const usePaletasExport = create<PaletasExportState>((set) => ({
  variants: [],
  format: "spritesheet",
  includeOriginal: false,
  layout: "rows",
  spacing: 0,
  trim: false,
  mode: "bevel",
  bevel: 2,
  strength: 1,
  flipY: false,
  normalSheet: false,
  set: (p) => set(p),
}));

type Context = { frame?: number; animation?: number; direction?: string; selection: Record<string, unknown> };

/** Argumentos de export_palette_variants (sem sprite, pasta e escala). */
export function variantExportArgs(ctx: Context): Record<string, unknown> {
  const o = usePaletasExport.getState();
  const common = { variants: o.variants, format: o.format, include_original: o.includeOriginal };
  if (o.format === "png") return { ...common, frame: ctx.frame };
  if (o.format === "gif") return { ...common, animation: ctx.animation, direction: ctx.direction };
  return { ...common, sheet: { layout: o.layout, spacing: o.spacing, trim: o.trim }, selection: ctx.selection, json: true };
}

/** Argumentos de export_normal_map (sem sprite, caminho e escala). */
export function normalMapArgs(ctx: Context): Record<string, unknown> {
  const o = usePaletasExport.getState();
  const base = { mode: o.mode, bevel: o.bevel, strength: o.strength, flip_y: o.flipY };
  return o.normalSheet ? { ...base, sheet: { layout: o.layout, spacing: o.spacing, trim: o.trim }, selection: ctx.selection } : { ...base, frame: ctx.frame };
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-muted">
      <span>{label}</span>
      {children}
    </label>
  );
}

function SheetRows() {
  const o = usePaletasExport();
  return (
    <>
      <Row label="Layout">
        <select value={o.layout} onChange={(e) => o.set({ layout: e.target.value as SheetLayout })} className="w-56">
          <option value="rows">Uma linha por animação/direção</option>
          <option value="horizontal">Horizontal</option>
          <option value="vertical">Vertical</option>
          <option value="grid">Grade</option>
          <option value="packed">Atlas compactado</option>
        </select>
      </Row>
      <Row label="Espaçamento (px)">
        <input type="number" min={0} max={32} value={o.spacing} onChange={(e) => o.set({ spacing: Number(e.target.value) })} className="w-20" />
      </Row>
      <Row label="Recortar bordas (alinhado pelo pivô)">
        <input type="checkbox" checked={o.trim} onChange={(e) => o.set({ trim: e.target.checked })} />
      </Row>
    </>
  );
}

export function VariantExportOptions({ hasAnimation }: { hasAnimation: boolean }) {
  const o = usePaletasExport();
  const variants = useEditor((s) => s.doc?.model.extras?.paletas?.variants ?? []);
  const toggle = (name: string, on: boolean) => o.set({ variants: on ? [...o.variants, name] : o.variants.filter((v) => v !== name) });
  if (variants.length === 0) return <p className="py-1 text-[12px] text-warn">O projeto não tem variantes de paleta: crie uma no painel Paleta.</p>;
  return (
    <>
      <Row label="Gerar">
        <select value={o.format} onChange={(e) => o.set({ format: e.target.value as PaletasExportState["format"] })} className="w-56">
          <option value="spritesheet">Uma sprite sheet + JSON por variante</option>
          <option value="png">Um PNG do frame atual por variante</option>
          <option value="gif" disabled={!hasAnimation}>
            Um GIF da animação por variante
          </option>
        </select>
      </Row>
      {o.format === "spritesheet" && <SheetRows />}
      <div className="py-1 text-muted">
        <span>Variantes (nenhuma marcada = todas)</span>
        <div className="mt-1 flex flex-wrap gap-2">
          {variants.map((v) => (
            <label key={v.name} className="flex items-center gap-1 text-[12px]">
              <input type="checkbox" checked={o.variants.includes(v.name)} onChange={(e) => toggle(v.name, e.target.checked)} />
              {v.name}
            </label>
          ))}
        </div>
      </div>
      <Row label="Exportar também o original">
        <input type="checkbox" checked={o.includeOriginal} onChange={(e) => o.set({ includeOriginal: e.target.checked })} />
      </Row>
      <p className="text-[12px] text-faint">Escolha uma pasta: cada arquivo se chama sprite_variante.</p>
    </>
  );
}

export function NormalMapExportOptions() {
  const o = usePaletasExport();
  return (
    <>
      <Row label="Altura">
        <select value={o.mode} onChange={(e) => o.set({ mode: e.target.value as PaletasExportState["mode"] })} className="w-56">
          <option value="bevel">Bordas arredondadas (bevel)</option>
          <option value="luminance">Brilho das cores</option>
          <option value="mix">Bordas + brilho</option>
        </select>
      </Row>
      {o.mode !== "luminance" && (
        <Row label="Largura da borda (px)">
          <input type="number" min={1} max={64} value={o.bevel} onChange={(e) => o.set({ bevel: Number(e.target.value) })} className="w-20" />
        </Row>
      )}
      <Row label="Intensidade">
        <input type="number" min={0} max={10} step={0.5} value={o.strength} onChange={(e) => o.set({ strength: Number(e.target.value) })} className="w-20" />
      </Row>
      <Row label="Convenção DirectX (verde para baixo)">
        <input type="checkbox" checked={o.flipY} onChange={(e) => o.set({ flipY: e.target.checked })} />
      </Row>
      <Row label="No layout de sprite sheet (casa com a sheet)">
        <input type="checkbox" checked={o.normalSheet} onChange={(e) => o.set({ normalSheet: e.target.checked })} />
      </Row>
      {o.normalSheet && <SheetRows />}
      <p className="text-[12px] text-faint">No Godot: CanvasTexture com a textura em Diffuse e este PNG em Normal Map. Use as mesmas opções e escala da sheet.</p>
    </>
  );
}
