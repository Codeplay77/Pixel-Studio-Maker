//! Text with bitmap fonts: layout (alignment, word wrap, integer scale), outline and shadow.
//!
//! Measuring and drawing share [`layout`], so the measured box is exactly where the pixels go:
//! nothing is drawn outside it.

mod builtin;
pub mod font;

use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image, ItemRef, Mask, Point, Rect};

pub use font::{
    BUILTIN_FONTS, BitmapFont, Glyph, SpriteFontDef, builtin_font, builtin_for, font_from_sheet, required_charset, standard_charset,
};

use crate::nine_slice::NineSlice;
use crate::sdc::ColorRef;
use crate::{Canvas, DrawError};

/// Longest text accepted by one operation.
pub const MAX_TEXT_CHARS: usize = 10_000;
/// Largest integer scale.
pub const MAX_TEXT_SCALE: u32 = 16;

/// Text data a sprite carries: its font definition and its 9-slice margins.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TextExtras {
    /// O sprite é uma fonte bitmap (font_define).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<SpriteFontDef>,
    /// Margens de 9-slice (nine_slice_set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nine_slice: Option<NineSlice>,
}

impl TextExtras {
    pub fn is_empty(&self) -> bool {
        self.font.is_none() && self.nine_slice.is_none()
    }
}

/// Horizontal alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

fn default_shadow_offset() -> Point {
    Point::new(1, 1)
}

/// A drop shadow under the text (and under its outline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextShadow {
    /// Cor da sombra.
    pub color: ColorRef,
    /// Deslocamento [dx, dy] em pixels (padrão [1, 1]).
    #[serde(default = "default_shadow_offset")]
    pub offset: Point,
}

fn default_font() -> ItemRef {
    ItemRef::Name("pixel-5x7".into())
}

fn one() -> u32 {
    1
}

/// Everything that describes a piece of text. Shared by the `text` SDC operation, `draw_text`
/// and `text_measure`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextSpec {
    /// O texto. "\n" quebra a linha; tabulação vira 4 espaços.
    pub text: String,
    /// Canto superior esquerdo da caixa do texto (a caixa inclui a zona dos acentos acima das maiúsculas,
    /// as descidas, o contorno e a sombra). Sem `max_width`, com `align` "center" ou "right" o x é o centro
    /// ou a última coluna (inclusive) do texto. Padrão [0, 0].
    #[serde(default)]
    pub at: Point,
    /// Fonte: "pixel-5x7" (padrão, maiúsculas de 7 px), "mini-3x5" (compacta) ou o nome/id de um sprite
    /// definido como fonte com font_define.
    #[serde(default = "default_font")]
    pub font: ItemRef,
    /// Cor do texto ("#hex", "clear" ou índice da paleta ativa). Obrigatória nas fontes embutidas; numa fonte
    /// de sprite, sem `color` os glyphs mantêm as cores desenhadas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRef>,
    /// Alinhamento: "left" (padrão), "center" ou "right". Com `max_width`, cada linha é alinhada dentro
    /// da largura que começa em `at`.
    #[serde(default)]
    pub align: TextAlign,
    /// Largura máxima da caixa em pixels: quebra as linhas por palavra (uma palavra maior que a linha é
    /// quebrada por letra).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    /// Espaço entre letras em pixels da fonte (padrão: o da fonte, 1 nas embutidas). Pode ser negativo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<i32>,
    /// Pixels da fonte a mais (ou a menos) entre as linhas, além da altura de linha da fonte. Padrão 0.
    #[serde(default)]
    pub line_spacing: i32,
    /// Ampliação inteira por vizinho mais próximo, de 1 a 16 (padrão 1). Espaçamentos crescem junto;
    /// contorno e sombra não.
    #[serde(default = "one")]
    pub scale: u32,
    /// Contorno de 1 px por fora das letras (8 vizinhos), nesta cor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<ColorRef>,
    /// Sombra projetada: `{"color": "#000000", "offset": [1, 1]}`, desenhada sob o texto e o contorno.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shadow: Option<TextShadow>,
}

impl TextSpec {
    pub fn new(text: impl Into<String>, at: Point) -> Self {
        TextSpec {
            text: text.into(),
            at,
            font: default_font(),
            color: None,
            align: TextAlign::Left,
            max_width: None,
            letter_spacing: None,
            line_spacing: 0,
            scale: 1,
            outline: None,
            shadow: None,
        }
    }
}

