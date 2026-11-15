//! Argumentos dos comandos de tiles: grade, colisões, terrenos, autotile, prévia de mapa e
//! exportação de TileSet do Godot 4.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, FrameRef, ItemRef, Point};
use sprite_drawing::autotile::{AutotileInput, AutotileOutput};
use sprite_drawing::tiles::{CollisionShape, PeeringBit, TerrainMode};

use super::{Background, SpriteTarget, one};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TileGridSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Largura do tile em pixels. Obrigatória na primeira vez; depois, omita para manter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_width: Option<u32>,
    /// Altura do tile em pixels (padrão: igual à largura).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_height: Option<u32>,
    /// Margem [x, y]: pixels antes do primeiro tile, à esquerda e no topo (padrão [0, 0]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin: Option<[u32; 2]>,
    /// Espaçamento [x, y] entre tiles vizinhos (padrão [0, 0]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<[u32; 2]>,
    /// Apaga a grade e todos os dados de tiles do sprite (colisões, terrenos, mapa de prévia).
    #[serde(default)]
    pub remove: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TileInfo {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Frame usado para achar os tiles vazios (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TileCollisionSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Um tile [coluna, linha].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<[u32; 2]>,
    /// Vários tiles [[coluna, linha], ...] que recebem a mesma colisão.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<[u32; 2]>,
    /// Atalho: `full` (tile inteiro), `top_half`, `bottom_half`, `left_half` ou `right_half`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<CollisionShape>,
    /// Polígonos em pixels do tile: listas de vértices [x, y] nos cantos dos pixels, origem no canto
    /// superior esquerdo do tile, x de 0 a largura e y de 0 a altura. Ex.: rampa num tile 16x16:
    /// [[0,16],[16,0],[16,16]].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polygons: Vec<Vec<Point>>,
    /// Soma aos polígonos que o tile já tem (padrão: substitui).
    #[serde(default)]
    pub add: bool,
    /// Remove a colisão dos tiles.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TileTerrainDef {
    /// Nome do terreno (ex.: "grama", "água").
    pub name: String,
    /// Cor que o Godot usa para mostrar o terreno no editor (padrão: uma cor distinta).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TileTerrainSetDef {
    /// Nome do conjunto. Se já existir, ele é substituído (mantém a posição).
    pub name: String,
    /// `match_corners_and_sides` (8 vizinhos, blob de 47), `match_corners` (só cantos) ou
    /// `match_sides` (só lados).
    #[serde(default)]
    pub mode: TerrainMode,
    /// Terrenos do conjunto, na ordem (o índice de cada um é a posição na lista).
    #[serde(default)]
    pub terrains: Vec<TileTerrainDef>,
}

/// Terreno de um lado/canto específico (para transições entre terrenos). Valores: nome ou índice
/// do terreno no conjunto.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TilePeeringRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_side: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_right_corner: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_side: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_left_corner: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_side: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_left_corner: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_side: Option<ItemRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_right_corner: Option<ItemRef>,
}

impl TilePeeringRefs {
    pub fn iter(&self) -> impl Iterator<Item = (PeeringBit, &ItemRef)> {
        use PeeringBit as B;
        [
            (B::RightSide, &self.right_side),
            (B::BottomRightCorner, &self.bottom_right_corner),
            (B::BottomSide, &self.bottom_side),
            (B::BottomLeftCorner, &self.bottom_left_corner),
            (B::LeftSide, &self.left_side),
            (B::TopLeftCorner, &self.top_left_corner),
            (B::TopSide, &self.top_side),
            (B::TopRightCorner, &self.top_right_corner),
        ]
        .into_iter()
        .filter_map(|(b, r)| r.as_ref().map(|r| (b, r)))
    }
}

/// Terreno de um ou mais tiles. Cada entrada substitui o que o tile tinha.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TileTerrainArgs {
    /// Tile [coluna, linha].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<[u32; 2]>,
    /// Vários tiles com os mesmos dados.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<[u32; 2]>,
    /// Conjunto de terreno (nome ou índice; padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<ItemRef>,
    /// Terreno do centro do tile (nome ou índice no conjunto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<ItemRef>,
    /// Lados e cantos cujo vizinho é do mesmo terreno do tile: right_side, bottom_right_corner,
    /// bottom_side, bottom_left_corner, left_side, top_left_corner, top_side, top_right_corner.
    /// No modo match_sides só valem os lados; em match_corners, só os cantos.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bits: Vec<PeeringBit>,
    /// Lados e cantos com um terreno diferente do centro (transições), ex.: {"top_side": "água"}.
    #[serde(default)]
    pub bit_terrains: TilePeeringRefs,
    /// Remove o terreno destes tiles.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TileTerrainSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Cria ou substitui conjuntos de terreno (pelo nome).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sets: Vec<TileTerrainSetDef>,
    /// Apaga conjuntos (nome ou índice); os tiles que os usavam perdem o terreno.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_sets: Vec<ItemRef>,
    /// Terreno e bits de vizinhança por tile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<TileTerrainArgs>,
}

