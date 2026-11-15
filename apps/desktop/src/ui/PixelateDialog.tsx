// Converter imagem em pixel art. Toda a conversão acontece no Rust (comando image_pixelate, o
// mesmo do MCP); aqui só ficam as opções, a prévia e o recorte desenhado sobre a original.
import { open } from "@tauri-apps/plugin-dialog";
import { Crop, Grid3x3, ImageUp, Loader2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import * as api from "../core/api";
import { currentSprite, useEditor } from "../core/store";

type Region = { x: number; y: number; w: number; h: number };
type Result = { b64: string; w: number; h: number; data: any; warnings: string[] };
type Source = { path: string; b64: string; w: number; h: number };

const SIDES = [16, 24, 32, 48, 64, 96, 128];

const DEFAULTS = {
  sizeMode: "side" as "side" | "exact" | "cell",
  side: 64,
  width: 64,
  height: 64,
  cell: 8,
  colors: 16,
  palette: "",
  sampling: "dominant",
  dither: "none",
  background: "auto",
  tolerance: 40,
  crop: true,
  cleanup: "light",
  outline: false,
  saturation: 1,
  contrast: 1,
  brightness: 0,
};
type Opts = typeof DEFAULTS;

function Field({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <label className="flex min-h-8 items-center justify-between gap-3 text-muted" title={hint}>
      <span className="shrink-0">{label}</span>
      <span className="flex min-w-0 items-center justify-end gap-1.5">{children}</span>
    </label>
  );
}

function Slider({ value, min, max, step, onChange, fmt }: { value: number; min: number; max: number; step: number; onChange: (v: number) => void; fmt?: (v: number) => string }) {
  return (
    <>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="w-32" />
      <span className="w-9 text-right font-mono text-[11px] text-ink">{fmt ? fmt(value) : value}</span>
    </>
  );
}

export function PixelateDialog() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const palettes = s.doc?.model.palettes ?? [];
  const [path, setPath] = useState<string>();
  const [name, setName] = useState("");
  const [o, setO] = useState<Opts>(DEFAULTS);
  const [region, setRegion] = useState<Region>();
  const [dest, setDest] = useState<"project" | "layer" | "sprite">("project");
  const [result, setResult] = useState<Result>();
  const [source, setSource] = useState<Source>();
  const [error, setError] = useState<string>();
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);
  const seq = useRef(0);
  const close = () => s.set({ dialog: null });
  const set = <K extends keyof Opts>(k: K, v: Opts[K]) => setO((prev) => ({ ...prev, [k]: v }));

  const args = useMemo(() => {
    const a: Record<string, unknown> = {
      sampling: o.sampling,
      dither: o.dither,
      background: o.background,
      background_tolerance: o.tolerance,
      crop: o.crop,
      cleanup: o.cleanup,
      outline: o.outline,
      saturation: o.saturation,
      contrast: o.contrast,
      brightness: o.brightness,
    };
    const pal = palettes.find((p) => p.name === o.palette);
    if (pal) a.palette = pal.colors;
    else a.colors = o.colors;
    if (region) a.region = region;
    if (dest === "layer" && sprite) {
      a.width = sprite.width;
      a.height = sprite.height;
    } else if (o.sizeMode === "side") a.max_side = o.side;
    else if (o.sizeMode === "exact") {
      a.width = o.width;
      a.height = o.height;
    } else a.cell = o.cell;
    return a;
  }, [o, region, dest, sprite, palettes]);
  const argsKey = JSON.stringify(args);
  // Campo vazio no meio da digitação: espera em vez de mostrar erro.
  const sizes = dest === "layer" ? [1] : o.sizeMode === "side" ? [o.side] : o.sizeMode === "exact" ? [o.width, o.height] : [o.cell];
  const valid = sizes.every((v) => Number.isFinite(v) && v >= 1);

  // Prévia: o mesmo comando, sem criar nada. Espera um instante entre mudanças.
  useEffect(() => {
    if (!path || !valid) return;
    const id = ++seq.current;
    setBusy(true);
    const wantSource = source?.path !== path;
    const t = setTimeout(async () => {
      try {
        const r = await api.call("image_pixelate", { path, ...args, preview: true, preview_scale: 1, with_source: wantSource });
        if (id !== seq.current) return;
        const [pix, src] = r.images ?? [];
        setResult({ b64: pix.base64, w: pix.width, h: pix.height, data: r.data, warnings: r.warnings ?? [] });
        if (src) setSource({ path, b64: src.base64, w: src.width, h: src.height });
        setError(undefined);
      } catch (e) {
        if (id === seq.current) setError(api.errorMessage(e));
      } finally {
        if (id === seq.current) setBusy(false);
      }
    }, 220);
    return () => clearTimeout(t);
  }, [path, argsKey, valid]);

  const choose = async () => {
    const p = await open({ multiple: false, filters: [{ name: "Imagens", extensions: ["png", "jpg", "jpeg", "gif"] }] });
    if (typeof p !== "string") return;
    setPath(p);
    setRegion(undefined);
    setResult(undefined);
    setSource(undefined);
    setName(p.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "");
  };

  const create = async () => {
    if (!path) return;
    setCreating(true);
    try {
      if (dest === "project") await s.run("image_pixelate", { path, ...args, name: name || undefined });
      else await s.run("import_image", { sprite: s.spriteId, path, mode: dest === "layer" ? "new_layer" : "new_sprite", name: name || undefined, pixelate: args });
      s.toast("Imagem convertida em pixel art");
      close();
    } catch {
      // o erro já aparece como aviso
    } finally {
      setCreating(false);
    }
  };

  const srcSize: [number, number] | undefined = result?.data?.source_size;
  const fileName = path?.split(/[\\/]/).pop();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55" onClick={close}>
      <div className="flex max-h-[94vh] w-[900px] flex-col rounded-xl bg-panel shadow-2xl ring-1 ring-line" onClick={(e) => e.stopPropagation()}>
        <header className="flex items-center gap-2 border-b border-line px-5 py-3">
          <Grid3x3 size={17} className="text-accent" />
          <h2 className="text-[15px] font-semibold">Converter imagem em pixel art</h2>
          <span className="ml-2 text-[12px] text-faint">imagem estática, sem animação</span>
          <button className="btn-icon ml-auto" onClick={close} title="Fechar">
            <X size={17} />
          </button>
        </header>

        <div className="flex min-h-0 flex-1 gap-5 overflow-y-auto p-5">
          {/* Imagens */}
          <div className="flex w-[430px] shrink-0 flex-col gap-3">
            <div className="flex items-center gap-2">
              <button className="btn shrink-0 bg-raised whitespace-nowrap" onClick={() => void choose()}>
                <ImageUp size={16} /> {path ? "Trocar imagem…" : "Escolher imagem…"}
              </button>
              {fileName && (
                <span className="truncate text-[12px] text-muted" title={path}>
                  {fileName}
                  {srcSize && <span className="text-faint"> · {srcSize[0]}×{srcSize[1]}</span>}
                </span>
              )}
            </div>
            {!path ? (
              <div className="flex h-[420px] flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-line text-center text-muted">
                <ImageUp size={28} className="text-faint" />
                <span>PNG, JPG ou GIF, de qualquer tamanho.</span>
                <span className="max-w-72 text-[12px] text-faint">Fotos, ilustrações ou pixel art ampliada. O fundo pode ser removido e a imagem recortada antes de converter.</span>
              </div>
            ) : (
              <>
                <SourceView source={source} srcSize={srcSize} region={region} used={result?.data?.source_rect} onRegion={setRegion} />
                <ResultView result={result} busy={busy} error={error} />
              </>
            )}
          </div>

          {/* Opções */}
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="panel-title mb-1">Tamanho da saída</span>
            {dest === "layer" && sprite ? (
              <p className="mb-1 text-[12px] text-muted">
                Cabe no sprite atual: {sprite.width}×{sprite.height}.
              </p>
            ) : (
              <>
                <Field label="Modo">
                  <select value={o.sizeMode} onChange={(e) => set("sizeMode", e.target.value as Opts["sizeMode"])} className="w-60">
                    <option value="side">Lado maior (proporcional)</option>
                    <option value="exact">Largura × altura</option>
                    <option value="cell">Pixel art ampliada</option>
                  </select>
                </Field>
                {o.sizeMode === "side" && (
                  <>
                    <Field label="Lado maior (px)">
                      <input type="number" min={1} max={4096} value={o.side} onChange={(e) => set("side", Number(e.target.value))} className="w-20" />
                    </Field>
                    <div className="mb-1 flex flex-wrap justify-end gap-1">
                      {SIDES.map((v) => (
                        <button key={v} className={`chip ${o.side === v ? "bg-accent/20 text-accent" : "bg-raised text-muted hover:bg-hover"}`} onClick={() => set("side", v)}>
                          {v}
                        </button>
                      ))}
                    </div>
                  </>
                )}
                {o.sizeMode === "exact" && (
                  <Field label="Tamanho (px)" hint="A imagem cabe dentro, centralizada, sem distorcer">
                    <input type="number" min={1} max={4096} value={o.width} onChange={(e) => set("width", Number(e.target.value))} className="w-16" />×
                    <input type="number" min={1} max={4096} value={o.height} onChange={(e) => set("height", Number(e.target.value))} className="w-16" />
                  </Field>
                )}
                {o.sizeMode === "cell" && (
                  <Field label="Pixel na imagem (px)" hint="Quantos pixels da imagem formam um pixel do resultado">
                    <input type="number" min={1} max={512} value={o.cell} onChange={(e) => set("cell", Number(e.target.value))} className="w-20" />
                  </Field>
                )}
              </>
            )}

            <span className="panel-title mt-3 mb-1">Fundo e recorte</span>
            <Field label="Fundo">
              <select value={o.background} onChange={(e) => set("background", e.target.value)} className="w-60">
                <option value="auto">Remover se for uma cor só</option>
                <option value="remove">Remover a cor das bordas</option>
                <option value="keep">Manter</option>
              </select>
            </Field>
            {o.background !== "keep" && (
              <Field label="Tolerância do fundo" hint="Quanto uma cor pode variar e ainda contar como fundo">
                <Slider value={o.tolerance} min={0} max={160} step={2} onChange={(v) => set("tolerance", v)} />
              </Field>
            )}
            <Field label="Recortar ao conteúdo">
              <input type="checkbox" checked={o.crop} onChange={(e) => set("crop", e.target.checked)} />
            </Field>
            <Field label="Recorte manual">
              {region ? (
                <>
                  <span className="font-mono text-[11px] text-ink">
                    {region.x},{region.y} · {region.w}×{region.h}
                  </span>
                  <button className="btn h-7 px-2 text-[12px]" onClick={() => setRegion(undefined)}>
                    Limpar
                  </button>
                </>
              ) : (
                <span className="flex items-center gap-1 text-[12px] text-faint">
                  <Crop size={13} /> arraste sobre a original
                </span>
              )}
            </Field>

            <span className="panel-title mt-3 mb-1">Cores</span>
            <Field label="Paleta">
              <select value={o.palette} onChange={(e) => set("palette", e.target.value)} className="w-60">
                <option value="">Calcular da imagem</option>
                {palettes.map((p) => (
                  <option key={p.name} value={p.name}>
                    {p.name} ({p.colors.length})
                  </option>
                ))}
              </select>
            </Field>
            {!o.palette && (
              <Field label="Número de cores">
                <Slider value={o.colors} min={2} max={64} step={1} onChange={(v) => set("colors", v)} />
              </Field>
            )}
            <Field label="Amostragem" hint="Como cada pixel é escolhido no bloco da imagem">
              <select value={o.sampling} onChange={(e) => set("sampling", e.target.value)} className="w-60">
                <option value="dominant">Cor dominante (nítido)</option>
                <option value="average">Média (mais detalhes)</option>
                <option value="nearest">Pixel central (pixel art)</option>
              </select>
            </Field>
            <Field label="Dithering">
              <select value={o.dither} onChange={(e) => set("dither", e.target.value)} className="w-60">
                <option value="none">Nenhum</option>
                <option value="ordered">Ordenado (padrão xadrez)</option>
                <option value="floyd_steinberg">Floyd-Steinberg (suave)</option>
              </select>
            </Field>
            <Field label="Saturação">
              <Slider value={o.saturation} min={0} max={2} step={0.05} onChange={(v) => set("saturation", v)} fmt={(v) => v.toFixed(2)} />
            </Field>
            <Field label="Contraste">
              <Slider value={o.contrast} min={0.5} max={2} step={0.05} onChange={(v) => set("contrast", v)} fmt={(v) => v.toFixed(2)} />
            </Field>
            <Field label="Brilho">
              <Slider value={o.brightness} min={-0.5} max={0.5} step={0.02} onChange={(v) => set("brightness", v)} fmt={(v) => v.toFixed(2)} />
            </Field>

            <span className="panel-title mt-3 mb-1">Acabamento</span>
            <Field label="Limpeza">
              <select value={o.cleanup} onChange={(e) => set("cleanup", e.target.value)} className="w-60">
                <option value="none">Nenhuma</option>
                <option value="light">Apagar pixels soltos</option>
                <option value="strong">Também pixels isolados</option>
              </select>
            </Field>
            <Field label="Contorno escuro">
              <input type="checkbox" checked={o.outline} onChange={(e) => set("outline", e.target.checked)} />
            </Field>
            <div className="mt-1 flex justify-end">
              <button className="btn h-7 text-[12px] text-faint" onClick={() => (setO(DEFAULTS), setRegion(undefined))}>
                Restaurar padrões
              </button>
            </div>
          </div>
        </div>

        <footer className="flex items-center gap-3 border-t border-line px-5 py-3">
          <select value={dest} onChange={(e) => setDest(e.target.value as typeof dest)} className="w-56">
            <option value="project">Criar um projeto novo</option>
            <option value="layer" disabled={!sprite}>
              Layer nova no sprite atual
            </option>
            <option value="sprite" disabled={!s.projectId}>
              Sprite novo neste projeto
            </option>
          </select>
          <input placeholder="nome" value={name} onChange={(e) => setName(e.target.value)} className="w-48" />
          <div className="ml-auto flex gap-2">
            <button className="btn" onClick={close}>
              Cancelar
            </button>
            <button className="btn btn-primary" disabled={!path || !result || !!error || !valid || creating} onClick={() => void create()}>
              {creating ? <Loader2 size={15} className="animate-spin" /> : <Grid3x3 size={15} />} Converter
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}

