//! The single command layer. The desktop UI and the MCP server both send [`Command`]s; nothing
//! about editing exists only in the frontend.
//!
//! A [`Session`] owns one open project. [`Session::execute`] runs a command inside a transaction
//! (implicit, or the explicit one opened by `transaction_begin`), returns a structured
//! [`Outcome`] with the new revision and what changed, and turns every failure into a
//! [`CommandError`] with a stable [`ErrorCode`].

pub mod args;
mod ops;
pub mod picture;
pub mod render;
mod target;

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::Rect;
use sprite_project::Document;

pub use args::*;

/// Stable error codes, shared by the runtime protocol and MCP tool results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    NotFound,
    OutOfBounds,
    RevisionConflict,
    LimitExceeded,
    TransactionState,
    Locked,
    Io,
    PermissionDenied,
    Unsupported,
    NoProject,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        CommandError { code, message: message.into(), details: None }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        CommandError::new(ErrorCode::InvalidArgument, message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        CommandError::new(ErrorCode::NotFound, message)
    }
}

impl From<sprite_animation::ModelError> for CommandError {
    fn from(e: sprite_animation::ModelError) -> Self {
        match e {
            sprite_animation::ModelError::NotFound { .. } => CommandError::not_found(e.to_string()),
            sprite_animation::ModelError::Invalid(m) => CommandError::invalid(m),
        }
    }
}

impl From<sprite_project::ProjectError> for CommandError {
    fn from(e: sprite_project::ProjectError) -> Self {
        use sprite_project::ProjectError as P;
        let code = match &e {
            P::Io(_) => ErrorCode::Io,
            P::Transaction(_) => ErrorCode::TransactionState,
            P::Format(_) | P::NewerVersion { .. } | P::Integrity(_) => ErrorCode::Unsupported,
            P::Image(_) => ErrorCode::InvalidArgument,
        };
        CommandError::new(code, e.to_string())
    }
}

impl From<sprite_drawing::DrawError> for CommandError {
    fn from(e: sprite_drawing::DrawError) -> Self {
        let code = match &e {
            sprite_drawing::DrawError::TooLarge(_) => ErrorCode::LimitExceeded,
            _ => ErrorCode::InvalidArgument,
        };
        CommandError::new(code, e.to_string())
    }
}

/// What a command changed, for UI refresh and agent feedback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Change {
    pub sprite: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    /// Changed area (pixel changes only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// `pixels` or `structure`.
    pub kind: String,
}

/// A rendered image returned to the caller (PNG bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedImage {
    pub label: String,
    pub mime: String,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Result of a command.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub data: serde_json::Value,
    pub images: Vec<RenderedImage>,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    pub revision: u64,
    /// Explicit transaction still open, if any.
    pub transaction: Option<u64>,
    /// History label of the step this command created (write commands).
    pub label: Option<String>,
}

impl Outcome {
    pub fn data(data: serde_json::Value) -> Self {
        Outcome { data, ..Default::default() }
    }
}

/// Configurable limits (protect the runtime from runaway requests).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Limits {
    pub max_canvas_side: u32,
    pub max_layers: usize,
    pub max_frames: usize,
    pub max_batch_ops: usize,
    pub max_preview_side: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_canvas_side: 1024, max_layers: 256, max_frames: 4096, max_batch_ops: 20_000, max_preview_side: 2048 }
    }
}

/// Where files may be read or written. Relative paths resolve against `base`.
#[derive(Debug, Clone)]
pub struct PathPolicy {
    pub base: PathBuf,
    pub allowed: Vec<PathBuf>,
    /// Desktop file dialogs: the user chose the path explicitly.
    pub allow_any: bool,
}

impl PathPolicy {
    pub fn permissive(base: impl Into<PathBuf>) -> Self {
        PathPolicy { base: base.into(), allowed: Vec::new(), allow_any: true }
    }

    /// Resolves and checks a path. `..` segments are normalized before the check.
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, CommandError> {
        if raw.trim().is_empty() {
            return Err(CommandError::invalid("caminho vazio"));
        }
        let p = Path::new(raw.trim());
        let joined = if p.is_absolute() { p.to_path_buf() } else { self.base.join(p) };
        let normal = normalize(&joined);
        if self.allow_any {
            return Ok(normal);
        }
        let ok = std::iter::once(&self.base).chain(self.allowed.iter()).any(|root| normal.starts_with(normalize(root)));
        if ok {
            Ok(normal)
        } else {
            Err(CommandError::new(
                ErrorCode::PermissionDenied,
                format!(
                    "{} fica fora das pastas permitidas ({})",
                    normal.display(),
                    std::iter::once(&self.base).chain(self.allowed.iter()).map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ),
            ))
        }
    }
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Per-call context.
#[derive(Debug, Clone)]
pub struct ExecContext {
    pub paths: PathPolicy,
    pub limits: Limits,
    /// Who is calling ("claude", "editor"...), recorded in history labels.
    pub client: String,
}

impl ExecContext {
    pub fn local(base: impl Into<PathBuf>) -> Self {
        ExecContext { paths: PathPolicy::permissive(base), limits: Limits::default(), client: "local".into() }
    }
}

/// How a command interacts with project state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Reads only.
    Read,
    /// Changes the project (runs in a transaction, can be undone).
    Write,
    /// Handled by the runtime (projects, status, transactions, history).
    Runtime,
}

