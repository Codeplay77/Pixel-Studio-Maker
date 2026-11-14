//! Region transforms: lift the visible pixels of an area, transform them, put them back.
//! Everything is nearest-neighbor; no new colors are ever created.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Mask, Point, Rect};

use crate::{Canvas, DrawError};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transform {
    /// Mirror left↔right inside the area's bounding box.
    FlipH,
    /// Mirror top↔bottom inside the area's bounding box.
    FlipV,
    /// Rotate around the area center (or `center`). Use `quarter_turns` (90° clockwise each)
    /// for exact turns, or `degrees` (clockwise) for nearest-neighbor rotation.
    Rotate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quarter_turns: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        degrees: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        center: Option<Point>,
    },
    /// Shift by (dx, dy). With `wrap`, pixels leaving one side enter on the other.
    Move {
        dx: i32,
        dy: i32,
        #[serde(default)]
        wrap: bool,
    },
    /// Nearest-neighbor scale around `origin` (default: area top-left). Negative factors flip.
    Scale {
        sx: f64,
        sy: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<Point>,
    },
}

/// Applies `t` to the visible pixels of `area`. Pixels moved away leave transparency behind.
pub fn transform_area(canvas: &mut Canvas, area: &Mask, t: Transform) -> Result<(), DrawError> {
    let Some(b) = area.bounds() else { return Ok(()) };
    let lifted: Vec<(Point, Color)> = area.points().map(|p| (p, canvas.get(p.x, p.y))).filter(|(_, c)| !c.is_transparent()).collect();
    if lifted.is_empty() {
        return Ok(());
    }
    let placed = place(&lifted, b, t, canvas.width(), canvas.height())?;
    for (p, _) in &lifted {
        canvas.put(p.x, p.y, Color::TRANSPARENT);
    }
    for (p, c) in placed {
        canvas.put(p.x, p.y, c);
    }
    Ok(())
}

/// Computes where each lifted pixel goes (and, for resampling transforms, which pixels appear).
pub fn place(lifted: &[(Point, Color)], b: Rect, t: Transform, width: u32, height: u32) -> Result<Vec<(Point, Color)>, DrawError> {
    let (bx0, by0, bx1, by1) = (b.x, b.y, b.right() - 1, b.bottom() - 1);
    let out = match t {
        Transform::FlipH => lifted.iter().map(|(p, c)| (Point::new(bx0 + bx1 - p.x, p.y), *c)).collect(),
        Transform::FlipV => lifted.iter().map(|(p, c)| (Point::new(p.x, by0 + by1 - p.y), *c)).collect(),
        Transform::Move { dx, dy, wrap } => lifted
            .iter()
            .map(|(p, c)| {
                let mut q = p.offset(dx, dy);
                if wrap {
                    q = Point::new(q.x.rem_euclid(width as i32), q.y.rem_euclid(height as i32));
                }
                (q, *c)
            })
            .collect(),
        Transform::Rotate { quarter_turns: Some(q), degrees: None, center: None } => {
            let (w, h) = (b.w as i32, b.h as i32);
            let turns = q.rem_euclid(4);
            let (nw, nh) = if turns % 2 == 1 { (h, w) } else { (w, h) };
            let (ox, oy) = (b.x + (w - nw) / 2, b.y + (h - nh) / 2);
            lifted
                .iter()
                .map(|(p, c)| {
                    let (lx, ly) = (p.x - b.x, p.y - b.y);
                    let (rx, ry) = match turns {
                        0 => (lx, ly),
                        1 => (h - 1 - ly, lx),
                        2 => (w - 1 - lx, h - 1 - ly),
                        _ => (ly, w - 1 - lx),
                    };
                    (Point::new(ox + rx, oy + ry), *c)
                })
                .collect()
        }
        Transform::Rotate { quarter_turns, degrees, center } => {
            let deg = degrees.unwrap_or(0.0) + quarter_turns.unwrap_or(0) as f64 * 90.0;
            let (cx, cy) = match center {
                Some(c) => (c.x as f64 + 0.5, c.y as f64 + 0.5),
                None => (b.x as f64 + b.w as f64 / 2.0, b.y as f64 + b.h as f64 / 2.0),
            };
            let (s, c) = deg.to_radians().sin_cos();
            resample(
                lifted,
                b,
                |x, y| {
                    // Inverse rotation (counter-clockwise by deg) of a destination center.
                    let (dx, dy) = (x - cx, y - cy);
                    (cx + dx * c + dy * s, cy - dx * s + dy * c)
                },
                |x, y| {
                    let (dx, dy) = (x - cx, y - cy);
                    (cx + dx * c - dy * s, cy + dx * s + dy * c)
                },
            )
        }
        Transform::Scale { sx, sy, origin } => {
            if !(sx.abs() >= 0.01 && sy.abs() >= 0.01 && sx.abs() <= 64.0 && sy.abs() <= 64.0) {
                return Err(DrawError::InvalidArgument("escala precisa ficar entre 0.01 e 64 (pode ser negativa)".into()));
            }
            let (ox, oy) = match origin {
                Some(o) => (o.x as f64, o.y as f64),
                None => (b.x as f64, b.y as f64),
            };
            resample(lifted, b, |x, y| (ox + (x - ox) / sx, oy + (y - oy) / sy), |x, y| (ox + (x - ox) * sx, oy + (y - oy) * sy))
        }
    };
    Ok(out)
}

