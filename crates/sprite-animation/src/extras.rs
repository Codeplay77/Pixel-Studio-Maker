//! Dados que as ferramentas de jogo guardam em cada sprite (tiles, hitboxes, modelos do RPG Maker,
//! fontes, 9-slice...). Cada área cuida do seu campo, sempre com valor padrão, para que projetos
//! antigos continuem abrindo.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod godot;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpriteExtras {
    /// Modelo de arquivo do RPG Maker (tileset, personagem, faces, battler, window) criado por `rpgmaker_template`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rpgmaker: Option<crate::rpgmaker::RpgMakerInfo>,
    /// Grade de tiles, colisões, terrenos e mapa de prévia (quando o sprite é um tileset).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiles: Option<sprite_drawing::tiles::TileData>,
    /// Hitboxes, hurtboxes e caixas de colisão por frame (exportadas para o Godot).
    #[serde(default, skip_serializing_if = "godot::GodotSprite::is_empty")]
    pub godot: godot::GodotSprite,
    /// Fonte bitmap definida neste sprite (font_define) e margens de 9-slice (nine_slice_set).
    #[serde(default, skip_serializing_if = "sprite_drawing::text::TextExtras::is_empty")]
    pub texto: sprite_drawing::text::TextExtras,
}

impl SpriteExtras {
    pub fn is_empty(&self) -> bool {
        *self == SpriteExtras::default()
    }
}
