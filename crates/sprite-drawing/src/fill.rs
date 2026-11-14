//! Flood fill and global color replacement.

use std::collections::VecDeque;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Mask, Point};

use crate::Canvas;

/// Which neighbors count as connected: 4 (up, down, left, right) or 8 (also diagonals).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Connectivity {
    #[default]
    Four,
    Eight,
}

impl Serialize for Connectivity {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(if *self == Connectivity::Four { 4 } else { 8 })
    }
}

impl<'de> Deserialize<'de> for Connectivity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            N(u8),
            S(String),
        }
        match Repr::deserialize(d)? {
            Repr::N(4) => Ok(Connectivity::Four),
            Repr::N(8) => Ok(Connectivity::Eight),
            Repr::S(s) if s == "4" => Ok(Connectivity::Four),
            Repr::S(s) if s == "8" => Ok(Connectivity::Eight),
            _ => Err(serde::de::Error::custom("connectivity precisa ser 4 ou 8")),
        }
    }
}

impl JsonSchema for Connectivity {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Connectivity".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "4 = vizinhos em cruz; 8 = inclui diagonais.",
            "enum": [4, 8]
        })
    }
}

impl Connectivity {
    pub fn offsets(self) -> &'static [(i32, i32)] {
        match self {
            Connectivity::Four => &[(1, 0), (-1, 0), (0, 1), (0, -1)],
            Connectivity::Eight => &[(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)],
        }
    }
}

/// Mask of the pixels a fill starting at `start` would reach (within `area`).
pub fn fill_region(canvas: &Canvas, start: Point, connectivity: Connectivity, tolerance: u8, area: &Mask) -> Mask {
    let (w, h) = (canvas.width(), canvas.height());
    let mut reached = Mask::new(w, h);
    if !canvas.image().in_bounds(start.x, start.y) || !area.get(start.x, start.y) {
        return reached;
    }
    let target = canvas.get(start.x, start.y);
    let mut queue = VecDeque::from([start]);
    reached.set(start.x, start.y, true);
    while let Some(p) = queue.pop_front() {
        for (dx, dy) in connectivity.offsets() {
            let (x, y) = (p.x + dx, p.y + dy);
            if canvas.image().in_bounds(x, y)
                && !reached.get(x, y)
                && area.get(x, y)
                && canvas.get(x, y).max_channel_delta(target) <= tolerance
            {
                reached.set(x, y, true);
                queue.push_back(Point::new(x, y));
            }
        }
    }
    reached
}

/// Paint bucket. `contiguous = false` recolors every matching pixel in the area.
pub fn flood_fill(
    canvas: &mut Canvas,
    start: Point,
    color: Color,
    connectivity: Connectivity,
    tolerance: u8,
    contiguous: bool,
    area: &Mask,
) {
    if !canvas.image().in_bounds(start.x, start.y) {
        return;
    }
    let region = if contiguous {
        fill_region(canvas, start, connectivity, tolerance, area)
    } else {
        let target = canvas.get(start.x, start.y);
        Mask::from_color(canvas.image(), target, tolerance).intersect(area)
    };
    let points: Vec<Point> = region.points().collect();
    canvas.put_all(points, color);
}

/// Replaces `from` (within `tolerance`) by `to` across the area.
pub fn replace_color(canvas: &mut Canvas, from: Color, to: Color, tolerance: u8, area: &Mask) {
    let matches: Vec<Point> = Mask::from_color(canvas.image(), from, tolerance).intersect(area).points().collect();
    canvas.put_all(matches, to);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Image;

    #[test]
    fn fill_stops_at_walls() {
        // A 5x5 box with a wall at column 2.
        let mut img = Image::new(5, 5);
        for y in 0..5 {
            img.set(2, y, Color::BLACK);
        }
        let area = Mask::full(5, 5);
        let mut canvas = Canvas::new(&mut img, None);
        flood_fill(&mut canvas, Point::new(0, 0), Color::WHITE, Connectivity::Four, 0, true, &area);
        assert_eq!(canvas.stats().changed, 10);
        assert_eq!(img.get(3, 0), Some(Color::TRANSPARENT));
        assert_eq!(img.get(1, 4), Some(Color::WHITE));
    }

    #[test]
    fn diagonal_leak_only_with_eight() {
        // Diagonal wall: 4-connectivity cannot cross it, 8 can.
        let mut img = Image::new(3, 3);
        img.set(0, 1, Color::BLACK);
        img.set(1, 0, Color::BLACK);
        let area = Mask::full(3, 3);
        let four = fill_region(&Canvas::new(&mut img.clone(), None), Point::new(0, 0), Connectivity::Four, 0, &area);
        assert_eq!(four.count(), 1);
        let eight = fill_region(&Canvas::new(&mut img, None), Point::new(0, 0), Connectivity::Eight, 0, &area);
        assert_eq!(eight.count(), 7);
    }

    #[test]
    fn global_replace_and_tolerance() {
        let mut img = Image::filled(4, 1, Color::rgb(100, 0, 0));
        img.set(1, 0, Color::rgb(104, 0, 0));
        img.set(2, 0, Color::BLACK);
        let area = Mask::full(4, 1);
        let mut canvas = Canvas::new(&mut img, None);
        replace_color(&mut canvas, Color::rgb(100, 0, 0), Color::WHITE, 5, &area);
        assert_eq!(canvas.stats().changed, 3);
        assert_eq!(img.get(2, 0), Some(Color::BLACK));
    }

    #[test]
    fn clip_is_respected() {
        let mut img = Image::new(4, 4);
        let clip = Mask::from_rect(4, 4, sprite_core::Rect::new(0, 0, 2, 2));
        let area = Mask::full(4, 4);
        let mut canvas = Canvas::new(&mut img, Some(&clip));
        flood_fill(&mut canvas, Point::new(0, 0), Color::WHITE, Connectivity::Four, 0, true, &area);
        assert_eq!(canvas.stats().changed, 4);
    }
}
