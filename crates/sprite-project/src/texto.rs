//! Text exports: bitmap fonts as AngelCode BMFont (text `.fnt` + PNG atlas, which Godot 4 imports
//! as a FontFile) and 9-slice textures as a Godot 4 scene with a configured `NinePatchRect`.

use sprite_core::{Color, Image, PasteMode, Point, Rect};
use sprite_drawing::nine_slice::{NineSlice, SliceMode};
use sprite_drawing::text::BitmapFont;

use crate::ProjectError;
use crate::export::scale_image;

/// Options of a BMFont export.
#[derive(Debug, Clone)]
pub struct BmFontOptions {
    /// Font name written in `info face`.
    pub face: String,
    /// Atlas file name written in `page file`.
    pub page_file: String,
    /// Integer scale of glyphs and metrics.
    pub scale: u32,
    /// Glyph color (`None` keeps the glyph colors).
    pub color: Option<Color>,
    /// Pixels between glyphs in the atlas.
    pub spacing: u32,
}

/// One glyph placed in the atlas.
#[derive(Debug, Clone, PartialEq)]
pub struct BmChar {
    pub ch: char,
    /// Area in the atlas (empty for the space).
    pub rect: Rect,
    pub xoffset: i32,
    pub yoffset: i32,
    pub xadvance: i32,
}

/// A BMFont ready to write: the atlas image, the `.fnt` text and the placed characters.
#[derive(Debug, Clone)]
pub struct BmFont {
    pub atlas: Image,
    pub text: String,
    pub chars: Vec<BmChar>,
}

fn quote(s: &str) -> String {
    s.replace('"', "'")
}

/// Packs the glyphs of `font` into an atlas and writes the BMFont text description.
pub fn bitmap_font(font: &BitmapFont, opts: &BmFontOptions) -> Result<BmFont, ProjectError> {
    let s = opts.scale.clamp(1, 16);
    let sp = opts.spacing.min(16);
    let ls = font.letter_spacing;
    // Glyph images at the export scale and color, in code point order.
    let glyphs: Vec<(char, Image, i32, i32, i32)> = font
        .glyphs
        .iter()
        .map(|(c, g)| {
            let mut img = g.image.clone();
            if let Some(color) = opts.color {
                for y in 0..img.height() as i32 {
                    for x in 0..img.width() as i32 {
                        if !img.get_or_clear(x, y).is_transparent() {
                            img.set(x, y, color);
                        }
                    }
                }
            }
            let img = scale_image(&img, s);
            (*c, img, g.xoffset * s as i32, g.yoffset * s as i32, ((g.advance + ls) * s as i32).max(0))
        })
        .collect();
    // Shelf packing: rows left to right, about as wide as tall.
    let area: u64 = glyphs.iter().map(|(_, i, ..)| ((i.width() + sp) as u64) * ((i.height() + sp) as u64)).sum();
    let widest = glyphs.iter().map(|(_, i, ..)| i.width() + sp).max().unwrap_or(1);
    let width = ((area as f64).sqrt().ceil() as u32).max(widest).max(1).div_ceil(8) * 8;
    let mut placed = Vec::with_capacity(glyphs.len());
    let (mut x, mut y, mut row_h) = (0u32, 0u32, 0u32);
    for (_, img, ..) in &glyphs {
        if x > 0 && x + img.width() > width {
            x = 0;
            y += row_h + sp;
            row_h = 0;
        }
        placed.push(Rect::new(x as i32, y as i32, img.width(), img.height()));
        x += img.width() + sp;
        row_h = row_h.max(img.height());
    }
    let height = (y + row_h).max(1);
    if width > sprite_core::image::MAX_IMAGE_SIDE || height > sprite_core::image::MAX_IMAGE_SIDE {
        return Err(ProjectError::Image(format!("atlas de {width}x{height} px passa do limite; use uma escala menor")));
    }
    let mut atlas = Image::new(width, height);
    let mut chars = Vec::with_capacity(glyphs.len() + 1);
    chars.push(BmChar {
        ch: ' ',
        rect: Rect::new(0, 0, 0, 0),
        xoffset: 0,
        yoffset: 0,
        xadvance: ((font.space_width as i32 + ls) * s as i32).max(0),
    });
    for ((c, img, xo, yo, adv), r) in glyphs.iter().zip(&placed) {
        atlas.paste(img, Point::new(r.x, r.y), PasteMode::Replace);
        chars.push(BmChar { ch: *c, rect: *r, xoffset: *xo, yoffset: *yo, xadvance: *adv });
    }
    let mut text = String::new();
    text.push_str(&format!(
        "info face=\"{}\" size={} bold=0 italic=0 charset=\"\" unicode=1 stretchH=100 smooth=0 aa=1 padding=0,0,0,0 spacing={sp},{sp} outline=0\n",
        quote(&opts.face),
        font.cell_height * s
    ));
    text.push_str(&format!(
        "common lineHeight={} base={} scaleW={width} scaleH={height} pages=1 packed=0\n",
        font.line_height * s,
        font.base * s
    ));
    text.push_str(&format!("page id=0 file=\"{}\"\n", quote(&opts.page_file)));
    text.push_str(&format!("chars count={}\n", chars.len()));
    for c in &chars {
        text.push_str(&format!(
            "char id={} x={} y={} width={} height={} xoffset={} yoffset={} xadvance={} page=0 chnl=15\n",
            c.ch as u32, c.rect.x, c.rect.y, c.rect.w, c.rect.h, c.xoffset, c.yoffset, c.xadvance
        ));
    }
    Ok(BmFont { atlas, text, chars })
}