/** A original reduzida, com o recorte manual (arrastar) e a área usada de fato (tracejada). */
function SourceView({
  source,
  srcSize,
  region,
  used,
  onRegion,
}: {
  source?: Source;
  srcSize?: [number, number];
  region?: Region;
  used?: Region;
  onRegion: (r: Region | undefined) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ x0: number; y0: number; x1: number; y1: number }>();
  if (!source || !srcSize) {
    return <div className="flex h-[200px] items-center justify-center rounded-lg bg-canvas text-faint">carregando…</div>;
  }
  const scale = Math.min(430 / source.w, 200 / source.h);
  const [dw, dh] = [source.w * scale, source.h * scale];
  const k = dw / srcSize[0]; // tela por pixel original
  const toOrig = (e: React.PointerEvent) => {
    const r = box.current!.getBoundingClientRect();
    const cx = Math.min(Math.max(e.clientX - r.left, 0), dw);
    const cy = Math.min(Math.max(e.clientY - r.top, 0), dh);
    return [Math.round(cx / k), Math.round(cy / k)];
  };
  const rectOf = (d: { x0: number; y0: number; x1: number; y1: number }): Region => ({
    x: Math.min(d.x0, d.x1),
    y: Math.min(d.y0, d.y1),
    w: Math.abs(d.x1 - d.x0),
    h: Math.abs(d.y1 - d.y0),
  });
  const shown = drag ? rectOf(drag) : region;
  const style = (r: Region) => ({ left: r.x * k, top: r.y * k, width: r.w * k, height: r.h * k });
  return (
    <div className="flex flex-col gap-1">
      <span className="panel-title">Original · arraste para recortar</span>
      <div className="flex justify-center rounded-lg bg-canvas p-1">
        <div
          ref={box}
          className="checker relative cursor-crosshair touch-none"
          style={{ width: dw, height: dh }}
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId);
            const [x, y] = toOrig(e);
            setDrag({ x0: x, y0: y, x1: x, y1: y });
          }}
          onPointerMove={(e) => drag && setDrag({ ...drag, ...(([x1, y1]) => ({ x1, y1 }))(toOrig(e)) })}
          onPointerUp={() => {
            if (!drag) return;
            const r = rectOf(drag);
            setDrag(undefined);
            onRegion(r.w * k < 4 || r.h * k < 4 ? undefined : r);
          }}
        >
          <img src={`data:image/png;base64,${source.b64}`} width={dw} height={dh} draggable={false} className="block" alt="Imagem original" />
          {used && !drag && <div className="pointer-events-none absolute border border-dashed border-accent-2/90" style={style(used)} title="Área convertida" />}
          {shown && <div className="pointer-events-none absolute border-2 border-accent shadow-[0_0_0_9999px_rgba(0,0,0,0.45)]" style={style(shown)} />}
        </div>
      </div>
    </div>
  );
}