fn default_outputs() -> Vec<AutotileOutput> {
    vec![AutotileOutput::Blob47]
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AutotileGenerate {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Frame lido (imagem composta; padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Formato da entrada. `rpgmaker_a2`: bloco de 2x3 tiles, (0,0) = ícone, (1,0) = os 4 cantos
    /// internos (cada quarto de tile é o canto interno da sua posição), linhas 1–2 = moldura em 4x4
    /// quartos (cantos externos nos cantos, bordas entre eles, interior no meio). `frame_3x3`: área de
    /// 4x3 tiles, colunas 0–2 = moldura 3x3 (cantos, bordas, centro) e (3,0) = cantos internos.
    pub input: AutotileInput,
    /// Tile [coluna, linha] do canto superior esquerdo da área de entrada (padrão [0, 0]).
    #[serde(default)]
    pub source: [u32; 2],
    /// Saídas: `rpgmaker_a2` (bloco 2x3) e/ou `blob_47` (47 tiles em linhas de 8, por máscara de
    /// vizinhos crescente, com terrenos para o Godot). Padrão: ["blob_47"].
    #[serde(default = "default_outputs")]
    pub output: Vec<AutotileOutput>,
    /// Nome do sprite novo (padrão: "<sprite>_autotile").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Se já existir um sprite com esse nome, troca o conteúdo dele (mesmo id).
    #[serde(default)]
    pub replace: bool,
    /// Nome do terreno registrado no sprite novo (padrão: o nome do sprite de origem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_name: Option<String>,
    /// Cor do terreno (padrão: a cor mais comum do interior).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_color: Option<Color>,
}

/// Uma célula do mapa de prévia.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TileMapCell {
    /// Posição no mapa [x, y], em células.
    pub at: [u32; 2],
    /// Tile [coluna, linha] mostrado, ou null para esvaziar.
    #[serde(default)]
    pub tile: Option<[u32; 2]>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TilePreviewMapSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Tamanho do mapa em células [largura, altura] (até 64x64). Obrigatório na primeira vez; ao
    /// mudar, o que cabe é mantido.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    /// O mapa inteiro, linha por linha (largura × altura itens): [coluna, linha] do tile ou null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<Option<[u32; 2]>>>,
    /// Preenche todas as células com este tile (antes de `paint`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u32; 2]>,
    /// Esvazia todas as células (antes de `fill` e `paint`).
    #[serde(default)]
    pub clear: bool,
    /// Células avulsas: [{"at": [x, y], "tile": [coluna, linha] ou null}].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paint: Vec<TileMapCell>,
    /// Pinta por terreno: uma string por linha do mapa, '#' (ou qualquer caractere fora '.' e ' ')
    /// = terreno, '.' = vazio. Cada célula de terreno recebe o tile cujos bits de vizinhança mais
    /// combinam com as células vizinhas (precisa de terrenos nos tiles, ex.: saída de
    /// autotile_generate). Sem `size`, o tamanho vem das linhas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrain_rows: Vec<String>,
    /// Terreno usado por `terrain_rows` (nome ou índice; padrão: o primeiro do conjunto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<ItemRef>,
    /// Conjunto de terreno usado por `terrain_rows` (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_set: Option<ItemRef>,
    /// Apaga o mapa de prévia.
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TilePreviewRender {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Frame de onde vêm os pixels dos tiles (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Ampliação (padrão: automática, ~512 px; 1 = tamanho real).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default)]
    pub background: Background,
    /// Desenhar as divisas entre células.
    #[serde(default)]
    pub grid: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportGodotTileset {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .tres (TileSet) de saída; o PNG é gravado ao lado com o mesmo nome.
    pub path: String,
    /// Caminho da textura dentro do projeto Godot (padrão: "res://<nome>.png").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_path: Option<String>,
    /// Frame exportado (imagem composta; padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Escala do PNG (vizinho mais próximo); tamanhos, margens e colisões acompanham.
    #[serde(default = "one")]
    pub scale: u32,
}
