// Ferramentas de paleta do painel "Paleta": arquivos de paleta e variantes (recolorações).
// Tudo é comando do runtime; a prévia da variante é renderizada em Rust (palette_variant_preview).
import { open, save } from "@tauri-apps/plugin-dialog";
import { FileDown, FileUp, Plus, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import * as api from "../../core/api";
import { currentPalette, useEditor } from "../../core/store";

const PALETTE_FILTERS = [
  { name: "Paletas (.gpl, .hex, .pal, .txt, .png, .gif)", extensions: ["gpl", "hex", "pal", "txt", "png", "gif"] },
  { name: "GIMP (.gpl)", extensions: ["gpl"] },
  { name: "Lospec (.hex)", extensions: ["hex"] },
  { name: "JASC-PAL (.pal)", extensions: ["pal"] },
  { name: "Paint.NET (.txt)", extensions: ["txt"] },
  { name: "Imagem PNG", extensions: ["png"] },
  { name: "Imagem GIF", extensions: ["gif"] },
];

/** Botões de importar e exportar arquivo de paleta. */
export function PaletteFileButtons({ palette }: { palette?: string }) {
  const s = useEditor();
  const importFile = async () => {
    const path = await open({ multiple: false, filters: [PALETTE_FILTERS[0]] });
    if (typeof path !== "string") return;
    await s
      .run("palette_import", { path })
      .then((r) => s.toast(`Paleta "${r.data.palette}" importada (${r.data.count} cores)`))
      .catch(() => {});
  };
  const exportFile = async () => {
    if (!palette) return;
    const path = await save({ defaultPath: `${palette.replace(/[^\w-]+/g, "_")}.gpl`, filters: PALETTE_FILTERS.slice(1) });
    if (!path) return;
    await s
      .run("palette_export", { palette, path })
      .then(() => s.toast("Paleta exportada"))
      .catch(() => {});
  };
  return (
    <>
      <button className="btn-icon h-7 w-7" title="Importar paleta (.gpl, .hex, .pal, .txt, .png, .gif)" aria-label="Importar paleta" onClick={() => void importFile()}>
        <FileDown size={15} />
      </button>
      <button className="btn-icon h-7 w-7" title="Exportar esta paleta" aria-label="Exportar paleta" disabled={!palette} onClick={() => void exportFile()}>
        <FileUp size={15} />
      </button>
    </>
  );
}

function uniqueName(base: string, names: string[]) {
  const taken = (n: string) => names.some((x) => x.toLowerCase() === n.toLowerCase());
  let i = names.length + 1;
  while (taken(`${base} ${i}`)) i++;
  return `${base} ${i}`;
}

const HEX = /^#([0-9a-f]{6}|[0-9a-f]{8})$/i;

/** Seção "Variantes": recolorações da paleta atual, com prévia lado a lado. */
export function PaletteVariantsSection() {
  const s = useEditor();
  const pal = currentPalette(s);
  const all = s.doc?.model.extras?.paletas?.variants ?? [];
  const variants = all.filter((v) => pal && v.base.toLowerCase() === pal.name.toLowerCase());
  const [selected, setSelected] = useState<string>();
  const [index, setIndex] = useState<number>();
  const [preview, setPreview] = useState<string>();
  const [hex, setHex] = useState("");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const variant = variants.find((v) => v.name === selected) ?? variants[0];
  const colorKey = variant?.colors.join(",");

  useEffect(() => {
    if (!variant || !s.projectId) {
      setPreview(undefined);
      return;
    }
    let alive = true;
    api
      .call("palette_variant_preview", { sprite: s.spriteId, frame: s.frameId, variant: variant.name, compare: true }, s.projectId)
      .then((r) => {
        const img = r.images?.[0];
        if (alive) setPreview(img ? `data:${img.mime};base64,${img.base64}` : undefined);
      })
      .catch(() => alive && setPreview(undefined));
    return () => {
      alive = false;
    };
  }, [variant?.name, colorKey, s.frameId, s.spriteId, s.projectId, s.epoch]);

  useEffect(() => {
    if (variant && index !== undefined) setHex(variant.colors[index] ?? "");
  }, [variant?.name, colorKey, index]);

  useEffect(() => () => clearTimeout(timer.current), []);

  if (!pal) return null;
  const setColor = (i: number, color: string) => {
    if (!variant || !HEX.test(color)) return;
    void s.run("palette_variant_set", { name: variant.name, set: [{ index: i, color }] }).catch(() => {});
  };
  // The native color picker fires while dragging: send only the last color.
  const setColorSoon = (i: number, color: string) => {
    setHex(color);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setColor(i, color), 300);
  };
  const create = () => {
    const name = uniqueName("Variante", all.map((v) => v.name));
    void s
      .run("palette_variant_set", { name, base: pal.name })
      .then(() => (setSelected(name), setIndex(undefined)))
      .catch(() => {});
  };
  const remove = () => {
    if (!variant) return;
    void s.run("palette_variant_delete", { name: variant.name }).then(() => (setSelected(undefined), setIndex(undefined))).catch(() => {});
  };

  return (
    <div className="mt-3 flex flex-col gap-1.5 rounded-md bg-panel p-2 text-muted">
      <div className="flex items-center justify-between">
        <span className="panel-title">Variantes</span>
        <span className="flex gap-0.5">
          <button className="btn-icon h-7 w-7" title={`Nova variante de "${pal.name}"`} aria-label="Nova variante" onClick={create}>
            <Plus size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Apagar esta variante" aria-label="Apagar variante" disabled={!variant} onClick={remove}>
            <Trash2 size={15} />
          </button>
        </span>
      </div>
      {!variant && (
        <span className="text-[12px] leading-4 text-faint">
          Recolore o sprite (inimigo de outra cor, time, versão noturna) sem mexer nos pixels: a cor de cada índice da paleta vira a cor da variante.
        </span>
      )}
      {variant && (
        <>
          <select className="w-full" value={variant.name} onChange={(e) => (setSelected(e.target.value), setIndex(undefined))}>
            {variants.map((v) => (
              <option key={v.name}>{v.name}</option>
            ))}
          </select>
          <div className="grid grid-cols-8 gap-1">
            {variant.colors.map((c, i) => (
              <button
                key={i}
                title={`${i}: ${pal.colors[i] ?? "?"} → ${c} (clique para editar)`}
                aria-label={`Cor ${i} da variante`}
                aria-pressed={index === i}
                className={`checker relative aspect-square overflow-hidden rounded ring-1 ${index === i ? "ring-2 ring-accent" : "ring-line"}`}
                onClick={() => setIndex(i)}
              >
                <span className="absolute inset-0" style={{ background: c }} />
                <span className="absolute top-0 left-0 h-1/3 w-1/3" style={{ background: pal.colors[i] }} />
              </button>
            ))}
          </div>
          {index !== undefined && index < variant.colors.length && (
            <div className="flex flex-wrap items-center gap-1.5">
              <span className="text-[12px]">Cor {index}</span>
              <input type="color" aria-label="Escolher cor" value={(hex || variant.colors[index]).slice(0, 7)} onChange={(e) => setColorSoon(index, e.target.value)} />
              <input
                className="w-24 font-mono"
                aria-label="Cor em hexadecimal"
                value={hex}
                onChange={(e) => setHex(e.target.value)}
                onBlur={() => hex !== variant.colors[index] && setColor(index, hex)}
                onKeyDown={(e) => e.key === "Enter" && setColor(index, hex)}
              />
              <button className="btn h-7 text-[12px]" title="Usar a cor principal" onClick={() => setColor(index, s.primary)}>
                Principal
              </button>
              <button className="btn h-7 text-[12px]" title="Voltar à cor da paleta base" onClick={() => setColor(index, pal.colors[index])}>
                Original
              </button>
            </div>
          )}
          {preview && <img src={preview} alt="Prévia: original à esquerda, variante à direita" className="w-full rounded" style={{ imageRendering: "pixelated" }} />}
        </>
      )}
    </div>
  );
}
