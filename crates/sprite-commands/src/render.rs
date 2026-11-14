//! Preview rendering for inspection: nearest-neighbor enlargement, backgrounds, grid, anchor
//! markers and region outlines. Never used for exports.

use sprite_core::codec::encode_png;
use sprite_core::compose::on_checkerboard;
use sprite_core::{Color, Image, Mask, PasteMode, Point};

use crate::{Background, CommandError, ErrorCode, RenderedImage};

/// Scale that makes the longest side about 512 px (1..=16), bounded by `max_side`.
pub fn auto_scale(w: u32, h: u32, max_side: u32) -> u32 {
    let side = w.max(h).max(1);
    (512 / side).clamp(1, 16).min((max_side / side).max(1))
}

pub fn check_scale(w: u32, h: u32, scale: u32, max_side: u32) -> Result<u32, CommandError> {
    let s = scale.max(1);
    if w.max(h) * s > max_side {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("prévia de {}x{} passa do limite de {max_side} px; use uma escala menor", w * s, h * s),
        ));
    }
    Ok(s)
}

/// Enlarges and puts a background under the image; draws a pixel grid when asked (scale ≥ 6).
pub fn preview(img: &Image, scale: u32, background: Background, grid: bool) -> Image {
    let big = img.scale_nearest(img.width() * scale, img.height() * scale);
    let mut out = match background {
        Background::Checker => on_checkerboard(&big, (scale * 2).clamp(4, 16), Color::rgb(206, 206, 212), Color::rgb(160, 160, 170)),
        Background::None => big,
        Background::White => {
            let mut b = Image::filled(big.width(), big.height(), Color::WHITE);
            b.paste(&big, Point::default(), PasteMode::Over);
            b
        }
        Background::Black => {
            let mut b = Image::filled(big.width(), big.height(), Color::rgb(16, 16, 20));
            b.paste(&big, Point::default(), PasteMode::Over);
            b
        }
    };
    if grid && scale >= 6 {
        let line = Color::rgba(0, 0, 0, 60);
        for y in 0..out.height() as i32 {
            for x in 0..out.width() as i32 {
                if x % scale as i32 == 0 || y % scale as i32 == 0 {
                    let c = line.over(out.get_or_clear(x, y));
                    out.set(x, y, c);
                }
            }
        }
    }
    out
}

/// A cross marker centered on sprite pixel (x, y) of a preview at `scale`.
pub fn mark_point(out: &mut Image, x: i32, y: i32, scale: u32, color: Color) {
    let s = scale as i32;
    let (cx, cy) = (x * s + s / 2, y * s + s / 2);
    let arm = (s * 3 / 2).max(3);
    for d in -arm..=arm {
        out.set(cx + d, cy, color);
        out.set(cx, cy + d, color);
    }
    for (dx, dy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        out.set(cx + dx, cy + dy, Color::BLACK);
    }
}

/// Outlines a mask's border on a preview at `scale`.
pub fn outline_mask(out: &mut Image, mask: &Mask, scale: u32, color: Color) {
    let s = scale as i32;
    for p in mask.points() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if mask.get(p.x + dx, p.y + dy) {
                continue;
            }
            for k in 0..s {
                let (x, y) = match (dx, dy) {
                    (1, 0) => (p.x * s + s - 1, p.y * s + k),
                    (-1, 0) => (p.x * s, p.y * s + k),
                    (0, 1) => (p.x * s + k, p.y * s + s - 1),
                    _ => (p.x * s + k, p.y * s),
                };
                out.set(x, y, color);
            }
        }
    }
}

/// Places images left to right with a gap, on a neutral strip.
pub fn side_by_side(images: &[Image], gap: u32) -> Image {
    let w: u32 = images.iter().map(|i| i.width()).sum::<u32>() + gap * images.len().saturating_sub(1) as u32;
    let h = images.iter().map(|i| i.height()).max().unwrap_or(1);
    let mut out = Image::filled(w.max(1), h.max(1), Color::rgb(60, 60, 70));
    let mut x = 0;
    for img in images {
        out.paste(img, Point::new(x, 0), PasteMode::Replace);
        x += (img.width() + gap) as i32;
    }
    out
}

pub fn png(label: impl Into<String>, img: &Image) -> Result<RenderedImage, CommandError> {
    let data = encode_png(img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
    Ok(RenderedImage { label: label.into(), mime: "image/png".into(), data, width: img.width(), height: img.height() })
}

/// Distinct colors for overlays (anchors, regions).
pub fn overlay_color(i: usize) -> Color {
    const C: [Color; 8] = [
        Color::rgb(255, 64, 129),
        Color::rgb(0, 229, 255),
        Color::rgb(255, 214, 0),
        Color::rgb(118, 255, 3),
        Color::rgb(213, 0, 249),
        Color::rgb(255, 109, 0),
        Color::rgb(41, 121, 255),
        Color::rgb(29, 233, 182),
    ];
    C[i % C.len()]
}
