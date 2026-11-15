// Opções das exportações para o RPG Maker (folha de personagem, faces, battler lateral). Quem monta
// as folhas são os comandos export_rpgmaker_* (Rust, os mesmos do MCP); aqui só ficam as escolhas
// do usuário e os argumentos de cada comando.
import { Plus, Trash2 } from "lucide-react";
import { useEffect } from "react";
import { create } from "zustand";
import { currentAnimation, currentSprite, timelineFrames, useEditor } from "../../core/store";
import type { Animation, Sprite } from "../../core/types";
import { RPG_ENGINES, SV_MOTIONS, tileOf, type RpgEngine } from "./templates";

export type RpgExportKind = "rpgmaker_character" | "rpgmaker_faces" | "rpgmaker_battler";

export const isRpgMakerExport = (kind: string): kind is RpgExportKind =>
  kind === "rpgmaker_character" || kind === "rpgmaker_faces" || kind === "rpgmaker_battler";

type CharacterRow = { sprite: number; animation?: number };

type Options = {
  engine: RpgEngine;
  characters: CharacterRow[];
  single: boolean;
  object: boolean;
  mirror: boolean;
  frames: string;
  cellW: string;
  cellH: string;
  faceSource: "frame" | "track";
  trackFrames: number[];
  motions: Record<string, number | undefined>;
  direction: string;
};

type OptionsState = Options & { set: (p: Partial<Options>) => void };

const useRpgExport = create<OptionsState>((set) => ({
  engine: "mz",
  characters: [],
  single: false,
  object: false,
  mirror: false,
  frames: "",
  cellW: "",
  cellH: "",
  faceSource: "frame",
  trackFrames: [],
  motions: {},
  direction: "",
  set: (p) => set(p),
}));

const fold = (t: string) =>
  t
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "");

const CHARACTER_DIRS = ["S", "W", "E", "N"];

/** Animação que parece uma caminhada do RPG Maker: tem trilhas S/W/E/N (de preferência "walk"). */
function guessWalk(sprite: Sprite): Animation | undefined {
  const withDirs = sprite.animations.filter((a) => a.tracks.some((t) => t.direction && CHARACTER_DIRS.includes(t.direction)));
  return withDirs.find((a) => ["walk", "andar", "caminhar"].includes(fold(a.name))) ?? withDirs[0] ?? sprite.animations[0];
}

function parseFrames(text: string): number[] | undefined {
  const parts = text.split(/[\s,;]+/).filter(Boolean);
  return parts.length ? parts.map(Number) : undefined;
}

function parseCell(w: string, h: string): [number, number] | undefined {
  return w.trim() && h.trim() ? [Number(w), Number(h)] : undefined;
}

/** Argumentos dos três comandos, montados com as opções do painel. */
export function rpgMakerExportCalls(base: { sprite: number; path: string; scale: number; frameId?: number }): Record<RpgExportKind, [string, Record<string, unknown>]> {
  const o = useRpgExport.getState();
  const frames = parseFrames(o.frames);
  const cell = parseCell(o.cellW, o.cellH);
  const faces = (o.faceSource === "frame" ? [base.frameId] : o.trackFrames).filter((f): f is number => f !== undefined).slice(0, 8);
  const motions = Object.fromEntries(Object.entries(o.motions).filter((e): e is [string, number] => e[1] !== undefined));
  return {
    rpgmaker_character: [
      "export_rpgmaker_character",
      {
        sprite: base.sprite,
        path: base.path,
        scale: base.scale,
        engine: o.engine,
        cell,
        single: o.single,
        object: o.object,
        mirror_missing: o.mirror,
        characters: o.characters.filter((c) => c.animation !== undefined).map((c) => ({ sprite: c.sprite, animation: c.animation, frames })),
      },
    ],
    rpgmaker_faces: ["export_rpgmaker_faces", { sprite: base.sprite, path: base.path, scale: base.scale, engine: o.engine, faces: faces.map((frame) => ({ frame })) }],
    rpgmaker_battler: [
      "export_rpgmaker_battler",
      { sprite: base.sprite, path: base.path, scale: base.scale, engine: o.engine, cell, frames, direction: o.direction || undefined, motions },
    ],
  };
}

