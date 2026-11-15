import { Maximize, Minus, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../core/api";
import { useFrameCanvas, useReferenceCanvases } from "../core/hooks";
import { currentSprite, timelineFrames, useEditor } from "../core/store";
import type { Point, Rect, RegionShape } from "../core/types";
import { drawTileOverlay, drawTileRepeat } from "./tiles/canvasTiles";
import { commitHitboxDrag, drawHitboxes } from "./godot/hitboxCanvas";
import { extendStroke, finishStroke, startStroke, type StrokeDrag } from "./tools/brush";
import { drawToolsOverlay } from "./tools/overlay";
import { lassoFinish, marqueeCombine, moveSelection, selectMode, wandAt } from "./tools/selection";
import type { SelectMode } from "./tools/types";
import { drawTextOverlays, startTextDraft } from "./texto/canvas";

type Drag =
  | { kind: "pan"; startX: number; startY: number; panX: number; panY: number }
  | StrokeDrag
  | { kind: "shape"; tool: "line" | "rect" | "ellipse"; from: Point; to: Point; color: string }
  | { kind: "select"; from: Point; to: Point; mode?: SelectMode }
  | { kind: "move"; from: Point; to: Point }
  | { kind: "lasso"; points: Point[]; mode: SelectMode }
  | { kind: "hitbox"; from: Point; to: Point };

const OVERLAY = ["#ff4081", "#00e5ff", "#ffd600", "#76ff03", "#d500f9", "#ff6d00", "#2979ff", "#1de9b6"];

function normRect(a: Point, b: Point): Rect {
  const x = Math.min(a[0], b[0]);
  const y = Math.min(a[1], b[1]);
  return { x, y, w: Math.abs(a[0] - b[0]) + 1, h: Math.abs(a[1] - b[1]) + 1 };
}

function tinted(src: HTMLCanvasElement, color: string): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = src.width;
  c.height = src.height;
  const g = c.getContext("2d")!;
  g.drawImage(src, 0, 0);
  g.globalCompositeOperation = "source-atop";
  g.fillStyle = color;
  g.globalAlpha = 0.55;
  g.fillRect(0, 0, c.width, c.height);
  return c;
}

function shapePath(g: CanvasRenderingContext2D, shape: RegionShape, ox: number, oy: number, z: number, color: string) {
  g.strokeStyle = color;
  g.fillStyle = color;
  g.lineWidth = 2;
  if (shape.type === "rect") {
    g.strokeRect(ox + shape.rect.x * z + 1, oy + shape.rect.y * z + 1, shape.rect.w * z - 2, shape.rect.h * z - 2);
  } else if (shape.type === "mask") {
    g.globalAlpha = 0.22;
    shape.rows.forEach((row, dy) => {
      [...row].forEach((ch, dx) => {
        if (ch !== "." && ch !== " ") g.fillRect(ox + (shape.at[0] + dx) * z, oy + (shape.at[1] + dy) * z, z, z);
      });
    });
    g.globalAlpha = 1;
  } else {
    g.beginPath();
    shape.points.forEach(([x, y], i) => (i ? g.lineTo(ox + (x + 0.5) * z, oy + (y + 0.5) * z) : g.moveTo(ox + (x + 0.5) * z, oy + (y + 0.5) * z)));
    g.closePath();
    g.stroke();
  }
}

