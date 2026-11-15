import { ArrowDown, ArrowUp, Copy, Eye, EyeOff, Image as ImageIcon, Layers, Lock, Merge, Palette as PaletteIcon, Plus, ScanSearch, Shapes, SlidersHorizontal, Target, Trash2, Unlock } from "lucide-react";
import { useState } from "react";
import { useFrameCanvas } from "../core/hooks";
import { currentPalette, currentSprite, useEditor } from "../core/store";
import { TilesPanel, TilesTabIcon } from "./tiles/TilesPanel";
import { Swords } from "lucide-react";
import { HitboxesPanel } from "./godot/HitboxesPanel";
import { Type as TextIcon } from "lucide-react";
import { TextPanel } from "./texto/TextPanel";
import { PaletteFileButtons, PaletteVariantsSection } from "./paletas/PaletteTools";

const TABS = [
  { id: "layers", icon: <Layers size={15} />, label: "Camadas" },
  { id: "palette", icon: <PaletteIcon size={15} />, label: "Paleta" },
  { id: "props", icon: <SlidersHorizontal size={15} />, label: "Ajustes" },
  { id: "regions", icon: <Shapes size={15} />, label: "Regiões" },
  { id: "anchors", icon: <Target size={15} />, label: "Pontos" },
  { id: "inspect", icon: <ScanSearch size={15} />, label: "Inspeção" },
  { id: "tiles", icon: <TilesTabIcon />, label: "Tiles" },
  { id: "hitboxes", icon: <Swords size={15} />, label: "Hitboxes" },
  { id: "text", icon: <TextIcon size={15} />, label: "Texto" },
] as const;

export function RightPanel() {
  const panel = useEditor((s) => s.panel);
  const set = useEditor((s) => s.set);
  return (
    <aside className="inspector" aria-label="Propriedades do projeto">
      <nav className="inspector-tabs" aria-label="Painéis de edição">
        {TABS.map((t) => (
          <button
            key={t.id}
            title={t.label}
            aria-pressed={panel === t.id}
            onClick={() => set({ panel: t.id })}
            className={`inspector-tab ${panel === t.id ? "is-active" : ""}`}
          >
            {t.icon}<span>{t.label}</span>
          </button>
        ))}
      </nav>
      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {panel === "layers" && <><LayersPanel /><div className="mt-6 border-t border-line pt-4"><PalettePanel compact /></div></>}
        {panel === "palette" && <PalettePanel />}
        {panel === "props" && <PropsPanel />}
        {panel === "regions" && <RegionsPanel />}
        {panel === "anchors" && <AnchorsPanel />}
        {panel === "inspect" && <InspectPanel />}
        {panel === "tiles" && <TilesPanel />}
        {panel === "hitboxes" && <HitboxesPanel />}
        {panel === "text" && <TextPanel />}
      </div>
    </aside>
  );
}

function Header({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="mb-2 flex items-center justify-between">
      <span className="panel-title">{title}</span>
      <div className="flex gap-0.5">{children}</div>
    </div>
  );
}

function LayerThumb({ layer }: { layer: number }) {
  const s = useEditor();
  const canvas = useFrameCanvas(s.spriteId, s.frameId, layer);
  return (
    <div className="checker h-8 w-8 shrink-0 overflow-hidden rounded ring-1 ring-line">
      {canvas && <img alt="" className="pixelated h-full w-full object-contain" src={canvas.toDataURL()} />}
    </div>
  );
}

function LayersPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const [editing, setEditing] = useState<number>();
  if (!sprite) return <Empty />;
  const layers = [...sprite.layers].reverse();
  const sel = sprite.layers.find((l) => l.id === s.layerId);
  const idx = sel ? sprite.layers.indexOf(sel) : -1;
  const t = { sprite: sprite.id };
  return (
    <div>
      <Header title="Camadas">
        <button className="btn-icon h-7 w-7" title="Nova camada" onClick={() => void s.run("layer_create", { ...t, name: uniqueName("Layer", sprite.layers.map((l) => l.name)), above: s.layerId })}>
          <Plus size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Duplicar" disabled={!sel} onClick={() => void s.run("layer_duplicate", { ...t, layer: s.layerId })}>
          <Copy size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Fundir com a de baixo" disabled={idx <= 0} onClick={() => void s.run("layer_merge", { ...t, layer: s.layerId })}>
          <Merge size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Subir" disabled={idx < 0 || idx >= sprite.layers.length - 1} onClick={() => void s.run("layer_reorder", { ...t, layer: s.layerId, index: idx + 1 })}>
          <ArrowUp size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Descer" disabled={idx <= 0} onClick={() => void s.run("layer_reorder", { ...t, layer: s.layerId, index: idx - 1 })}>
          <ArrowDown size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Apagar camada" disabled={sprite.layers.length <= 1} onClick={() => void s.run("layer_delete", { ...t, layer: s.layerId })}>
          <Trash2 size={15} />
        </button>
      </Header>
      <ul className="flex flex-col gap-1">
        {layers.map((l) => (
          <li
            key={l.id}
            onClick={() => s.set({ layerId: l.id })}
            tabIndex={0}
            aria-label={`Camada ${l.name}${l.id === s.layerId ? ", selecionada" : ""}`}
            onKeyDown={(e) => { if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) { e.preventDefault(); s.set({ layerId: l.id }); } }}
            className={`flex cursor-pointer items-center gap-2 rounded-md px-1.5 py-1 ${l.id === s.layerId ? "bg-raised ring-1 ring-accent/50" : "hover:bg-panel"}`}
          >
            <LayerThumb layer={l.id} />
            {editing === l.id ? (
              <input
                autoFocus
                defaultValue={l.name}
                className="min-w-0 flex-1 py-0.5"
                onBlur={(e) => {
                  setEditing(undefined);
                  if (e.target.value && e.target.value !== l.name) void s.run("layer_properties", { ...t, layer: l.id, name: e.target.value });
                }}
                onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
              />
            ) : (
              <span className={`min-w-0 flex-1 truncate ${l.visible ? "" : "text-faint"}`} onDoubleClick={() => setEditing(l.id)}>
                {l.name}
                {l.reference && <span className="chip ml-1 bg-accent-2/15 text-accent-2">ref</span>}
              </span>
            )}
            <button className="btn-icon h-6 w-6" title={l.visible ? "Esconder" : "Mostrar"} onClick={(e) => (e.stopPropagation(), void s.run("layer_properties", { ...t, layer: l.id, visible: !l.visible }))}>
              {l.visible ? <Eye size={14} /> : <EyeOff size={14} />}
            </button>
            <button className="btn-icon h-6 w-6" title={l.locked ? "Desbloquear" : "Bloquear"} onClick={(e) => (e.stopPropagation(), void s.run("layer_properties", { ...t, layer: l.id, locked: !l.locked }))}>
              {l.locked ? <Lock size={14} className="text-warn" /> : <Unlock size={14} />}
            </button>
          </li>
        ))}
      </ul>
      {sel && (
        <div className="mt-3 flex flex-col gap-2 rounded-md bg-panel p-2">
          <label className="flex items-center gap-2 text-muted">
            Opacidade
            <input type="range" min={0} max={255} defaultValue={sel.opacity} key={`${sel.id}-${sel.opacity}`} className="flex-1" onMouseUp={(e) => void s.run("layer_properties", { ...t, layer: sel.id, opacity: Number((e.target as HTMLInputElement).value) })} />
            <span className="w-8 text-right font-mono text-[11px]">{Math.round((sel.opacity / 255) * 100)}%</span>
          </label>
          <label className="flex items-center gap-2 text-muted">
            Mistura
            <select value={sel.blend} className="flex-1" onChange={(e) => void s.run("layer_properties", { ...t, layer: sel.id, blend: e.target.value })}>
              {["normal", "multiply", "screen", "overlay", "darken", "lighten", "add", "subtract"].map((b) => (
                <option key={b}>{b}</option>
              ))}
            </select>
          </label>
          <label className="flex items-center gap-2 text-muted">
            <input type="checkbox" checked={sel.reference} onChange={(e) => void s.run("layer_properties", { ...t, layer: sel.id, reference: e.target.checked })} />
            Camada de referência (não exporta)
          </label>
        </div>
      )}
    </div>
  );
}

