//! Argument types of every command. Their doc comments become the JSON schemas agents read,
//! so they are written for the agent (in Portuguese, like the rest of the project).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::sheet::SheetOptions;
use sprite_animation::{AnchorKind, CycleInfo, LoopMode, PoseKind, RegionShape};
use sprite_core::{BlendMode, Color, FrameRef, ItemRef, PasteMode, Point, Rect};
use sprite_drawing::effects::{DitherPattern, GradientShape, LightDirection, OutlineMode};
use sprite_drawing::fill::Connectivity;
use sprite_drawing::pixelate::PixelateOptions;
use sprite_drawing::sdc::{ColorRef, DrawOp, PixelItem};
use sprite_drawing::transform::Transform;
use sprite_project::export::SheetSelection;
use sprite_project::import::SliceOptions;

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

/// Onde a operação acontece. Tudo é opcional: sprite = o primeiro, layer = a de cima (que não é
/// referência), frame = o primeiro da linha do tempo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Target {
    /// Sprite (id ou nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<ItemRef>,
    /// Layer (id ou nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Frame: id, ou {"animation": "walk", "direction": "E", "index": 0}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
}

/// Só o sprite.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SpriteTarget {
    /// Sprite (id ou nome). Padrão: o primeiro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<ItemRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Empty {}

// ---- projetos -------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectCreate {
    /// Nome do projeto.
    pub name: String,
    /// Largura do primeiro sprite em pixels (padrão 64).
    #[serde(default = "default_side")]
    pub width: u32,
    /// Altura do primeiro sprite em pixels (padrão 64).
    #[serde(default = "default_side")]
    pub height: u32,
    /// Nome do primeiro sprite (padrão: o nome do projeto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite_name: Option<String>,
    /// Paleta inicial; sem ela, usa a "Studio 32".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<Color>,
    /// Nome da paleta inicial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette_name: Option<String>,
    /// Cor de fundo da primeira layer (padrão: transparente).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

