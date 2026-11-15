//! Arguments of the drawing-tool commands: magic wand, lasso and reading a selection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Point, Rect};
use sprite_drawing::fill::Connectivity;
use sprite_drawing::select::{SelectMode, SelectionMask};

use super::{SpriteTarget, Target};

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectColor {
    #[serde(flatten)]
    pub target: Target,
    /// Coluna do pixel clicado.
    pub x: i32,
    /// Linha do pixel clicado.
    pub y: i32,
    /// `true` (padrão): só os pixels parecidos ligados ao clicado; `false`: todos os pixels parecidos da imagem.
    #[serde(default = "yes")]
    pub contiguous: bool,
    /// Diferença máxima por canal (0-255) para um pixel contar como parecido (padrão 0 = mesma cor).
    #[serde(default)]
    pub tolerance: u8,
    /// Vizinhos ligados no modo contíguo: 4 (padrão) ou 8 (inclui diagonais).
    #[serde(default)]
    pub connectivity: Connectivity,
    /// Amostrar a imagem composta do frame (todas as layers visíveis) em vez da layer.
    #[serde(default)]
    pub composite: bool,
    /// Seleção atual (`bounds` + `rows`), combinada com a nova conforme `mode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<SelectionMask>,
    /// Como combinar com `base`: replace (padrão), add, subtract ou intersect.
    #[serde(default)]
    pub mode: SelectMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SelectPolygon {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Vértices do laço, em ordem ([[x,y], ...]); o último liga no primeiro. Regra par-ímpar, bordas incluídas.
    pub points: Vec<Point>,
    /// Seleção atual (`bounds` + `rows`), combinada com a nova conforme `mode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<SelectionMask>,
    /// Como combinar com `base`: replace (padrão), add, subtract ou intersect.
    #[serde(default)]
    pub mode: SelectMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SelectionRead {
    #[serde(flatten)]
    pub target: Target,
    /// Máscara da seleção (`bounds` + `rows`, como select_color e select_polygon devolvem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<SelectionMask>,
    /// Retângulo que limita a leitura (sozinho ou junto com `mask`). Sem os dois: a cel inteira.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rect: Option<Rect>,
    /// Ler a imagem composta do frame em vez da layer.
    #[serde(default)]
    pub composite: bool,
}
