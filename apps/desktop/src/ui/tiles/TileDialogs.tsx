// Diálogos de tiles: "Gerar autotile", "Prévia de mapa" e o aviso do TileSet no diálogo de exportação.
// As imagens vêm prontas do Rust (autotile_generate, tile_preview_map_set, tile_preview_render).
import { X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { currentSprite, useEditor } from "../../core/store";
import { gridSize, tileData, type TileCoord } from "../../core/tiles";
import { TilePicker } from "./TilesPanel";

type Shown = { b64: string; w: number; h: number; scale: number; mapW: number; mapH: number };

function TileModal({ title, children, onClose, wide = false }: { title: string; children: React.ReactNode; onClose: () => void; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);
  return (
    <dialog ref={ref} className="editor-dialog" style={wide ? { width: 720 } : undefined} aria-labelledby={titleId} onCancel={(e) => { e.preventDefault(); onClose(); }}>
      <div className="mb-5 flex items-center justify-between"><h2 id={titleId} className="text-lg font-semibold tracking-tight">{title}</h2><button className="btn-icon" aria-label="Fechar diálogo" onClick={onClose}><X size={18} /></button></div>
      {children}
    </dialog>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-muted">
      <span>{label}</span>
      {children}
    </label>
  );
}

function NoGrid() {
  return <p className="text-muted">Este sprite ainda não tem grade de tiles. Defina o tamanho do tile na aba Tiles do painel direito.</p>;
}

export function AutotileDialog() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const d = tileData(sprite);
  const [input, setInput] = useState("rpgmaker_a2");
  const [col, setCol] = useState(s.tileSelected[0]);
  const [row, setRow] = useState(s.tileSelected[1]);
  const [a2, setA2] = useState(false);
  const [blob, setBlob] = useState(true);
  const [name, setName] = useState(sprite ? `${sprite.name}_autotile` : "autotile");
  const [replace, setReplace] = useState(true);
  const [terrain, setTerrain] = useState(sprite?.name ?? "terreno");
  const [result, setResult] = useState<{ b64: string; w: number; h: number; sprite: number }>();
  const close = () => s.set({ dialog: null });
  if (!sprite) return null;
  const go = () => {
    const output = [...(a2 ? ["rpgmaker_a2"] : []), ...(blob ? ["blob_47"] : [])];
    void s
      .run("autotile_generate", { sprite: sprite.id, frame: s.frameId, input, source: [col, row], output, name, replace, terrain_name: terrain })
      .then((r) => {
        const img = r.images?.[0];
        if (img) setResult({ b64: img.base64, w: img.width, h: img.height, sprite: r.data.sprite });
      })
      .catch(() => {});
  };
  return (
    <TileModal title="Gerar autotile" onClose={close}>
      {!d ? <NoGrid /> : (
        <>
          <p className="mb-3 text-[12px] leading-5 text-muted">Monta os tiles por quartos (meio tile) a partir de um bloco de entrada. O tile precisa ter largura e altura pares.</p>
          <Row label="Entrada">
            <select value={input} onChange={(e) => setInput(e.target.value)} className="w-56">
              <option value="rpgmaker_a2">RPG Maker A2 (2×3 tiles)</option>
              <option value="frame_3x3">Moldura 3×3 + cantos internos (4×3 tiles)</option>
            </select>
          </Row>
          <Row label="Origem [coluna, linha]">
            <span className="flex items-center gap-1">
              <input type="number" min={0} value={col} onChange={(e) => setCol(Number(e.target.value))} className="w-16" />
              <input type="number" min={0} value={row} onChange={(e) => setRow(Number(e.target.value))} className="w-16" />
            </span>
          </Row>
          <Row label="Saída: bloco A2 do RPG Maker"><input type="checkbox" checked={a2} onChange={(e) => setA2(e.target.checked)} /></Row>
          <Row label="Saída: blob de 47 tiles (Godot)"><input type="checkbox" checked={blob} onChange={(e) => setBlob(e.target.checked)} /></Row>
          <Row label="Sprite novo"><input value={name} onChange={(e) => setName(e.target.value)} className="w-56" /></Row>
          <Row label="Substituir se já existir"><input type="checkbox" checked={replace} onChange={(e) => setReplace(e.target.checked)} /></Row>
          <Row label="Nome do terreno"><input value={terrain} onChange={(e) => setTerrain(e.target.value)} className="w-56" /></Row>
          {result && (
            <div className="mt-3 flex flex-col items-start gap-2">
              <img src={`data:image/png;base64,${result.b64}`} width={Math.min(result.w, 400)} className="pixelated rounded-sm" alt="Autotile gerado" />
              <button className="btn h-7 text-[12px]" onClick={() => (s.set({ spriteId: result.sprite, layerId: undefined, frameId: undefined, animationId: undefined, dialog: null }), void s.refresh())}>Abrir o sprite gerado</button>
            </div>
          )}
          <div className="mt-4 flex justify-end gap-2">
            <button className="btn" onClick={close}>Fechar</button>
            <button className="btn btn-primary" disabled={(!a2 && !blob) || !name.trim()} onClick={go}>Gerar</button>
          </div>
        </>
      )}
    </TileModal>
  );
}