function uniqueName(base: string, names: string[]) {
  let i = names.length + 1;
  while (names.includes(`${base} ${i}`)) i++;
  return `${base} ${i}`;
}

function PalettePanel({ compact = false }: { compact?: boolean }) {
  const s = useEditor();
  const pal = currentPalette(s);
  const palettes = s.doc?.model.palettes ?? [];
  if (!s.doc) return <Empty />;
  return (
    <div>
      <Header title="Paleta">
        {!compact && <PaletteFileButtons palette={pal?.name} />}
        <button className="btn-icon h-7 w-7" title="Adicionar a cor principal" disabled={!pal} onClick={() => pal && void s.run("palette_update", { palette: pal.name, add: [s.primary] })}>
          <Plus size={15} />
        </button>
      </Header>
      <select className="mb-2 w-full" value={pal?.name ?? ""} onChange={(e) => void s.run("palette_update", { palette: e.target.value, activate: true })}>
        {palettes.map((p) => (
          <option key={p.name}>{p.name}</option>
        ))}
      </select>
      <div className="grid grid-cols-8 gap-1">
        {pal?.colors.map((c, i) => (
          <button
            key={`${c}-${i}`}
            title={`${i}: ${c} (clique: principal, direito: secundária, Alt+clique: remover)`}
            className={`checker relative aspect-square overflow-hidden rounded ring-1 ${c === s.primary ? "ring-2 ring-accent" : c === s.secondary ? "ring-2 ring-accent-2" : "ring-line"}`}
            onClick={(e) => (e.altKey ? void s.run("palette_update", { palette: pal.name, remove: [c] }) : s.set({ primary: c }))}
            onContextMenu={(e) => (e.preventDefault(), s.set({ secondary: c }))}
          >
            <span className="absolute inset-0" style={{ background: c }} />
            <span className="absolute right-0.5 bottom-0 font-mono text-[8px] text-white mix-blend-difference">{i}</span>
          </button>
        ))}
      </div>
      <div className="mt-3 flex items-center gap-2">
        <input className="w-24 font-mono" value={s.primary} onChange={(e) => s.set({ primary: e.target.value })} title="Cor principal em hexadecimal (#rrggbb ou #rrggbbaa)" />
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("palette_create", { name: uniqueName("Paleta", palettes.map((p) => p.name)), colors: [s.primary] })}>
          Nova paleta
        </button>
      </div>
      {!compact && <div className="mt-3 flex flex-col gap-1.5 rounded-md bg-panel p-2 text-muted">
        <span className="panel-title">Ajustes no sprite</span>
        <button className="btn h-7 justify-start text-[12px]" onClick={() => void s.run("palette_replace", { sprite: s.spriteId, from: s.secondary, to: s.primary })}>
          Trocar secundária → principal (todos os frames)
        </button>
        <button className="btn h-7 justify-start text-[12px]" disabled={!pal} onClick={() => pal && void s.run("palette_quantize", { sprite: s.spriteId, palette: pal.name })}>
          Levar todas as cores para esta paleta
        </button>
      </div>}
      {!compact && <PaletteVariantsSection />}
    </div>
  );
}

function PropsPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const [w, setW] = useState<number>();
  const [h, setH] = useState<number>();
  if (!sprite || !s.doc) return <Empty />;
  const frame = sprite.frames.find((f) => f.id === s.frameId);
  return (
    <div className="flex flex-col gap-3">
      <div>
        <Header title="Sprite" />
        <select className="mb-2 w-full" value={sprite.id} onChange={(e) => s.set({ spriteId: Number(e.target.value), layerId: undefined, frameId: undefined, animationId: undefined })}>
          {s.doc.model.sprites.map((sp) => (
            <option key={sp.id} value={sp.id}>
              {sp.name} ({sp.width}×{sp.height})
            </option>
          ))}
        </select>
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("sprite_create", { name: uniqueName("Sprite", s.doc!.model.sprites.map((x) => x.name)), width: sprite.width, height: sprite.height })}>
          <Plus size={14} /> Novo sprite
        </button>
      </div>
      <div className="flex flex-col gap-2 rounded-md bg-panel p-2">
        <span className="panel-title">Tamanho</span>
        <div className="flex items-center gap-1.5">
          <input type="number" className="w-16" defaultValue={sprite.width} key={`w${sprite.width}`} onChange={(e) => setW(Number(e.target.value))} />×
          <input type="number" className="w-16" defaultValue={sprite.height} key={`h${sprite.height}`} onChange={(e) => setH(Number(e.target.value))} />
        </div>
        <div className="flex gap-1">
          <button className="btn h-7 text-[12px]" onClick={() => void s.run("sprite_update", { sprite: sprite.id, size: [w ?? sprite.width, h ?? sprite.height], anchor: "bottom_center" })}>
            Mudar canvas
          </button>
          <button className="btn h-7 text-[12px]" onClick={() => void s.run("sprite_update", { sprite: sprite.id, size: [w ?? sprite.width, h ?? sprite.height], scale_content: true })}>
            Escalar
          </button>
        </div>
        <span className="panel-title mt-1">Pivô (origem)</span>
        <div className="flex items-center gap-1.5 text-muted">
          [{sprite.origin[0]}, {sprite.origin[1]}]
          <button className="btn h-7 text-[12px]" title="Usa o centro da base da seleção" disabled={!s.selection} onClick={() => s.selection && void s.run("sprite_update", { sprite: sprite.id, origin: [s.selection.x + Math.floor(s.selection.w / 2), s.selection.y + s.selection.h - 1] })}>
            Da seleção
          </button>
        </div>
      </div>
      {frame && (
        <div className="flex flex-col gap-2 rounded-md bg-panel p-2">
          <span className="panel-title">Frame {frame.id}</span>
          <label className="flex items-center gap-2 text-muted">
            Duração
            <input type="number" className="w-20" defaultValue={frame.duration_ms} key={`d${frame.id}-${frame.duration_ms}`} onBlur={(e) => Number(e.target.value) !== frame.duration_ms && void s.run("frame_edit", { sprite: sprite.id, frame: frame.id, duration_ms: Number(e.target.value) })} />
            ms
          </label>
          <label className="flex items-center gap-2 text-muted">
            Tags
            <input className="flex-1" defaultValue={(frame.tags ?? []).join(", ")} key={`t${frame.id}-${(frame.tags ?? []).join()}`} onBlur={(e) => void s.run("frame_edit", { sprite: sprite.id, frame: frame.id, tags: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })} />
          </label>
        </div>
      )}
      <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
        <span className="panel-title">Referências</span>
        {s.doc.model.references.filter((r) => r.sprite === undefined || r.sprite === sprite.id).map((r) => (
          <div key={r.id} className="flex items-center gap-1.5">
            <ImageIcon size={14} className="text-faint" />
            <span className="min-w-0 flex-1 truncate">{r.name}</span>
            <button className="btn-icon h-6 w-6" onClick={() => void s.run("reference_update", { reference: r.id, visible: !r.visible })}>
              {r.visible ? <Eye size={13} /> : <EyeOff size={13} />}
            </button>
            <button className="btn-icon h-6 w-6" onClick={() => void s.run("reference_update", { reference: r.id, delete: true })}>
              <Trash2 size={13} />
            </button>
          </div>
        ))}
        <label className="flex items-center gap-2 text-muted">
          <input type="checkbox" checked={s.showReferences} onChange={(e) => s.set({ showReferences: e.target.checked })} />
          Mostrar no canvas
        </label>
      </div>
      <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
        <span className="panel-title">Snapshots</span>
        {s.doc.snapshots.map((sn) => (
          <div key={sn.id} className="flex items-center gap-1.5">
            <span className="min-w-0 flex-1 truncate">{sn.name}</span>
            <button className="btn h-6 text-[11px]" onClick={() => void s.run("snapshot_restore", { id: sn.id })}>
              Restaurar
            </button>
          </div>
        ))}
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("snapshot_create", { name: `Snapshot ${s.doc!.snapshots.length + 1}` })}>
          <Plus size={14} /> Guardar snapshot
        </button>
      </div>
    </div>
  );
}

function RegionsPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  if (!sprite) return <Empty />;
  const layer = sprite.layers.find((l) => l.id === s.layerId);
  return (
    <div>
      <Header title="Regiões semânticas" />
      <label className="mb-2 flex items-center gap-2 text-muted">
        <input type="checkbox" checked={s.showRegions} onChange={(e) => s.set({ showRegions: e.target.checked })} />
        Mostrar no canvas
      </label>
      <ul className="mb-3 flex flex-col gap-1">
        {sprite.regions.map((r) => (
          <li key={r.id} className="flex items-center gap-2 rounded-md bg-panel px-2 py-1.5">
            <span className="min-w-0 flex-1 truncate">
              {r.name} {r.part && <span className="text-faint">· {r.part}</span>}
            </span>
            <span className="text-[11px] text-faint">{Object.keys(r.shapes ?? {}).length} por frame</span>
            <button className="btn-icon h-6 w-6" title="Apagar" onClick={() => void s.run("region_update", { sprite: sprite.id, region: r.id, delete: true })}>
              <Trash2 size={13} />
            </button>
          </li>
        ))}
        {sprite.regions.length === 0 && <p className="text-faint">Nenhuma região ainda. Regiões dividem o personagem em partes (cabeça, braço, arma...) para recortar e animar.</p>}
      </ul>
      <RegionCreator layerName={layer?.name} />
    </div>
  );
}

function RegionCreator({ layerName }: { layerName?: string }) {
  const s = useEditor();
  const [name, setName] = useState("");
  const [part, setPart] = useState("");
  return (
    <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
      <span className="panel-title">Nova região</span>
      <input placeholder="nome (ex.: braco_dir)" value={name} onChange={(e) => setName(e.target.value)} />
      <input placeholder="parte (ex.: arm_right)" value={part} onChange={(e) => setPart(e.target.value)} />
      <div className="flex gap-1">
        <button className="btn h-7 text-[12px]" disabled={!name || !s.selection} onClick={() => void s.run("region_create", { sprite: s.spriteId, name, part: part || undefined, shape: { type: "rect", rect: s.selection } }).then(() => setName(""))}>
          Da seleção
        </button>
        <button className="btn h-7 text-[12px]" disabled={!name || !layerName} onClick={() => void s.run("region_create", { sprite: s.spriteId, name, part: part || undefined, layer: s.layerId, from_frame: s.frameId }).then(() => setName(""))}>
          Dos pixels de "{layerName}"
        </button>
      </div>
    </div>
  );
}

function AnchorsPanel() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const [name, setName] = useState("");
  const [kind, setKind] = useState("joint");
  if (!sprite) return <Empty />;
  return (
    <div>
      <Header title="Anchors" />
      <label className="mb-2 flex items-center gap-2 text-muted">
        <input type="checkbox" checked={s.showAnchors} onChange={(e) => s.set({ showAnchors: e.target.checked })} />
        Mostrar no canvas (com o pivô)
      </label>
      <ul className="mb-3 flex flex-col gap-1">
        {sprite.anchors.map((a) => {
          const p = a.positions?.[String(s.frameId)] ?? a.default;
          const active = s.tool === "anchor" && s.anchorTarget === a.name;
          return (
            <li key={a.id} className={`flex items-center gap-2 rounded-md px-2 py-1.5 ${active ? "bg-raised ring-1 ring-accent" : "bg-panel"}`}>
              <span className="min-w-0 flex-1 truncate">
                {a.name} <span className="text-faint">· {a.kind}</span>
              </span>
              <span className="font-mono text-[11px] text-muted">{p ? `${p[0]},${p[1]}` : "—"}</span>
              <button className="btn-icon h-6 w-6" title="Clique no canvas para posicionar neste frame" onClick={() => s.set({ tool: "anchor", anchorTarget: a.name, showAnchors: true })}>
                <Target size={13} />
              </button>
              <button className="btn-icon h-6 w-6" title="Apagar" onClick={() => void s.run("anchor_update", { sprite: sprite.id, anchor: a.id, delete: true })}>
                <Trash2 size={13} />
              </button>
            </li>
          );
        })}
      </ul>
      <div className="flex flex-col gap-1.5 rounded-md bg-panel p-2">
        <span className="panel-title">Novo anchor</span>
        <input placeholder="nome (ex.: mao_dir)" value={name} onChange={(e) => setName(e.target.value)} />
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          {["point", "pivot", "ground", "joint", "attachment"].map((k) => (
            <option key={k}>{k}</option>
          ))}
        </select>
        <button
          className="btn h-7 text-[12px]"
          disabled={!name}
          onClick={() =>
            void s.run("anchor_create", { sprite: sprite.id, name, kind, position: s.selection ? [s.selection.x, s.selection.y] : [Math.floor(sprite.width / 2), Math.floor(sprite.height / 2)] }).then(() => {
              s.set({ tool: "anchor", anchorTarget: name, showAnchors: true });
              setName("");
            })
          }
        >
          <Plus size={14} /> Criar e posicionar
        </button>
      </div>
    </div>
  );
}

