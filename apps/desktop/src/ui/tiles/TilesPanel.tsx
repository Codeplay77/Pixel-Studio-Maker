// Aba "Tiles" do painel direito, botão "Modo tile" e o seletor de tiles. A interface só chama os
// comandos tile_* do runtime e desenha o resultado; nenhuma regra de tiles mora aqui.
import { LayoutGrid, Plus, Repeat, Trash2, Undo2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useFrameCanvas } from "../../core/hooks";
import { currentSprite, useEditor } from "../../core/store";
import {
  BIT_GRID,
  TERRAIN_MODES,
  bitAllowed,
  collisionOf,
  gridSize,
  sameTile,
  terrainOf,
  tileAt,
  tileData,
  tileRect,
  type PeeringBit,
  type TerrainMode,
  type TileCoord,
  type TileData,
} from "../../core/tiles";
import type { Point, Sprite } from "../../core/types";

export function TilesTabIcon() {
  return <LayoutGrid size={15} />;
}

export function TileModeButton() {
  const tileMode = useEditor((s) => s.tileMode);
  const set = useEditor((s) => s.set);
  return (
    <button className={`btn h-8 ${tileMode ? "text-accent bg-accent/10" : "text-muted"}`} aria-pressed={tileMode} title="Modo tile: mostra o sprite repetido em volta para conferir as emendas" onClick={() => set({ tileMode: !tileMode })}>
      <Repeat size={14} /> Modo tile
    </button>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
      <span className="panel-title">{title}</span>
      {children}
    </div>
  );
}

function Num({ label, value, onChange, min = 0 }: { label: string; value: number; onChange: (v: number) => void; min?: number }) {
  return (
    <label className="flex items-center gap-1 text-[12px] text-muted">
      {label}
      <input type="number" min={min} max={1024} className="w-14" value={value} onChange={(e) => onChange(Number(e.target.value))} />
    </label>
  );
}

export function TilesPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  if (!sprite) return <p className="text-faint">Abra ou crie um projeto.</p>;
  const d = tileData(sprite);
  return (
    <div className="flex flex-col gap-3">
      <GridSection key={`${sprite.id}:${d?.tile_width}:${d?.tile_height}:${d?.margin}:${d?.spacing}`} sprite={sprite} data={d} />
      {d && (
        <>
          <Section title="Tile escolhido">
            <TilePicker sprite={sprite} data={d} />
            <span className="text-[12px] text-muted">
              [{s.tileSelected[0]}, {s.tileSelected[1]}] · clique numa miniatura para escolher
            </span>
          </Section>
          <CollisionSection sprite={sprite} data={d} />
          <TerrainSection sprite={sprite} data={d} />
          <div className="flex flex-wrap gap-1">
            <button className="btn h-7 text-[12px]" onClick={() => s.set({ dialog: "autotile" })}>Gerar autotile…</button>
            <button className="btn h-7 text-[12px]" onClick={() => s.set({ dialog: "tilemap" })}>Prévia de mapa…</button>
          </div>
        </>
      )}
    </div>
  );
}