/// A Godot node name: no `. : @ / " %` characters.
pub fn godot_node_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if ".:@/\"%".contains(c) || c.is_control() { '_' } else { c }).collect();
    if cleaned.trim().is_empty() { "NinePatch".into() } else { cleaned.trim().to_string() }
}

/// A Godot 4 scene with one `NinePatchRect` showing `texture_res_path` with the given margins.
pub fn nine_patch_scene(node_name: &str, texture_res_path: &str, margins: NineSlice, size: [u32; 2], h: SliceMode, v: SliceMode) -> String {
    let mut out = String::new();
    out.push_str("[gd_scene load_steps=2 format=3]\n\n");
    out.push_str(&format!("[ext_resource type=\"Texture2D\" path=\"{}\" id=\"1_tex\"]\n\n", quote(texture_res_path)));
    out.push_str(&format!("[node name=\"{}\" type=\"NinePatchRect\"]\n", quote(&godot_node_name(node_name))));
    // Pixel art: nearest filtering.
    out.push_str("texture_filter = 1\n");
    out.push_str(&format!("offset_right = {}.0\n", size[0]));
    out.push_str(&format!("offset_bottom = {}.0\n", size[1]));
    out.push_str("texture = ExtResource(\"1_tex\")\n");
    out.push_str(&format!("patch_margin_left = {}\n", margins.left));
    out.push_str(&format!("patch_margin_top = {}\n", margins.top));
    out.push_str(&format!("patch_margin_right = {}\n", margins.right));
    out.push_str(&format!("patch_margin_bottom = {}\n", margins.bottom));
    out.push_str(&format!("axis_stretch_horizontal = {}\n", h.godot_axis_stretch()));
    out.push_str(&format!("axis_stretch_vertical = {}\n", v.godot_axis_stretch()));
    out
}

#[cfg(test)]
mod tests {
    use sprite_drawing::text::builtin_font;

    use super::*;

    fn field(line: &str, key: &str) -> i64 {
        line.split_whitespace().find_map(|kv| kv.strip_prefix(&format!("{key}="))).unwrap().parse().unwrap()
    }

    #[test]
    fn bmfont_coordinates_point_at_the_glyph_pixels() {
        let font = builtin_font("pixel-5x7").unwrap();
        let opts = BmFontOptions { face: "pixel-5x7".into(), page_file: "p.png".into(), scale: 2, color: Some(Color::WHITE), spacing: 1 };
        let bm = bitmap_font(font, &opts).unwrap();
        let lines: Vec<&str> = bm.text.lines().collect();
        assert!(lines[0].starts_with("info face=\"pixel-5x7\" size=22 bold=0 italic=0 charset=\"\" unicode=1 stretchH=100 smooth=0 aa=1 padding=0,0,0,0 spacing=1,1 outline=0"));
        assert!(lines[1].starts_with("common lineHeight=24 base=18 scaleW="));
        assert_eq!(field(lines[1], "scaleW"), bm.atlas.width() as i64);
        assert_eq!(lines[2], "page id=0 file=\"p.png\"");
        assert_eq!(lines[3], format!("chars count={}", font.glyphs.len() + 1));
        let a = lines.iter().find(|l| l.starts_with("char id=65 ")).unwrap();
        let (x, y, w, h) = (field(a, "x"), field(a, "y"), field(a, "width"), field(a, "height"));
        assert_eq!((w, h, field(a, "yoffset"), field(a, "xadvance")), (10, 14, 4, 12));
        let crop = bm.atlas.crop(Rect::new(x as i32, y as i32, w as u32, h as u32));
        assert_eq!(crop, scale_image(&font.glyph('A').unwrap().image, 2));
        let space = lines.iter().find(|l| l.starts_with("char id=32 ")).unwrap();
        assert_eq!((field(space, "width"), field(space, "xadvance")), (0, 6));
        // No two glyphs overlap in the atlas.
        for (i, a) in bm.chars.iter().enumerate() {
            for b in &bm.chars[i + 1..] {
                if !a.rect.is_empty() && !b.rect.is_empty() {
                    assert!(a.rect.intersect(&b.rect).is_empty(), "{} e {} se sobrepõem", a.ch, b.ch);
                }
            }
        }
        // Accented letters are there with their Unicode ids.
        assert!(bm.text.contains(&format!("char id={} ", 'ã' as u32)));
    }

    #[test]
    fn nine_patch_scene_is_configured() {
        let m = NineSlice { left: 4, top: 3, right: 5, bottom: 6 };
        let t = nine_patch_scene("Painel: 1", "res://ui/painel.png", m, [96, 48], SliceMode::Tile, SliceMode::TileFit);
        assert!(t.starts_with("[gd_scene load_steps=2 format=3]"));
        assert!(t.contains("[ext_resource type=\"Texture2D\" path=\"res://ui/painel.png\" id=\"1_tex\"]"));
        assert!(t.contains("[node name=\"Painel_ 1\" type=\"NinePatchRect\"]"));
        for line in [
            "texture = ExtResource(\"1_tex\")",
            "offset_right = 96.0",
            "offset_bottom = 48.0",
            "patch_margin_left = 4",
            "patch_margin_top = 3",
            "patch_margin_right = 5",
            "patch_margin_bottom = 6",
            "axis_stretch_horizontal = 1",
            "axis_stretch_vertical = 2",
        ] {
            assert!(t.lines().any(|l| l == line), "falta {line}");
        }
    }
}
