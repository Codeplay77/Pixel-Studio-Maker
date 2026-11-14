import { ChevronLeft, ChevronRight, Copy, FlipHorizontal2, Ghost, Pause, Play, Plus, Repeat, SkipBack, SkipForward, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useFrameCanvas } from "../core/hooks";
import { currentAnimation, currentSprite, timelineFrames, useEditor } from "../core/store";
import type { LoopMode } from "../core/types";

function Thumb({ frame, active, index, duration, onClick }: { frame: number; active: boolean; index: number; duration: number; onClick: () => void }) {
  const s = useEditor();
  const canvas = useFrameCanvas(s.spriteId, frame);
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    const g = c.getContext("2d")!;
    g.clearRect(0, 0, c.width, c.height);
    if (!canvas) return;
    g.imageSmoothingEnabled = false;
    const k = Math.min(c.width / canvas.width, c.height / canvas.height);
    const w = canvas.width * k;
    const h = canvas.height * k;
    g.drawImage(canvas, (c.width - w) / 2, (c.height - h) / 2, w, h);
  }, [canvas]);
  return (
    <button onClick={onClick} className={`group flex shrink-0 flex-col items-center gap-1 rounded-md p-1 ${active ? "bg-raised ring-2 ring-accent" : "hover:bg-panel"}`} aria-pressed={active} aria-label={`Frame ${index}, ${duration} milissegundos`} title={`Frame ${index}`}>
      <canvas ref={ref} width={64} height={64} className="checker h-16 w-16 rounded" />
      <span className="flex w-full justify-between px-0.5 font-mono text-[10px] text-faint">
        <span className={active ? "text-ink" : ""}>{index}</span>
        <span>{duration}ms</span>
      </span>
    </button>
  );
}

function playbackOrder(frames: number[], mode: LoopMode): number[] {
  if (mode === "reverse") return [...frames].reverse();
  if (mode === "ping_pong" && frames.length > 2) return [...frames, ...frames.slice(1, -1).reverse()];
  return frames;
}

