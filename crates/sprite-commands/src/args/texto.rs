//! Arguments of the text tools: text drawing and measuring, bitmap fonts and 9-slice.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{FrameRef, ItemRef};
use sprite_drawing::nine_slice::{NineSlice, SliceMode};
use sprite_drawing::sdc::ColorRef;
use sprite_drawing::text::TextSpec;

use super::{Background, SpriteTarget, Target};

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
fn sixteen() -> u32 {
    16
}

// ---- texto ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DrawText {
    #[serde(flatten)]
    pub target: Target,
    #[serde(flatten)]
    pub spec: TextSpec,
    /// Só pinta dentro desta região semântica (id ou nome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<ItemRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TextMeasure {
    /// Os mesmos campos de draw_text (`color` é ignorado). A medida considera fonte, escala, espaçamentos,
    /// quebra por `max_width`, contorno e sombra, exatamente como o desenho.
    #[serde(flatten)]
    pub spec: TextSpec,
}

// ---- fontes ---------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FontDefine {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Tamanho de cada célula [largura, altura] em pixels. Campos omitidos mantêm o valor atual quando o
    /// sprite já é uma fonte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Os caracteres na ordem das células, linha a linha, a partir do canto superior esquerdo (ex.: " !\"#...").
    /// O espaço pode ocupar uma célula vazia; células vazias de outros caracteres ficam fora da fonte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Células por linha (padrão: quantas cabem na largura do sprite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    /// Linhas do topo da célula até a linha de base, onde as letras sem descida terminam (padrão: a altura da
    /// célula).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<u32>,
    /// Espaço entre letras em pixels (padrão 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<i32>,
    /// Avanço do espaço em pixels (padrão: metade da largura da célula).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_width: Option<u32>,
    /// Distância entre o topo de duas linhas de texto (padrão: a altura da célula).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<u32>,
    /// Frame que guarda os glyphs (padrão: o primeiro da linha do tempo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Deixa de ser fonte (apaga a definição; os pixels ficam).
    #[serde(default)]
    pub clear: bool,
    /// Texto da amostra devolvida como imagem (padrão: todos os caracteres da fonte).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FontInfo {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Fonte: "pixel-5x7", "mini-3x5" ou um sprite (id/nome) definido como fonte. Padrão: o sprite de `sprite`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<ItemRef>,
    /// Texto da amostra devolvida como imagem (padrão: todos os caracteres da fonte).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample: Option<String>,
    /// Ampliação da amostra (padrão: automática).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// Incluir o avanço (largura) de cada glyph.
    #[serde(default = "yes")]
    pub widths: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontTemplate {
    /// Nome do sprite novo (padrão "Fonte").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Fonte embutida ("pixel-5x7" ou "mini-3x5") para já preencher as células como ponto de partida; também
    /// dá as métricas padrão (célula, linha de base, espaçamentos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_font: Option<String>,
    /// Tamanho de cada célula [largura, altura] (padrão: o da fonte base, ou o da pixel-5x7: [6, 12]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Caracteres, na ordem das células (padrão: ASCII 32–126 + letras acentuadas do português e do espanhol
    /// + ¡ ¿ º ª °).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    /// Células por linha (padrão 16).
    #[serde(default = "sixteen")]
    pub columns: u32,
    /// Cor dos glyphs do preenchimento (padrão branco, que engines tingem com facilidade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
    /// Linha de base (padrão: a da fonte base).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<u32>,
    /// Criar uma região por caractere (nome "glifo X").
    #[serde(default = "yes")]
    pub regions: bool,
    /// Criar uma layer de referência "guias" (não exportada) com as células e a linha de base.
    #[serde(default = "yes")]
    pub guides: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportBitmapFont {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Fonte: "pixel-5x7", "mini-3x5" ou um sprite (id/nome) definido como fonte. Padrão: o sprite de `sprite`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<ItemRef>,
    /// Arquivo .fnt (BMFont texto) de saída; o atlas .png é gravado ao lado com o mesmo nome.
    pub path: String,
    /// Nome da fonte no arquivo (padrão: o nome da fonte).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<String>,
    /// Ampliação inteira dos glyphs e das métricas (1 a 16).
    #[serde(default = "one")]
    pub scale: u32,
    /// Cor dos glyphs no atlas. Padrão: branco nas fontes embutidas; as fontes de sprite mantêm as cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
    /// Pixels entre os glyphs no atlas (0 a 16, padrão 1).
    #[serde(default = "one")]
    pub spacing: u32,
}

// ---- 9-slice --------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NineSliceSet {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// As quatro margens com o mesmo valor (os campos individuais têm prioridade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<u32>,
    /// Margem esquerda em pixels. Margens omitidas mantêm o valor atual (ou 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<u32>,
    /// Remove o 9-slice do sprite.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NineSliceRender {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Frame de origem (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Largura final em pixels.
    pub width: u32,
    /// Altura final em pixels.
    pub height: u32,
    /// Como as bordas preenchem o espaço: "stretch" (padrão), "tile" ou "tile_fit".
    #[serde(default)]
    pub edges: SliceMode,
    /// Como o centro preenche o espaço (padrão: o mesmo de `edges`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<SliceMode>,
    /// Margens a usar no lugar das salvas (para testar antes de nine_slice_set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margins: Option<NineSlice>,
    /// Ampliação da prévia (padrão: automática).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default)]
    pub background: Background,
    /// Desenhar as linhas das margens na prévia.
    #[serde(default)]
    pub guides: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportNineSlice {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .tscn (cena do Godot 4) de saída; o .png e o .json são gravados ao lado com o mesmo nome.
    pub path: String,
    /// Frame exportado (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Caminho da textura dentro do projeto Godot (padrão: "res://<nome>.png").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub res_path: Option<String>,
    /// Ampliação inteira da textura e das margens (1 a 16).
    #[serde(default = "one")]
    pub scale: u32,
    /// Tamanho inicial do NinePatchRect na cena [largura, altura] (padrão: 3× a textura).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
    /// Eixo horizontal (bordas de cima/baixo e centro): "stretch" (0, padrão), "tile" (1) ou "tile_fit" (2).
    #[serde(default)]
    pub axis_stretch_horizontal: SliceMode,
    /// Eixo vertical (bordas laterais e centro): "stretch" (0, padrão), "tile" (1) ou "tile_fit" (2).
    #[serde(default)]
    pub axis_stretch_vertical: SliceMode,
    /// Nome do nó na cena (padrão: o nome do sprite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_name: Option<String>,
}