/// One laid-out line.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayoutLine {
    pub text: String,
    /// Top-left of the line on the canvas.
    pub x: i32,
    pub y: i32,
    /// Width of the line in pixels.
    pub width: u32,
}

/// Where one character goes: the pen position (top of the line box) on the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placement {
    c: char,
    x: i32,
    y: i32,
}

/// The result of laying out a text: its box, lines and missing characters.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    /// The box on the canvas (text plus outline and shadow). Empty text gives a 0-width box.
    pub rect: Rect,
    pub lines: Vec<LayoutLine>,
    /// Characters the font lacks (drawn as a replacement box), each once, in order.
    pub missing: Vec<char>,
    pub scale: u32,
    placements: Vec<Placement>,
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "").replace('\t', "    ")
}

/// Advance of one character in font pixels (without letter spacing).
fn advance(font: &BitmapFont, c: char) -> i64 {
    if c == ' ' {
        return font.space_width as i64;
    }
    font.glyph(c).unwrap_or(&font.replacement).advance as i64
}

/// Width of a run of characters in font pixels.
fn run_width(font: &BitmapFont, chars: &[char], ls: i64) -> i64 {
    if chars.is_empty() {
        return 0;
    }
    let sum: i64 = chars.iter().map(|c| advance(font, *c)).sum();
    (sum + ls * (chars.len() as i64 - 1)).max(0)
}

/// Splits one paragraph into lines no wider than `max` font pixels (greedy, by word).
fn wrap(font: &BitmapFont, para: &[char], ls: i64, max: Option<i64>) -> Vec<Vec<char>> {
    let Some(max) = max else { return vec![para.to_vec()] };
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut current: Option<Vec<char>> = None;
    for word in para.split(|c| *c == ' ') {
        let candidate = match &current {
            None => word.to_vec(),
            Some(cur) => {
                let mut v = cur.clone();
                v.push(' ');
                v.extend_from_slice(word);
                v
            }
        };
        if run_width(font, &candidate, ls) <= max {
            current = Some(candidate);
            continue;
        }
        if let Some(cur) = current.take() {
            lines.push(cur);
        }
        if run_width(font, word, ls) <= max {
            current = Some(word.to_vec());
            continue;
        }
        // A word wider than the line: break it by letter.
        let mut piece: Vec<char> = Vec::new();
        for c in word {
            let mut next = piece.clone();
            next.push(*c);
            if !piece.is_empty() && run_width(font, &next, ls) > max {
                lines.push(std::mem::take(&mut piece));
                piece.push(*c);
            } else {
                piece = next;
            }
        }
        current = Some(piece);
    }
    lines.push(current.unwrap_or_default());
    lines
}

fn bad(m: impl Into<String>) -> DrawError {
    DrawError::InvalidArgument(m.into())
}

