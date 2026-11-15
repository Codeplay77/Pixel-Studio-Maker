//! Bitmap fonts: the built-in ones and fonts drawn inside a sprite.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image, ItemRef, Rect};

use super::builtin::{self, Accent, FontData};
use crate::DrawError;

/// One glyph: its pixels and where they go relative to the pen.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// Glyph pixels (transparent = no ink). Built-in glyphs are white; drawing tints them.
    pub image: Image,
    /// Horizontal offset of the image from the pen position.
    pub xoffset: i32,
    /// Vertical offset of the image from the top of the line box.
    pub yoffset: i32,
    /// How far the pen moves after this glyph, without the letter spacing.
    pub advance: i32,
}

impl Glyph {
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }
}

/// A bitmap font ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapFont {
    pub name: String,
    pub builtin: bool,
    pub glyphs: BTreeMap<char, Glyph>,
    /// Height of the line box that holds every glyph (accents to descenders).
    pub cell_height: u32,
    /// Distance between the tops of two lines.
    pub line_height: u32,
    /// Rows from the top of the line box to the baseline.
    pub base: u32,
    /// Default space between letters.
    pub letter_spacing: i32,
    /// Advance of the space character (letter spacing is added on top).
    pub space_width: u32,
    /// Drawn (and reported) for characters the font lacks.
    pub replacement: Glyph,
}

impl BitmapFont {
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.glyphs.get(&c)
    }

    /// Whether the font can draw `c` (space and the glyphs it has).
    pub fn has(&self, c: char) -> bool {
        c == ' ' || self.glyphs.contains_key(&c)
    }

    /// Every character the font draws, space included, in code point order.
    pub fn charset(&self) -> String {
        std::iter::once(' ').chain(self.glyphs.keys().copied()).collect()
    }

    /// Widest glyph box (offset included).
    pub fn max_glyph_width(&self) -> u32 {
        self.glyphs.values().map(|g| (g.xoffset.max(0) as u32) + g.width()).max().unwrap_or(1)
    }
}

/// Names of the built-in fonts.
pub const BUILTIN_FONTS: &[&str] = &["pixel-5x7", "mini-3x5"];

/// Characters every built-in font draws: printable ASCII plus Portuguese and Spanish letters.
pub fn standard_charset() -> String {
    let mut s: String = (32u8..=126).map(char::from).collect();
    s.push_str("ÁÀÂÃÄÉÈÊËÍÌÎÏÓÒÔÕÖÚÙÛÜÇÑáàâãäéèêëíìîïóòôõöúùûüçñ¡¿ºª°");
    s
}

/// The minimum set required of the built-in fonts (ASCII 32–126, PT/ES letters, ¡ ¿ º ª °).
pub fn required_charset() -> String {
    let mut s: String = (32u8..=126).map(char::from).collect();
    s.push_str("áàâãäéêíóôõöúüçñÁÀÂÃÄÉÊÍÓÔÕÖÚÜÇÑ¡¿ºª°");
    s
}

fn rows_image(rows: &[&str]) -> Image {
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(1).max(1) as u32;
    let h = rows.len().max(1) as u32;
    let mut img = Image::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                img.set(x as i32, y as i32, Color::WHITE);
            }
        }
    }
    img
}

/// Places `accent` above (or, for the cedilla, below) `base`, both centered on the wider one.
fn compose(base: &Glyph, accent: [&str; 2], below: bool) -> Glyph {
    let mark = rows_image(&accent);
    let (bw, bh) = (base.width(), base.height());
    let (aw, ah) = (mark.width(), mark.height());
    let gw = bw.max(aw);
    let bx = ((gw - bw) / 2) as i32;
    let ax = ((gw - aw) / 2) as i32;
    let mut img = Image::new(gw, bh + ah);
    let (base_y, mark_y, yoffset) = if below { (0, bh as i32, base.yoffset) } else { (ah as i32, 0, base.yoffset - ah as i32) };
    img.paste(&base.image, sprite_core::Point::new(bx, base_y), sprite_core::PasteMode::SkipTransparent);
    img.paste(&mark, sprite_core::Point::new(ax, mark_y), sprite_core::PasteMode::SkipTransparent);
    Glyph { image: img, xoffset: 0, yoffset, advance: gw as i32 }
}

