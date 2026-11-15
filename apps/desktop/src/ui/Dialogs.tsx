import { open, save } from "@tauri-apps/plugin-dialog";
import { X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { currentAnimation, currentSprite, useEditor } from "../core/store";
import { BLANK_TEMPLATE, choiceSize, createRpgMakerProject, RpgMakerTemplateFields, type RpgTemplateChoice } from "./rpgmaker/NewProjectTemplate";
import { ExportRpgMakerPanel, isRpgMakerExport, RpgMakerExportOptions, rpgMakerExportCalls } from "./rpgmaker/ExportRpgMaker";
import { TilesetExportNote } from "./tiles/TileDialogs";
import { ExportOnSaveOption, rememberExportJob } from "./godot/ExportJobs";
import { NineSliceExportRows } from "./texto/TextPanel";
import { nineSliceAxes } from "./texto/state";
import { NormalMapExportOptions, VariantExportOptions, normalMapArgs, variantExportArgs } from "./paletas/ExportOptions";

function Modal({ title, children, onClose }: { title: string; children: React.ReactNode; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const dialog = ref.current;
    dialog?.showModal();
    return () => dialog?.close();
  }, []);
  return (
    <dialog ref={ref} className="editor-dialog" aria-labelledby={titleId} onCancel={(e) => { e.preventDefault(); onClose(); }} onClick={(e) => {
      if (e.target !== e.currentTarget) return;
      const bounds = e.currentTarget.getBoundingClientRect();
      if (e.clientX < bounds.left || e.clientX > bounds.right || e.clientY < bounds.top || e.clientY > bounds.bottom) onClose();
    }}>
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

const SIZES = [16, 24, 32, 48, 64, 96, 128];

export function NewProjectDialog() {
  const s = useEditor();
  const [name, setName] = useState("Meu sprite");
  const [w, setW] = useState(64);
  const [h, setH] = useState(64);
  const [bg, setBg] = useState(false);
  const [tpl, setTpl] = useState<RpgTemplateChoice>(BLANK_TEMPLATE);
  const fixed = choiceSize(tpl);
  const close = () => s.set({ dialog: null });
  return (
    <Modal title="Novo projeto" onClose={close}>
      <p className="mb-4 leading-5 text-muted">Escolha o tamanho da sua tela. Cada pixel é uma possibilidade.</p>
      <Row label="Nome">
        <input autoFocus value={name} onChange={(e) => setName(e.target.value)} className="w-56" />
      </Row>
      <RpgMakerTemplateFields value={tpl} onChange={setTpl} />
      {!tpl.kind && (<>
      <Row label="Tamanho">
        <span className="flex items-center gap-1">
          <input type="number" min={1} max={1024} value={w} onChange={(e) => setW(Number(e.target.value))} className="w-16" />×
          <input type="number" min={1} max={1024} value={h} onChange={(e) => setH(Number(e.target.value))} className="w-16" />
        </span>
      </Row>
      <div className="mb-2 flex flex-wrap justify-end gap-1">
        {SIZES.map((v) => (
          <button key={v} className={`chip ${w === v && h === v ? "bg-accent/20 text-accent" : "bg-raised text-muted hover:bg-hover"}`} onClick={() => (setW(v), setH(v))}>
            {v}×{v}
          </button>
        ))}
      </div>
      <Row label="Fundo branco na primeira camada">
        <input type="checkbox" checked={bg} onChange={(e) => setBg(e.target.checked)} />
      </Row>
      </>)}
      <div className="mt-4 flex justify-end gap-2">
        <button className="btn" onClick={close}>
          Cancelar
        </button>
        <button className="btn btn-primary" disabled={!name.trim() || (tpl.kind ? !fixed : !Number.isInteger(w) || !Number.isInteger(h) || w < 1 || h < 1 || w > 1024 || h > 1024)} onClick={() => void (tpl.kind ? createRpgMakerProject(s.run, name, tpl) : s.run("project_create", { name, width: w, height: h, background: bg ? "#ffffff" : undefined })).then(close).catch(() => {})}>
          Criar projeto
        </button>
      </div>
    </Modal>
  );
}

export function ImportDialog() {
  const s = useEditor();
  const [mode, setMode] = useState("new_layer");
  const [slice, setSlice] = useState(false);
  const [cw, setCw] = useState(32);
  const [ch, setCh] = useState(32);
  const [anim, setAnim] = useState("");
  const close = () => s.set({ dialog: null });
  const go = async () => {
    const path = await open({ multiple: false, filters: [{ name: "Imagens e Aseprite", extensions: ["png", "gif", "aseprite", "ase"] }] });
    if (typeof path !== "string") return;
    await s
      .run("import_image", { sprite: s.spriteId, path, mode, slice: slice ? { cell_width: cw, cell_height: ch } : undefined, animation: anim || undefined })
      .then(() => {
        s.toast("Imagem importada");
        close();
      })
      .catch(() => {});
  };
  return (
    <Modal title="Importar PNG, GIF ou Aseprite" onClose={close}>
      <Row label="Como">
        <select value={mode} onChange={(e) => setMode(e.target.value)} className="w-56">
          <option value="new_sprite">Sprite novo</option>
          <option value="new_layer">Layer nova neste sprite</option>
          <option value="frames">Frames novos neste sprite</option>
          <option value="reference">Imagem de referência</option>
        </select>
      </Row>
      <Row label="Cortar como sprite sheet">
        <input type="checkbox" checked={slice} onChange={(e) => setSlice(e.target.checked)} />
      </Row>
      {slice && (
        <Row label="Célula">
          <span className="flex items-center gap-1">
            <input type="number" value={cw} onChange={(e) => setCw(Number(e.target.value))} className="w-16" />×
            <input type="number" value={ch} onChange={(e) => setCh(Number(e.target.value))} className="w-16" />
          </span>
        </Row>
      )}
      <Row label="Criar animação (opcional)">
        <input placeholder="nome" value={anim} onChange={(e) => setAnim(e.target.value)} className="w-40" />
      </Row>
      <p className="mt-2 text-[12px] text-faint">Os pixels são preservados exatamente, sem suavização. Um .aseprite como sprite novo mantém layers, frames, tags (animações) e paleta.</p>
      <div className="mt-4 flex justify-end gap-2">
        <button className="btn" onClick={close}>
          Cancelar
        </button>
        <button className="btn btn-primary" onClick={() => void go()}>
          Escolher arquivo…
        </button>
      </div>
    </Modal>
  );
}

export function ExportDialog() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const anim = currentAnimation(s);
  const [kind, setKind] = useState("spritesheet");
  const [layout, setLayout] = useState("rows");
  const [scale, setScale] = useState(1);
  const [spacing, setSpacing] = useState(0);
  const [trim, setTrim] = useState(false);
  const [onlyAnim, setOnlyAnim] = useState(!!anim);
  const close = () => s.set({ dialog: null });
  if (!sprite) return null;
  const base = sprite.name.replace(/[^\w-]+/g, "_");
  const selection = onlyAnim && anim ? { animations: [anim.id] } : {};
  const go = async () => {
    const exts: Record<string, string> = {
      png: "png",
      spritesheet: "png",
      gif: "gif",
      godot: "tres",
      frames: "",
      rpgmaker_character: "png",
      rpgmaker_faces: "png",
      rpgmaker_battler: "png",
      tileset: "tres",
      godot_scene: "tscn",
      bmfont: "fnt",
      nineslice: "tscn",
      palette_variants: "",
      normal_map: "png",
    };
    // Formatos que gravam vários arquivos pedem uma pasta.
    const dirKinds: string[] = [
      "frames",
      "palette_variants",
    ];
    const ext = exts[kind] ?? "png";
    let path: string | null;
    if (dirKinds.includes(kind)) {
      const dir = await open({ directory: true });
      path = typeof dir === "string" ? dir : null;
    } else {
      path = await save({ defaultPath: `${base}${kind === "spritesheet" ? "_sheet" : ""}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    }
    if (!path) return;
    const t = { sprite: sprite.id };
    const sheet = { layout, spacing, trim };
    const calls: Record<string, [string, Record<string, unknown>]> = {
      png: ["export_png", { ...t, path, frame: s.frameId, scale }],
      spritesheet: ["export_spritesheet", { ...t, path, scale, sheet, selection, json: true }],
      gif: ["export_gif", { ...t, path, scale, animation: anim?.id, direction: s.direction ?? undefined }],
      godot: ["export_godot_metadata", { ...t, path, scale, sheet, selection }],
      frames: ["export_frames", { ...t, directory: path, scale, selection }],
      ...rpgMakerExportCalls({ ...t, path, scale, frameId: s.frameId }),
      tileset: ["export_godot_tileset", { ...t, path, frame: s.frameId, scale }],
      godot_scene: ["export_godot_scene", { ...t, path, scale, sheet, selection }],
      // With "Exportar sempre que salvar" on, the same command and arguments become an export job.
      ...rememberExportJob(() => calls[kind]),
      bmfont: ["export_bitmap_font", { ...t, font: sprite.id, path, scale }],
      nineslice: ["export_nine_slice", { ...t, path, scale, frame: s.frameId, ...nineSliceAxes() }],
      palette_variants: ["export_palette_variants", { ...t, directory: path, scale, ...variantExportArgs({ frame: s.frameId, animation: anim?.id, direction: s.direction ?? undefined, selection }) }],
      normal_map: ["export_normal_map", { ...t, path, scale, ...normalMapArgs({ frame: s.frameId, selection }) }],
    };
    const [name, args] = calls[kind];
    await s.run(name, args).then(() => (s.toast("Exportado"), close())).catch(() => {});
  };
  return (
    <Modal title="Exportar" onClose={close}>
      <Row label="Formato">
        <select value={kind} onChange={(e) => setKind(e.target.value)} className="w-56">
          <option value="png">PNG do frame atual</option>
          <option value="spritesheet">Sprite sheet PNG + JSON</option>
          <option value="godot">Godot 4 (SpriteFrames .tres + PNG)</option>
          <option value="gif" disabled={!anim}>
            GIF da animação {anim ? `"${anim.name}"` : "(escolha uma animação)"}
          </option>
          <option value="frames">Sequência de PNGs</option>
          <RpgMakerExportOptions />
          <option value="tileset">Godot 4: TileSet (.tres + PNG)</option>
          <option value="godot_scene">Godot 4: cena (.tscn com AnimationPlayer, hitboxes e pontos)</option>
          <option value="bmfont" disabled={!sprite.extras?.texto?.font}>Fonte bitmap (.fnt + PNG){sprite.extras?.texto?.font ? "" : " — defina a fonte na aba Texto"}</option>
          <option value="nineslice" disabled={!sprite.extras?.texto?.nine_slice}>9-slice (Godot NinePatchRect){sprite.extras?.texto?.nine_slice ? "" : " — defina as margens na aba Texto"}</option>
          <option value="palette_variants" disabled={!s.doc?.model.extras?.paletas?.variants?.length}>Variantes de paleta (uma sheet/PNG/GIF por variante)</option>
          <option value="normal_map">Normal map (Godot)</option>
        </select>
      </Row>
      {(kind === "spritesheet" || kind === "godot") && (
        <>
          <Row label="Layout">
            <select value={layout} onChange={(e) => setLayout(e.target.value)} className="w-56">
              <option value="rows">Uma linha por animação/direção</option>
              <option value="horizontal">Horizontal</option>
              <option value="vertical">Vertical</option>
              <option value="grid">Grade</option>
              <option value="packed">Atlas compactado</option>
            </select>
          </Row>
          <Row label="Espaçamento (px)">
            <input type="number" min={0} max={32} value={spacing} onChange={(e) => setSpacing(Number(e.target.value))} className="w-20" />
          </Row>
          <Row label="Recortar bordas (alinhado pelo pivô)">
            <input type="checkbox" checked={trim} onChange={(e) => setTrim(e.target.checked)} />
          </Row>
        </>
      )}
      {anim && ["spritesheet", "godot", "frames", "godot_scene", "palette_variants", "normal_map"].includes(kind) && (
        <Row label={`Só a animação "${anim.name}"`}>
          <input type="checkbox" checked={onlyAnim} onChange={(e) => setOnlyAnim(e.target.checked)} />
        </Row>
      )}
      {isRpgMakerExport(kind) && <ExportRpgMakerPanel kind={kind} />}
      {kind === "tileset" && <TilesetExportNote />}
      <ExportOnSaveOption kind={kind} />
      {kind === "nineslice" && <NineSliceExportRows />}
      {kind === "palette_variants" && <VariantExportOptions hasAnimation={!!anim} />}
      {kind === "normal_map" && <NormalMapExportOptions />}
      <Row label="Escala">
        <select value={scale} onChange={(e) => setScale(Number(e.target.value))} className="w-20">
          {[1, 2, 3, 4, 6, 8].map((v) => (
            <option key={v} value={v}>
              {v}x
            </option>
          ))}
        </select>
      </Row>
      <div className="mt-4 flex justify-end gap-2">
        <button className="btn" onClick={close}>
          Cancelar
        </button>
        <button className="btn btn-primary" onClick={() => void go()}>
          Exportar…
        </button>
      </div>
    </Modal>
  );
}