export function Timeline() {
  const s = useEditor();
  const sprite = currentSprite(s);
  const anim = currentAnimation(s);
  const frames = timelineFrames(s);
  const [newAnim, setNewAnim] = useState<{ name: string; frames: number; directions: string } | null>(null);
  const timer = useRef<number | undefined>(undefined);

  // Playback: follows each frame's duration and the loop mode (display only).
  useEffect(() => {
    if (!s.playing || !sprite || frames.length === 0) return;
    const order = playbackOrder(frames, anim?.loop_mode ?? "loop");
    let i = Math.max(0, order.indexOf(s.frameId ?? order[0]));
    const tick = () => {
      const st = useEditor.getState();
      const fr = currentSprite(st)?.frames.find((f) => f.id === order[i]);
      st.set({ frameId: order[i] });
      timer.current = window.setTimeout(() => {
        i = i + 1;
        if (i >= order.length) {
          if (anim?.loop_mode === "once") {
            st.set({ playing: false });
            return;
          }
          i = 0;
        }
        tick();
      }, fr?.duration_ms ?? 100);
    };
    tick();
    return () => window.clearTimeout(timer.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.playing, anim?.id, anim?.loop_mode, frames.join(","), sprite?.id]);

  if (!sprite) return <footer className="h-[150px] shrink-0 border-t border-line bg-shell" />;

  const pos = s.frameId !== undefined ? frames.indexOf(s.frameId) : -1;
  const t = { sprite: sprite.id };
  const trackArgs = anim ? { animation: anim.id, direction: s.direction ?? undefined } : {};
  const go = (d: number) => frames.length && s.set({ frameId: frames[(Math.max(0, pos) + d + frames.length) % frames.length] });

  return (
    <footer className="timeline" aria-label="Linha do tempo">
      <div className="timeline-controls">
        <select aria-label="Animação" className="h-8 max-w-44" value={anim?.id ?? ""} onChange={(e) => s.set({ animationId: e.target.value ? Number(e.target.value) : undefined, direction: e.target.value ? (sprite.animations.find((a) => a.id === Number(e.target.value))?.tracks[0]?.direction ?? null) : undefined, playing: false })}>
          <option value="">Todos os frames ({sprite.frames.length})</option>
          {sprite.animations.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
        {anim && (
          <div className="flex gap-0.5">
            {anim.tracks.map((tr) => (
              <button key={tr.direction ?? "-"} className={`chip h-7 min-w-8 ${(tr.direction ?? null) === (s.direction ?? null) ? "bg-accent-2/20 text-accent-2" : "text-muted hover:bg-hover"}`} onClick={() => s.set({ direction: tr.direction ?? null, frameId: tr.frames[0], playing: false })} title={sprite.directions.find((d) => d.name === tr.direction)?.label ?? "sem direção"}>
                {tr.direction ?? "—"}
              </button>
            ))}
          </div>
        )}
        <button className="btn h-7 text-[12px]" onClick={() => setNewAnim({ name: "", frames: 4, directions: "" })} title="Nova animação">
          <Plus size={14} /> Animação
        </button>
        {anim && (
          <select className="h-7" value={anim.loop_mode} title="Repetição" onChange={(e) => void s.run("animation_set_timing", { ...t, animation: anim.id, loop_mode: e.target.value })}>
            <option value="loop">loop</option>
            <option value="once">uma vez</option>
            <option value="ping_pong">ping-pong</option>
            <option value="reverse">reverso</option>
          </select>
        )}
        <div className="mx-1 h-5 w-px bg-line" />
        <button className="btn-icon h-7 w-7" title="Primeiro" onClick={() => frames.length && s.set({ frameId: frames[0] })}>
          <SkipBack size={15} />
        </button>
        <button className="btn-icon h-7 w-7" title="Anterior (,)" onClick={() => go(-1)}>
          <ChevronLeft size={16} />
        </button>
        <button className={`btn-icon h-7 w-7 ${s.playing ? "text-accent" : ""}`} aria-pressed={s.playing} title={s.playing ? "Pausar (Enter)" : "Reproduzir (Enter)"} onClick={() => s.set({ playing: !s.playing })}>
          {s.playing ? <Pause size={16} /> : <Play size={16} />}
        </button>
        <button className="btn-icon h-7 w-7" title="Próximo (.)" onClick={() => go(1)}>
          <ChevronRight size={16} />
        </button>
        <button className="btn-icon h-7 w-7" title="Último" onClick={() => frames.length && s.set({ frameId: frames[frames.length - 1] })}>
          <SkipForward size={15} />
        </button>
        <div className="mx-1 h-5 w-px bg-line" />
        <button className={`btn h-7 text-[12px] ${s.onion.enabled ? "text-accent-2" : ""}`} title="Onion skin: frames vizinhos como fantasma" onClick={() => s.set({ onion: { ...s.onion, enabled: !s.onion.enabled } })}>
          <Ghost size={15} /> Fantasma
        </button>
        {s.onion.enabled && (
          <span className="flex items-center gap-1 text-[11px] text-muted">
            antes
            <input type="number" min={0} max={2} className="h-6 w-10" value={s.onion.before} onChange={(e) => s.set({ onion: { ...s.onion, before: Number(e.target.value) } })} />
            depois
            <input type="number" min={0} max={1} className="h-6 w-10" value={s.onion.after} onChange={(e) => s.set({ onion: { ...s.onion, after: Number(e.target.value) } })} />
          </span>
        )}
        <div className="ml-auto flex items-center gap-0.5">
          <button className="btn-icon h-7 w-7" title="Novo frame vazio" onClick={() => void s.run("frame_create", { ...t, ...trackArgs, index: pos + 1 }).then((r) => s.set({ frameId: r.data.frames[0] }))}>
            <Plus size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Duplicar frame" disabled={s.frameId === undefined} onClick={() => void s.run("frame_duplicate", { ...t, frame: s.frameId }).then((r) => s.set({ frameId: r.data.frames[0] }))}>
            <Copy size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Duplicar espelhado" disabled={s.frameId === undefined} onClick={() => void s.run("frame_duplicate", { ...t, frame: s.frameId, mirror: true }).then((r) => s.set({ frameId: r.data.frames[0] }))}>
            <FlipHorizontal2 size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Mover para a esquerda" disabled={pos <= 0} onClick={() => void s.run("frame_move", { ...t, ...trackArgs, frame: s.frameId, index: pos - 1 })}>
            <ChevronLeft size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Mover para a direita" disabled={pos < 0 || pos >= frames.length - 1} onClick={() => void s.run("frame_move", { ...t, ...trackArgs, frame: s.frameId, index: pos + 1 })}>
            <ChevronRight size={15} />
          </button>
          <button className="btn-icon h-7 w-7" title="Apagar frame" disabled={sprite.frames.length <= 1 || s.frameId === undefined} onClick={() => void s.run("frame_delete", { ...t, frames: [s.frameId] })}>
            <Trash2 size={15} />
          </button>
        </div>
      </div>
      <div className="flex min-h-0 flex-1 items-center gap-3 overflow-x-auto px-4 py-3">
        {frames.map((f, i) => (
          <Thumb key={`${f}-${i}`} frame={f} index={i + 1} active={f === s.frameId} duration={sprite.frames.find((x) => x.id === f)?.duration_ms ?? 0} onClick={() => s.set({ frameId: f, playing: false })} />
        ))}
        {frames.length === 0 && <span className="text-faint">Esta trilha não tem frames.</span>}
        {anim && <Repeat size={14} className="ml-2 shrink-0 text-faint" />}
      </div>
      {newAnim && (
        <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/50" onClick={() => setNewAnim(null)}>
          <div className="w-80 rounded-xl bg-panel p-4 shadow-2xl ring-1 ring-line" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-3 font-semibold">Nova animação</h3>
            <div className="flex flex-col gap-2">
              <input autoFocus placeholder="nome (idle, walk, attack...)" value={newAnim.name} onChange={(e) => setNewAnim({ ...newAnim, name: e.target.value })} />
              <label className="flex items-center gap-2 text-muted">
                Frames por direção
                <input type="number" min={1} max={64} className="w-16" value={newAnim.frames} onChange={(e) => setNewAnim({ ...newAnim, frames: Number(e.target.value) })} />
              </label>
              <input placeholder="direções: vazio, E, ou all" value={newAnim.directions} onChange={(e) => setNewAnim({ ...newAnim, directions: e.target.value })} />
              <p className="text-[11px] text-faint">Os frames novos começam como cópia do frame atual, para você redesenhar cada pose.</p>
              <div className="mt-1 flex justify-end gap-2">
                <button className="btn" onClick={() => setNewAnim(null)}>
                  Cancelar
                </button>
                <button
                  className="btn btn-primary"
                  disabled={!newAnim.name}
                  onClick={() =>
                    void s
                      .run("animation_create", { ...t, name: newAnim.name, frames: newAnim.frames, copy_from: s.frameId, directions: newAnim.directions.split(/[\s,]+/).filter(Boolean) })
                      .then((r) => {
                        s.set({ animationId: r.data.animation, direction: r.data.tracks[0]?.direction ?? null, frameId: r.data.tracks[0]?.frames[0] });
                        setNewAnim(null);
                      })
                  }
                >
                  Criar
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </footer>
  );
}
