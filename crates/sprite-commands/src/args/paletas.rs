//! Argumentos das ferramentas de paleta: arquivos de paleta, variantes de paleta e normal map.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::sheet::SheetOptions;
use sprite_core::{Color, FrameRef, ItemRef};
use sprite_drawing::normal_map::HeightMode;
use sprite_project::export::SheetSelection;

use super::{Background, PaletteSet, SpriteTarget};

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}
fn eight() -> u32 {
    8
}
fn two() -> u32 {
    2
}
fn unit() -> f64 {
    1.0
}

// ---- arquivos de paleta ---------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaletteImport {
    /// Arquivo de paleta; o formato vem da extensão: .gpl (GIMP), .hex (Lospec), .pal (JASC-PAL ou
    /// RIFF PAL), .txt (Paint.NET) ou imagem .png/.gif (cores únicas na ordem de leitura, sem as
    /// transparentes). Máximo de 256 cores.
    pub path: String,
    /// Nome da paleta no projeto (padrão: o nome gravado no arquivo ou o nome do arquivo; se já
    /// existir, ganha um número).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tornar a paleta ativa (padrão: sim).
    #[serde(default = "yes")]
    pub activate: bool,
    /// Se já existir uma paleta com o `name` pedido, trocar as cores dela em vez de dar erro.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PaletteExport {
    /// Arquivo de saída; o formato vem da extensão: .gpl, .hex, .pal (JASC-PAL), .txt (Paint.NET),
    /// .png ou .gif (uma faixa de amostras).
    pub path: String,
    /// Paleta (padrão: a ativa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Exportar as cores de uma variante de paleta em vez de uma paleta.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Imagens: lado de cada amostra em px (1 a 256, padrão 8).
    #[serde(default = "eight")]
    pub swatch: u32,
    /// Imagens: amostras por linha (padrão: todas numa linha só).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
}

// ---- variantes de paleta --------------------------------------------------------------------

/// Troca por cor: a cor `from` da paleta base passa a ser `to` na variante.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VariantColorSwap {
    /// Cor da paleta base.
    pub from: Color,
    /// Cor nova na variante.
    pub to: Color,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteVariantSet {
    /// Nome da variante (cria se não existir, senão altera).
    pub name: String,
    /// Paleta base. Na criação, padrão: a do sprite, senão a ativa; a variante começa com as cores
    /// dela. Trocar a base de uma variante existente exige uma base com o mesmo número de cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Lista completa das cores da variante, do mesmo tamanho da base (a cor i da base vira a cor
    /// i daqui).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<Color>>,
    /// Trocas por índice: [{"index": 3, "color": "#d23939"}].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<PaletteSet>,
    /// Trocas por cor da base: [{"from": "#3976d2", "to": "#d23939"}].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replace: Vec<VariantColorSwap>,
    /// Novo nome da variante.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteVariantDelete {
    /// Nome da variante.
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteVariantList {
    /// Só as variantes desta paleta base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PaletteVariantPreview {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Variante salva no projeto. Sem ela, use `colors` (e `base`) para testar cores sem salvar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Cores de teste (mesmo tamanho da base), quando não há `variant`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<Color>>,
    /// Paleta base das cores de teste (padrão: a do sprite, senão a ativa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Frame (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Ampliação (padrão: automática, ~512 px).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default)]
    pub background: Background,
    /// Mostrar o original à esquerda e a variante à direita.
    #[serde(default)]
    pub compare: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VariantExportFormat {
    /// Um PNG de um frame (`frame`).
    Png,
    /// Uma sprite sheet PNG (+ JSON) com as mesmas opções de export_spritesheet.
    #[default]
    Spritesheet,
    /// Um GIF de uma animação (`animation`, `direction`).
    Gif,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExportPaletteVariants {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Pasta de saída. Cada arquivo se chama `<prefixo>_<variante>.<ext>`.
    pub directory: String,
    /// Variantes a exportar (padrão: todas).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// png (um frame), spritesheet (padrão) ou gif (uma animação).
    #[serde(default)]
    pub format: VariantExportFormat,
    /// Prefixo dos arquivos (padrão: o nome do sprite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Também exportar o original, com o nome da paleta base no lugar da variante.
    #[serde(default)]
    pub include_original: bool,
    /// png: o frame (padrão: o primeiro).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// spritesheet: layout (como em export_spritesheet).
    #[serde(default)]
    pub sheet: SheetOptions,
    /// spritesheet: quais animações/direções entram (como em export_spritesheet).
    #[serde(default)]
    pub selection: SheetSelection,
    /// spritesheet: gravar também o JSON de metadados de cada sheet.
    #[serde(default = "yes")]
    pub json: bool,
    /// spritesheet: cor de fundo (padrão transparente; nunca é recolorida).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    /// gif: a animação.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<ItemRef>,
    /// gif: a direção (omita em animações sem direção).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default = "one")]
    pub scale: u32,
}

// ---- normal map -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportNormalMap {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png de saída (RGB = normal, alfa copiado do sprite).
    pub path: String,
    /// Como calcular a altura: bevel (padrão: distância até a borda), luminance (brilho) ou mix.
    #[serde(default)]
    pub mode: HeightMode,
    /// Largura do arredondamento das bordas em px, para bevel/mix (1 a 64, padrão 2).
    #[serde(default = "two")]
    pub bevel: u32,
    /// Intensidade do relevo (padrão 1; 0 = plano; 2 ou 3 = mais marcado).
    #[serde(default = "unit")]
    pub strength: f64,
    /// Convenção DirectX (verde para baixo). Padrão: OpenGL (verde para cima), a do Godot.
    #[serde(default)]
    pub flip_y: bool,
    /// Frame (padrão: o primeiro). Ignorado quando há `sheet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Gerar no layout de uma sprite sheet (mesmas opções de export_spritesheet), para casar pixel
    /// a pixel com a sheet exportada com as mesmas opções.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<SheetOptions>,
    /// Com `sheet`: quais animações/direções entram (como em export_spritesheet).
    #[serde(default)]
    pub selection: SheetSelection,
    #[serde(default = "one")]
    pub scale: u32,
}