/// Lays out `spec` with `font`. Pure: nothing is drawn.
pub fn layout(spec: &TextSpec, font: &BitmapFont) -> Result<TextLayout, DrawError> {
    let text = normalize(&spec.text);
    let count = text.chars().count();
    if count > MAX_TEXT_CHARS {
        return Err(DrawError::TooLarge(format!("texto com {count} caracteres (máximo {MAX_TEXT_CHARS})")));
    }
    if spec.scale == 0 || spec.scale > MAX_TEXT_SCALE {
        return Err(bad(format!("escala {} inválida (1 a {MAX_TEXT_SCALE})", spec.scale)));
    }
    let ls = spec.letter_spacing.unwrap_or(font.letter_spacing);
    if !(-16..=64).contains(&ls) {
        return Err(bad(format!("letter_spacing {ls} fora de -16..64")));
    }
    if !(-256..=256).contains(&spec.line_spacing) {
        return Err(bad(format!("line_spacing {} fora de -256..256", spec.line_spacing)));
    }
    let s = spec.scale as i64;
    let ls = ls as i64;
    let o: i64 = if spec.outline.is_some() { 1 } else { 0 };
    let (sdx, sdy) = spec.shadow.as_ref().map(|sh| (sh.offset.x as i64, sh.offset.y as i64)).unwrap_or((0, 0));
    if sdx.abs() > 64 || sdy.abs() > 64 {
        return Err(bad("deslocamento da sombra fora de -64..64"));
    }
    let (pad_l, pad_t) = (o + (-sdx).max(0), o + (-sdy).max(0));
    let (pad_r, pad_b) = (o + sdx.max(0), o + sdy.max(0));
    let max_units = match spec.max_width {
        Some(mw) => {
            let avail = mw as i64 - pad_l - pad_r;
            if avail / s < 1 {
                return Err(bad(format!("max_width {mw} pequeno demais para a escala {} com contorno/sombra", spec.scale)));
            }
            Some(avail / s)
        }
        None => None,
    };
    let chars: Vec<char> = text.chars().collect();
    let mut lines: Vec<Vec<char>> = Vec::new();
    for para in chars.split(|c| *c == '\n') {
        lines.extend(wrap(font, para, ls, max_units));
    }
    let widths: Vec<i64> = lines.iter().map(|l| run_width(font, l, ls)).collect();
    let block_w = widths.iter().copied().max().unwrap_or(0);
    let empty = chars.iter().all(|c| *c == '\n');
    let line_adv = (font.line_height as i64 + spec.line_spacing as i64).max(1);
    let text_h = (lines.len() as i64 - 1) * line_adv + font.cell_height as i64;
    let area_w = match spec.max_width {
        Some(mw) => mw as i64 - pad_l - pad_r,
        None => block_w * s,
    };
    let (box_w, box_h) = if empty { (0, 0) } else { (area_w + pad_l + pad_r, text_h * s + pad_t + pad_b) };
    if box_w > crate::sdc::MAX_SHAPE_SIDE as i64 || box_h > crate::sdc::MAX_SHAPE_SIDE as i64 {
        return Err(DrawError::TooLarge(format!(
            "texto de {box_w}x{box_h} px passa do limite de {} px por lado",
            crate::sdc::MAX_SHAPE_SIDE
        )));
    }
    let at = spec.at;
    let box_x = match (spec.max_width, spec.align) {
        (Some(_), _) | (None, TextAlign::Left) => at.x as i64,
        (None, TextAlign::Center) => at.x as i64 - (box_w - 1).max(0) / 2,
        (None, TextAlign::Right) => at.x as i64 - box_w + 1,
    };
    let box_y = at.y as i64;
    let mut out_lines = Vec::new();
    let mut placements = Vec::new();
    let mut missing: Vec<char> = Vec::new();
    for (i, (line, w)) in lines.iter().zip(&widths).enumerate() {
        let lw = w * s;
        let dx = match spec.align {
            TextAlign::Left => 0,
            TextAlign::Center => (area_w - lw) / 2,
            TextAlign::Right => area_w - lw,
        };
        let lx = box_x + pad_l + dx;
        let ly = box_y + pad_t + i as i64 * line_adv * s;
        let mut pen = 0i64;
        for c in line {
            if *c != ' ' {
                if !font.has(*c) && !missing.contains(c) {
                    missing.push(*c);
                }
                placements.push(Placement { c: *c, x: (lx + pen * s) as i32, y: ly as i32 });
            }
            pen += advance(font, *c) + ls;
        }
        out_lines.push(LayoutLine { text: line.iter().collect(), x: lx as i32, y: ly as i32, width: lw.max(0) as u32 });
    }
    Ok(TextLayout {
        rect: Rect::new(box_x as i32, box_y as i32, box_w.max(0) as u32, box_h.max(0) as u32),
        lines: out_lines,
        missing,
        scale: spec.scale,
        placements,
    })
}

/// The pixels of a laid-out text, relative to its box: what each written pixel becomes.
pub struct TextPixels {
    /// Pixels the text writes (a written pixel can be transparent when the color is "clear").
    pub written: Mask,
    pub colors: Image,
    /// Top-left of the box on the canvas.
    pub origin: Point,
}

impl TextPixels {
    /// Bounding box of the written pixels, on the canvas.
    pub fn ink(&self) -> Option<Rect> {
        self.written.bounds().map(|b| b.translate(self.origin.x, self.origin.y))
    }
}

/// Resolved colors of a text.
pub struct TextColors {
    /// `None` keeps the glyph colors (sprite fonts).
    pub text: Option<Color>,
    pub outline: Option<Color>,
    pub shadow: Option<Color>,
}

