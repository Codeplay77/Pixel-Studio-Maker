//! Argumentos dos comandos da integração com o Godot: hitboxes por frame, cena animada e
//! tarefas de exportação.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::extras::godot::HitboxKind;
use sprite_animation::sheet::SheetOptions;
use sprite_core::{Color, FrameRef, ItemRef, Rect};
use sprite_project::export::SheetSelection;

use super::SpriteTarget;

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
fn rows_sheet() -> SheetOptions {
    SheetOptions { layout: sprite_animation::sheet::SheetLayout::Rows, ..Default::default() }
}

/// Em quais frames a caixa muda. Combine à vontade; repetidos contam uma vez.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct HitboxFrames {
    /// Um frame: id, ou {"animation": "attack", "direction": "E", "index": 2}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Vários frames (ids ou posições em animações).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<FrameRef>,
    /// Todos os frames de uma animação (id ou nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    /// Com `animation`: só a trilha desta direção ("E", "NW"...). Sem ela, todas as trilhas da animação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Todos os frames do sprite.
    #[serde(default)]
    pub all_frames: bool,
}

impl HitboxFrames {
    pub fn is_empty(&self) -> bool {
        self.frame.is_none() && self.frames.is_empty() && self.animation.is_none() && !self.all_frames
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HitboxCreate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Nome da caixa (vira o nome do nó Area2D no Godot). Ex.: "golpe", "corpo", "pes".
    pub name: String,
    /// hitbox (causa dano), hurtbox (recebe dano), collision (corpo físico) ou custom.
    #[serde(default)]
    pub kind: HitboxKind,
    /// Cor da caixa no editor e na prévia (padrão: pela categoria).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Retângulo inicial em pixels do sprite ({x, y, w, h}); informe também onde (frame, frames, animation ou all_frames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    #[serde(flatten)]
    pub at: HitboxFrames,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HitboxUpdate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Caixa (id ou nome).
    pub hitbox: ItemRef,
    /// Novo nome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<HitboxKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Apagar a caixa (com os retângulos de todos os frames).
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HitboxSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Caixa (id ou nome).
    pub hitbox: ItemRef,
    /// Retângulo em pixels do sprite ({x, y, w, h}) para os frames escolhidos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Desligar a caixa nos frames escolhidos (apaga o retângulo deles).
    #[serde(default)]
    pub clear: bool,
    #[serde(flatten)]
    pub at: HitboxFrames,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct HitboxList {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Só os retângulos deste frame (padrão: todos os frames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Só esta caixa.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<ItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportGodotScene {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .tscn da cena. Ao lado ficam a sheet .png e o SpriteFrames .tres com o mesmo nome.
    pub path: String,
    /// Pasta desses arquivos dentro do projeto Godot, ex.: "res://personagens/heroi". Padrão: se o .tscn for gravado dentro de um projeto Godot (pasta com project.godot acima), o caminho res:// dessa pasta; senão "res://".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_dir: Option<String>,
    /// Layout da sheet (padrão: uma linha por trilha).
    #[serde(default = "rows_sheet")]
    pub sheet: SheetOptions,
    #[serde(default)]
    pub selection: SheetSelection,
    /// Escala inteira (posições e tamanhos das caixas acompanham).
    #[serde(default = "one")]
    pub scale: u32,
    /// O AnimationPlayer começa tocando a primeira animação.
    #[serde(default = "yes")]
    pub autoplay: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportJobSet {
    /// Nome da tarefa (cria ou atualiza a tarefa com este nome).
    pub name: String,
    /// Comando de exportação: export_png, export_spritesheet, export_frames, export_gif, export_godot_metadata, export_godot_scene... Obrigatório ao criar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Argumentos do comando, como na chamada direta. Caminhos relativos partem da pasta do .psmproj.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Map<String, serde_json::Value>>,
    /// Rodar sozinha depois de cada project_save (padrão ao criar: false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_save: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportJobDelete {
    /// Nome da tarefa.
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExportJobsRun {
    /// Tarefas a rodar, na ordem dada (padrão: todas, na ordem do projeto).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<String>,
    /// Só as tarefas marcadas com `on_save` (é o que o salvamento roda).
    #[serde(default)]
    pub on_save: bool,
}