/** As três opções do seletor de formato do diálogo de exportação. */
export function RpgMakerExportOptions() {
  return (
    <>
      <option value="rpgmaker_character">RPG Maker: personagem</option>
      <option value="rpgmaker_faces">RPG Maker: faces</option>
      <option value="rpgmaker_battler">RPG Maker: battler lateral</option>
    </>
  );
}

function Row({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <label className="flex items-center justify-between gap-3 py-1 text-muted" title={hint}>
      <span>{label}</span>
      {children}
    </label>
  );
}

function CellInputs({ o, placeholder }: { o: OptionsState; placeholder: number }) {
  return (
    <span className="flex items-center gap-1">
      <input type="number" min={1} max={512} placeholder={String(placeholder)} value={o.cellW} onChange={(e) => o.set({ cellW: e.target.value })} className="w-16" />×
      <input type="number" min={1} max={512} placeholder={String(placeholder)} value={o.cellH} onChange={(e) => o.set({ cellH: e.target.value })} className="w-16" />
    </span>
  );
}

export function ExportRpgMakerPanel({ kind }: { kind: RpgExportKind }) {
  const s = useEditor();
  const o = useRpgExport();
  const sprite = currentSprite(s);
  const anim = currentAnimation(s);
  const sprites = s.doc?.model.sprites ?? [];

  // Ao abrir: motor do modelo do sprite, a animação atual (ou a que parece uma caminhada), os
  // movimentos do battler pelos nomes das animações.
  useEffect(() => {
    if (!sprite) return;
    const engine = sprite.extras?.rpgmaker?.engine ?? "mz";
    const walk = anim ?? guessWalk(sprite);
    const tile = tileOf(engine);
    const motions: Record<string, number | undefined> = {};
    for (const [key, pt] of SV_MOTIONS) motions[key] = sprite.animations.find((a) => [key, fold(pt)].includes(fold(a.name)))?.id;
    if (Object.values(motions).every((v) => v === undefined) && anim) motions.wait = anim.id;
    o.set({
      engine,
      characters: [{ sprite: sprite.id, animation: walk?.id }],
      single: sprite.width > tile || sprite.height > tile,
      trackFrames: anim ? timelineFrames(s).slice(0, 8) : [],
      faceSource: "frame",
      motions,
      direction: "",
    });
    // Only when the dialog opens or the sprite changes.
  }, [sprite?.id]);

  if (!sprite) return null;
  const tile = tileOf(o.engine);
  const engines = kind === "rpgmaker_battler" ? RPG_ENGINES.filter((e) => e.id !== "vxace") : RPG_ENGINES;
  const engineRow = (
    <Row label="Motor">
      <select value={o.engine} onChange={(e) => o.set({ engine: e.target.value as RpgEngine })} className="w-56">
        {engines.map((e) => (
          <option key={e.id} value={e.id}>
            {e.label}
          </option>
        ))}
      </select>
    </Row>
  );

  if (kind === "rpgmaker_faces") {
    return (
      <>
        {engineRow}
        <Row label="Faces">
          <select value={o.faceSource} onChange={(e) => o.set({ faceSource: e.target.value as Options["faceSource"] })} className="w-56">
            <option value="frame">Só o frame atual</option>
            <option value="track" disabled={!o.trackFrames.length}>
              Frames da trilha atual ({Math.min(8, o.trackFrames.length)})
            </option>
          </select>
        </Row>
        <p className="mt-1 text-[12px] leading-5 text-faint">
          Grade 4×2 de células {3 * tile}×{3 * tile} px, cada face centralizada. A escala amplia as faces, que precisam caber na célula.
        </p>
      </>
    );
  }

  if (kind === "rpgmaker_battler") {
    const directions = [...new Set(sprite.animations.flatMap((a) => a.tracks.map((t) => t.direction).filter((d): d is string => !!d)))];
    return (
      <>
        {engineRow}
        <Row label="Direção das trilhas">
          <select value={o.direction} onChange={(e) => o.set({ direction: e.target.value })} className="w-40">
            <option value="">Primeira trilha</option>
            {directions.map((d) => (
              <option key={d} value={d}>
                {d}
              </option>
            ))}
          </select>
        </Row>
        <Row label="Frames (3 índices)" hint="Índices na trilha para os 3 quadros; vazio = 0, 1, 2">
          <input placeholder="0,1,2" value={o.frames} onChange={(e) => o.set({ frames: e.target.value })} className="w-24" />
        </Row>
        <Row label="Célula (vazio = 64×64)">
          <CellInputs o={o} placeholder={64} />
        </Row>
        <div className="mt-2 grid max-h-56 grid-cols-2 gap-x-3 gap-y-1 overflow-y-auto pr-1">
          {SV_MOTIONS.map(([key, pt]) => (
            <label key={key} className="flex items-center justify-between gap-2 text-[12px] text-muted" title={key}>
              <span className="truncate">{pt}</span>
              <select
                value={o.motions[key] ?? ""}
                onChange={(e) => o.set({ motions: { ...o.motions, [key]: e.target.value === "" ? undefined : Number(e.target.value) } })}
                className="w-28 py-1"
              >
                <option value="">—</option>
                {sprite.animations.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.name}
                  </option>
                ))}
              </select>
            </label>
          ))}
        </div>
        <p className="mt-2 text-[12px] leading-5 text-faint">9×6 células; cada movimento ocupa 3 células. Movimentos sem animação ficam vazios.</p>
      </>
    );
  }

  // Folha de personagem.
  const rows = o.characters;
  const setRow = (i: number, p: Partial<CharacterRow>) => o.set({ characters: rows.map((r, k) => (k === i ? { ...r, ...p } : r)) });
  const animsOf = (id: number) => sprites.find((x) => x.id === id)?.animations ?? [];
  return (
    <>
      {engineRow}
      <div className="py-1 text-muted">Personagens</div>
      {rows.map((r, i) => (
        <div key={i} className="flex items-center gap-1 py-0.5">
          <span className="w-4 text-[11px] text-faint">{i}</span>
          <select
            value={r.sprite}
            onChange={(e) => {
              const id = Number(e.target.value);
              const sp = sprites.find((x) => x.id === id);
              setRow(i, { sprite: id, animation: sp ? guessWalk(sp)?.id : undefined });
            }}
            className="min-w-0 flex-1"
          >
            {sprites.map((x) => (
              <option key={x.id} value={x.id}>
                {x.name}
              </option>
            ))}
          </select>
          <select value={r.animation ?? ""} onChange={(e) => setRow(i, { animation: e.target.value === "" ? undefined : Number(e.target.value) })} className="min-w-0 flex-1">
            <option value="">(sem animação)</option>
            {animsOf(r.sprite).map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          <button className="btn-icon h-8 w-8" aria-label="Remover personagem" disabled={rows.length <= 1} onClick={() => o.set({ characters: rows.filter((_, k) => k !== i) })}>
            <Trash2 size={14} />
          </button>
        </div>
      ))}
      <div className="flex justify-end">
        <button className="btn h-8" disabled={o.single || rows.length >= 8} onClick={() => o.set({ characters: [...rows, { sprite: sprite.id, animation: guessWalk(sprite)?.id }] })}>
          <Plus size={14} /> Personagem
        </button>
      </div>
      <Row label="Um personagem só (prefixo $)" hint="Folha 3×4 de um personagem; a célula pode ter qualquer tamanho">
        <input type="checkbox" checked={o.single} disabled={rows.length > 1} onChange={(e) => o.set({ single: e.target.checked })} />
      </Row>
      <Row label="Objeto (prefixo !)" hint="Portas, baús, tochas: o RPG Maker não desloca o sprite para cima">
        <input type="checkbox" checked={o.object} onChange={(e) => o.set({ object: e.target.checked })} />
      </Row>
      <Row label="Espelhar W/E que faltar">
        <input type="checkbox" checked={o.mirror} onChange={(e) => o.set({ mirror: e.target.checked })} />
      </Row>
      <Row label="Frames (3 índices)" hint="Índices na trilha de cada direção para as colunas 0, 1 e 2; vazio = 0, 1, 2">
        <input placeholder="0,1,2" value={o.frames} onChange={(e) => o.set({ frames: e.target.value })} className="w-24" />
      </Row>
      <Row label={`Célula (vazio = ${o.single ? "automática" : `${tile}×${tile}`})`}>
        <CellInputs o={o} placeholder={tile} />
      </Row>
      <p className="mt-1 text-[12px] leading-5 text-faint">
        Linhas: baixo (S), esquerda (W), direita (E), cima (N). A coluna do meio é a pose parada. Cada frame fica com o pivô no centro da base da célula.
      </p>
    </>
  );
}
