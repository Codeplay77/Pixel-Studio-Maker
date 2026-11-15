//! Selection masks: the magic wand, the lasso, combining selections (add, subtract,
//! intersect) and moving a selection along with the pixels a transform moved.
//!
//! Masks travel as text (`bounds` + `rows`, `#` selected, `.` not), the same layout as
//! [`Mask::to_rows`], so agents and the editor can read and send them back.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image, Mask, Point, Rect};

use crate::fill::{Connectivity, fill_region};
use crate::sdc::MAX_SHAPE_SIDE;
use crate::transform::{Transform, place};
use crate::{Canvas, DrawError, raster};

/// A selection mask as text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SelectionMask {
    /// Caixa da máscara: `x`,`y` é onde a primeira linha começa (`w`,`h` são informativos). `null` = seleção vazia.
    #[serde(default)]
    pub bounds: Option<Rect>,
    /// Linhas da máscara a partir do canto de `bounds`: `#` = selecionado, `.` = fora da seleção.
    #[serde(default)]
    pub rows: Vec<String>,
}

impl SelectionMask {
    /// The text form of a mask (empty mask: no bounds, no rows).
    pub fn from_mask(mask: &Mask) -> Self {
        let (bounds, rows) = mask.to_rows();
        SelectionMask { bounds, rows }
    }

    /// Decodes the rows onto a `width`×`height` canvas. Bits outside the canvas are dropped.
    pub fn to_mask(&self, width: u32, height: u32) -> Result<Mask, DrawError> {
        if self.rows.len() > MAX_SHAPE_SIDE as usize || self.rows.iter().any(|r| r.chars().count() > MAX_SHAPE_SIDE as usize) {
            return Err(DrawError::TooLarge(format!("máscara maior que {MAX_SHAPE_SIDE} px de lado")));
        }
        let origin = self.bounds.map(|b| Point::new(b.x, b.y)).unwrap_or_default();
        Ok(Mask::from_rows(width, height, origin, &self.rows))
    }
}

/// How a new selection combines with the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SelectMode {
    /// A nova seleção substitui a atual (padrão).
    #[default]
    Replace,
    /// Soma à seleção atual (Shift no editor).
    Add,
    /// Tira da seleção atual (Alt no editor).
    Subtract,
    /// Fica só o que está nas duas.
    Intersect,
}

/// Combines `new` with `base` (no base behaves as an empty selection).
pub fn combine(base: Option<&Mask>, new: &Mask, mode: SelectMode) -> Mask {
    match (base, mode) {
        (_, SelectMode::Replace) | (None, SelectMode::Add) => new.clone(),
        (None, _) => Mask::new(new.width(), new.height()),
        (Some(b), SelectMode::Add) => b.union(new),
        (Some(b), SelectMode::Subtract) => b.subtract(new),
        (Some(b), SelectMode::Intersect) => b.intersect(new),
    }
}

/// Magic wand: the pixels similar to the one at `start` (each channel within `tolerance`),
/// connected to it (`contiguous`) or anywhere in the image.
pub fn magic_wand(image: &Image, start: Point, contiguous: bool, tolerance: u8, connectivity: Connectivity) -> Mask {
    let (w, h) = (image.width(), image.height());
    if !image.in_bounds(start.x, start.y) {
        return Mask::new(w, h);
    }
    if !contiguous {
        let target: Color = image.get_or_clear(start.x, start.y);
        return Mask::from_color(image, target, tolerance);
    }
    let mut copy = image.clone();
    let canvas = Canvas::new(&mut copy, None);
    fill_region(&canvas, start, connectivity, tolerance, &Mask::full(w, h))
}

/// Lasso: the polygon through `points` (even-odd rule, edges included), clipped to the canvas.
pub fn lasso(width: u32, height: u32, points: &[Point]) -> Result<Mask, DrawError> {
    if points.is_empty() {
        return Err(DrawError::InvalidArgument("o laço precisa de pelo menos 1 ponto".into()));
    }
    if points.len() > crate::sdc::MAX_POINTS {
        return Err(DrawError::TooLarge(format!("{} pontos (máximo {})", points.len(), crate::sdc::MAX_POINTS)));
    }
    let span = |f: fn(&Point) -> i32| {
        let (lo, hi) = points.iter().map(f).fold((i32::MAX, i32::MIN), |(a, b), v| (a.min(v), b.max(v)));
        (hi as i64 - lo as i64) as u64
    };
    if span(|p| p.x) > MAX_SHAPE_SIDE as u64 || span(|p| p.y) > MAX_SHAPE_SIDE as u64 {
        return Err(DrawError::TooLarge(format!("laço maior que {MAX_SHAPE_SIDE} px de lado")));
    }
    let mut mask = Mask::new(width, height);
    for p in raster::polygon_fill(points) {
        mask.set(p.x, p.y, true);
    }
    Ok(mask)
}

/// Where a selection goes when `t` moves its pixels: the same mapping the pixel transform uses,
/// applied to every selected pixel (transparent ones included). Pixels leaving the canvas are lost.
pub fn transform_mask(mask: &Mask, t: Transform) -> Result<Mask, DrawError> {
    let Some(bounds) = mask.bounds() else { return Ok(mask.clone()) };
    let lifted: Vec<(Point, Color)> = mask.points().map(|p| (p, Color::BLACK)).collect();
    let placed = place(&lifted, bounds, t, mask.width(), mask.height())?;
    let mut out = Mask::new(mask.width(), mask.height());
    for (p, c) in placed {
        if !c.is_transparent() {
            out.set(p.x, p.y, true);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_roundtrip() {
        let mut m = Mask::new(6, 6);
        m.set(1, 2, true);
        m.set(3, 3, true);
        let text = SelectionMask::from_mask(&m);
        assert_eq!(text.bounds, Some(Rect::new(1, 2, 3, 2)));
        assert_eq!(text.rows, vec!["#..".to_string(), "..#".to_string()]);
        assert_eq!(text.to_mask(6, 6).unwrap(), m);
        assert!(SelectionMask::default().to_mask(6, 6).unwrap().is_empty());
    }

    #[test]
    fn combine_modes() {
        let a = Mask::from_rect(4, 4, Rect::new(0, 0, 2, 2));
        let b = Mask::from_rect(4, 4, Rect::new(1, 1, 2, 2));
        assert_eq!(combine(Some(&a), &b, SelectMode::Replace).count(), 4);
        assert_eq!(combine(Some(&a), &b, SelectMode::Add).count(), 7);
        assert_eq!(combine(Some(&a), &b, SelectMode::Subtract).count(), 3);
        assert_eq!(combine(Some(&a), &b, SelectMode::Intersect).count(), 1);
        assert_eq!(combine(None, &b, SelectMode::Add).count(), 4);
        assert!(combine(None, &b, SelectMode::Subtract).is_empty());
    }

    #[test]
    fn transform_moves_the_selection_with_the_pixels() {
        let m = Mask::from_rows(5, 5, Point::new(0, 0), &["##".to_string(), "#.".to_string()]);
        let flipped = transform_mask(&m, Transform::FlipH).unwrap();
        assert!(flipped.get(0, 0) && flipped.get(1, 0) && flipped.get(1, 1) && !flipped.get(0, 1));
        let moved = transform_mask(&m, Transform::Move { dx: 2, dy: 3, wrap: false }).unwrap();
        assert_eq!(moved.bounds(), Some(Rect::new(2, 3, 2, 2)));
        assert_eq!(moved.count(), 3);
    }
}