function GridSection({ sprite, data }: { sprite: Sprite; data?: TileData }) {
  const s = useEditor();
  const [tw, setTw] = useState(data?.tile_width ?? Math.min(16, sprite.width));
  const [th, setTh] = useState(data?.tile_height ?? Math.min(16, sprite.height));
  const [margin, setMargin] = useState<[number, number]>(data?.margin ?? [0, 0]);
  const [spacing, setSpacing] = useState<[number, number]>(data?.spacing ?? [0, 0]);
  const size = data ? gridSize(sprite, data) : undefined;
  return (
    <Section title="Grade de tiles">
      <div className="flex flex-wrap gap-x-3 gap-y-1">
        <Num label="Largura" min={1} value={tw} onChange={setTw} />
        <Num label="Altura" min={1} value={th} onChange={setTh} />
        <Num label="Margem x" value={margin[0]} onChange={(v) => setMargin([v, margin[1]])} />
        <Num label="y" value={margin[1]} onChange={(v) => setMargin([margin[0], v])} />
        <Num label="Espaço x" value={spacing[0]} onChange={(v) => setSpacing([v, spacing[1]])} />
        <Num label="y" value={spacing[1]} onChange={(v) => setSpacing([spacing[0], v])} />
      </div>
      <div className="flex flex-wrap items-center gap-1">
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("tile_grid_set", { sprite: sprite.id, tile_width: tw, tile_height: th, margin, spacing })}>
          {data ? "Aplicar" : "Usar como tileset"}
        </button>
        {data && (
          <button className="btn-icon h-7 w-7" title="Remover a grade e os dados de tiles" onClick={() => void s.run("tile_grid_set", { sprite: sprite.id, remove: true })}>
            <Trash2 size={14} />
          </button>
        )}
        {size && <span className="text-[12px] text-muted">{size.columns} × {size.rows} tiles</span>}
      </div>
      {data && (
        <label className="flex items-center gap-2 text-[12px] text-muted">
          <input type="checkbox" checked={s.showTileGrid} onChange={(e) => s.set({ showTileGrid: e.target.checked })} />
          Mostrar grade e colisões no canvas
        </label>
      )}
    </Section>
  );
}

/** Miniatura do frame com a grade; clicar escolhe o tile. */
export function TilePicker({ sprite, data, max = 260 }: { sprite: Sprite; data: TileData; max?: number }) {
  const s = useEditor();
  const frame = useFrameCanvas(sprite.id, s.frameId ?? sprite.frames[0]?.id);
  const ref = useRef<HTMLCanvasElement>(null);
  const k = Math.max(1, Math.floor(max / Math.max(sprite.width, sprite.height)));
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    c.width = sprite.width * k;
    c.height = sprite.height * k;
    const g = c.getContext("2d")!;
    g.imageSmoothingEnabled = false;
    g.clearRect(0, 0, c.width, c.height);
    if (frame) g.drawImage(frame, 0, 0, c.width, c.height);
    const { columns, rows } = gridSize(sprite, data);
    g.lineWidth = 1;
    for (let r = 0; r < rows; r++) {
      for (let col = 0; col < columns; col++) {
        const t = tileRect(data, [col, r]);
        g.strokeStyle = "rgba(0,229,255,0.6)";
        g.strokeRect(t.x * k + 0.5, t.y * k + 0.5, t.w * k - 1, t.h * k - 1);
        if (collisionOf(data, [col, r])) {
          g.fillStyle = "#ff4081";
          g.fillRect(t.x * k + 2, t.y * k + 2, 4, 4);
        }
        if (terrainOf(data, [col, r])) {
          g.fillStyle = "#ffd600";
          g.fillRect(t.x * k + 7, t.y * k + 2, 4, 4);
        }
      }
    }
    const sel = s.tileSelected;
    if (sel[0] < columns && sel[1] < rows) {
      const t = tileRect(data, sel);
      g.strokeStyle = "#ffd600";
      g.lineWidth = 2;
      g.strokeRect(t.x * k + 1, t.y * k + 1, t.w * k - 2, t.h * k - 2);
    }
  }, [frame, sprite, data, k, s.tileSelected]);
  return (
    <canvas
      ref={ref}
      className="checker pixelated block max-w-full cursor-pointer self-start rounded-sm"
      aria-label="Tiles do sprite: clique para escolher um tile"
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        const x = Math.floor(((e.clientX - r.left) / r.width) * sprite.width);
        const y = Math.floor(((e.clientY - r.top) / r.height) * sprite.height);
        const t = tileAt(sprite, data, x, y);
        if (t) s.set({ tileSelected: t });
      }}
    />
  );
}