macro_rules! commands {
    ($( $name:literal => $variant:ident($args:ty), $kind:ident, $desc:literal; )*) => {
        /// Every operation of the editor.
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(tag = "cmd", content = "args")]
        pub enum Command {
            $( #[serde(rename = $name)] $variant($args), )*
        }

        /// Static description of a command (used to build MCP tools).
        pub struct CommandSpec {
            pub name: &'static str,
            pub description: &'static str,
            pub kind: Kind,
            pub schema: fn() -> schemars::Schema,
        }

        pub fn specs() -> Vec<CommandSpec> {
            vec![$( CommandSpec { name: $name, description: $desc, kind: Kind::$kind, schema: || schema_of::<$args>() }, )*]
        }

        impl Command {
            pub fn name(&self) -> &'static str {
                match self { $( Command::$variant(_) => $name, )* }
            }
            pub fn kind(&self) -> Kind {
                match self { $( Command::$variant(_) => Kind::$kind, )* }
            }
        }
    };
}

/// Inline JSON schema (no `$ref`), suitable as an MCP tool input schema.
pub fn schema_of<T: JsonSchema>() -> schemars::Schema {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
        s.meta_schema = None;
    });
    settings.into_generator().into_root_schema_for::<T>()
}

commands! {
    // projetos (runtime)
    "project_create" => ProjectCreate(args::ProjectCreate), Runtime, "Cria um projeto novo com um sprite (tamanho em pixels) e o torna o projeto atual. Retorna o id do projeto.";
    "project_open" => ProjectOpen(args::ProjectOpen), Runtime, "Abre um arquivo .psmproj e o torna o projeto atual (se já estiver aberto, reaproveita a mesma sessão).";
    "project_save" => ProjectSave(args::ProjectSave), Runtime, "Salva o projeto atual (salvamento atômico). Com `path`, salva como um arquivo novo.";
    "project_close" => ProjectClose(args::ProjectClose), Runtime, "Fecha o projeto atual. Recusa se houver alterações não salvas, a menos que `save` ou `discard`.";
    "project_list" => ProjectList(args::Empty), Runtime, "Lista os projetos abertos no runtime e as cópias de recuperação disponíveis.";
    "project_info" => ProjectInfo(args::ProjectInfo), Read, "Resumo do projeto: sprites, layers, frames, animações e direções, paletas, revisão, histórico.";
    "project_validate" => ProjectValidate(args::ProjectValidate), Read, "Verificações automáticas: frames vazios ou duplicados, saltos bruscos, mudanças anormais, cores fora da paleta, buracos, pivôs e chão desalinhados. Gera alertas, não bloqueia nada.";
    // sprites, referências, snapshots
    "sprite_create" => SpriteCreate(args::SpriteCreate), Write, "Cria outro sprite no projeto (personagem, item, cenário...).";
    "sprite_update" => SpriteUpdate(args::SpriteUpdate), Write, "Renomeia, define o pivô/origem, a paleta ou o tamanho de um sprite (canvas ou escala por vizinho mais próximo).";
    "reference_add" => ReferenceAdd(args::ReferenceAdd), Write, "Adiciona uma imagem de referência ao sprite (aparece no editor, nunca é exportada).";
    "reference_update" => ReferenceUpdate(args::ReferenceUpdate), Write, "Move, escala, mostra/esconde ou remove uma referência.";
    "snapshot_create" => SnapshotCreate(args::SnapshotCreate), Write, "Guarda uma cópia nomeada do projeto inteiro para restaurar depois.";
    "snapshot_restore" => SnapshotRestore(args::SnapshotRestore), Write, "Volta o projeto ao estado de um snapshot (pode ser desfeito).";
    "snapshot_list" => SnapshotList(args::Empty), Read, "Lista os snapshots do projeto.";
    // pixels
    "pixel_get" => PixelGet(args::PixelGet), Read, "Lê a cor de um pixel (de uma layer ou da imagem composta).";
    "pixel_set" => PixelSet(args::PixelSet), Write, "Pinta um pixel.";
    "pixel_set_batch" => PixelSetBatch(args::PixelSetBatch), Write, "Pinta muitos pixels em uma chamada: [[x,y], ...] com `color`, ou [[x,y,cor], ...].";
    "pixel_clear" => PixelClear(args::PixelClear), Write, "Apaga (deixa transparente) um pixel, um retângulo ou a cel inteira.";
    "pixel_read_region" => PixelReadRegion(args::PixelReadRegion), Read, "Lê uma região como matriz textual compacta: `legend` (char → cor) + `rows`. É o mesmo formato aceito por pixel_write_region.";
    "pixel_write_region" => PixelWriteRegion(args::PixelWriteRegion), Write, "Escreve uma matriz de pixels a partir de `at`: `rows` + `legend` (char → cor, `.` mantém, \"clear\" apaga), opcionalmente RLE, ou `pixels` numéricos com `palette`.";
    "pixel_apply_patch" => PixelApplyPatch(args::PixelApplyPatch), Write, "Aplica um patch de diferenças (matriz com `.` = manter e/ou lista [[x,y,cor]]) sem reenviar o resto.";
    // desenho
    "draw_batch" => DrawBatch(args::DrawBatch), Write, "Executa várias operações SDC em ordem, de forma atômica (um passo no histórico). É a forma recomendada de desenhar: silhueta, preenchimento, contorno, sombra e detalhes numa só chamada. Retorna pixels alterados, região alterada, revisão e transação.";
    "draw_line" => DrawLine(args::DrawLine), Write, "Linha reta pixel-perfect (Bresenham), com espessura.";
    "draw_shape" => DrawShape(args::DrawShape), Write, "Retângulo, elipse ou círculo (contorno ou cheio), sem antialiasing.";
    "draw_polygon" => DrawPolygon(args::DrawPolygon), Write, "Polígono pelos vértices (cheio por padrão) — bom para silhuetas.";
    "draw_fill" => DrawFill(args::DrawFill), Write, "Balde de tinta a partir de um pixel (contíguo ou global, conectividade 4/8, tolerância).";
    "draw_outline" => DrawOutline(args::DrawOutline), Write, "Contorno em volta (outside) ou na borda (inside) das formas visíveis da layer.";
    "draw_dither" => DrawDither(args::DrawDither), Write, "Dithering ordenado (Bayer 2/4/8, xadrez, linhas) com densidade 0..1.";
    "draw_gradient" => DrawGradient(args::DrawGradient), Write, "Degradê discreto (faixas) com as cores dadas, linear ou radial, com dithering opcional.";
    "draw_shade" => DrawShade(args::DrawShade), Write, "Sombras e luzes pela direção da luz: bordas viradas para a luz clareiam, as opostas escurecem (cores fixas ou automáticas com desvio de matiz).";
    "draw_transform" => DrawTransform(args::DrawTransform), Write, "Espelha, gira (90° exatos ou graus), move ou escala (vizinho mais próximo) os pixels de uma área ou região.";
    "draw_replace_color" => DrawReplaceColor(args::DrawReplaceColor), Write, "Troca uma cor por outra na cel, na layer, no frame ou no sprite inteiro.";
    // estruturas
    "region_create" => RegionCreate(args::RegionCreate), Write, "Cria uma região semântica (cabeça, tronco, braço, arma...) por retângulo, máscara, polígono ou pelos pixels visíveis de uma layer.";
    "region_update" => RegionUpdate(args::RegionUpdate), Write, "Altera uma região: nome, parte, layer, forma padrão, formas por frame, ou apaga.";
    "region_list" => RegionList(args::ListArgs), Read, "Lista as regiões semânticas (com a caixa de cada uma no frame pedido).";
    "region_copy" => RegionCopy(args::RegionCopy), Write, "Copia os pixels de uma região de um frame para outros frames (com deslocamento e espelho opcionais).";
    "region_transform" => RegionTransform(args::RegionTransform), Write, "Transforma (move, gira, espelha, escala) os pixels de uma região em um ou mais frames.";
    "anchor_create" => AnchorCreate(args::AnchorCreate), Write, "Cria um anchor (pivô, articulação, mão, pé, chão...) com posição padrão e/ou por frame.";
    "anchor_update" => AnchorUpdate(args::AnchorUpdate), Write, "Move, renomeia ou apaga um anchor; define posições por frame.";
    "anchor_list" => AnchorList(args::ListArgs), Read, "Lista os anchors e suas posições (no frame pedido, ou todas).";
    "pose_create" => PoseCreate(args::PoseCreate), Write, "Registra uma pose (inicial, final, chave, contato, passagem...) com posições de anchors, chão e notas.";
    "pose_update" => PoseUpdate(args::PoseUpdate), Write, "Altera ou apaga uma pose registrada.";
    "pose_list" => PoseList(args::ListArgs), Read, "Lista as poses registradas.";
    // layers
    "layer_create" => LayerCreate(args::LayerCreate), Write, "Cria uma layer (no topo ou acima de outra).";
    "layer_delete" => LayerDelete(args::LayerRefArgs), Write, "Apaga uma layer e suas cels.";
    "layer_duplicate" => LayerDuplicate(args::LayerDuplicate), Write, "Duplica uma layer com todas as cels.";
    "layer_reorder" => LayerReorder(args::LayerReorder), Write, "Muda a ordem de uma layer (índice, acima de ou abaixo de outra).";
    "layer_merge" => LayerMerge(args::LayerMerge), Write, "Funde uma layer na de baixo (ou em `into`) em todos os frames.";
    "layer_properties" => LayerProperties(args::LayerProperties), Write, "Lê e altera nome, visibilidade, bloqueio, opacidade, modo de mistura ou se é referência.";
    // animações e direções
    "animation_create" => AnimationCreate(args::AnimationCreate), Write, "Cria uma animação (idle, walk, attack...) com N frames por direção, opcionalmente copiando um frame base para redesenhar.";
    "animation_delete" => AnimationDelete(args::AnimationDelete), Write, "Apaga uma animação (e os frames que só ela usava).";
    "animation_info" => AnimationInfo(args::AnimationInfo), Read, "Lista animações ou detalha uma: trilhas por direção, frames, durações, modo de repetição, ciclo e métricas por frame.";
    "animation_update" => AnimationUpdate(args::AnimationUpdate), Write, "Renomeia, define tags ou o ciclo de movimento (quadros-chave, contatos, linha do chão).";
    "animation_set_direction" => AnimationSetDirection(args::AnimationSetDirection), Write, "Cria ou altera a trilha de uma direção: frames existentes, frames novos, cópia (opcionalmente espelhada) de outra direção, referência, ou remove.";
    "animation_set_timing" => AnimationSetTiming(args::AnimationSetTiming), Write, "Define durações (iguais, por frame ou por FPS) e o modo de repetição (loop, once, ping_pong, reverse).";
    "animation_preview" => AnimationPreview(args::AnimationPreview), Read, "Imagem da animação para inspeção: frames lado a lado (strip) ou GIF.";
    "direction_create" => DirectionCreate(args::DirectionCreate), Write, "Cria uma direção ou vista personalizada além das 8 padrão.";
    "direction_update" => DirectionUpdate(args::DirectionUpdate), Write, "Altera rótulo, ângulo, espelho sugerido ou referência de uma direção; apaga direções personalizadas.";
    // frames
    "frame_create" => FrameCreate(args::FrameCreate), Write, "Cria frames vazios (ou cópias de um frame), opcionalmente dentro de uma animação/direção.";
    "frame_duplicate" => FrameDuplicate(args::FrameDuplicate), Write, "Duplica um frame logo depois dele (cópia, cópia espelhada ou cels vinculadas).";
    "frame_delete" => FrameDelete(args::FrameDelete), Write, "Apaga frames (de todas as animações).";
    "frame_move" => FrameMove(args::FrameMove), Write, "Muda a posição de um frame na trilha de uma animação ou na linha do tempo.";
    "frame_edit" => FrameEdit(args::FrameEdit), Write, "Altera duração/tags de um frame, desvincula cels e/ou desenha nele com operações SDC.";
    "frame_compare" => FrameCompare(args::FrameCompare), Read, "Compara dois frames: pixels alterados, caixa das diferenças, patch textual de A para B e imagem lado a lado com as diferenças.";
    "frame_preview" => FramePreview(args::FramePreview), Read, "Renderiza um frame (ou região) como imagem ampliada para inspeção, com grade, anchors, regiões e onion skin opcionais.";
    "frame_apply_patch" => FrameApplyPatch(args::FrameApplyPatch), Write, "Aplica um patch de diferenças num frame (só os pixels que mudam).";
    "frame_inspect" => FrameInspect(args::FrameInspect), Read, "Dados de um frame: caixa dos pixels visíveis, contagem, cores usadas (e se estão na paleta), semitransparência, buracos, anchors e regiões.";
    // paletas
    "palette_create" => PaletteCreate(args::PaletteCreate), Write, "Cria uma paleta (e a ativa). Índices da paleta ativa podem ser usados como cores em qualquer comando.";
    "palette_update" => PaletteUpdate(args::PaletteUpdate), Write, "Altera uma paleta: renomeia, troca/adiciona/remove cores, ativa ou apaga (não mexe nos pixels).";
    "palette_replace" => PaletteReplace(args::PaletteReplace), Write, "Troca uma cor por outra em todos os pixels do sprite (todas as layers e frames) e na paleta.";
    "palette_quantize" => PaletteQuantize(args::PaletteQuantize), Write, "Reduz as cores do sprite (median cut) ou as leva para uma paleta.";
    // importação e exportação
    "import_image" => ImportImage(args::ImportImage), Write, "Importa PNG/GIF/Aseprite como sprite novo, layer, frames ou referência (.aseprite como sprite novo mantém layers, tags e paleta); corta sprite sheets e cria animação com os frames. Com `pixelate`, converte a imagem em pixel art antes (como image_pixelate).";
    "image_pixelate" => ImagePixelate(args::ImagePixelate), Runtime, "Converte uma imagem PNG, JPG ou GIF de qualquer tamanho (foto, ilustração, pixel art ampliada) em pixel art estática, no tamanho pedido: remove o fundo, recorta (ao conteúdo ou numa região), reduz por blocos, calcula a paleta (ou usa uma fixa), com dithering, limpeza e contorno opcionais. Cria um projeto novo com o resultado; com `preview`, só devolve a imagem. Processamento determinístico, nada é gerado por IA.";
    "export_png" => ExportPng(args::ExportPng), Read, "Exporta um frame como PNG (com escala).";
    "export_spritesheet" => ExportSpritesheet(args::ExportSpritesheet), Read, "Exporta uma sprite sheet PNG (horizontal, vertical, grade, uma linha por trilha ou atlas compactado), com JSON de metadados.";
    "export_frames" => ExportFrames(args::ExportFrames), Read, "Exporta cada frame como um PNG numerado.";
    "export_gif" => ExportGif(args::ExportGif), Read, "Exporta uma animação (uma direção) como GIF animado.";
    "export_godot_metadata" => ExportGodot(args::ExportGodot), Read, "Exporta sheet PNG + SpriteFrames .tres do Godot 4 (uma animação por trilha, durações e loop).";
    // histórico e controle
    "history_undo" => HistoryUndo(args::HistoryStep), Runtime, "Desfaz passos do histórico.";
    "history_redo" => HistoryRedo(args::HistoryStep), Runtime, "Refaz passos desfeitos.";
    "transaction_begin" => TransactionBegin(args::TransactionBegin), Runtime, "Abre uma transação: os comandos seguintes viram um único passo no histórico e outros clientes esperam. Feche com commit ou rollback.";
    "transaction_commit" => TransactionCommit(args::Empty), Runtime, "Confirma a transação aberta.";
    "transaction_rollback" => TransactionRollback(args::Empty), Runtime, "Desfaz tudo o que foi feito desde transaction_begin.";
    "editor_status" => EditorStatus(args::Empty), Runtime, "Estado do runtime: projetos abertos, projeto atual, clientes conectados (editor, MCP), transação aberta.";
    "editor_get_capabilities" => EditorGetCapabilities(args::Empty), Runtime, "Limites, operações SDC disponíveis, formatos e versão do schema.";
    "rpgmaker_template" => RpgMakerTemplate(args::RpgMakerTemplate), Write, "Cria um sprite no tamanho exato de um arquivo do RPG Maker MZ/MV (tile 48) ou VX Ace (tile 32): tilesets A1–A5 e B–E, folha de personagens, faces, battler lateral ou Window.png, com uma região semântica nomeada para cada bloco (ex.: \"A2 bloco 3,1\", \"personagem 0 baixo\", \"andar\", \"moldura\"). Com `sprite`, aplica o modelo a um sprite existente do mesmo tamanho. Retorna o tamanho, a grade e as regiões.";
    "export_rpgmaker_character" => ExportRpgMakerCharacter(args::ExportRpgMakerCharacter), Read, "Grava a folha de personagem do RPG Maker (img/characters) a partir de animações: linhas baixo (S), esquerda (W), direita (E) e cima (N), 3 colunas por direção (o RPG Maker toca 0, 1, 2, 1; a do meio é a pose parada), cada frame com o pivô no centro da base da célula. Até 8 personagens (grade 12×8) ou um só com `single` (prefixo $); `object` põe o prefixo !. Devolve a imagem.";
    "export_rpgmaker_faces" => ExportRpgMakerFaces(args::ExportRpgMakerFaces), Read, "Grava uma imagem de faces do RPG Maker (img/faces): até 8 faces (frames) numa grade 4×2 de células 144×144 (96×96 no VX Ace), centralizadas, com escala inteira opcional. Devolve a imagem.";
    "export_rpgmaker_battler" => ExportRpgMakerBattler(args::ExportRpgMakerBattler), Read, "Grava um battler de visão lateral do RPG Maker MV/MZ (img/sv_actors): 9×6 células (64×64 por padrão) com os 18 movimentos (walk, wait, chant, guard, damage, evade, thrust, swing, missile, skill, spell, item, escape, victory, dying, abnormal, sleep, dead), 3 quadros cada, tirados de animações; o movimento i fica na coluna floor(i/6)*3 + quadro e na linha i%6. Devolve a imagem.";
    "tile_grid_set" => TileGridSet(args::TileGridSet), Write, "Transforma o sprite num tileset: define o tamanho do tile, a margem e o espaçamento da grade (tiles endereçados por [coluna, linha]). Também altera ou apaga a grade.";
    "tile_info" => TileInfo(args::TileInfo), Read, "Grade de tiles do sprite: colunas, linhas, tiles vazios (pela imagem composta do frame), colisões, terrenos e mapa de prévia.";
    "tile_collision_set" => TileCollisionSet(args::TileCollisionSet), Write, "Define a colisão de um ou mais tiles: polígonos em pixels do tile (origem no canto superior esquerdo) ou atalhos full, top_half, bottom_half, left_half, right_half; ou limpa.";
    "tile_terrain_set" => TileTerrainSet(args::TileTerrainSet), Write, "Define conjuntos de terreno (nome, modo e terrenos com cor) e, por tile, o terreno e os bits de vizinhança (lados e cantos), como os terrenos do Godot 4.";
    "autotile_generate" => AutotileGenerate(args::AutotileGenerate), Write, "Monta um autotile a partir de um bloco A2 do RPG Maker ou de uma moldura 3x3 com cantos internos: cria um sprite novo com o bloco A2 e/ou os 47 tiles do blob (com terrenos para o Godot), montados por quartos de tile.";
    "tile_preview_map_set" => TilePreviewMapSet(args::TilePreviewMapSet), Write, "Guarda um mapa pequeno de prévia (largura x altura em células, cada uma com um tile [coluna, linha] ou vazia); também pinta por terreno escolhendo os tiles pelos bits de vizinhança.";
    "tile_preview_render" => TilePreviewRender(args::TilePreviewRender), Read, "Imagem PNG do mapa de prévia montado com os tiles do sprite, para ver como eles se encaixam.";
    "export_godot_tileset" => ExportGodotTileset(args::ExportGodotTileset), Read, "Exporta o tileset para o Godot 4: PNG (imagem composta de um frame) + TileSet .tres com a grade, os tiles não vazios, colisões (camada de física 0) e terrenos.";
    "hitbox_create" => HitboxCreate(args::HitboxCreate), Write, "Cria uma caixa de jogo no sprite: hitbox (causa dano), hurtbox (recebe dano), collision ou custom. Opcionalmente já com um retângulo em frames escolhidos. Exportada para o Godot como Area2D + CollisionShape2D.";
    "hitbox_update" => HitboxUpdate(args::HitboxUpdate), Write, "Renomeia uma caixa, muda o tipo ou a cor, ou a apaga (`delete`).";
    "hitbox_set" => HitboxSet(args::HitboxSet), Write, "Define o retângulo de uma caixa (pixels do sprite) ou a desliga (`clear`) num frame, numa lista de frames, em todos os frames de uma animação/direção ou em todos os frames. Frame sem retângulo = caixa desligada nele.";
    "hitbox_list" => HitboxList(args::HitboxList), Read, "Lista as caixas (hitbox/hurtbox/collision) com tipo, cor e retângulos por frame (ou só os de um frame), com alertas de retângulos fora do canvas.";
    "export_godot_scene" => ExportGodotScene(args::ExportGodotScene), Read, "Exporta uma cena Godot 4 (.tscn) pronta: Node2D com AnimatedSprite2D (sheet PNG + SpriteFrames .tres gravados ao lado), um Area2D/CollisionShape2D por hitbox, um Marker2D por anchor e um AnimationPlayer com uma animação por trilha que troca os frames e move/liga/desliga as caixas e os pontos frame a frame. Posições relativas ao pivô.";
    "export_job_set" => ExportJobSet(args::ExportJobSet), Write, "Cria ou atualiza uma tarefa de exportação salva no projeto (um comando export_* com seus argumentos). Com `on_save`, roda sozinha depois de cada project_save. Caminhos relativos partem da pasta do .psmproj.";
    "export_job_delete" => ExportJobDelete(args::ExportJobDelete), Write, "Apaga uma tarefa de exportação.";
    "export_job_list" => ExportJobList(args::Empty), Read, "Lista as tarefas de exportação do projeto (comando, argumentos, se roda ao salvar) e a pasta base dos caminhos relativos.";
    "export_jobs_run" => ExportJobsRun(args::ExportJobsRun), Read, "Roda as tarefas de exportação (todas, as nomeadas ou só as de `on_save`) e devolve o resultado de cada uma; uma falha não impede as outras. É a exportação em lote (também: `psm export projeto.psmproj`).";
    "select_color" => SelectColor(args::SelectColor), Read, "Varinha mágica: seleciona os pixels parecidos com o de (x, y), contíguos ou na imagem toda (tolerância por canal, conectividade 4/8, layer ou imagem composta). Combina com a seleção `base` (replace, add, subtract, intersect) e devolve a máscara `bounds` + `rows` (`#` = selecionado).";
    "select_polygon" => SelectPolygon(args::SelectPolygon), Read, "Laço: seleciona o polígono pelos pontos (pode ser côncavo; bordas incluídas). Combina com a seleção `base` (replace, add, subtract, intersect) e devolve a máscara `bounds` + `rows` (`#` = selecionado).";
    "selection_read" => SelectionRead(args::SelectionRead), Read, "Lê só os pixels selecionados (máscara e/ou retângulo) de uma layer ou da imagem composta, como matriz pronta para colar com pixel_write_region: `.` = fora da seleção ou transparente (mantém o pixel ao colar).";
    "draw_text" => DrawText(args::DrawText), Write, "Escreve texto com uma fonte bitmap (\"pixel-5x7\", \"mini-3x5\" ou uma fonte feita num sprite): alinhamento, quebra por palavra (`max_width`), escala inteira, espaçamentos, contorno e sombra. Atalho para um draw_batch com a op `text`. Caracteres sem glyph viram um retângulo e entram nos avisos.";
    "text_measure" => TextMeasure(args::TextMeasure), Read, "Mede um texto sem desenhar: largura, altura e caixa (igual à do desenho), linhas com posição e largura, caixa dos pixels e caracteres sem glyph.";
    "font_define" => FontDefine(args::FontDefine), Write, "Marca um sprite como fonte bitmap: grade de células (`cell`, `columns`), caracteres na ordem das células (`charset`), linha de base, espaçamentos e altura de linha. O avanço de cada glyph é a largura real (colunas opacas) da célula. Devolve uma amostra.";
    "font_info" => FontInfo(args::FontInfo), Read, "Detalha uma fonte (embutida ou de sprite): métricas, caracteres, avanço de cada glyph, células vazias, caracteres padrão que faltam e uma amostra renderizada.";
    "font_template" => FontTemplate(args::FontTemplate), Write, "Cria um sprite NOVO com a grade de células de uma fonte (uma região por caractere, layer de guias), já definido como fonte e, opcionalmente, preenchido com uma fonte embutida para servir de ponto de partida.";
    "export_bitmap_font" => ExportBitmapFont(args::ExportBitmapFont), Read, "Exporta uma fonte (embutida ou de sprite) como BMFont texto da AngelCode: <nome>.fnt + atlas <nome>.png, que o Godot 4 importa como FontFile.";
    "nine_slice_set" => NineSliceSet(args::NineSliceSet), Write, "Define (ou remove) as margens de 9-slice do sprite (left, top, right, bottom em pixels); o centro precisa sobrar com pelo menos 1 px.";
    "nine_slice_render" => NineSliceRender(args::NineSliceRender), Read, "Prévia do 9-slice: renderiza o frame esticado para width×height (cantos intactos; bordas e centro esticados ou repetidos) e devolve a imagem.";
    "export_nine_slice" => ExportNineSlice(args::ExportNineSlice), Read, "Exporta o 9-slice para o Godot 4: PNG + cena .tscn com um NinePatchRect configurado (textura, patch_margin_*, axis_stretch_*) + JSON com as margens.";
    "palette_import" => PaletteImport(args::PaletteImport), Write, "Importa um arquivo de paleta como paleta do projeto (e a ativa): .gpl (GIMP), .hex (Lospec), .pal (JASC-PAL ou RIFF PAL), .txt (Paint.NET) ou imagem .png/.gif (cores únicas na ordem de leitura). Até 256 cores; um erro no arquivo diz a linha.";
    "palette_export" => PaletteExport(args::PaletteExport), Read, "Grava uma paleta (ou as cores de uma variante) em arquivo, no formato da extensão: .gpl, .hex, .pal (JASC-PAL), .txt (Paint.NET) ou imagem .png/.gif com uma amostra por cor.";
    "palette_variant_set" => PaletteVariantSet(args::PaletteVariantSet), Write, "Cria ou altera uma variante de paleta (recoloração: inimigo vermelho, time azul, versão noturna): a cor i da paleta base vira a cor i da variante. Não mexe nos pixels; veja com palette_variant_preview e exporte com export_palette_variants.";
    "palette_variant_delete" => PaletteVariantDelete(args::PaletteVariantDelete), Write, "Apaga uma variante de paleta.";
    "palette_variant_list" => PaletteVariantList(args::PaletteVariantList), Read, "Lista as variantes de paleta: base, cores, quantas cores mudam e se ainda casam com a paleta base.";
    "palette_variant_preview" => PaletteVariantPreview(args::PaletteVariantPreview), Read, "Imagem de um frame recolorido por uma variante (ou por cores de teste, sem salvar), opcionalmente ao lado do original. Troca exata de cor sobre a imagem composta; diz quantos pixels mudaram e quais cores não estão na base.";
    "export_palette_variants" => ExportPaletteVariants(args::ExportPaletteVariants), Read, "Exporta um arquivo por variante de paleta (padrão: todas): PNG de um frame, sprite sheet (mesmas opções e seleção de export_spritesheet, com JSON) ou GIF de uma animação. Recolore as imagens compostas por troca exata de cor (alfa preservado), sem mudar o projeto. Nomes: <prefixo>_<variante>.<ext>.";
    "export_normal_map" => ExportNormalMap(args::ExportNormalMap), Read, "Exporta o normal map PNG de um frame ou de uma sprite sheet (mesmo layout de export_spritesheet, casa pixel a pixel) para a iluminação 2D do Godot (CanvasTexture): altura pela borda (bevel), pelo brilho (luminance) ou mix, Sobel com `strength`, convenção OpenGL (ou DirectX com flip_y). Área plana = (128,128,255); alfa copiado.";
}

