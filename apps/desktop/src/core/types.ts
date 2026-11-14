// Tipos espelhando o JSON do runtime (crates/sprite-animation e sprite-project).

export type Point = [number, number];
export type Rect = { x: number; y: number; w: number; h: number };

export type BlendMode = "normal" | "multiply" | "screen" | "overlay" | "darken" | "lighten" | "add" | "subtract";
export type LoopMode = "loop" | "once" | "ping_pong" | "reverse";

export type Layer = {
  id: number;
  name: string;
  visible: boolean;
  locked: boolean;
  opacity: number;
  blend: BlendMode;
  reference: boolean;
};

export type Frame = { id: number; duration_ms: number; tags?: string[] };
export type Cel = { layer: number; frame: number; image: number; opacity: number };
export type Track = { direction?: string | null; frames: number[] };

export type CycleInfo = { key_frames?: number[]; contact_frames?: number[]; ground_y?: number; notes?: string };

export type Animation = {
  id: number;
  name: string;
  loop_mode: LoopMode;
  tags?: string[];
  tracks: Track[];
  cycle?: CycleInfo;
};

export type Direction = {
  name: string;
  label: string;
  angle?: number;
  mirror_of?: string;
  reference?: string;
  custom: boolean;
};

export type Anchor = {
  id: number;
  name: string;
  kind: string;
  default?: Point;
  positions?: Record<string, Point>;
  color?: string;
};

export type RegionShape =
  | { type: "rect"; rect: Rect }
  | { type: "mask"; at: Point; rows: string[] }
  | { type: "polygon"; points: Point[] };

export type Region = {
  id: number;
  name: string;
  part?: string;
  layer?: number;
  default?: RegionShape;
  shapes?: Record<string, RegionShape>;
  color?: string;
};

export type Pose = {
  id: number;
  name: string;
  kind: string;
  animation?: number;
  direction?: string;
  frame?: number;
  anchors?: Record<string, Point>;
  ground_y?: number;
  notes?: string;
};

export type Sprite = {
  id: number;
  name: string;
  width: number;
  height: number;
  origin: Point;
  layers: Layer[];
  frames: Frame[];
  cels: Cel[];
  animations: Animation[];
  directions: Direction[];
  anchors: Anchor[];
  regions: Region[];
  poses: Pose[];
  palette?: string;
};

export type Palette = { name: string; colors: string[] };

export type Reference = {
  id: number;
  name: string;
  image: number;
  sprite?: number;
  position: Point;
  scale: number;
  opacity: number;
  visible: boolean;
};

export type ProjectModel = {
  uid: string;
  name: string;
  sprites: Sprite[];
  palettes: Palette[];
  active_palette?: string;
  references: Reference[];
  metadata: Record<string, unknown>;
  next_id: number;
};

export type ProjectState = {
  project: string;
  model: ProjectModel;
  path?: string | null;
  dirty: boolean;
  revision: number;
  transaction?: number | null;
  transaction_owner?: number | null;
  undo: string[];
  redo: string[];
  snapshots: { id: number; name: string; created: string }[];
};

export type Change = { sprite: number; layer?: number; frame?: number; rect?: Rect; kind: "pixels" | "structure" };

export type CallResult = {
  project?: string;
  data: any;
  images?: { label: string; mime: string; width: number; height: number; base64: string }[];
  changes?: Change[];
  warnings?: string[];
  revision: number;
  transaction?: number;
};

export type CommandError = { code: string; message: string; details?: unknown };

export type OpRecord = {
  time: string;
  client: string;
  client_kind: string;
  project?: string;
  command: string;
  ok: boolean;
  summary: string;
  millis: number;
};

export type ProjectSummary = { id: string; name: string; path?: string; dirty: boolean; revision: number; transaction_owner?: string };
export type ClientSummary = { id: number; kind: string; name: string; project?: string };

export type RuntimeEvent =
  | { event: "changed"; project: string; revision: number; source: string; command: string; changes: Change[]; transaction?: number }
  | ({ event: "operation" } & OpRecord)
  | { event: "projects"; projects: ProjectSummary[] }
  | { event: "clients"; clients: ClientSummary[] }
  | { event: "resync" };

export type RuntimeInfo = {
  connected: boolean;
  endpoint: string;
  psm: string;
  psm_exists: boolean;
  claude_code_config: unknown;
  claude_code_command: string;
  tools: number;
};