/** O tile escolhido ampliado, com os polígonos; clicar adiciona um vértice ao rascunho. */
function TileZoom({ sprite, data, tile, draft, onPoint }: { sprite: Sprite; data: TileData; tile: TileCoord; draft: Point[]; onPoint: (p: Point) => void }) {
  const s = useEditor();
  const frame = useFrameCanvas(sprite.id, s.frameId ?? sprite.frames[0]?.id);
  const ref = useRef<HTMLCanvasElement>(null);
  const k = Math.max(2, Math.floor(200 / Math.max(data.tile_width, data.tile_height)));
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const { w, h, x, y } = tileRect(data, tile);
    c.width = w * k + 1;
    c.height = h * k + 1;
    const g = c.getContext("2d")!;
    g.imageSmoothingEnabled = false;
    g.clearRect(0, 0, c.width, c.height);
    if (frame) g.drawImage(frame, x, y, w, h, 0, 0, w * k, h * k);
    const path = (poly: Point[], close: boolean) => {
      g.beginPath();
      poly.forEach(([px, py], i) => (i ? g.lineTo(px * k + 0.5, py * k + 0.5) : g.moveTo(px * k + 0.5, py * k + 0.5)));
      if (close) g.closePath();
    };
    for (const poly of collisionOf(data, tile)?.polygons ?? []) {
      path(poly, true);
      g.fillStyle = "rgba(255,64,129,0.25)";
      g.fill();
      g.strokeStyle = "#ff4081";
      g.lineWidth = 2;
      g.stroke();
    }
    if (draft.length) {
      path(draft, draft.length > 2);
      g.strokeStyle = "#ffd600";
      g.lineWidth = 2;
      g.stroke();
      g.fillStyle = "#ffd600";
      draft.forEach(([px, py]) => g.fillRect(px * k - 3, py * k - 3, 7, 7));
    }
  }, [frame, data, tile, draft, k]);
  return (
    <canvas
      ref={ref}
      className="checker block max-w-full cursor-crosshair self-start rounded-sm"
      aria-label="Tile ampliado: clique para adicionar um vértice do polígono"
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        const px = Math.round((((e.clientX - r.left) / r.width) * e.currentTarget.width) / k);
        const py = Math.round((((e.clientY - r.top) / r.height) * e.currentTarget.height) / k);
        onPoint([Math.max(0, Math.min(data.tile_width, px)), Math.max(0, Math.min(data.tile_height, py))]);
      }}
    />
  );
}

const SHAPES: { shape: string; label: string }[] = [
  { shape: "full", label: "Cheio" },
  { shape: "top_half", label: "Metade de cima" },
  { shape: "bottom_half", label: "Metade de baixo" },
  { shape: "left_half", label: "Esquerda" },
  { shape: "right_half", label: "Direita" },
];

function parsePoints(text: string): Point[] {
  return text
    .trim()
    .split(/\s+/)
    .map((p) => p.split(",").map(Number))
    .filter((p) => p.length === 2 && p.every(Number.isFinite))
    .map((p) => [p[0], p[1]] as Point);
}