/// An open project.
#[derive(Debug, Clone)]
pub struct Session {
    pub doc: Document,
    pub path: Option<PathBuf>,
    pub saved_revision: u64,
    cache: CompositeCache,
}

/// Composited frames kept between renders. A pixel change recomposes only its rectangle;
/// structural changes drop the sprite's entries.
#[derive(Debug, Clone, Default)]
pub struct CompositeCache {
    frames: std::collections::HashMap<(u32, u32), sprite_core::Image>,
    pub hits: u64,
    pub partial_updates: u64,
    pub misses: u64,
}

impl Session {
    pub fn new(doc: Document, path: Option<PathBuf>) -> Self {
        let saved_revision = doc.revision();
        Session { doc, path, saved_revision, cache: CompositeCache::default() }
    }

    /// The composite of visible layers of a frame, from the cache when possible.
    pub fn composite(&mut self, sprite: u32, frame: u32) -> Option<sprite_core::Image> {
        let sp = self.doc.model.sprites.iter().find(|s| s.id == sprite)?;
        sp.frame(frame)?;
        if let Some(img) = self.cache.frames.get(&(sprite, frame)) {
            self.cache.hits += 1;
            return Some(img.clone());
        }
        self.cache.misses += 1;
        let img = sprite_animation::compose::compose_frame(sp, self.doc.images(), frame, &Default::default());
        self.cache.frames.insert((sprite, frame), img.clone());
        Some(img)
    }

