import { useEffect, useState } from "react";
import * as api from "./api";
import { useEditor } from "./store";

/** Imagem (canvas fora da tela) de um frame composto, recarregada quando ele muda. */
export function useFrameCanvas(sprite?: number, frame?: number, layer?: number): HTMLCanvasElement | undefined {
  const projectId = useEditor((s) => s.projectId);
  const frameEpoch = useEditor((s) => (frame !== undefined ? (s.frameEpoch[frame] ?? 0) : 0));
  const epoch = useEditor((s) => s.epoch);
  const connected = useEditor((s) => s.connected);
  const [canvas, setCanvas] = useState<HTMLCanvasElement>();
  useEffect(() => {
    if (!projectId || sprite === undefined || frame === undefined || !connected) {
      setCanvas(undefined);
      return;
    }
    let alive = true;
    api
      .renderFrame(projectId, sprite, frame, layer)
      .then((img) => alive && setCanvas(api.toCanvas(img)))
      .catch(() => alive && setCanvas(undefined));
    return () => {
      alive = false;
    };
  }, [projectId, sprite, frame, layer, frameEpoch, epoch, connected]);
  return canvas;
}

/** Imagens de referência do sprite (carregadas uma vez por mudança estrutural). */
export function useReferenceCanvases(): { id: number; canvas: HTMLCanvasElement }[] {
  const projectId = useEditor((s) => s.projectId);
  const refs = useEditor((s) => s.doc?.model.references ?? []);
  const spriteId = useEditor((s) => s.spriteId);
  const key = refs.map((r) => `${r.id}:${r.image}`).join(",");
  const [list, setList] = useState<{ id: number; canvas: HTMLCanvasElement }[]>([]);
  useEffect(() => {
    if (!projectId) return;
    let alive = true;
    const mine = refs.filter((r) => r.sprite === undefined || r.sprite === spriteId);
    Promise.all(mine.map((r) => api.renderImage(projectId, r.image).then((img) => ({ id: r.id, canvas: api.toCanvas(img) }))))
      .then((l) => alive && setList(l))
      .catch(() => {});
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, key, spriteId]);
  return list;
}

export function useKey(handler: (e: KeyboardEvent) => void) {
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
      handler(e);
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [handler]);
}
