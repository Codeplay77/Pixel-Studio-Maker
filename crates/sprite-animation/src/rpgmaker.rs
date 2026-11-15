//! RPG Maker metadata stored in a sprite: which RPG Maker file the sprite models (tileset, character
//! sheet, faces, side-view battler, window skin) and for which engine. The geometry of each file
//! lives in `sprite_project::rpgmaker`; the commands live in `sprite_commands` (`rpgmaker_*`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Versão do RPG Maker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RpgMakerEngine {
    /// RPG Maker MZ (tiles de 48 px).
    #[default]
    Mz,
    /// RPG Maker MV (tiles de 48 px; mesmos formatos de imagem do MZ).
    Mv,
    /// RPG Maker VX Ace (tiles de 32 px).
    Vxace,
}

impl RpgMakerEngine {
    /// Tile size in pixels.
    pub fn tile(self) -> u32 {
        match self {
            RpgMakerEngine::Mz | RpgMakerEngine::Mv => 48,
            RpgMakerEngine::Vxace => 32,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RpgMakerEngine::Mz => "RPG Maker MZ",
            RpgMakerEngine::Mv => "RPG Maker MV",
            RpgMakerEngine::Vxace => "RPG Maker VX Ace",
        }
    }

    /// Engine id as written in JSON (`mz`, `mv`, `vxace`).
    pub fn id(self) -> &'static str {
        match self {
            RpgMakerEngine::Mz => "mz",
            RpgMakerEngine::Mv => "mv",
            RpgMakerEngine::Vxace => "vxace",
        }
    }
}

/// Tipo de arquivo de imagem do RPG Maker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RpgMakerKind {
    /// Tileset A1: água animada, cascatas e decorações sobre a água (autotiles em blocos de 2×3 tiles, 8×4 blocos).
    TilesetA1,
    /// Tileset A2: chão (autotiles em blocos de 2×3 tiles, 8×4 blocos).
    TilesetA2,
    /// Tileset A3: construções, telhados e paredes alternando por linha (blocos de 2×2 tiles, 8×4 blocos).
    TilesetA3,
    /// Tileset A4: paredes; 3 pares de linhas: topo (blocos 2×3 tiles) e face (blocos 2×2 tiles), 8 colunas.
    TilesetA4,
    /// Tileset A5: tiles comuns, 8×16 tiles.
    TilesetA5,
    /// Tileset B: objetos, 16×16 tiles (duas metades de 8×16).
    TilesetB,
    /// Tileset C: objetos, 16×16 tiles (duas metades de 8×16).
    TilesetC,
    /// Tileset D: objetos, 16×16 tiles (duas metades de 8×16).
    TilesetD,
    /// Tileset E: objetos, 16×16 tiles (duas metades de 8×16).
    TilesetE,
    /// Folha de personagens (img/characters): 4×2 personagens de 3×4 células, ou um só com `single` (prefixo `$`).
    Character,
    /// Faces (img/faces): 4×2 faces de 144×144 (96×96 no VX Ace).
    Face,
    /// Battler de visão lateral (img/sv_actors, só MV/MZ): 9×6 células de 64×64, 18 movimentos.
    SvBattler,
    /// Window.png (img/system): fundo, moldura, setas, cursor, sinal de pausa e as 32 cores do texto.
    Window,
}

impl RpgMakerKind {
    pub const ALL: [RpgMakerKind; 13] = [
        RpgMakerKind::TilesetA1,
        RpgMakerKind::TilesetA2,
        RpgMakerKind::TilesetA3,
        RpgMakerKind::TilesetA4,
        RpgMakerKind::TilesetA5,
        RpgMakerKind::TilesetB,
        RpgMakerKind::TilesetC,
        RpgMakerKind::TilesetD,
        RpgMakerKind::TilesetE,
        RpgMakerKind::Character,
        RpgMakerKind::Face,
        RpgMakerKind::SvBattler,
        RpgMakerKind::Window,
    ];

    /// Kind id as written in JSON (`tileset_a1`, `character`...).
    pub fn id(self) -> &'static str {
        match self {
            RpgMakerKind::TilesetA1 => "tileset_a1",
            RpgMakerKind::TilesetA2 => "tileset_a2",
            RpgMakerKind::TilesetA3 => "tileset_a3",
            RpgMakerKind::TilesetA4 => "tileset_a4",
            RpgMakerKind::TilesetA5 => "tileset_a5",
            RpgMakerKind::TilesetB => "tileset_b",
            RpgMakerKind::TilesetC => "tileset_c",
            RpgMakerKind::TilesetD => "tileset_d",
            RpgMakerKind::TilesetE => "tileset_e",
            RpgMakerKind::Character => "character",
            RpgMakerKind::Face => "face",
            RpgMakerKind::SvBattler => "sv_battler",
            RpgMakerKind::Window => "window",
        }
    }

    /// Default sprite name for a new template sprite (Portuguese, shown in the editor).
    pub fn default_name(self) -> &'static str {
        match self {
            RpgMakerKind::TilesetA1 => "Tileset A1",
            RpgMakerKind::TilesetA2 => "Tileset A2",
            RpgMakerKind::TilesetA3 => "Tileset A3",
            RpgMakerKind::TilesetA4 => "Tileset A4",
            RpgMakerKind::TilesetA5 => "Tileset A5",
            RpgMakerKind::TilesetB => "Tileset B",
            RpgMakerKind::TilesetC => "Tileset C",
            RpgMakerKind::TilesetD => "Tileset D",
            RpgMakerKind::TilesetE => "Tileset E",
            RpgMakerKind::Character => "Personagens",
            RpgMakerKind::Face => "Faces",
            RpgMakerKind::SvBattler => "Battler lateral",
            RpgMakerKind::Window => "Window",
        }
    }
}

/// Which RPG Maker file a sprite models. Written by `rpgmaker_template`; read by the editor and
/// by the RPG Maker exports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RpgMakerInfo {
    pub kind: RpgMakerKind,
    #[serde(default)]
    pub engine: RpgMakerEngine,
    /// Tile size in pixels (48 on MV/MZ, 32 on VX Ace).
    pub tile: u32,
    /// Cell size [w, h] of character, face and battler sheets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Character sheet holding a single character (`$` file prefix).
    #[serde(default, skip_serializing_if = "is_false")]
    pub single: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_serde_names() {
        for k in RpgMakerKind::ALL {
            assert_eq!(serde_json::to_value(k).unwrap(), serde_json::json!(k.id()));
        }
        for e in [RpgMakerEngine::Mz, RpgMakerEngine::Mv, RpgMakerEngine::Vxace] {
            assert_eq!(serde_json::to_value(e).unwrap(), serde_json::json!(e.id()));
        }
        assert_eq!(RpgMakerEngine::default().tile(), 48);
        assert_eq!(RpgMakerEngine::Vxace.tile(), 32);
    }

    #[test]
    fn info_round_trips_with_defaults() {
        let info: RpgMakerInfo = serde_json::from_value(serde_json::json!({ "kind": "tileset_a2", "tile": 48 })).unwrap();
        assert_eq!(info.engine, RpgMakerEngine::Mz);
        assert!(!info.single);
        let text = serde_json::to_string(&info).unwrap();
        assert!(!text.contains("single") && !text.contains("cell"), "{text}");
    }
}