    pub fn cache_stats(&self) -> (usize, u64, u64, u64) {
        (self.cache.frames.len(), self.cache.hits, self.cache.partial_updates, self.cache.misses)
    }

    /// Drops every cached composite (after undo, redo or a rollback).
    pub fn invalidate_cache(&mut self) {
        self.cache.frames.clear();
    }

    fn update_cache(&mut self, changes: &[Change]) {
        for c in changes {
            match (c.kind.as_str(), c.frame, c.rect) {
                ("pixels", Some(frame), Some(rect)) => {
                    let Some(sp) = self.doc.model.sprites.iter().find(|s| s.id == c.sprite) else { continue };
                    if let Some(img) = self.cache.frames.get_mut(&(c.sprite, frame)) {
                        sprite_animation::compose::recompose_region(sp, self.doc.images(), frame, &Default::default(), img, rect);
                        self.cache.partial_updates += 1;
                    }
                }
                ("pixels", Some(frame), None) => {
                    self.cache.frames.remove(&(c.sprite, frame));
                }
                _ => self.cache.frames.retain(|(s, _), _| *s != c.sprite),
            }
        }
    }

    pub fn dirty(&self) -> bool {
        self.doc.revision() != self.saved_revision || self.path.is_none()
    }

    /// Runs a project-level command (Read or Write kinds; Runtime kinds are rejected).
    pub fn execute(&mut self, cmd: Command, ctx: &ExecContext) -> Result<Outcome, CommandError> {
        match cmd.kind() {
            Kind::Runtime => Err(CommandError::new(ErrorCode::Unsupported, format!("{} é tratado pelo runtime", cmd.name()))),
            Kind::Read => {
                let mut out = ops::run(self, cmd, ctx)?;
                out.revision = self.doc.revision();
                out.transaction = self.doc.transaction_id();
                Ok(out)
            }
            Kind::Write => {
                let label = format!("{} ({})", ops::label(&cmd), ctx.client);
                if self.doc.in_transaction() {
                    self.doc.savepoint();
                    match ops::run(self, cmd, ctx) {
                        Ok(mut out) => {
                            self.doc.release_savepoint();
                            self.update_cache(&out.changes);
                            out.revision = self.doc.revision();
                            out.transaction = self.doc.transaction_id();
                            out.label = Some(label);
                            Ok(out)
                        }
                        Err(e) => {
                            self.doc.rollback_savepoint();
                            self.invalidate_cache();
                            Err(e)
                        }
                    }
                } else {
                    self.doc.begin(label.clone())?;
                    match ops::run(self, cmd, ctx) {
                        Ok(mut out) => {
                            let info = self.doc.commit()?;
                            self.update_cache(&out.changes);
                            out.revision = info.revision;
                            out.transaction = Some(info.transaction);
                            out.label = Some(label);
                            if info.is_empty() {
                                out.warnings.push("nada mudou".into());
                            }
                            Ok(out)
                        }
                        Err(e) => {
                            self.doc.rollback()?;
                            self.invalidate_cache();
                            Err(e)
                        }
                    }
                }
            }
        }
    }
}