function InspectPanel() {
  const s = useEditor();
  const [inspect, setInspect] = useState<any>();
  const [issues, setIssues] = useState<any>();
  if (!s.doc) return <Empty />;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-1">
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("frame_inspect", { sprite: s.spriteId, frame: s.frameId }).then((r) => setInspect(r.data))}>
          Inspecionar frame
        </button>
        <button className="btn h-7 text-[12px]" onClick={() => void s.run("project_validate", { sprite: s.spriteId }).then((r) => setIssues(r.data))}>
          Validar sprite
        </button>
      </div>
      {inspect && (
        <div className="flex flex-col gap-1 rounded-md bg-panel p-2 text-muted">
          <span className="panel-title">Frame {inspect.frame}</span>
          <span>Pixels visíveis: {inspect.opaque_pixels} · semitransparentes: {inspect.translucent_pixels}</span>
          <span>Caixa: {inspect.bounds ? `${inspect.bounds.x},${inspect.bounds.y} ${inspect.bounds.w}×${inspect.bounds.h}` : "vazio"} · base y={inspect.ground_y ?? "—"}</span>
          <span>Buracos: {inspect.holes.count} · cores fora da paleta: {inspect.off_palette_colors}</span>
          <div className="mt-1 grid grid-cols-6 gap-1">
            {inspect.colors.map((c: any) => (
              <div key={c.color} title={`${c.color}: ${c.pixels}px${c.palette_index === null ? " (fora da paleta)" : ""}`} className={`checker relative aspect-square overflow-hidden rounded ${c.palette_index === null ? "ring-2 ring-warn" : "ring-1 ring-line"}`}>
                <span className="absolute inset-0" style={{ background: c.color }} />
              </div>
            ))}
          </div>
        </div>
      )}
      {issues && (
        <div className="flex flex-col gap-1 rounded-md bg-panel p-2">
          <span className="panel-title">
            {issues.warnings} alertas · {issues.infos} notas
          </span>
          {issues.issues.length === 0 && <span className="text-ok">Nenhum problema técnico encontrado.</span>}
          {issues.issues.map((i: any, k: number) => (
            <div key={k} className={`text-[12px] ${i.severity === "warning" ? "text-warn" : "text-muted"}`}>
              <span className="font-mono text-[11px]">{i.code}</span> {i.message}
              {i.frame !== undefined && <span className="text-faint"> · frame {i.frame}</span>}
              {i.animation && <span className="text-faint"> · {i.animation}{i.direction ? `/${i.direction}` : ""}</span>}
            </div>
          ))}
          <span className="mt-1 text-[11px] text-faint">Verificações automáticas não dizem se ficou bonito: olhe o resultado.</span>
        </div>
      )}
    </div>
  );
}

function Empty() {
  return <p className="text-faint">Abra ou crie um projeto.</p>;
}