function CollisionSection({ sprite, data }: { sprite: Sprite; data: TileData }) {
  const s = useEditor();
  const tile = s.tileSelected;
  const [draft, setDraft] = useState<Point[]>([]);
  const [text, setText] = useState("");
  useEffect(() => setDraft([]), [tile[0], tile[1], sprite.id]);
  const polygons = collisionOf(data, tile)?.polygons ?? [];
  const run = (args: Record<string, unknown>) => s.run("tile_collision_set", { sprite: sprite.id, tile, ...args });
  return (
    <Section title={`Colisão do tile [${tile[0]}, ${tile[1]}]`}>
      <div className="flex flex-wrap gap-1">
        {SHAPES.map((x) => (
          <button key={x.shape} className="btn h-7 text-[12px]" onClick={() => void run({ shape: x.shape })}>{x.label}</button>
        ))}
        <button className="btn h-7 text-[12px]" disabled={!polygons.length} onClick={() => void run({ clear: true })}>Limpar</button>
      </div>
      <span className="text-[12px] text-muted">{polygons.length ? `${polygons.length} polígono(s)` : "Sem colisão"} · clique no tile para criar um polígono (vértices nos cantos dos pixels)</span>
      <TileZoom sprite={sprite} data={data} tile={tile} draft={draft} onPoint={(p) => setDraft([...draft, p])} />
      <div className="flex flex-wrap items-center gap-1">
        <button className="btn h-7 text-[12px]" disabled={draft.length < 3} onClick={() => void run({ polygons: [draft], add: true }).then(() => setDraft([]))}>
          <Plus size={13} /> Adicionar polígono
        </button>
        <button className="btn-icon h-7 w-7" title="Desfazer o último ponto" disabled={!draft.length} onClick={() => setDraft(draft.slice(0, -1))}><Undo2 size={13} /></button>
        <button className="btn h-7 text-[12px]" disabled={!draft.length} onClick={() => setDraft([])}>Descartar</button>
      </div>
      <div className="flex items-center gap-1">
        <input className="min-w-0 flex-1 font-mono text-[12px]" placeholder="ou digite: 0,16 16,0 16,16" value={text} onChange={(e) => setText(e.target.value)} />
        <button className="btn h-7 text-[12px]" disabled={parsePoints(text).length < 3} onClick={() => void run({ polygons: [parsePoints(text)], add: true }).then(() => setText(""))}>Adicionar</button>
      </div>
    </Section>
  );
}