/// Builds a new project session for `project_create`.
pub fn create_project(a: &ProjectCreate, ctx: &ExecContext) -> Result<Session, CommandError> {
    let max = ctx.limits.max_canvas_side;
    if a.width == 0 || a.height == 0 || a.width > max || a.height > max {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("tamanho {}x{} inválido: cada lado de 1 a {max} px", a.width, a.height),
        ));
    }
    if a.name.trim().is_empty() {
        return Err(CommandError::invalid("o projeto precisa de um nome"));
    }
    let mut doc = Document::new(a.name.trim());
    doc.begin("project_create")?;
    if !a.palette.is_empty() {
        if a.palette.len() > 256 {
            return Err(CommandError::invalid("uma paleta tem no máximo 256 cores"));
        }
        let name = a.palette_name.clone().unwrap_or_else(|| "Projeto".into());
        doc.model.palettes.insert(0, sprite_core::Palette::new(name.clone(), a.palette.clone()));
        doc.model.active_palette = Some(name);
    }
    let sprite_name = a.sprite_name.clone().unwrap_or_else(|| a.name.trim().to_string());
    let si = ops::structure::add_sprite(&mut doc, &sprite_name, a.width, a.height);
    if let Some(bg) = a.background {
        let (layer, frame) = (doc.model.sprites[si].layers[0].id, doc.model.sprites[si].frames[0].id);
        let image = doc.insert_image(sprite_core::Image::filled(a.width, a.height, bg));
        doc.model.sprites[si].cels.push(sprite_animation::Cel { layer, frame, image, opacity: 255 });
    }
    doc.commit()?;
    doc.reset_history();
    Ok(Session::new(doc, None))
}

/// Parses a command from a tool name and its JSON arguments.
pub fn parse_command(name: &str, args: serde_json::Value) -> Result<Command, CommandError> {
    let args = if args.is_null() { serde_json::json!({}) } else { args };
    serde_json::from_value(serde_json::json!({ "cmd": name, "args": args })).map_err(|e| {
        if specs().iter().any(|s| s.name == name) {
            CommandError::invalid(format!("argumentos inválidos para {name}: {e}"))
        } else {
            CommandError::not_found(format!("comando desconhecido: {name}"))
        }
    })
}

#[cfg(test)]
mod tests;