export function TileMapDialog() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const d = tileData(sprite);
  const map = d?.preview_map;
  const [w, setW] = useState(map?.width ?? 8);
  const [h, setH] = useState(map?.height ?? 6);
  const [rows, setRows] = useState("");
  const [shown, setShown] = useState<Shown>();
  const close = () => s.set({ dialog: null });
  const show = (r: { images?: { base64: string; width: number; height: number }[]; data: any }) => {
    const img = r.images?.[0];
    if (img) setShown({ b64: img.base64, w: img.width, h: img.height, scale: r.data.scale, mapW: r.data.map_size[0], mapH: r.data.map_size[1] });
  };
  const hasMap = !!map;
  useEffect(() => {
    if (!sprite || !hasMap) return;
    void s.run("tile_preview_render", { sprite: sprite.id, frame: s.frameId, grid: true }).then(show).catch(() => {});
  }, [sprite?.id, hasMap]);
  if (!sprite) return null;
  const edit = (args: Record<string, unknown>) => void s.run("tile_preview_map_set", { sprite: sprite.id, ...args }).then(show).catch(() => {});
  const paintAt = (e: React.MouseEvent<HTMLImageElement>, erase: boolean) => {
    if (!shown || !d) return;
    const r = e.currentTarget.getBoundingClientRect();
    const x = Math.floor(((e.clientX - r.left) / r.width) * shown.mapW);
    const y = Math.floor(((e.clientY - r.top) / r.height) * shown.mapH);
    const tile: TileCoord | null = erase ? null : s.tileSelected;
    edit({ paint: [{ at: [x, y], tile }] });
  };
  const size = d && gridSize(sprite, d);
  const sel = s.tileSelected;
  const selValid = !!size && sel[0] < size.columns && sel[1] < size.rows;
  return (
    <TileModal title="Prévia de mapa" onClose={close} wide>
      {!d ? <NoGrid /> : (
        <div className="flex gap-4">
          <div className="flex w-[240px] shrink-0 flex-col gap-2">
            <span className="panel-title">Tile para pintar</span>
            <TilePicker sprite={sprite} data={d} max={230} />
            <span className="text-[12px] text-muted">[{sel[0]}, {sel[1]}] · clique: pinta · botão direito: apaga</span>
            <span className="panel-title mt-2">Tamanho do mapa</span>
            <div className="flex items-center gap-1">
              <input type="number" min={1} max={64} value={w} onChange={(e) => setW(Number(e.target.value))} className="w-14" />×
              <input type="number" min={1} max={64} value={h} onChange={(e) => setH(Number(e.target.value))} className="w-14" />
              <button className="btn h-7 text-[12px]" onClick={() => edit({ size: [w, h] })}>{map ? "Mudar" : "Criar"}</button>
            </div>
            <div className="flex flex-wrap gap-1">
              <button className="btn h-7 text-[12px]" disabled={!map || !selValid} onClick={() => edit({ fill: sel })}>Preencher</button>
              <button className="btn h-7 text-[12px]" disabled={!map} onClick={() => edit({ clear: true })}>Esvaziar</button>
            </div>
            {(d.terrain_sets?.length ?? 0) > 0 && (
              <>
                <span className="panel-title mt-2">Pintar por terreno</span>
                <textarea className="h-24 font-mono text-[12px]" placeholder={"........\n.####...\n.####...\n........"} value={rows} onChange={(e) => setRows(e.target.value)} />
                <button className="btn h-7 text-[12px]" disabled={!rows.trim()} onClick={() => edit({ terrain_rows: rows.split("\n").filter((l) => l.length) })}>
                  Montar com os bits de vizinhança
                </button>
                <span className="text-[11px] text-faint"># = terreno, . = vazio. Cada célula recebe o tile que mais combina com as vizinhas.</span>
              </>
            )}
          </div>
          <div className="min-w-0 flex-1">
            {shown ? (
              <img
                src={`data:image/png;base64,${shown.b64}`}
                width={shown.w}
                height={shown.h}
                draggable={false}
                className="pixelated max-w-full cursor-crosshair rounded-sm"
                alt="Mapa de prévia montado com os tiles"
                onClick={(e) => paintAt(e, false)}
                onContextMenu={(e) => (e.preventDefault(), paintAt(e, true))}
              />
            ) : (
              <p className="text-muted">{map ? "Carregando…" : "Crie um mapa para testar como os tiles se encaixam."}</p>
            )}
          </div>
        </div>
      )}
    </TileModal>
  );
}

/** Aviso no diálogo de exportação quando o formato é o TileSet do Godot. */
export function TilesetExportNote() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const d = tileData(sprite);
  if (!d) return <p className="py-1 text-[12px] text-warn">Este sprite não tem grade de tiles: defina na aba Tiles antes de exportar.</p>;
  const size = sprite && gridSize(sprite, d);
  return (
    <p className="py-1 text-[12px] leading-5 text-faint">
      Usa o frame atual e a grade de tiles ({d.tile_width}×{d.tile_height}, {size?.columns}×{size?.rows}). Tiles vazios ficam de fora; colisões vão para a camada de física 0 e os terrenos para os terrain sets.
    </p>
  );
}