fn build_builtin(data: &FontData) -> BitmapFont {
    let mut glyphs = BTreeMap::new();
    for (c, top, rows) in data.glyphs {
        let image = rows_image(rows);
        let advance = image.width() as i32;
        glyphs.insert(*c, Glyph { image, xoffset: 0, yoffset: (data.zone + top) as i32, advance });
    }
    for (c, base, accent) in builtin::COMPOSED {
        let Some(b) = glyphs.get(base).cloned() else { continue };
        let Some((_, odd, even)) = data.accents.iter().find(|(a, _, _)| a == accent) else { continue };
        let marks = if b.width() % 2 == 1 { *odd } else { *even };
        glyphs.insert(*c, compose(&b, marks, *accent == Accent::Cedilla));
    }
    let replacement = rows_image(data.replacement);
    let advance = replacement.width() as i32;
    BitmapFont {
        name: data.name.to_string(),
        builtin: true,
        glyphs,
        cell_height: data.zone + data.cap_height + data.descent,
        line_height: data.line_height,
        base: data.zone + data.cap_height,
        letter_spacing: data.letter_spacing,
        space_width: data.space_width,
        replacement: Glyph { image: replacement, xoffset: 0, yoffset: data.zone as i32, advance },
    }
}

/// A built-in font by name (case-insensitive).
pub fn builtin_font(name: &str) -> Option<&'static BitmapFont> {
    static FONTS: OnceLock<Vec<BitmapFont>> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| builtin::FONTS.iter().map(|d| build_builtin(d)).collect());
    fonts.iter().find(|f| f.name.eq_ignore_ascii_case(name.trim()))
}

/// The built-in font a reference names, if it names one.
pub fn builtin_for(r: &ItemRef) -> Option<&'static BitmapFont> {
    match r {
        ItemRef::Name(n) => builtin_font(n),
        ItemRef::Id(_) => None,
    }
}

fn default_letter_spacing() -> i32 {
    1
}

/// A font drawn inside a sprite: a grid of cells, one character per cell, row by row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpriteFontDef {
    /// Tamanho de cada célula [largura, altura] em pixels.
    pub cell: [u32; 2],
    /// Os caracteres na ordem das células, linha a linha (o espaço pode ocupar uma célula vazia).
    pub charset: String,
    /// Células por linha da grade.
    pub columns: u32,
    /// Linhas do topo da célula até a linha de base (onde as letras "pousam").
    pub baseline: u32,
    /// Espaço entre letras em pixels.
    #[serde(default = "default_letter_spacing")]
    pub letter_spacing: i32,
    /// Avanço do caractere espaço.
    pub space_width: u32,
    /// Distância entre o topo de duas linhas de texto.
    pub line_height: u32,
    /// Frame que guarda os glyphs (padrão: o primeiro da linha do tempo).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
}

impl SpriteFontDef {
    /// Rows of the cell grid.
    pub fn rows(&self) -> u32 {
        let n = self.charset.chars().count() as u32;
        n.div_ceil(self.columns.max(1))
    }

    /// The cell of the `index`-th character.
    pub fn cell_rect(&self, index: usize) -> Rect {
        let cols = self.columns.max(1) as usize;
        let (col, row) = ((index % cols) as i32, (index / cols) as i32);
        Rect::new(col * self.cell[0] as i32, row * self.cell[1] as i32, self.cell[0], self.cell[1])
    }