function ResultView({ result, busy, error }: { result?: Result; busy: boolean; error?: string }) {
  // Aumenta por inteiros quando cabe; resultados grandes são reduzidos só para caber na prévia.
  const fit = result ? Math.min(430 / result.w, 200 / result.h) : 1;
  const zoom = fit >= 1 ? Math.floor(fit) : fit;
  const d = result?.data;
  return (
    <div className="flex flex-col gap-1">
      <span className="panel-title flex items-center gap-2">
        Pixel art {busy && <Loader2 size={12} className="animate-spin text-accent-2" />}
      </span>
      <div className="flex h-[212px] items-center justify-center overflow-hidden rounded-lg bg-canvas p-1">
        {error ? (
          <span className="px-4 text-center text-[12px] text-danger">{error}</span>
        ) : result ? (
          <img src={`data:image/png;base64,${result.b64}`} width={result.w * zoom} height={result.h * zoom} className="checker pixelated" alt="Prévia em pixel art" />
        ) : (
          <span className="text-faint">convertendo…</span>
        )}
      </div>
      {d && !error && (
        <>
          <span className="text-[12px] text-muted">
            {d.size[0]}×{d.size[1]} px · {d.color_count} cores · {d.opaque_pixels} pixels visíveis
            {d.background_removed && (
              <>
                {" "}
                · fundo removido <span className="inline-block h-2.5 w-2.5 rounded-sm align-middle ring-1 ring-line" style={{ background: d.background_removed }} />
              </>
            )}
          </span>
          <div className="flex flex-wrap gap-0.5">
            {(d.colors as string[]).map((c) => (
              <span key={c} className="h-4 w-4 rounded-sm ring-1 ring-black/40" style={{ background: c }} title={c} />
            ))}
          </div>
          {result!.warnings.map((w) => (
            <span key={w} className="text-[11px] text-warn">
              {w}
            </span>
          ))}
        </>
      )}
    </div>
  );
}