function TerrainSection({ sprite, data }: { sprite: Sprite; data: TileData }) {
  const s = useEditor();
  const tile = s.tileSelected;
  const sets = data.terrain_sets ?? [];
  const current = terrainOf(data, tile);
  const [setIndex, setSetIndex] = useState(current?.set ?? 0);
  const [terrain, setTerrain] = useState<number | undefined>(current?.terrain ?? (sets[0]?.terrains.length ? 0 : undefined));
  const [bits, setBits] = useState<Partial<Record<PeeringBit, number>>>(current?.peering ?? {});
  const [newSet, setNewSet] = useState("");
  const [newTerrain, setNewTerrain] = useState("");
  const [color, setColor] = useState("#4caf50");
  const key = `${tile[0]},${tile[1]}:${JSON.stringify(current ?? null)}`;
  useEffect(() => {
    setSetIndex(current?.set ?? 0);
    setTerrain(current?.terrain ?? (sets[0]?.terrains.length ? 0 : undefined));
    setBits(current?.peering ?? {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const set = sets[setIndex];
  const sendSet = (name: string, mode: TerrainMode, terrains: { name: string; color: string }[]) =>
    s.run("tile_terrain_set", { sprite: sprite.id, sets: [{ name, mode, terrains }] });
  const addTerrain = () => {
    const target = sets.find((x) => x.name === (newSet || set?.name));
    const name = newTerrain.trim();
    if (!name) return;
    if (target) void sendSet(target.name, target.mode, [...target.terrains, { name, color }]).then(() => setNewTerrain(""));
    else void sendSet(newSet.trim() || "Terrenos", "match_corners_and_sides", [{ name, color }]).then(() => (setNewTerrain(""), setNewSet("")));
  };
  const toggle = (bit: PeeringBit) => {
    const next = { ...bits };
    if (next[bit] !== undefined) delete next[bit];
    else if (terrain !== undefined) next[bit] = terrain;
    setBits(next);
  };
  const apply = () => {
    const sameBits = Object.entries(bits).filter(([, v]) => v === terrain).map(([b]) => b);
    const other = Object.fromEntries(Object.entries(bits).filter(([, v]) => v !== terrain));
    void s.run("tile_terrain_set", { sprite: sprite.id, tiles: [{ tile, set: setIndex, terrain, bits: sameBits, bit_terrains: other }] });
  };
  return (
    <Section title="Terrenos (Godot 4)">
      {sets.map((x, i) => (
        <div key={x.name} className="flex flex-col gap-1 rounded bg-raised p-1.5">
          <div className="flex items-center gap-1">
            <span className="min-w-0 flex-1 truncate text-[12px]">{x.name}</span>
            <select className="text-[11px]" value={x.mode} onChange={(e) => void sendSet(x.name, e.target.value as TerrainMode, x.terrains)}>
              {TERRAIN_MODES.map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
            </select>
            <button className="btn-icon h-6 w-6" title="Apagar conjunto" onClick={() => void s.run("tile_terrain_set", { sprite: sprite.id, remove_sets: [i] })}><Trash2 size={12} /></button>
          </div>
          <div className="flex flex-wrap gap-1">
            {x.terrains.map((t, j) => (
              <span key={t.name} className="chip flex items-center gap-1 bg-panel text-muted">
                <span className="inline-block h-2.5 w-2.5 rounded-sm" style={{ background: t.color }} />
                {j}: {t.name}
              </span>
            ))}
          </div>
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-1">
        <input className="w-24 text-[12px]" placeholder={set ? `conjunto (${set.name})` : "conjunto"} value={newSet} onChange={(e) => setNewSet(e.target.value)} />
        <input className="w-24 text-[12px]" placeholder="terreno" value={newTerrain} onChange={(e) => setNewTerrain(e.target.value)} />
        <input type="color" aria-label="Cor do terreno" className="h-7 w-8" value={color} onChange={(e) => setColor(e.target.value)} />
        <button className="btn-icon h-7 w-7" title="Adicionar terreno" disabled={!newTerrain.trim()} onClick={addTerrain}><Plus size={13} /></button>
      </div>
      {set && (
        <div className="mt-1 flex flex-col gap-1.5 border-t border-line pt-2">
          <span className="text-[12px] text-muted">Tile [{tile[0]}, {tile[1]}]{current ? "" : " (sem terreno)"}</span>
          <div className="flex items-center gap-1">
            <select className="text-[12px]" value={setIndex} onChange={(e) => setSetIndex(Number(e.target.value))}>
              {sets.map((x, i) => <option key={x.name} value={i}>{x.name}</option>)}
            </select>
            <select className="text-[12px]" value={terrain ?? ""} onChange={(e) => setTerrain(e.target.value === "" ? undefined : Number(e.target.value))}>
              <option value="">(sem terreno no centro)</option>
              {set.terrains.map((t, j) => <option key={t.name} value={j}>{t.name}</option>)}
            </select>
          </div>
          <div className="grid w-[108px] grid-cols-3 gap-1" aria-label="Bits de vizinhança">
            {BIT_GRID.flat().map((bit, i) => {
              if (!bit) {
                const c = terrain !== undefined ? set.terrains[terrain]?.color : undefined;
                return <span key={i} className="h-8 w-8 rounded ring-1 ring-line" style={{ background: c ?? "transparent" }} title="Terreno do centro" />;
              }
              const on = bits[bit] !== undefined;
              const allowed = bitAllowed(set.mode, bit);
              const c = on ? set.terrains[bits[bit]!]?.color : undefined;
              return (
                <button key={bit} title={bit} aria-pressed={on} disabled={!allowed || (!on && terrain === undefined)} onClick={() => toggle(bit)} className={`h-8 w-8 rounded ring-1 ${on ? "ring-accent" : "ring-line"} disabled:opacity-30`} style={{ background: c ?? "transparent" }} />
              );
            })}
          </div>
          <div className="flex gap-1">
            <button className="btn h-7 text-[12px]" disabled={terrain === undefined && !Object.keys(bits).length} onClick={apply}>Aplicar ao tile</button>
            <button className="btn h-7 text-[12px]" disabled={!current} onClick={() => void s.run("tile_terrain_set", { sprite: sprite.id, tiles: [{ tile, clear: true }] })}>Limpar terreno</button>
          </div>
          {!sameTile(tile, current?.tile) && <span className="text-[11px] text-faint">Marque os lados e cantos em que o vizinho é do mesmo terreno.</span>}
        </div>
      )}
    </Section>
  );
}