    /// Checks the definition against a sprite of `width`×`height`.
    pub fn validate(&self, width: u32, height: u32) -> Result<(), DrawError> {
        let [cw, ch] = self.cell;
        let bad = |m: String| Err(DrawError::InvalidArgument(m));
        if cw == 0 || ch == 0 || cw > 256 || ch > 256 {
            return bad(format!("célula {cw}x{ch} inválida: cada lado de 1 a 256 px"));
        }
        if self.columns == 0 {
            return bad("`columns` precisa ser pelo menos 1".into());
        }
        let chars: Vec<char> = self.charset.chars().collect();
        if chars.is_empty() {
            return bad("`charset` vazio: informe os caracteres na ordem das células".into());
        }
        if chars.len() > 4096 {
            return bad(format!("`charset` com {} caracteres (máximo 4096)", chars.len()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in &chars {
            if c.is_control() {
                return bad(format!("caractere de controle U+{:04X} no `charset`", *c as u32));
            }
            if !seen.insert(*c) {
                return bad(format!("o caractere '{c}' aparece duas vezes no `charset`"));
            }
        }
        let grid_w = self.columns.saturating_mul(cw);
        let grid_h = self.rows().saturating_mul(ch);
        if grid_w > width || grid_h > height {
            return bad(format!(
                "a grade ({} colunas × {} linhas de {cw}x{ch} = {grid_w}x{grid_h} px) não cabe no sprite {width}x{height}",
                self.columns,
                self.rows()
            ));
        }
        if self.baseline > ch {
            return bad(format!("`baseline` {} maior que a altura da célula ({ch})", self.baseline));
        }
        if self.line_height == 0 || self.line_height > 1024 {
            return bad(format!("`line_height` {} inválido (1 a 1024)", self.line_height));
        }
        if self.space_width > 256 {
            return bad(format!("`space_width` {} grande demais (máximo 256)", self.space_width));
        }
        if !(-16..=64).contains(&self.letter_spacing) {
            return bad(format!("`letter_spacing` {} fora de -16..64", self.letter_spacing));
        }
        Ok(())
    }
}

/// Builds a font from the pixels of a sprite frame. Returns the font and the characters whose
/// cells are empty (they are left out of the font).
pub fn font_from_sheet(name: &str, sheet: &Image, def: &SpriteFontDef) -> Result<(BitmapFont, Vec<char>), DrawError> {
    def.validate(sheet.width(), sheet.height())?;
    let mut glyphs = BTreeMap::new();
    let mut empty = Vec::new();
    for (i, c) in def.charset.chars().enumerate() {
        if c == ' ' {
            continue;
        }
        let cell = sheet.crop(def.cell_rect(i));
        match cell.opaque_bounds() {
            None => empty.push(c),
            Some(b) => {
                // The advance is the real (opaque) width of the glyph.
                glyphs.insert(c, Glyph { image: cell.crop(b), xoffset: 0, yoffset: b.y, advance: b.w as i32 });
            }
        }
    }
    let [cw, ch] = def.cell;
    let rw = (cw / 2).max(2);
    let rh = def.baseline.max(3).min(ch.max(1));
    let mut rep = Image::new(rw, rh);
    for y in 0..rh as i32 {
        for x in 0..rw as i32 {
            if x == 0 || y == 0 || x == rw as i32 - 1 || y == rh as i32 - 1 {
                rep.set(x, y, Color::WHITE);
            }
        }
    }
    let rep_y = def.baseline.saturating_sub(rh) as i32;
    Ok((
        BitmapFont {
            name: name.to_string(),
            builtin: false,
            glyphs,
            cell_height: ch,
            line_height: def.line_height,
            base: def.baseline,
            letter_spacing: def.letter_spacing,
            space_width: def.space_width,
            replacement: Glyph { image: rep, xoffset: 0, yoffset: rep_y, advance: rw as i32 },
        },
        empty,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_fonts_cover_the_required_charset() {
        for name in BUILTIN_FONTS {
            let font = builtin_font(name).unwrap();
            for c in required_charset().chars().chain(standard_charset().chars()) {
                assert!(font.has(c), "{name}: falta o glyph '{c}'");
                if c == ' ' {
                    continue;
                }
                let g = font.glyph(c).unwrap();
                assert!(!g.image.is_blank(), "{name}: glyph '{c}' vazio");
                assert!(g.yoffset >= 0, "{name}: '{c}' sai por cima da linha");
                assert!(g.yoffset as u32 + g.height() <= font.cell_height, "{name}: '{c}' sai por baixo da linha");
                assert_eq!(g.advance as u32, g.width(), "{name}: avanço de '{c}'");
            }
        }
    }

    #[test]
    fn builtin_rows_are_rectangular_and_only_use_ink_or_dot() {
        for data in builtin::FONTS {
            for (c, top, rows) in data.glyphs {
                let w = rows[0].chars().count();
                for r in *rows {
                    assert_eq!(r.chars().count(), w, "{}: linhas de '{c}' com larguras diferentes", data.name);
                    assert!(r.chars().all(|x| x == '#' || x == '.'), "{}: '{c}' com caractere estranho", data.name);
                }
                assert!(top + rows.len() as u32 <= data.cap_height + data.descent, "{}: '{c}' passa da descida", data.name);
            }
            let mut seen = std::collections::BTreeSet::new();
            for (c, _, _) in data.glyphs {
                assert!(seen.insert(*c), "{}: '{c}' definido duas vezes", data.name);
            }
        }
    }

    #[test]
    fn accents_sit_above_capitals_and_lowercase() {
        let f = builtin_font("pixel-5x7").unwrap();
        // Á: accent in the zone (rows 0-1), the A below it from row 2.
        let a = f.glyph('Á').unwrap();
        assert_eq!(a.yoffset, 0);
        assert_eq!(a.height(), 9);
        assert_eq!(a.image.get(3, 0), Some(Color::WHITE), "acento agudo inclinado para a direita");
        assert_eq!(a.image.get(2, 1), Some(Color::WHITE));
        assert_eq!(a.image.get(1, 2), Some(Color::WHITE), "topo do A");
        // ç: cedilla below the baseline.
        let c = f.glyph('ç').unwrap();
        assert_eq!(c.yoffset as u32 + c.height(), f.cell_height);
        assert_eq!(f.base, 9);
        assert_eq!(f.cell_height, 11);
        assert!(builtin_font("MINI-3x5").is_some());
        assert!(builtin_font("nada").is_none());
    }

    #[test]
    fn sheet_fonts_use_the_real_width() {
        let mut sheet = Image::new(16, 8);
        // Cell 0 ('A'): 3 px wide, starting at x = 2.
        for y in 1..6 {
            for x in 2..5 {
                sheet.set(x, y, Color::rgb(200, 10, 10));
            }
        }
        // Cell 1 ('B'): 1 px at (9, 3).
        sheet.set(9, 3, Color::BLACK);
        let def = SpriteFontDef {
            cell: [8, 8],
            charset: "AB C".into(),
            columns: 2,
            baseline: 6,
            letter_spacing: 1,
            space_width: 3,
            line_height: 9,
            frame: None,
        };
        let err = font_from_sheet("x", &sheet, &def).unwrap_err();
        assert!(err.to_string().contains("não cabe"), "{err}");
        let def = SpriteFontDef { charset: "ABC".into(), ..def };
        let mut tall = Image::new(16, 16);
        tall.paste(&sheet, sprite_core::Point::new(0, 0), sprite_core::PasteMode::Replace);
        let (font, empty) = font_from_sheet("x", &tall, &def).unwrap();
        assert_eq!(empty, vec!['C']);
        let a = font.glyph('A').unwrap();
        assert_eq!((a.advance, a.yoffset, a.width(), a.height()), (3, 1, 3, 5));
        let b = font.glyph('B').unwrap();
        assert_eq!((b.advance, b.yoffset), (1, 3));
        assert_eq!(font.base, 6);
        assert!(font.has(' '));
        assert!(!font.has('C'));
    }
}