/// Nearest-neighbor resampling. `inverse` maps a destination pixel center to source coordinates;
/// `forward` maps source coordinates to destination (used to find the destination bounds).
fn resample(
    lifted: &[(Point, Color)],
    b: Rect,
    inverse: impl Fn(f64, f64) -> (f64, f64),
    forward: impl Fn(f64, f64) -> (f64, f64),
) -> Vec<(Point, Color)> {
    let lookup: std::collections::HashMap<(i32, i32), Color> = lifted.iter().map(|(p, c)| ((p.x, p.y), *c)).collect();
    let corners =
        [(b.x as f64, b.y as f64), (b.right() as f64, b.y as f64), (b.x as f64, b.bottom() as f64), (b.right() as f64, b.bottom() as f64)];
    let mapped: Vec<(f64, f64)> = corners.iter().map(|(x, y)| forward(*x, *y)).collect();
    let x0 = mapped.iter().map(|m| m.0).fold(f64::INFINITY, f64::min).floor() as i32 - 1;
    let x1 = mapped.iter().map(|m| m.0).fold(f64::NEG_INFINITY, f64::max).ceil() as i32 + 1;
    let y0 = mapped.iter().map(|m| m.1).fold(f64::INFINITY, f64::min).floor() as i32 - 1;
    let y1 = mapped.iter().map(|m| m.1).fold(f64::NEG_INFINITY, f64::max).ceil() as i32 + 1;
    let mut out = Vec::new();
    for y in y0.max(-8192)..y1.min(8192) {
        for x in x0.max(-8192)..x1.min(8192) {
            let (sx, sy) = inverse(x as f64 + 0.5, y as f64 + 0.5);
            if let Some(c) = lookup.get(&(sx.floor() as i32, sy.floor() as i32)) {
                out.push((Point::new(x, y), *c));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Image;

    fn arrow() -> Image {
        // 4x3 at (1,1): a horizontal bar with a tip.
        let mut img = Image::new(8, 8);
        for x in 1..5 {
            img.set(x, 2, Color::BLACK);
        }
        img.set(4, 1, Color::rgb(255, 0, 0));
        img.set(4, 3, Color::rgb(0, 0, 255));
        img
    }

    fn run(img: &mut Image, area: Option<Rect>, t: Transform) {
        let mask = match area {
            Some(r) => Mask::from_rect(img.width(), img.height(), r),
            None => Mask::full(img.width(), img.height()),
        };
        let mut c = Canvas::new(img, None);
        transform_area(&mut c, &mask, t).unwrap();
    }

    #[test]
    fn flips_mirror_inside_area() {
        let mut img = arrow();
        run(&mut img, Some(Rect::new(1, 1, 4, 3)), Transform::FlipH);
        assert_eq!(img.get(1, 1), Some(Color::rgb(255, 0, 0)));
        assert_eq!(img.get(4, 1), Some(Color::TRANSPARENT));
        assert_eq!(img.opaque_count(), 6);
        run(&mut img, Some(Rect::new(1, 1, 4, 3)), Transform::FlipH);
        assert_eq!(img, arrow());
    }

    #[test]
    fn quarter_rotation_is_exact_and_reversible() {
        let mut img = arrow();
        run(&mut img, Some(Rect::new(1, 1, 4, 3)), Transform::Rotate { quarter_turns: Some(1), degrees: None, center: None });
        assert_eq!(img.opaque_count(), 6);
        // Clockwise: the bar becomes vertical.
        let b = img.opaque_bounds().unwrap();
        assert_eq!((b.w, b.h), (3, 4));
        run(&mut img, Some(b), Transform::Rotate { quarter_turns: Some(3), degrees: None, center: None });
        assert_eq!(img.opaque_count(), 6);
        assert_eq!(img.opaque_bounds().unwrap(), Rect::new(1, 1, 4, 3));
    }

    #[test]
    fn move_and_wrap() {
        let mut img = arrow();
        run(&mut img, None, Transform::Move { dx: 2, dy: 0, wrap: false });
        assert_eq!(img.get(6, 1), Some(Color::rgb(255, 0, 0)));
        let mut img = arrow();
        run(&mut img, None, Transform::Move { dx: 5, dy: 0, wrap: true });
        assert_eq!(img.get(1, 1), Some(Color::rgb(255, 0, 0)), "x=4+5 wraps to 1");
        assert_eq!(img.opaque_count(), 6);
    }

    #[test]
    fn scale_by_two_doubles_pixels() {
        let mut img = Image::new(8, 8);
        img.set(1, 1, Color::BLACK);
        img.set(2, 1, Color::WHITE);
        run(&mut img, None, Transform::Scale { sx: 2.0, sy: 2.0, origin: Some(Point::new(1, 1)) });
        assert_eq!(img.opaque_count(), 8);
        assert_eq!(img.get(2, 2), Some(Color::BLACK));
        assert_eq!(img.get(3, 1), Some(Color::WHITE));
        assert!(Canvas::new(&mut img.clone(), None).area(None).count() > 0);
    }

    #[test]
    fn free_rotation_keeps_colors() {
        let mut img = arrow();
        run(&mut img, None, Transform::Rotate { quarter_turns: None, degrees: Some(45.0), center: None });
        let colors = img.color_counts();
        assert!(colors.keys().all(|c| [Color::BLACK, Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)].contains(c)));
        assert!(img.opaque_count() >= 4);
    }
}
