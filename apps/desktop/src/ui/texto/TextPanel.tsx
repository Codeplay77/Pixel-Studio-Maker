import { save } from "@tauri-apps/plugin-dialog";
import { Download, Eye, LayoutTemplate, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { currentSprite, useEditor } from "../../core/store";
import type { Sprite } from "../../core/types";
import { FontSelect } from "./TextToolOptions";
import { BUILTIN_FONTS, SLICE_MODES, imageUrl, nineSliceAxes, useTextUi, type SliceMode } from "./state";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-2 rounded-md bg-panel p-2">
      <span className="panel-title">{title}</span>
      {children}
    </section>
  );
}

function Num({ label, value, onChange, min = 0, max = 1024 }: { label: string; value: number; onChange: (v: number) => void; min?: number; max?: number }) {
  return (
    <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
      <span>{label}</span>
      <input type="number" className="w-16" min={min} max={max} value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

function ModeSelect({ label, value, onChange }: { label: string; value: SliceMode; onChange: (v: SliceMode) => void }) {
  return (
    <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
      <span>{label}</span>
      <select value={value} onChange={(e) => onChange(e.target.value as SliceMode)}>
        {SLICE_MODES.map((m) => (
          <option key={m.id} value={m.id}>
            {m.label}
          </option>
        ))}
      </select>
    </label>
  );
}

const fileBase = (sp: Sprite) => sp.name.replace(/[^\w-]+/g, "_");

/** Define (ou edita) a fonte bitmap guardada no sprite atual. */
function SpriteFontSection({ sprite }: { sprite: Sprite }) {
  const run = useEditor((s) => s.run);
  const def = sprite.extras?.texto?.font;
  const [cellW, setCellW] = useState(def?.cell[0] ?? 8);
  const [cellH, setCellH] = useState(def?.cell[1] ?? 8);
  const [columns, setColumns] = useState(def?.columns ?? Math.max(1, Math.floor(sprite.width / (def?.cell[0] ?? 8))));
  const [charset, setCharset] = useState(def?.charset ?? "");
  const [baseline, setBaseline] = useState(def?.baseline ?? cellH);
  const [spacing, setSpacing] = useState(def?.letter_spacing ?? 1);
  const [space, setSpace] = useState(def?.space_width ?? 4);
  const [lineHeight, setLineHeight] = useState(def?.line_height ?? cellH);
  const [sample, setSample] = useState<string>();
  const key = JSON.stringify(def ?? null);
  useEffect(() => {
    if (!def) return;
    setCellW(def.cell[0]);
    setCellH(def.cell[1]);
    setColumns(def.columns);
    setCharset(def.charset);
    setBaseline(def.baseline);
    setSpacing(def.letter_spacing);
    setSpace(def.space_width);
    setLineHeight(def.line_height);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, sprite.id]);
  useEffect(() => setSample(undefined), [sprite.id]);

  const define = () =>
    void run("font_define", {
      sprite: sprite.id,
      cell: [cellW, cellH],
      charset,
      columns,
      baseline,
      letter_spacing: spacing,
      space_width: space,
      line_height: lineHeight,
    })
      .then((r) => setSample(imageUrl(r)))
      .catch(() => {});

  return (
    <Section title={def ? "Fonte deste sprite" : "Usar este sprite como fonte"}>
      <div className="flex items-center gap-1.5 text-[12px] text-muted">
        Célula
        <input type="number" aria-label="Largura da célula" className="w-14" min={1} value={cellW} onChange={(e) => setCellW(Number(e.target.value))} />×
        <input type="number" aria-label="Altura da célula" className="w-14" min={1} value={cellH} onChange={(e) => setCellH(Number(e.target.value))} />
      </div>
      <Num label="Colunas" value={columns} min={1} onChange={setColumns} />
      <label className="flex flex-col gap-1 text-[12px] text-muted">
        Caracteres, na ordem das células
        <textarea rows={3} className="font-mono text-[12px]" value={charset} placeholder={' !"#$%&\'()*+,-./0123…'} onChange={(e) => setCharset(e.target.value)} />
      </label>
      <Num label="Linha de base" value={baseline} onChange={setBaseline} />
      <Num label="Espaço entre letras" value={spacing} min={-16} max={64} onChange={setSpacing} />
      <Num label="Largura do espaço" value={space} onChange={setSpace} />
      <Num label="Altura da linha" value={lineHeight} min={1} onChange={setLineHeight} />
      <div className="flex flex-wrap gap-1">
        <button className="btn btn-primary h-7 text-[12px]" disabled={!charset} onClick={define}>
          {def ? "Atualizar fonte" : "Definir fonte"}
        </button>
        {def && (
          <>
            <button className="btn h-7 text-[12px]" title="Amostra com todos os glyphs" onClick={() => void run("font_info", { sprite: sprite.id, widths: false }).then((r) => setSample(imageUrl(r))).catch(() => {})}>
              <Eye size={13} /> Amostra
            </button>
            <button className="btn h-7 text-[12px]" title="Deixar de ser fonte (os pixels ficam)" onClick={() => void run("font_define", { sprite: sprite.id, clear: true }).then(() => setSample(undefined)).catch(() => {})}>
              <Trash2 size={13} /> Remover
            </button>
          </>
        )}
      </div>
      {sample && <img alt="Amostra da fonte" className="pixelated max-w-full rounded ring-1 ring-line" src={sample} />}
    </Section>
  );
}

/** Cria um sprite novo com a grade de uma fonte, opcionalmente já preenchida. */
function TemplateSection() {
  const s = useEditor();
  const [name, setName] = useState("Fonte");
  const [base, setBase] = useState<string>("pixel-5x7");
  const [columns, setColumns] = useState(16);
  const create = () =>
    void s
      .run("font_template", { name, base_font: base || undefined, columns })
      .then((r) => s.set({ spriteId: r.data.sprite, layerId: r.data.layer, frameId: r.data.frame, animationId: undefined }))
      .catch(() => {});
  return (
    <Section title="Novo modelo de fonte">
      <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
        Nome <input className="w-32" value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
        Começar de
        <select value={base} onChange={(e) => setBase(e.target.value)}>
          <option value="">Grade vazia</option>
          {BUILTIN_FONTS.map((f) => (
            <option key={f} value={f}>
              {f}
            </option>
          ))}
        </select>
      </label>
      <Num label="Colunas" value={columns} min={1} max={64} onChange={setColumns} />
      <button className="btn h-7 text-[12px]" disabled={!name.trim()} onClick={create}>
        <LayoutTemplate size={13} /> Criar modelo
      </button>
      <p className="text-[11px] text-muted">Cria um sprite com uma célula (e uma região) por caractere, já definido como fonte.</p>
    </Section>
  );
}

/** Exporta uma fonte como BMFont (.fnt + PNG) para o Godot e outras engines. */
function ExportFontSection({ sprite }: { sprite: Sprite }) {
  const run = useEditor((s) => s.run);
  const toast = useEditor((s) => s.toast);
  const [font, setFont] = useState(sprite.extras?.texto?.font ? sprite.name : "pixel-5x7");
  const [scale, setScale] = useState(1);
  useEffect(() => setFont(sprite.extras?.texto?.font ? sprite.name : "pixel-5x7"), [sprite.id, sprite.name, sprite.extras?.texto?.font]);
  const go = async () => {
    const path = await save({ defaultPath: `${font.replace(/[^\w-]+/g, "_")}.fnt`, filters: [{ name: "BMFont", extensions: ["fnt"] }] });
    if (!path) return;
    await run("export_bitmap_font", { font, path, scale }).then(() => toast("Fonte exportada")).catch(() => {});
  };
  return (
    <Section title="Exportar fonte bitmap">
      <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
        Fonte <FontSelect value={font} onChange={setFont} />
      </label>
      <label className="flex items-center justify-between gap-2 text-[12px] text-muted">
        Escala
        <select value={scale} onChange={(e) => setScale(Number(e.target.value))}>
          {[1, 2, 3, 4].map((v) => (
            <option key={v} value={v}>
              {v}x
            </option>
          ))}
        </select>
      </label>
      <button className="btn h-7 text-[12px]" onClick={() => void go()}>
        <Download size={13} /> Exportar .fnt + PNG…
      </button>
    </Section>
  );
}

/** Margens do 9-slice, guias no canvas, prévia renderizada no Rust e exportação para o Godot. */
function NineSliceSection({ sprite }: { sprite: Sprite }) {
  const s = useEditor();
  const ui = useTextUi();
  const m = sprite.extras?.texto?.nine_slice;
  const [left, setLeft] = useState(m?.left ?? 2);
  const [top, setTop] = useState(m?.top ?? 2);
  const [right, setRight] = useState(m?.right ?? 2);
  const [bottom, setBottom] = useState(m?.bottom ?? 2);
  const [w, setW] = useState(sprite.width * 3);
  const [h, setH] = useState(sprite.height * 2);
  const [img, setImg] = useState<string>();
  const key = JSON.stringify(m ?? null);
  useEffect(() => {
    if (!m) return;
    setLeft(m.left);
    setTop(m.top);
    setRight(m.right);
    setBottom(m.bottom);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, sprite.id]);
  useEffect(() => {
    setImg(undefined);
    setW(sprite.width * 3);
    setH(sprite.height * 2);
  }, [sprite.id, sprite.width, sprite.height]);

  const margins = { left, top, right, bottom };
  const preview = () =>
    void s
      .run("nine_slice_render", { sprite: sprite.id, frame: s.frameId, width: w, height: h, edges: ui.edges, center: ui.center, margins, guides: true })
      .then((r) => setImg(imageUrl(r)))
      .catch(() => {});
  const exportScene = async () => {
    const path = await save({ defaultPath: `${fileBase(sprite)}.tscn`, filters: [{ name: "Cena do Godot", extensions: ["tscn"] }] });
    if (!path) return;
    await s
      .run("export_nine_slice", { sprite: sprite.id, frame: s.frameId, path, ...nineSliceAxes() })
      .then(() => s.toast("9-slice exportado"))
      .catch(() => {});
  };
  return (
    <Section title="9-slice">
      <div className="grid grid-cols-2 gap-x-3">
        <Num label="Esquerda" value={left} onChange={setLeft} />
        <Num label="Direita" value={right} onChange={setRight} />
        <Num label="Cima" value={top} onChange={setTop} />
        <Num label="Baixo" value={bottom} onChange={setBottom} />
      </div>
      <div className="flex flex-wrap gap-1">
        <button className="btn btn-primary h-7 text-[12px]" onClick={() => void s.run("nine_slice_set", { sprite: sprite.id, ...margins }).catch(() => {})}>
          {m ? "Atualizar margens" : "Definir margens"}
        </button>
        {m && (
          <button className="btn h-7 text-[12px]" onClick={() => void s.run("nine_slice_set", { sprite: sprite.id, clear: true }).catch(() => {})}>
            <Trash2 size={13} /> Remover
          </button>
        )}
      </div>
      <label className="flex items-center gap-2 text-[12px] text-muted">
        <input type="checkbox" checked={s.nineSliceGuides} onChange={(e) => s.set({ nineSliceGuides: e.target.checked })} /> Guias no canvas
      </label>
      <span className="panel-title mt-1">Prévia</span>
      <div className="flex items-center gap-1.5 text-[12px] text-muted">
        <input type="number" aria-label="Largura da prévia" className="w-16" min={1} value={w} onChange={(e) => setW(Number(e.target.value))} />×
        <input type="number" aria-label="Altura da prévia" className="w-16" min={1} value={h} onChange={(e) => setH(Number(e.target.value))} />
      </div>
      <ModeSelect label="Bordas" value={ui.edges} onChange={(edges) => ui.set({ edges })} />
      <ModeSelect label="Centro" value={ui.center} onChange={(center) => ui.set({ center })} />
      <button className="btn h-7 text-[12px]" onClick={preview}>
        <Eye size={13} /> Renderizar prévia
      </button>
      {img && <img alt="Prévia do 9-slice" className="pixelated max-w-full rounded ring-1 ring-line" src={img} />}
      <span className="panel-title mt-1">Godot (NinePatchRect)</span>
      <ModeSelect label="Eixo horizontal" value={ui.axisH} onChange={(axisH) => ui.set({ axisH })} />
      <ModeSelect label="Eixo vertical" value={ui.axisV} onChange={(axisV) => ui.set({ axisV })} />
      <button className="btn h-7 text-[12px]" disabled={!m} title={m ? "" : "Defina as margens primeiro"} onClick={() => void exportScene()}>
        <Download size={13} /> Exportar .tscn + PNG…
      </button>
    </Section>
  );
}

/** Aba "Texto" do painel direito: fontes bitmap e 9-slice. */
export function TextPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  if (!sprite) return <p className="text-muted">Nenhum sprite.</p>;
  return (
    <div className="flex flex-col gap-3">
      <SpriteFontSection key={`f${sprite.id}`} sprite={sprite} />
      <TemplateSection />
      <ExportFontSection sprite={sprite} />
      <NineSliceSection key={`n${sprite.id}`} sprite={sprite} />
    </div>
  );
}

/** Opções do 9-slice no diálogo de exportação. */
export function NineSliceExportRows() {
  const ui = useTextUi();
  return (
    <>
      <ModeSelect label="Eixo horizontal (NinePatchRect)" value={ui.axisH} onChange={(axisH) => ui.set({ axisH })} />
      <ModeSelect label="Eixo vertical (NinePatchRect)" value={ui.axisV} onChange={(axisV) => ui.set({ axisV })} />
    </>
  );
}