impl TextColors {
    pub fn resolve(spec: &TextSpec, font: &BitmapFont, palette: &[Color]) -> Result<Self, DrawError> {
        let text = spec.color.as_ref().map(|c| c.resolve(palette)).transpose()?;
        if text.is_none() && font.builtin {
            return Err(bad(format!("informe `color`: a fonte embutida {} não tem cores próprias", font.name)));
        }
        Ok(TextColors {
            text,
            outline: spec.outline.as_ref().map(|c| c.resolve(palette)).transpose()?,
            shadow: spec.shadow.as_ref().map(|sh| sh.color.resolve(palette)).transpose()?,
        })
    }
}

/// Renders a layout into box-relative pixels (shadow, then outline, then the letters).
pub fn render(layout: &TextLayout, font: &BitmapFont, spec: &TextSpec, colors: &TextColors) -> Option<TextPixels> {
    let r = layout.rect;
    if r.w == 0 || r.h == 0 {
        return None;
    }
    let s = layout.scale as i32;
    let mut letters = Mask::new(r.w, r.h);
    let mut colors_img = Image::new(r.w, r.h);
    for p in &layout.placements {
        let g = font.glyph(p.c).unwrap_or(&font.replacement);
        let (gx, gy) = (p.x - r.x + g.xoffset * s, p.y - r.y + g.yoffset * s);
        for y in 0..g.height() as i32 {
            for x in 0..g.width() as i32 {
                let src = g.image.get_or_clear(x, y);
                if src.is_transparent() {
                    continue;
                }
                let c = colors.text.unwrap_or(src);
                for by in 0..s {
                    for bx in 0..s {
                        let (px, py) = (gx + x * s + bx, gy + y * s + by);
                        letters.set(px, py, true);
                        colors_img.set(px, py, c);
                    }
                }
            }
        }
    }
    let mut silhouette = letters.clone();
    let mut written = letters.clone();
    let mut out = Image::new(r.w, r.h);
    if colors.outline.is_some() {
        let mut grown = letters.clone();
        for p in letters.points() {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    grown.set(p.x + dx, p.y + dy, true);
                }
            }
        }
        silhouette = grown;
    }
    if let (Some(c), Some(sh)) = (colors.shadow, &spec.shadow) {
        for p in silhouette.translate(sh.offset.x, sh.offset.y).points() {
            out.set(p.x, p.y, c);
            written.set(p.x, p.y, true);
        }
    }
    if let Some(c) = colors.outline {
        for p in silhouette.subtract(&letters).points() {
            out.set(p.x, p.y, c);
            written.set(p.x, p.y, true);
        }
    }
    for p in letters.points() {
        out.set(p.x, p.y, colors_img.get_or_clear(p.x, p.y));
    }
    Some(TextPixels { written, colors: out, origin: Point::new(r.x, r.y) })
}

/// Draws a text on the canvas. Returns the layout (box, lines, missing characters).
pub fn draw(canvas: &mut Canvas, spec: &TextSpec, font: &BitmapFont, palette: &[Color]) -> Result<TextLayout, DrawError> {
    if spec.text.trim_matches(['\n', '\r']).is_empty() {
        return Err(bad("texto vazio"));
    }
    let colors = TextColors::resolve(spec, font, palette)?;
    let lay = layout(spec, font)?;
    if let Some(px) = render(&lay, font, spec, &colors) {
        for p in px.written.points() {
            canvas.put(px.origin.x + p.x, px.origin.y + p.y, px.colors.get_or_clear(p.x, p.y));
        }
    }
    Ok(lay)
}

/// The font a `text` operation uses: the one loaded by the command layer, or a built-in one.
pub fn op_font<'a>(spec: &TextSpec, loaded: Option<&'a Arc<BitmapFont>>) -> Result<&'a BitmapFont, DrawError> {
    if let Some(f) = loaded {
        return Ok(f.as_ref());
    }
    builtin_for(&spec.font).ok_or_else(|| {
        DrawError::Source(format!(
            "fonte {} não carregada: use \"pixel-5x7\", \"mini-3x5\" ou o nome de um sprite definido com font_define",
            spec.font
        ))
    })
}

#[cfg(test)]
mod tests;
