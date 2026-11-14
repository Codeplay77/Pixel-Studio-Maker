// Ponte com o lado nativo (src-tauri). Toda edição é um comando do runtime: o mesmo que o MCP usa.
import { invoke } from "@tauri-apps/api/core";
import type { CallResult, CommandError, ProjectState, RuntimeInfo } from "./types";

export const isTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as CommandError).message);
  return String(e);
}

export function errorCode(e: unknown): string | undefined {
  return e && typeof e === "object" && "code" in e ? String((e as CommandError).code) : undefined;
}

export async function call(name: string, args: Record<string, unknown> = {}, project?: string, expectedRevision?: number): Promise<CallResult> {
  return invoke<CallResult>("runtime_call", { project: project ?? null, name, args, expectedRevision: expectedRevision ?? null });
}

export async function getState(project?: string): Promise<ProjectState> {
  return invoke<ProjectState>("runtime_state", { project: project ?? null });
}

export async function selectProject(project: string): Promise<void> {
  await invoke("runtime_select", { project });
}

export async function runtimeInfo(): Promise<RuntimeInfo> {
  return invoke<RuntimeInfo>("runtime_info");
}

function decodeRaw(buf: ArrayBuffer): ImageData {
  const view = new DataView(buf);
  const w = view.getUint32(0, true);
  const h = view.getUint32(4, true);
  const pixels = new Uint8ClampedArray(buf, 8, w * h * 4);
  return new ImageData(new Uint8ClampedArray(pixels), w, h);
}

/** Pixels compostos de um frame (ou de uma layer), em RGBA. */
export async function renderFrame(project: string, sprite: number, frame: number, layer?: number): Promise<ImageData> {
  const buf = await invoke<ArrayBuffer>("runtime_render", { project, sprite, frame, layer: layer ?? null, allLayers: false });
  return decodeRaw(buf);
}

export async function renderImage(project: string, image: number): Promise<ImageData> {
  const buf = await invoke<ArrayBuffer>("runtime_render_image", { project, image });
  return decodeRaw(buf);
}

/** Canvas fora da tela com os pixels, para desenhar ampliado sem suavização. */
export function toCanvas(img: ImageData): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = img.width;
  c.height = img.height;
  c.getContext("2d")!.putImageData(img, 0, 0);
  return c;
}
