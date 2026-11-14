use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::geom::{DirtyRect, Point, Rect};
use crate::image::Image;

/// One pixel that differs between two images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelChange {
    pub at: Point,
    pub before: Color,
    pub after: Color,
}

/// Summary of the differences between two same-sized images.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiffStats {
    /// Pixels whose value differs.
    pub changed: u32,
    /// Pixels compared (width × height).
    pub total: u32,
    /// `changed / total`, 0..1.
    pub ratio: f64,
    /// Bounding box of the changes.
    pub bounds: Option<Rect>,
    /// Pixels that went from transparent to visible.
    pub added: u32,
    /// Pixels that went from visible to transparent.
    pub removed: u32,
}

/// Lists every differing pixel. Images of different sizes are compared over their common area,
/// and the extra area of the larger one counts as changed against transparent.
pub fn diff_pixels(before: &Image, after: &Image) -> Vec<PixelChange> {
    let w = before.width().max(after.width()) as i32;
    let h = before.height().max(after.height()) as i32;
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let (b, a) = (before.get_or_clear(x, y), after.get_or_clear(x, y));
            if a != b {
                out.push(PixelChange { at: Point::new(x, y), before: b, after: a });
            }
        }
    }
    out
}

pub fn diff_stats(before: &Image, after: &Image) -> DiffStats {
    let changes = diff_pixels(before, after);
    let total = before.width().max(after.width()) * before.height().max(after.height());
    let mut bounds = DirtyRect::default();
    let (mut added, mut removed) = (0, 0);
    for c in &changes {
        bounds.add(c.at);
        if c.before.is_transparent() && !c.after.is_transparent() {
            added += 1;
        }
        if !c.before.is_transparent() && c.after.is_transparent() {
            removed += 1;
        }
    }
    let changed = changes.len() as u32;
    DiffStats { changed, total, ratio: if total == 0 { 0.0 } else { changed as f64 / total as f64 }, bounds: bounds.rect(), added, removed }
}

/// Image highlighting the changes: unchanged pixels dimmed, added green, removed red,
/// recolored yellow. Meant for visual inspection only.
pub fn diff_visual(before: &Image, after: &Image) -> Image {
    let w = before.width().max(after.width());
    let h = before.height().max(after.height());
    let mut out = Image::new(w, h);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let (b, a) = (before.get_or_clear(x, y), after.get_or_clear(x, y));
            let c = if a == b {
                if a.is_transparent() { Color::TRANSPARENT } else { Color::rgba(a.r, a.g, a.b, 70) }
            } else if b.is_transparent() {
                Color::rgb(60, 220, 90)
            } else if a.is_transparent() {
                Color::rgb(235, 60, 60)
            } else {
                Color::rgb(250, 210, 50)
            };
            out.set(x, y, c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_count_added_removed() {
        let mut a = Image::new(4, 4);
        a.set(0, 0, Color::BLACK);
        a.set(1, 0, Color::BLACK);
        let mut b = a.clone();
        b.set(0, 0, Color::TRANSPARENT);
        b.set(3, 3, Color::WHITE);
        b.set(1, 0, Color::WHITE);
        let s = diff_stats(&a, &b);
        assert_eq!(s.changed, 3);
        assert_eq!((s.added, s.removed), (1, 1));
        assert_eq!(s.bounds, Some(Rect::new(0, 0, 4, 4)));
        assert_eq!(diff_stats(&a, &a).changed, 0);
    }
}