export function CanvasView() {
  const wrap = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 800, h: 600 });
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [hover, setHover] = useState<Point>();
  const [drag, setDrag] = useState<Drag>();
  const [space, setSpace] = useState(false);
  const queue = useRef<Promise<unknown>>(Promise.resolve());

  const s = useEditor();
  const sprite = currentSprite(s);
  const frameCanvas = useFrameCanvas(sprite?.id, s.frameId);
  const frames = timelineFrames(s);
  const pos = s.frameId !== undefined ? frames.indexOf(s.frameId) : -1;
  const prevId = s.onion.enabled && pos > 0 ? frames[pos - 1] : undefined;
  const prev2Id = s.onion.enabled && s.onion.before > 1 && pos > 1 ? frames[pos - 2] : undefined;
  const nextId = s.onion.enabled && s.onion.after > 0 && pos >= 0 && pos < frames.length - 1 ? frames[pos + 1] : undefined;
  const prevCanvas = useFrameCanvas(sprite?.id, prevId);
  const prev2Canvas = useFrameCanvas(sprite?.id, prev2Id);
  const nextCanvas = useFrameCanvas(sprite?.id, nextId);
  const refCanvases = useReferenceCanvases();

  // Fit the sprite on first load / sprite change.
  const spriteKey = sprite ? `${s.projectId}:${sprite.id}:${sprite.width}x${sprite.height}` : "";
  useEffect(() => {
    if (!sprite) return;
    const z = Math.max(1, Math.min(32, Math.floor(Math.min((size.w - 80) / sprite.width, (size.h - 80) / sprite.height))));
    s.set({ zoom: z });
    setPan({ x: 0, y: 0 });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [spriteKey]);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const down = (e: KeyboardEvent) => e.code === "Space" && (e.target as HTMLElement).tagName !== "INPUT" && setSpace(true);
    const up = (e: KeyboardEvent) => e.code === "Space" && setSpace(false);
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, []);

  const z = s.zoom;
  const ox = sprite ? Math.round(size.w / 2 - (sprite.width * z) / 2 + pan.x) : 0;
  const oy = sprite ? Math.round(size.h / 2 - (sprite.height * z) / 2 + pan.y) : 0;

  // ---- drawing ------------------------------------------------------------------------------
  useEffect(() => {
    const c = canvasRef.current;
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(size.w * dpr);
    c.height = Math.round(size.h * dpr);
    const g = c.getContext("2d")!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.imageSmoothingEnabled = false;
    g.fillStyle = "#111315";
    g.fillRect(0, 0, size.w, size.h);
    if (!sprite) return;
    const W = sprite.width * z;
    const H = sprite.height * z;
    // Checkerboard.
    const cell = Math.max(4, Math.min(16, z * 2));
    g.save();
    g.beginPath();
    g.rect(ox, oy, W, H);
    g.clip();
    for (let y = 0; y < H; y += cell) {
      for (let x = 0; x < W; x += cell) {
        g.fillStyle = ((x / cell + y / cell) & 1) === 0 ? "#343a3d" : "#41484b";
        g.fillRect(ox + x, oy + y, cell, cell);
      }
    }
    g.restore();
    // References.
    if (s.showReferences) {
      for (const r of s.doc?.model.references ?? []) {
        const rc = refCanvases.find((x) => x.id === r.id);
        if (!rc || !r.visible || (r.sprite !== undefined && r.sprite !== sprite.id)) continue;
        g.globalAlpha = r.opacity / 255;
        g.drawImage(rc.canvas, ox + r.position[0] * z, oy + r.position[1] * z, rc.canvas.width * r.scale * z, rc.canvas.height * r.scale * z);
        g.globalAlpha = 1;
      }
    }
    // Onion skin.
    const ghost = (cv: HTMLCanvasElement | undefined, color: string, alpha: number) => {
      if (!cv) return;
      g.globalAlpha = alpha;
      g.drawImage(tinted(cv, color), ox, oy, W, H);
      g.globalAlpha = 1;
    };
    ghost(prev2Canvas, "#ff4d4d", 0.16);
    ghost(prevCanvas, "#ff4d4d", 0.32);
    ghost(nextCanvas, "#4d8dff", 0.32);
    if (s.tileMode && frameCanvas) drawTileRepeat(g, frameCanvas, ox, oy, W, H);
    if (frameCanvas) g.drawImage(frameCanvas, ox, oy, W, H);
    // Grid.
    if (s.grid && z >= 6) {
      g.strokeStyle = "rgba(255,255,255,0.10)";
      g.lineWidth = 1;
      g.beginPath();
      for (let x = 0; x <= sprite.width; x++) {
        g.moveTo(ox + x * z + 0.5, oy);
        g.lineTo(ox + x * z + 0.5, oy + H);
      }
      for (let y = 0; y <= sprite.height; y++) {
        g.moveTo(ox, oy + y * z + 0.5);
        g.lineTo(ox + W, oy + y * z + 0.5);
      }
      g.stroke();
      g.strokeStyle = "rgba(255,255,255,0.22)";
      g.beginPath();
      for (let x = 0; x <= sprite.width; x += 8) {
        g.moveTo(ox + x * z + 0.5, oy);
        g.lineTo(ox + x * z + 0.5, oy + H);
      }
      for (let y = 0; y <= sprite.height; y += 8) {
        g.moveTo(ox, oy + y * z + 0.5);
        g.lineTo(ox + W, oy + y * z + 0.5);
      }
      g.stroke();
    }
    g.strokeStyle = "rgba(0,0,0,0.6)";
    g.strokeRect(ox - 0.5, oy - 0.5, W + 1, H + 1);
    // Guides: vertical center and the ground line (animation cycle, else the sprite pivot).
    if (s.guides) {
      const anim = sprite.animations.find((a) => a.id === s.animationId);
      const ground = anim?.cycle?.ground_y ?? sprite.origin[1];
      g.setLineDash([6, 4]);
      g.lineWidth = 1;
      g.strokeStyle = "rgba(95,227,255,0.85)";
      g.beginPath();
      g.moveTo(ox + Math.floor(sprite.width / 2) * z + 0.5, oy);
      g.lineTo(ox + Math.floor(sprite.width / 2) * z + 0.5, oy + H);
      g.stroke();
      g.strokeStyle = "rgba(255,198,90,0.9)";
      g.beginPath();
      g.moveTo(ox, oy + (ground + 1) * z + 0.5);
      g.lineTo(ox + W, oy + (ground + 1) * z + 0.5);
      g.stroke();
      g.setLineDash([]);
    }
    // Regions and anchors.
    if (s.showRegions && s.frameId !== undefined) {
      sprite.regions.forEach((r, i) => {
        const shape = r.shapes?.[String(s.frameId)] ?? r.default;
        if (shape) shapePath(g, shape, ox, oy, z, r.color ?? OVERLAY[i % OVERLAY.length]);
      });
    }
    if (s.showAnchors && s.frameId !== undefined) {
      sprite.anchors.forEach((a, i) => {
        const p = a.positions?.[String(s.frameId)] ?? a.default;
        if (!p) return;
        const cx = ox + (p[0] + 0.5) * z;
        const cy = oy + (p[1] + 0.5) * z;
        const arm = Math.max(5, z);
        g.strokeStyle = "#000";
        g.lineWidth = 4;
        g.beginPath();
        g.moveTo(cx - arm, cy);
        g.lineTo(cx + arm, cy);
        g.moveTo(cx, cy - arm);
        g.lineTo(cx, cy + arm);
        g.stroke();
        g.strokeStyle = a.color ?? OVERLAY[(i + 3) % OVERLAY.length];
        g.lineWidth = 2;
        g.stroke();
        g.font = "11px Segoe UI";
        g.fillStyle = "#000";
        g.fillText(a.name, cx + arm + 3, cy - 3);
        g.fillStyle = "#fff";
        g.fillText(a.name, cx + arm + 2, cy - 4);
      });
      const [px, py] = sprite.origin;
      g.strokeStyle = "#5fe3ff";
      g.lineWidth = 1;
      g.strokeRect(ox + px * z + 0.5, oy + py * z + 0.5, z - 1, z - 1);
    }
    // Selection.
    const sel = drag?.kind === "select" ? normRect(drag.from, drag.to) : s.selection;
    if (sel && !(s.selectionMask && drag?.kind !== "select")) {
      const off = drag?.kind === "move" ? [drag.to[0] - drag.from[0], drag.to[1] - drag.from[1]] : [0, 0];
      g.setLineDash([5, 4]);
      g.lineWidth = 1.5;
      g.strokeStyle = "#ffffff";
      g.strokeRect(ox + (sel.x + off[0]) * z + 0.5, oy + (sel.y + off[1]) * z + 0.5, sel.w * z, sel.h * z);
      g.strokeStyle = "#000000";
      g.lineDashOffset = 4;
      g.strokeRect(ox + (sel.x + off[0]) * z + 0.5, oy + (sel.y + off[1]) * z + 0.5, sel.w * z, sel.h * z);
      g.setLineDash([]);
      g.lineDashOffset = 0;
    }
    drawTileOverlay(g, sprite, ox, oy, z, s.showTileGrid, s.panel === "tiles" ? s.tileSelected : undefined);
    drawHitboxes(g, sprite, s, ox, oy, z, drag?.kind === "hitbox" ? drag : undefined);
    drawTextOverlays(g, sprite, ox, oy, z, s);
    drawToolsOverlay(g, s, sprite, ox, oy, z, {
      move: drag?.kind === "move" ? [drag.to[0] - drag.from[0], drag.to[1] - drag.from[1]] : undefined,
      lasso: drag?.kind === "lasso" ? drag.points : undefined,
      hover: drag ? undefined : hover,
    });
    // Shape preview (only a guide; Rust rasterizes the real shape on release).
    if (drag?.kind === "shape") {
      g.strokeStyle = drag.color.startsWith("#") && drag.color.length >= 7 ? drag.color.slice(0, 7) : "#ffffff";
      g.lineWidth = Math.max(1, z * 0.6);
      g.globalAlpha = 0.85;
      const r = normRect(drag.from, drag.to);
      if (drag.tool === "line") {
        g.beginPath();
        g.moveTo(ox + (drag.from[0] + 0.5) * z, oy + (drag.from[1] + 0.5) * z);
        g.lineTo(ox + (drag.to[0] + 0.5) * z, oy + (drag.to[1] + 0.5) * z);
        g.stroke();
      } else if (drag.tool === "rect") {
        g.strokeRect(ox + r.x * z + z / 2, oy + r.y * z + z / 2, (r.w - 1) * z, (r.h - 1) * z);
      } else {
        g.beginPath();
        g.ellipse(ox + (r.x + r.w / 2) * z, oy + (r.y + r.h / 2) * z, Math.max(1, (r.w * z) / 2 - z / 2), Math.max(1, (r.h * z) / 2 - z / 2), 0, 0, Math.PI * 2);
        g.stroke();
      }
      g.globalAlpha = 1;
    }
    if (drag?.kind === "move" && frameCanvas && !s.selection) {
      g.globalAlpha = 0.5;
      g.drawImage(frameCanvas, ox + (drag.to[0] - drag.from[0]) * z, oy + (drag.to[1] - drag.from[1]) * z, W, H);
      g.globalAlpha = 1;
    }
    // Hover pixel.
    if (hover && !drag && ["pencil", "eraser", "fill", "eyedropper", "line", "rect", "ellipse", "anchor"].includes(s.tool)) {
      const b = s.tool === "pencil" || s.tool === "eraser" ? s.brush : 1;
      const lo = -Math.floor((b - 1) / 2);
      g.strokeStyle = "rgba(255,255,255,0.9)";
      g.lineWidth = 1;
      g.strokeRect(ox + (hover[0] + lo) * z + 0.5, oy + (hover[1] + lo) * z + 0.5, b * z - 1, b * z - 1);
    }
  }, [
    size, sprite, z, ox, oy, frameCanvas, prevCanvas, prev2Canvas, nextCanvas, refCanvases, s.grid, s.guides, s.animationId, s.showRegions, s.showAnchors, s.showReferences, s.selection, s.frameId, s.tool, s.brush, drag, hover, s.doc,
    s.tileMode, s.showTileGrid, s.tileSelected, s.panel,
    s.showHitboxes, s.hiddenHitboxes, s.hitboxTarget,
    s.textDraft, s.nineSliceGuides,
    s.selectionMask, s.symmetry,
  ]);

  // Real-size preview (1x and 2x) in the corner; follows playback.
  const previewRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = previewRef.current;
    if (!c || !sprite) return;
    // Sprites grandes (uma imagem convertida, por exemplo): só a cópia 1x, reduzida para caber no canto.
    const big = Math.max(sprite.width, sprite.height) > 128;
    const k = big ? 1 : Math.max(1, Math.min(2, Math.floor(128 / Math.max(sprite.width, sprite.height))));
    c.width = big ? sprite.width + 8 : sprite.width * (1 + k) + 12;
    c.height = big ? sprite.height + 8 : sprite.height * k + 8;
    const g = c.getContext("2d")!;
    g.imageSmoothingEnabled = false;
    g.clearRect(0, 0, c.width, c.height);
    if (!frameCanvas) return;
    g.drawImage(frameCanvas, 4, 4 + sprite.height * (k - 1), sprite.width, sprite.height);
    if (!big) g.drawImage(frameCanvas, sprite.width + 8, 4, sprite.width * k, sprite.height * k);
  }, [frameCanvas, sprite, s.showPreview]);

  // ---- input --------------------------------------------------------------------------------
  const toPixel = (e: { clientX: number; clientY: number }): Point => {
    const r = canvasRef.current!.getBoundingClientRect();
    return [Math.floor((e.clientX - r.left - ox) / z), Math.floor((e.clientY - r.top - oy) / z)];
  };

  const target = useCallback(() => ({ sprite: s.spriteId, layer: s.layerId, frame: s.frameId }), [s.spriteId, s.layerId, s.frameId]);

  const enqueue = (fn: () => Promise<unknown>) => {
    queue.current = queue.current.then(fn).catch((e) => s.toast(api.errorMessage(e), "error"));
    return queue.current;
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (!sprite) return;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    if (e.button === 1 || space) {
      setDrag({ kind: "pan", startX: e.clientX, startY: e.clientY, panX: pan.x, panY: pan.y });
      return;
    }
    const p = toPixel(e);
    const color = e.button === 2 ? s.secondary : s.primary;
    const pid = s.projectId;
    switch (s.tool) {
      case "pencil":
      case "eraser":
        setDrag(startStroke(s.tool, p, e.button, enqueue, target, pid));
        return;
      case "fill":
        void s.run("draw_fill", { ...target(), x: p[0], y: p[1], color, contiguous: !e.shiftKey });
        return;
      case "eyedropper":
        void s.run("pixel_get", { sprite: s.spriteId, frame: s.frameId, x: p[0], y: p[1], composite: true }).then((r) => {
          const c = r.data.color as string;
          if (!r.data.transparent) s.set(e.button === 2 ? { secondary: c } : { primary: c });
        });
        return;
      case "line":
      case "rect":
      case "ellipse":
        setDrag({ kind: "shape", tool: s.tool, from: p, to: p, color });
        return;
      case "select":
        setDrag({ kind: "select", from: p, to: p, mode: selectMode(e) });
        return;
      case "move":
        setDrag({ kind: "move", from: p, to: p });
        return;
      case "anchor": {
        const name = s.anchorTarget;
        if (name && s.frameId !== undefined) void s.run("anchor_update", { sprite: s.spriteId, anchor: name, positions: [{ frame: s.frameId, position: p }] });
        return;
      }
      case "dither":
      case "shade":
        setDrag(startStroke(s.tool, p, e.button, enqueue, target, pid));
        return;
      case "wand":
        wandAt(p, e, target());
        return;
      case "lasso":
        setDrag({ kind: "lasso", points: [p], mode: selectMode(e) });
        return;
      case "text":
        startTextDraft(p, e.clientX, e.clientY);
        return;
      case "hitbox":
        if (s.hitboxTarget && s.frameId !== undefined) setDrag({ kind: "hitbox", from: p, to: p });
        else s.toast("Escolha uma caixa no painel Hitboxes (ou crie uma)");
        return;
    }
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!sprite) return;
    const p = toPixel(e);
    setHover(p);
    if (!drag) return;
    if (drag.kind === "pan") {
      setPan({ x: drag.panX + e.clientX - drag.startX, y: drag.panY + e.clientY - drag.startY });
    } else if (drag.kind === "stroke") {
      extendStroke(drag, p, enqueue, target, s.projectId);
    } else if (drag.kind === "shape" || drag.kind === "select" || drag.kind === "move") {
      if (drag.to[0] !== p[0] || drag.to[1] !== p[1]) setDrag({ ...drag, to: p });
    }
    if (drag.kind === "lasso") {
      const last = drag.points[drag.points.length - 1];
      if (last[0] !== p[0] || last[1] !== p[1]) setDrag({ ...drag, points: [...drag.points, p] });
    }
    if (drag.kind === "hitbox" && (drag.to[0] !== p[0] || drag.to[1] !== p[1])) setDrag({ ...drag, to: p });
  };

  const onPointerUp = () => {
    const d = drag;
    setDrag(undefined);
    if (!d || !sprite) return;
    const pid = s.projectId;
    if (d.kind === "stroke") {
      // Replace the live preview segments with one pixel-perfect stroke (one undo step).
      finishStroke(d, enqueue, target, pid);
    } else if (d.kind === "shape") {
      const r = normRect(d.from, d.to);
      if (d.tool === "line") void s.run("draw_line", { ...target(), from: d.from, to: d.to, color: d.color, thickness: s.brush });
      else void s.run("draw_shape", { ...target(), shape: d.tool, rect: r, color: d.color, fill: s.filledShapes, thickness: 1 });
    } else if (d.kind === "select") {
      const r = normRect(d.from, d.to);
      const clipped = { x: Math.max(0, r.x), y: Math.max(0, r.y), w: 0, h: 0 };
      clipped.w = Math.min(sprite.width, r.x + r.w) - clipped.x;
      clipped.h = Math.min(sprite.height, r.y + r.h) - clipped.y;
      const valid = clipped.w > 0 && clipped.h > 0 && !(r.w === 1 && r.h === 1);
      const mode = d.mode ?? "replace";
      if (mode === "replace" || (mode === "add" && !s.selection)) s.set({ selection: valid ? clipped : undefined });
      else if (valid && s.selection) marqueeCombine(clipped, mode);
    } else if (d.kind === "move") {
      const dx = d.to[0] - d.from[0];
      const dy = d.to[1] - d.from[1];
      if (dx || dy) moveSelection(target(), dx, dy);
    }
    if (d.kind === "lasso") lassoFinish(d.points, d.mode);
    if (d.kind === "hitbox") void commitHitboxDrag(d.from, d.to);
  };

  const onWheel = (e: React.WheelEvent) => {
    if (!sprite) return;
    const factor = e.deltaY < 0 ? 1 : -1;
    const steps = [1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64];
    const idx = steps.findIndex((v) => v >= z);
    const next = steps[Math.max(0, Math.min(steps.length - 1, (idx < 0 ? steps.length - 1 : idx) + factor))];
    if (next === z) return;
    // Keep the pixel under the cursor fixed.
    const r = canvasRef.current!.getBoundingClientRect();
    const mx = e.clientX - r.left;
    const my = e.clientY - r.top;
    const px = (mx - ox) / z;
    const py = (my - oy) / z;
    const nox = mx - px * next;
    const noy = my - py * next;
    setPan({ x: nox - (size.w / 2 - (sprite.width * next) / 2), y: noy - (size.h / 2 - (sprite.height * next) / 2) });
    s.set({ zoom: next });
  };

  const cursor = drag?.kind === "pan" || space ? "grab" : s.tool === "move" ? "move" : "crosshair";

  return (
    <div ref={wrap} className="relative min-h-0 min-w-0 flex-1 overflow-hidden" onContextMenu={(e) => e.preventDefault()}>
      <canvas
        ref={canvasRef}
        style={{ width: size.w, height: size.h, cursor }}
        className="block"
        aria-label="Canvas de edição de pixel art"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={() => setHover(undefined)}
        onWheel={onWheel}
      />
      {sprite && s.showPreview && (
        <div className="pointer-events-none absolute top-3 right-3 rounded-md bg-shell/85 p-1 ring-1 ring-line" title="Prévia em tamanho real">
          <span className="mb-1 block px-1 text-[9px] tracking-wider text-muted uppercase">Prévia do sprite</span>
          <canvas ref={previewRef} className="checker pixelated block max-h-[160px] max-w-[260px] rounded-sm" />
        </div>
      )}
      {sprite && (
        <div className="pointer-events-none absolute bottom-5 left-4 flex gap-3 rounded-md bg-shell/80 px-2 py-1 font-mono text-[11px] text-muted">
          <span>{sprite.width}×{sprite.height}</span>
          <span>{z}x</span>
          {hover && hover[0] >= 0 && hover[1] >= 0 && hover[0] < sprite.width && hover[1] < sprite.height && <span>[{hover[0]}, {hover[1]}]</span>}
          {s.selection && <span>seleção {s.selection.w}×{s.selection.h}</span>}
        </div>
      )}
      {sprite && <div className="canvas-controls" aria-label="Zoom do canvas">
        <button className="btn-icon h-7 w-7" aria-label="Diminuir zoom" disabled={z <= 1} onClick={() => s.set({ zoom: Math.max(1, z - 1) })}><Minus size={14} /></button>
        <span className="w-12 text-center font-mono text-[11px] text-muted">{z * 100}%</span>
        <button className="btn-icon h-7 w-7" aria-label="Aumentar zoom" disabled={z >= 64} onClick={() => s.set({ zoom: Math.min(64, z + 1) })}><Plus size={14} /></button>
        <span className="mx-1 h-4 w-px bg-line" />
        <button className="btn h-7 text-[11px]" title="Centralizar e ajustar à área disponível" onClick={() => {
          s.set({ zoom: Math.max(1, Math.min(32, Math.floor(Math.min((size.w - 80) / sprite.width, (size.h - 100) / sprite.height)))) });
          setPan({ x: 0, y: 0 });
        }}><Maximize size={13} /> Ajustar</button>
      </div>}

    </div>
  );
}