fn default_side() -> u32 {
    64
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectOpen {
    /// Caminho do arquivo .psmproj (relativo à pasta de trabalho, ou absoluto dentro de uma pasta permitida).
    pub path: String,
    /// Abrir a cópia de recuperação (autosave) quando ela for mais nova que o arquivo.
    #[serde(default)]
    pub recover: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectSave {
    /// Onde salvar (.psmproj). Omita para salvar no mesmo arquivo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectClose {
    /// Salvar antes de fechar.
    #[serde(default)]
    pub save: bool,
    /// Fechar mesmo com alterações não salvas (elas ficam na recuperação automática).
    #[serde(default)]
    pub discard: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectInfo {
    /// Incluir detalhes completos (cels, frames de cada trilha, regiões, anchors, poses, histórico).
    #[serde(default)]
    pub full: bool,
}

// ---- sprites, referências, snapshots -------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SpriteCreate {
    pub name: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SpriteUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Novo pivô/origem do sprite (usado para alinhar frames na exportação).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    /// Paleta deste sprite (nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Novo tamanho [largura, altura].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    /// Ao mudar o tamanho: `true` escala os pixels (vizinho mais próximo); `false` só muda o canvas.
    #[serde(default)]
    pub scale_content: bool,
    /// Ao mudar o canvas sem escalar: onde fica o conteúdo antigo (padrão `bottom_center`).
    #[serde(default)]
    pub anchor: ResizeAnchor,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResizeAnchor {
    TopLeft,
    Center,
    #[default]
    BottomCenter,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReferenceAdd {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Imagem PNG/GIF de referência.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub position: Point,
    #[serde(default = "one")]
    pub scale: u32,
    #[serde(default = "default_ref_opacity")]
    pub opacity: u8,
}

fn default_ref_opacity() -> u8 {
    128
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReferenceUpdate {
    /// Referência (id ou nome).
    pub reference: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotCreate {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotRestore {
    /// Id do snapshot.
    pub id: u32,
}

// ---- pixels ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct PixelGet {
    #[serde(flatten)]
    pub target: Target,
    pub x: i32,
    pub y: i32,
    /// Ler a imagem composta (todas as layers visíveis) em vez de uma layer.
    #[serde(default)]
    pub composite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PixelSet {
    #[serde(flatten)]
    pub target: Target,
    pub x: i32,
    pub y: i32,
    /// Cor: "#rrggbb", "#rrggbbaa", "clear" ou índice da paleta ativa.
    pub color: ColorRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PixelSetBatch {
    #[serde(flatten)]
    pub target: Target,
    /// `[[x,y], ...]` usando `color`, ou `[[x,y,cor], ...]`.
    pub pixels: Vec<PixelItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PixelClear {
    #[serde(flatten)]
    pub target: Target,
    /// Um pixel...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// ...ou um retângulo. Sem os dois, apaga a cel inteira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Só apaga os pixels desta máscara de seleção (`bounds` + `rows`, de select_color/select_polygon).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<sprite_drawing::select::SelectionMask>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PixelReadRegion {
    #[serde(flatten)]
    pub target: Target,
    /// Região a ler (padrão: o sprite inteiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Ler a imagem composta em vez de uma layer.
    #[serde(default)]
    pub composite: bool,
    /// Comprimir as linhas com RLE ("4.3k").
    #[serde(default)]
    pub rle: bool,
    /// Recortar a região aos pixels visíveis.
    #[serde(default)]
    pub trim: bool,
}

/// Uma matriz de pixels: textual (`rows` + `legend`) ou numérica (`pixels` + `palette`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct MatrixData {
    /// Linhas de caracteres; cada char vira a cor da `legend`. `.` mantém o pixel.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<String>,
    /// char → cor ("#hex", índice da paleta, "clear" para apagar ou null para manter).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub legend: BTreeMap<String, Option<ColorRef>>,
    /// As linhas estão em RLE ("3k2." = "kkk..").
    #[serde(default)]
    pub rle: bool,
    /// Forma numérica: índices em `palette` (ou na paleta ativa); null mantém.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pixels: Vec<Vec<Option<u32>>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<Color>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PixelWriteRegion {
    #[serde(flatten)]
    pub target: Target,
    /// Canto superior esquerdo onde a matriz começa.
    pub at: Point,
    #[serde(flatten)]
    pub data: MatrixData,
}

/// Patch de diferenças: só os pixels que mudam. Matriz (com `.` = manter) a partir de `at`,
/// e/ou uma lista `[[x,y,cor], ...]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Patch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    #[serde(flatten)]
    pub matrix: MatrixData,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<PixelItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PixelApplyPatch {
    #[serde(flatten)]
    pub target: Target,
    pub patch: Patch,
}

// ---- desenho --------------------------------------------------------------------------------

/// Uma operação do lote, com alvo opcional próprio.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchOp {
    /// Layer desta operação (padrão: a do lote).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Frame desta operação (padrão: o do lote).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Só pinta dentro desta região semântica (id ou nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ItemRef>,
    #[serde(flatten)]
    pub op: DrawOp,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatchPolicy {
    /// Qualquer erro desfaz o lote inteiro (padrão).
    #[default]
    AllOrNothing,
    /// Operações inválidas são puladas e relatadas; as outras ficam.
    SkipInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawBatch {
    #[serde(flatten)]
    pub target: Target,
    /// Operações SDC em ordem. Cada uma tem "op" (pixel, pixels, matrix, line, polyline, rect,
    /// ellipse, circle, polygon, curve, fill, replace_color, clear, outline, dither, gradient,
    /// shade, drop_shadow, pattern, transform, copy, cleanup, quantize, adjust, stroke, text).
    pub ops: Vec<BatchOp>,
    #[serde(default)]
    pub policy: BatchPolicy,
    /// Região semântica que limita todas as operações.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ItemRef>,
    /// Rótulo no histórico.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawLine {
    #[serde(flatten)]
    pub target: Target,
    pub from: Point,
    pub to: Point,
    pub color: ColorRef,
    #[serde(default = "one")]
    pub thickness: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Circle,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawShape {
    #[serde(flatten)]
    pub target: Target,
    pub shape: ShapeKind,
    /// Retângulo/elipse: caixa x,y,w,h.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Círculo: centro e raio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<u32>,
    pub color: ColorRef,
    #[serde(default)]
    pub fill: bool,
    #[serde(default = "one")]
    pub thickness: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawPolygon {
    #[serde(flatten)]
    pub target: Target,
    pub points: Vec<Point>,
    pub color: ColorRef,
    #[serde(default = "yes")]
    pub fill: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawFill {
    #[serde(flatten)]
    pub target: Target,
    pub x: i32,
    pub y: i32,
    pub color: ColorRef,
    #[serde(default = "yes")]
    pub contiguous: bool,
    #[serde(default)]
    pub connectivity: Connectivity,
    #[serde(default)]
    pub tolerance: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawOutline {
    #[serde(flatten)]
    pub target: Target,
    pub color: ColorRef,
    #[serde(default)]
    pub mode: OutlineMode,
    #[serde(default)]
    pub connectivity: Connectivity,
    #[serde(default = "one")]
    pub thickness: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawDither {
    #[serde(flatten)]
    pub target: Target,
    pub color: ColorRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<ColorRef>,
    /// 0..1: fração dos pixels que recebem `color`.
    pub density: f64,
    #[serde(default)]
    pub pattern: DitherPattern,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Só sobre pixels já visíveis.
    #[serde(default)]
    pub only_opaque: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawGradient {
    #[serde(flatten)]
    pub target: Target,
    pub colors: Vec<ColorRef>,
    pub shape: GradientShape,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dither: Option<DitherPattern>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    #[serde(default)]
    pub only_opaque: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawShade {
    #[serde(flatten)]
    pub target: Target,
    #[serde(default)]
    pub light: LightDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<ColorRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<ColorRef>,
    #[serde(default = "default_amount")]
    pub highlight_amount: f64,
    #[serde(default = "default_amount")]
    pub shadow_amount: f64,
    #[serde(default = "one")]
    pub highlight_depth: u32,
    #[serde(default = "one")]
    pub shadow_depth: u32,
    #[serde(default = "yes")]
    pub hue_shift: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Limitar a uma região semântica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ItemRef>,
}

fn default_amount() -> f64 {
    0.3
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawTransform {
    #[serde(flatten)]
    pub target: Target,
    pub transform: Transform,
    /// Área a transformar (padrão: a cel inteira).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Ou uma região semântica.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<ItemRef>,
    /// Só os pixels desta máscara de seleção (`bounds` + `rows`). A resposta traz `selection`: a máscara já transformada.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<sprite_drawing::select::SelectionMask>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColorScope {
    /// Só a cel alvo.
    #[default]
    Cel,
    /// A layer em todos os frames.
    Layer,
    /// Todas as layers do frame.
    Frame,
    /// O sprite inteiro.
    Sprite,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawReplaceColor {
    #[serde(flatten)]
    pub target: Target,
    pub from: ColorRef,
    pub to: ColorRef,
    #[serde(default)]
    pub tolerance: u8,
    #[serde(default)]
    pub scope: ColorScope,
}

// ---- estruturas: regiões, anchors, poses ---------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegionCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Nome único, ex.: "cabeca", "braco_esq", "espada".
    pub name: String,
    /// Categoria livre: head, torso, arm_left, arm_right, leg_left, leg_right, hair, equipment, weapon, accessory...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    /// Layer que guarda essa parte (se tiver uma própria).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Forma padrão (vale para todos os frames sem forma própria).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<RegionShape>,
    /// Sem `shape`: usar os pixels visíveis da `layer` neste frame como máscara.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_frame: Option<FrameRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameShape {
    pub frame: FrameRef,
    pub shape: RegionShape,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegionUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Região (id ou nome).
    pub region: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Nova forma padrão.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<RegionShape>,
    /// Formas específicas por frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_shapes: Vec<FrameShape>,
    /// Remover as formas específicas destes frames.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear_frames: Vec<FrameRef>,
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegionCopy {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub region: ItemRef,
    /// Frame de onde copiar os pixels da região.
    pub from_frame: FrameRef,
    /// Frames de destino.
    pub to_frames: Vec<FrameRef>,
    /// Layers copiadas (padrão: a layer da região, ou todas).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ItemRef>,
    /// Deslocamento [dx, dy] aplicado no destino.
    #[serde(default)]
    pub offset: Point,
    #[serde(default)]
    pub flip_h: bool,
    /// Apagar a região no destino antes de colar.
    #[serde(default)]
    pub clear_destination: bool,
    #[serde(default = "skip_transparent")]
    pub mode: PasteMode,
}

fn skip_transparent() -> PasteMode {
    PasteMode::SkipTransparent
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RegionTransform {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub region: ItemRef,
    /// Frames afetados (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<FrameRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ItemRef>,
    pub transform: Transform,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListArgs {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Mostrar posições/formas neste frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FramePoint {
    pub frame: FrameRef,
    pub position: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnchorCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Ex.: "pescoco", "ombro_dir", "mao_esq", "quadril", "pe_dir", "chao", "pivo".
    pub name: String,
    #[serde(default)]
    pub kind: AnchorKind,
    /// Posição padrão (todos os frames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Point>,
    /// Posições por frame.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<FramePoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnchorUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub anchor: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<AnchorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<FramePoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clear_frames: Vec<FrameRef>,
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PoseCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub name: String,
    #[serde(default)]
    pub kind: PoseKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Nome do anchor → posição nesta pose.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub anchors: BTreeMap<String, Point>,
    /// Copiar as posições dos anchors do `frame` (quando `anchors` estiver vazio).
    #[serde(default = "yes")]
    pub capture_anchors: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_y: Option<i32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PoseUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub pose: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PoseKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub anchors: BTreeMap<String, Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_y: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub delete: bool,
}

// ---- layers ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub name: String,
    /// Criar logo acima desta layer (padrão: no topo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<ItemRef>,
    #[serde(default = "full")]
    pub opacity: u8,
    #[serde(default)]
    pub blend: BlendMode,
    /// Layer de referência (não sai na exportação).
    #[serde(default)]
    pub reference: bool,
}

fn full() -> u8 {
    255
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerRefArgs {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub layer: ItemRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerDuplicate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub layer: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerReorder {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub layer: ItemRef,
    /// Posição final (0 = embaixo de todas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Ou: logo acima desta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<ItemRef>,
    /// Ou: logo abaixo desta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<ItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerMerge {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Layer de cima, que some.
    pub layer: ItemRef,
    /// Layer que recebe (padrão: a imediatamente abaixo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub into: Option<ItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LayerProperties {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub layer: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend: Option<BlendMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<bool>,
}

// ---- animações e direções ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Ex.: idle, walk, run, jump, attack, hit, death, cast, interact.
    pub name: String,
    /// Direções ("E", "NW", "all" para as 8). Vazio = animação sem direção.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    /// Frames novos por trilha.
    #[serde(default = "one")]
    pub frames: u32,
    #[serde(default = "default_duration")]
    pub duration_ms: u32,
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// Cada frame novo começa como cópia deste (para depois redesenhar a pose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_from: Option<FrameRef>,
    /// Usar frames existentes (em vez de criar) na trilha sem direção / primeira direção.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub use_frames: Vec<FrameRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn default_duration() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationRefArgs {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationDelete {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
    /// Apagar também os frames que nenhuma outra animação usa.
    #[serde(default = "yes")]
    pub delete_frames: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct AnimationInfo {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Animação (id ou nome). Omita para listar todas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    /// Incluir métricas por frame (caixa, base, centro, anchors).
    #[serde(default)]
    pub metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CopyTrack {
    /// Direção de origem.
    pub direction: String,
    /// Espelhar horizontalmente os frames copiados (opcional; vistas opostas raramente são simétricas).
    #[serde(default)]
    pub mirror: bool,
    /// Vincular as cels em vez de copiar (editar uma muda a outra). Não combina com `mirror`.
    #[serde(default)]
    pub link: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationSetDirection {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
    /// Direção da trilha (N, NE, E, SE, S, SW, W, NW ou personalizada).
    pub direction: String,
    /// Usar estes frames existentes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<FrameRef>,
    /// Criar N frames novos vazios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<u32>,
    /// Criar a trilha copiando outra direção.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_from: Option<CopyTrack>,
    /// Direção de referência visual para esta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Remover a trilha.
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationSetTiming {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
    /// Só esta trilha (padrão: todas).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Mesma duração para todos os frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    /// Ou quadros por segundo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    /// Ou uma duração por frame, na ordem da trilha.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub durations: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_mode: Option<LoopMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Ciclo de movimento: quadros-chave, contatos com o chão, linha do chão, notas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle: Option<CycleInfo>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMode {
    /// Frames lado a lado (para inspecionar poses).
    #[default]
    Strip,
    /// GIF animado.
    Gif,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AnimationPreview {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub animation: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default)]
    pub mode: PreviewMode,
    /// Ampliação (padrão: automática).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// Mostrar o frame anterior como fantasma em cada célula.
    #[serde(default)]
    pub onion: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DirectionCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Nome curto único (ex.: "3/4", "iso_left", "top").
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Ângulo em graus (E=0, S=90, W=180, N=270).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DirectionUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub direction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Direção espelho sugerida ("" remove).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_of: Option<String>,
    /// Direção de referência ("" remove).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Apagar (só direções personalizadas sem trilhas).
    #[serde(default)]
    pub delete: bool,
}

// ---- frames ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FrameCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Animação onde inserir (opcional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Posição na trilha (padrão: no fim).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default = "default_duration")]
    pub duration_ms: u32,
    /// Começar como cópia deste frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_from: Option<FrameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameDuplicate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub frame: FrameRef,
    #[serde(default = "one")]
    pub count: u32,
    /// Vincular as cels (editar uma muda as outras) em vez de copiar.
    #[serde(default)]
    pub link: bool,
    /// Espelhar horizontalmente as cópias.
    #[serde(default)]
    pub mirror: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameDelete {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub frames: Vec<FrameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameMove {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub frame: FrameRef,
    /// Nova posição (0-based).
    pub index: usize,
    /// Mover dentro desta animação/direção (padrão: na linha do tempo geral).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameEdit {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub frame: FrameRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Desenhar neste frame (layer padrão: `layer`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ops: Vec<BatchOp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Desvincular as cels deste frame (cada uma ganha cópia própria).
    #[serde(default)]
    pub unlink: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompareOutput {
    /// Só os números.
    Summary,
    /// Números + patch textual (de A para B) + imagem lado a lado com as diferenças.
    #[default]
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameCompare {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub a: FrameRef,
    pub b: FrameRef,
    /// Comparar só esta layer (padrão: imagem composta).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    #[serde(default)]
    pub output: CompareOutput,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    /// Xadrez cinza (mostra a transparência).
    #[default]
    Checker,
    /// Transparente.
    None,
    White,
    Black,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FramePreview {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Frame (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Só uma região do frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Só estas layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ItemRef>,
    /// Ampliação (padrão: automática, ~512 px).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default)]
    pub background: Background,
    /// Grade entre os pixels (a partir de 6x).
    #[serde(default)]
    pub grid: bool,
    /// Desenhar os anchors.
    #[serde(default)]
    pub anchors: bool,
    /// Contornar as regiões semânticas.
    #[serde(default)]
    pub regions: bool,
    /// Frames anteriores/seguintes como fantasma (onion skin) dentro de uma animação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onion: Option<OnionArgs>,
    /// Desenhar a grade de tiles e os polígonos de colisão (sprites com grade definida em tile_grid_set).
    #[serde(default)]
    pub tiles: bool,
    /// Desenhar as hitboxes/hurtboxes do frame (contorno na cor de cada caixa).
    #[serde(default)]
    pub hitboxes: bool,
    /// Desenhar as 4 linhas-guia do 9-slice (margens definidas com nine_slice_set).
    #[serde(default)]
    pub nine_slice: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct OnionArgs {
    #[serde(default = "one_usize")]
    pub before: usize,
    #[serde(default)]
    pub after: usize,
}

fn one_usize() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FrameApplyPatch {
    #[serde(flatten)]
    pub target: Target,
    pub patch: Patch,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FrameInspect {
    #[serde(flatten)]
    pub target: Target,
    /// Inspecionar a imagem composta (padrão) ou só `layer`.
    #[serde(default)]
    pub layer_only: bool,
}

// ---- paletas --------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaletteCreate {
    pub name: String,
    pub colors: Vec<Color>,
    /// Tornar a paleta ativa.
    #[serde(default = "yes")]
    pub activate: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteUpdate {
    /// Paleta (padrão: a ativa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    /// Substituir a lista inteira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<Color>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<Color>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<Color>,
    /// Trocar a cor de um índice: {"index": 3, "color": "#ff0000"} (não mexe nos pixels).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<PaletteSet>,
    #[serde(default)]
    pub activate: bool,
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaletteSet {
    pub index: usize,
    pub color: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaletteReplace {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Cor atual nos pixels.
    pub from: ColorRef,
    /// Cor nova.
    pub to: ColorRef,
    #[serde(default)]
    pub tolerance: u8,
    /// Também trocar a cor na paleta ativa.
    #[serde(default = "yes")]
    pub update_palette: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteQuantize {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Reduzir para no máximo N cores (median cut)...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_colors: Option<u32>,
    /// ...ou levar tudo para as cores desta paleta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Salvar o resultado como paleta com este nome (e ativá-la).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_as: Option<String>,
}

// ---- importação e exportação ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Um sprite novo do tamanho da imagem (ou da célula).
    #[default]
    NewSprite,
    /// Uma layer nova no sprite, no frame alvo (ou em frames seguidos, para GIF/sheet).
    NewLayer,
    /// Frames novos no sprite (um por quadro do GIF ou célula da sheet).
    Frames,
    /// Imagem de referência (não exporta).
    Reference,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportImage {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo PNG, GIF ou Aseprite (.aseprite/.ase: no modo new_sprite mantém layers, frames,
    /// tags como animações, paleta e slices como regiões; nos outros modos, a imagem composta).
    pub path: String,
    #[serde(default)]
    pub mode: ImportMode,
    /// Nome do sprite/layer criado.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Cortar a imagem como sprite sheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice: Option<SliceOptions>,
    /// Criar uma animação com os frames importados.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Posição da imagem no canvas (new_layer / frames).
    #[serde(default)]
    pub at: Point,
    /// Converter a imagem em pixel art antes de importar (só imagens estáticas; mesmas opções de
    /// `image_pixelate`). Em layer/frames, sem tamanho informado, o resultado cabe no sprite. A
    /// paleta do resultado é guardada no projeto (sem ativar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixelate: Option<PixelateOptions>,
}

/// Converter uma imagem em pixel art estática. Todas as opções têm padrão: basta `path`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImagePixelate {
    /// Imagem PNG (até 16384 px por lado) ou GIF (usa o primeiro quadro).
    pub path: String,
    #[serde(flatten)]
    pub options: PixelateOptions,
    /// Só devolver a prévia, sem criar projeto (para testar opções).
    #[serde(default)]
    pub preview: bool,
    /// Devolver também a imagem original reduzida, para comparar.
    #[serde(default)]
    pub with_source: bool,
    /// Escala da imagem devolvida (padrão: automática, até uns 512 px; 1 = tamanho real).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_scale: Option<u32>,
    /// Nome do projeto e do sprite (padrão: o nome do arquivo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportPng {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png de saída.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    #[serde(default = "one")]
    pub scale: u32,
    /// Só estas layers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportSpritesheet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png da sheet.
    pub path: String,
    #[serde(default)]
    pub sheet: SheetOptions,
    #[serde(default)]
    pub selection: SheetSelection,
    #[serde(default = "one")]
    pub scale: u32,
    /// Também gravar o JSON de metadados ao lado (.json).
    #[serde(default = "yes")]
    pub json: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportFrames {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Pasta de saída.
    pub directory: String,
    /// Prefixo dos arquivos (padrão: nome do sprite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default)]
    pub selection: SheetSelection,
    #[serde(default = "one")]
    pub scale: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportGif {
    #[serde(flatten)]
    pub target: SpriteTarget,
    pub path: String,
    pub animation: ItemRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default = "one")]
    pub scale: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportGodot {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .tres (SpriteFrames) de saída; a sheet .png é gravada ao lado com o mesmo nome.
    pub path: String,
    /// Caminho da textura dentro do projeto Godot (padrão: "res://<nome>.png").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_path: Option<String>,
    #[serde(default = "rows_sheet")]
    pub sheet: SheetOptions,
    #[serde(default)]
    pub selection: SheetSelection,
    #[serde(default = "one")]
    pub scale: u32,
}

fn rows_sheet() -> SheetOptions {
    SheetOptions { layout: sprite_animation::sheet::SheetLayout::Rows, ..Default::default() }
}

// ---- controle -------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct HistoryStep {
    /// Quantos passos (padrão 1).
    #[serde(default = "one")]
    pub steps: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TransactionBegin {
    /// Rótulo da transação no histórico.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProjectValidate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Verificar cores fora da paleta (a do sprite ou a ativa).
    #[serde(default = "yes")]
    pub palette_check: bool,
    /// Deslocamento máximo do centro entre frames seguidos, em px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_jump: Option<f64>,
}

// Argumentos de cada área ficam em args/<área>.rs, registrados aqui.
mod rpgmaker;
pub use rpgmaker::*;
mod tiles;
pub use tiles::*;
mod godot;
pub use godot::*;
mod ferramentas;
pub use ferramentas::*;
mod texto;
pub use texto::*;
mod paletas;
pub use paletas::*;
