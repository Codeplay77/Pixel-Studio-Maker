//! Artistic operations: outlines, dithering, discrete gradients, light-based shading, drop
//! shadows, tiled patterns, cleanup of stray pixels, color quantization and HSV adjustments.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::matrix::Grid;
use sprite_core::{Color, Mask, Palette, Point};

use crate::Canvas;
use crate::fill::Connectivity;

// ---------------------------------------------------------------------------------------------
// Outline

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutlineMode {
    /// Paint transparent pixels touching the shape (the outline grows the sprite by 1 px).
    #[default]
    Outside,
    /// Paint the shape's own border pixels.
    Inside,
}

/// Outlines every shape formed by non-transparent pixels. `area` limits where it paints.
pub fn outline(canvas: &mut Canvas, color: Color, mode: OutlineMode, connectivity: Connectivity, thickness: u32, area: &Mask) {
    for _ in 0..thickness.max(1) {
        let shape = Mask::from_opaque(canvas.image());
        let mut targets = Vec::new();
        for y in 0..canvas.height() as i32 {
            for x in 0..canvas.width() as i32 {
                if !area.get(x, y) {
                    continue;
                }
                let inside = shape.get(x, y);
                let touches = |want: bool| {
                    connectivity.offsets().iter().any(|(dx, dy)| {
                        let (nx, ny) = (x + dx, y + dy);
                        let in_img = canvas.image().in_bounds(nx, ny);
                        if want { in_img && shape.get(nx, ny) } else { !in_img || !shape.get(nx, ny) }
                    })
                };
                let hit = match mode {
                    OutlineMode::Outside => !inside && touches(true),
                    OutlineMode::Inside => inside && touches(false),
                };
                if hit {
                    targets.push(Point::new(x, y));
                }
            }
        }
        canvas.put_all(targets, color);
    }
}

// ---------------------------------------------------------------------------------------------
// Dithering

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DitherPattern {
    /// 2×2 ordered (Bayer) matrix.
    Bayer2,
    /// 4×4 ordered (Bayer) matrix — the usual choice.
    #[default]
    Bayer4,
    /// 8×8 ordered (Bayer) matrix, smoothest steps.
    Bayer8,
    /// Plain checkerboard (50%).
    Checker,
    /// Horizontal lines.
    LinesH,
    /// Vertical lines.
    LinesV,
    /// Diagonal lines.
    Diagonal,
}

const BAYER2: [[u8; 2]; 2] = [[0, 2], [3, 1]];
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Bayer matrix value of size 2^order at (x, y), built by bit interleaving: the lowest coordinate
/// bits pick the most significant 2×2 digit. order 2 equals `BAYER4`.
fn bayer(order: u32, x: usize, y: usize) -> u32 {
    let mut v = 0;
    for bit in 0..order {
        let (xb, yb) = ((x >> bit) as u32 & 1, (y >> bit) as u32 & 1);
        v |= (((xb ^ yb) << 1) | yb) << (2 * (order - 1 - bit));
    }
    v
}

fn bayer8(x: usize, y: usize) -> u8 {
    bayer(3, x % 8, y % 8) as u8
}

impl DitherPattern {
    /// Threshold in 0..1 for pixel (x, y); a pixel is "on" when `density > threshold`.
    pub fn threshold(self, x: i32, y: i32) -> f64 {
        let (ux, uy) = (x.rem_euclid(64) as usize, y.rem_euclid(64) as usize);
        match self {
            DitherPattern::Bayer2 => (BAYER2[uy % 2][ux % 2] as f64 + 0.5) / 4.0,
            DitherPattern::Bayer4 => (BAYER4[uy % 4][ux % 4] as f64 + 0.5) / 16.0,
            DitherPattern::Bayer8 => (bayer8(ux, uy) as f64 + 0.5) / 64.0,
            DitherPattern::Checker => {
                if (ux + uy) % 2 == 0 {
                    0.25
                } else {
                    0.75
                }
            }
            DitherPattern::LinesH => {
                if uy % 2 == 0 {
                    0.25
                } else {
                    0.75
                }
            }
            DitherPattern::LinesV => {
                if ux % 2 == 0 {
                    0.25
                } else {
                    0.75
                }
            }
            DitherPattern::Diagonal => (((ux + uy) % 4) as f64 + 0.5) / 4.0,
        }
    }
}

/// Ordered dither: pixels where the pattern is "on" at `density` get `color`; the others get
/// `background` (or stay as they are). `only_opaque` restricts it to existing visible pixels.
pub fn dither(
    canvas: &mut Canvas,
    area: &Mask,
    color: Color,
    background: Option<Color>,
    density: f64,
    pattern: DitherPattern,
    only_opaque: bool,
) {
    let density = density.clamp(0.0, 1.0);
    let points: Vec<Point> = area.points().collect();
    for p in points {
        if only_opaque && canvas.get(p.x, p.y).is_transparent() {
            continue;
        }
        if density > pattern.threshold(p.x, p.y) {
            canvas.put(p.x, p.y, color);
        } else if let Some(bg) = background {
            canvas.put(p.x, p.y, bg);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Gradients

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GradientShape {
    /// Straight bands. `angle` in degrees: 0 = left→right, 90 = top→bottom.
    Linear {
        #[serde(default)]
        angle: f64,
    },
    /// Rings around `center`, from the first color (center) to the last at `radius`.
    Radial { center: Point, radius: f64 },
}

/// Discrete gradient across the area's bounding box using exactly the given colors (bands).
/// With `dither`, band edges are blended with an ordered pattern instead of a hard step.
pub fn gradient(
    canvas: &mut Canvas,
    area: &Mask,
    colors: &[Color],
    shape: GradientShape,
    dither: Option<DitherPattern>,
    only_opaque: bool,
) {
    if colors.is_empty() {
        return;
    }
    let Some(b) = area.bounds() else { return };
    let n = colors.len();
    let points: Vec<Point> = area.points().collect();
    for p in points {
        if only_opaque && canvas.get(p.x, p.y).is_transparent() {
            continue;
        }
        let t = match shape {
            GradientShape::Linear { angle } => {
                let (s, c) = angle.to_radians().sin_cos();
                let corners = [(b.x, b.y), (b.right() - 1, b.y), (b.x, b.bottom() - 1), (b.right() - 1, b.bottom() - 1)];
                let proj = |x: i32, y: i32| x as f64 * c + y as f64 * s;
                let lo = corners.iter().map(|(x, y)| proj(*x, *y)).fold(f64::INFINITY, f64::min);
                let hi = corners.iter().map(|(x, y)| proj(*x, *y)).fold(f64::NEG_INFINITY, f64::max);
                if hi - lo < 1e-9 { 0.0 } else { (proj(p.x, p.y) - lo) / (hi - lo) }
            }
            GradientShape::Radial { center, radius } => {
                let d = (((p.x - center.x).pow(2) + (p.y - center.y).pow(2)) as f64).sqrt();
                if radius <= 0.0 { 1.0 } else { d / radius }
            }
        }
        .clamp(0.0, 1.0);
        let color = match dither {
            None => colors[((t * n as f64) as usize).min(n - 1)],
            Some(pattern) => {
                if n == 1 {
                    colors[0]
                } else {
                    let pos = t * (n - 1) as f64;
                    let i = (pos.floor() as usize).min(n - 2);
                    let frac = pos - i as f64;
                    if frac > pattern.threshold(p.x, p.y) { colors[i + 1] } else { colors[i] }
                }
            }
        };
        canvas.put(p.x, p.y, color);
    }
}

// ---------------------------------------------------------------------------------------------
// Shading

/// Where the light comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LightDirection {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl LightDirection {
    /// Unit step pointing toward the light.
    pub fn toward_light(self) -> (i32, i32) {
        match self {
            LightDirection::TopLeft => (-1, -1),
            LightDirection::Top => (0, -1),
            LightDirection::TopRight => (1, -1),
            LightDirection::Left => (-1, 0),
            LightDirection::Right => (1, 0),
            LightDirection::BottomLeft => (-1, 1),
            LightDirection::Bottom => (0, 1),
            LightDirection::BottomRight => (1, 1),
        }
    }
}

/// How shading picks its colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShadeColor {
    /// Always this color.
    Fixed(Color),
    /// The pixel's own color lightened (> 0) or darkened (< 0) by this amount, hue-shifted.
    Relative(f64, bool),
}

impl ShadeColor {
    fn pick(self, base: Color) -> Color {
        match self {
            ShadeColor::Fixed(c) => c,
            ShadeColor::Relative(amount, hue_shift) => base.shade(amount, hue_shift),
        }
    }
}

/// Rim shading from a light direction: pixels of the shape whose neighbor toward the light is
/// outside the shape become highlights; those whose neighbor away from the light is outside
/// become shadows. `depth` widens each band. Works on the visible pixels inside `area`.
pub fn shade(
    canvas: &mut Canvas,
    area: &Mask,
    light: LightDirection,
    highlight: Option<ShadeColor>,
    shadow: Option<ShadeColor>,
    highlight_depth: u32,
    shadow_depth: u32,
) {
    let shape = Mask::from_opaque(canvas.image()).intersect(area);
    let (lx, ly) = light.toward_light();
    let snapshot = canvas.image().clone();
    let edge_within = |p: Point, dx: i32, dy: i32, depth: u32| (1..=depth.max(1) as i32).any(|k| !shape.get(p.x + dx * k, p.y + dy * k));
    let mut writes = Vec::new();
    for p in shape.points() {
        let base = snapshot.get_or_clear(p.x, p.y);
        let lit = highlight.is_some() && highlight_depth > 0 && edge_within(p, lx, ly, highlight_depth);
        let dark = shadow.is_some() && shadow_depth > 0 && edge_within(p, -lx, -ly, shadow_depth);
        let new = match (lit, dark) {
            (true, false) => highlight.map(|h| h.pick(base)),
            (false, true) => shadow.map(|s| s.pick(base)),
            _ => None,
        };
        if let Some(c) = new {
            writes.push((p, c));
        }
    }
    for (p, c) in writes {
        canvas.put(p.x, p.y, c);
    }
}

/// Paints the silhouette shifted by (dx, dy) in `color`, only on transparent pixels — a cast
/// shadow behind the sprite.
pub fn drop_shadow(canvas: &mut Canvas, area: &Mask, dx: i32, dy: i32, color: Color) {
    let shape = Mask::from_opaque(canvas.image()).intersect(area);
    let targets: Vec<Point> = shape.points().map(|p| p.offset(dx, dy)).filter(|p| canvas.get(p.x, p.y).is_transparent()).collect();
    canvas.put_all(targets, color);
}

// ---------------------------------------------------------------------------------------------
// Patterns, cleanup, color operations

/// Tiles a cell grid across the area, anchored at `offset`.
pub fn pattern(canvas: &mut Canvas, area: &Mask, grid: &Grid, offset: Point, only_opaque: bool) {
    if grid.width == 0 || grid.height == 0 {
        return;
    }
    let points: Vec<Point> = area.points().collect();
    for p in points {
        let existing = canvas.get(p.x, p.y);
        if only_opaque && existing.is_transparent() {
            continue;
        }
        let gx = (p.x - offset.x).rem_euclid(grid.width as i32) as u32;
        let gy = (p.y - offset.y).rem_euclid(grid.height as i32) as u32;
        let new = grid.get(gx, gy).resolve(existing);
        canvas.put(p.x, p.y, new);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CleanupMode {
    /// Remove visible pixels with no visible neighbor (8 directions).
    #[default]
    Orphans,
    /// Recolor single pixels whose 4 neighbors all share another color.
    Noise,
}

pub fn cleanup(canvas: &mut Canvas, area: &Mask, mode: CleanupMode) {
    let snap = canvas.image().clone();
    let mut writes = Vec::new();
    for p in area.points() {
        let c = snap.get_or_clear(p.x, p.y);
        match mode {
            CleanupMode::Orphans => {
                if !c.is_transparent()
                    && Connectivity::Eight.offsets().iter().all(|(dx, dy)| snap.get_or_clear(p.x + dx, p.y + dy).is_transparent())
                {
                    writes.push((p, Color::TRANSPARENT));
                }
            }
            CleanupMode::Noise => {
                let around: Vec<Color> = Connectivity::Four.offsets().iter().filter_map(|(dx, dy)| snap.get(p.x + dx, p.y + dy)).collect();
                if around.len() == 4 && around.iter().all(|n| *n == around[0]) && around[0] != c {
                    writes.push((p, around[0]));
                }
            }
        }
    }
    for (p, c) in writes {
        canvas.put(p.x, p.y, c);
    }
}

/// Maps every visible pixel to its nearest palette color (alpha kept as is).
pub fn quantize_to(canvas: &mut Canvas, area: &Mask, palette: &Palette) {
    if palette.colors.is_empty() {
        return;
    }
    let points: Vec<Point> = area.points().collect();
    for p in points {
        let c = canvas.get(p.x, p.y);
        if c.is_transparent() {
            continue;
        }
        if let Some((_, n)) = palette.nearest(Color::rgb(c.r, c.g, c.b)) {
            canvas.put(p.x, p.y, Color::rgba(n.r, n.g, n.b, c.a));
        }
    }
}

/// Shifts hue (degrees), saturation and value (each -1..1) of visible pixels.
pub fn adjust_hsv(canvas: &mut Canvas, area: &Mask, hue: f64, saturation: f64, value: f64) {
    let points: Vec<Point> = area.points().collect();
    for p in points {
        let c = canvas.get(p.x, p.y);
        if c.is_transparent() {
            continue;
        }
        let (h, s, v) = c.to_hsv();
        let n = Color::from_hsv(h + hue, (s + saturation).clamp(0.0, 1.0), (v + value).clamp(0.0, 1.0), c.a);
        canvas.put(p.x, p.y, n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::{Image, Rect};

    fn square() -> Image {
        let mut img = Image::new(6, 6);
        for p in Rect::new(1, 1, 4, 4).points() {
            img.set(p.x, p.y, Color::rgb(100, 100, 200));
        }
        img
    }

    #[test]
    fn outline_outside_and_inside() {
        let mut img = square();
        let area = Mask::full(6, 6);
        let mut c = Canvas::new(&mut img, None);
        outline(&mut c, Color::BLACK, OutlineMode::Outside, Connectivity::Four, 1, &area);
        // 4-connectivity: a ring of 16 without corners.
        assert_eq!(c.stats().changed, 16);
        assert_eq!(img.get(0, 0), Some(Color::TRANSPARENT));
        assert_eq!(img.get(0, 1), Some(Color::BLACK));

        let mut img = square();
        let mut c = Canvas::new(&mut img, None);
        outline(&mut c, Color::BLACK, OutlineMode::Outside, Connectivity::Eight, 1, &area);
        assert_eq!(c.stats().changed, 20);

        let mut img = square();
        let mut c = Canvas::new(&mut img, None);
        outline(&mut c, Color::BLACK, OutlineMode::Inside, Connectivity::Four, 1, &area);
        assert_eq!(c.stats().changed, 12);
        assert_eq!(img.get(2, 2), Some(Color::rgb(100, 100, 200)));
    }

    #[test]
    fn dither_density() {
        let mut img = Image::new(8, 8);
        let area = Mask::full(8, 8);
        let mut c = Canvas::new(&mut img, None);
        dither(&mut c, &area, Color::BLACK, None, 0.5, DitherPattern::Bayer4, false);
        assert_eq!(c.stats().changed, 32);
        for pattern in [DitherPattern::Bayer2, DitherPattern::Bayer8, DitherPattern::Checker, DitherPattern::Diagonal] {
            let mut img = Image::new(8, 8);
            let mut c = Canvas::new(&mut img, None);
            dither(&mut c, &area, Color::BLACK, None, 0.5, pattern, false);
            assert_eq!(c.stats().changed, 32, "{pattern:?}");
        }
        // The generic construction matches the 4x4 table.
        for (y, row) in BAYER4.iter().enumerate() {
            for (x, v) in row.iter().enumerate() {
                assert_eq!(bayer(2, x, y), *v as u32);
            }
        }
        // Bayer8 thresholds are a permutation of 0..64.
        let mut seen: Vec<u8> = (0..8).flat_map(|y| (0..8).map(move |x| bayer8(x, y))).collect();
        seen.sort();
        assert_eq!(seen, (0..64).collect::<Vec<u8>>());
    }

    #[test]
    fn gradient_uses_only_given_colors() {
        let colors = [Color::BLACK, Color::rgb(128, 128, 128), Color::WHITE];
        let mut img = Image::new(9, 3);
        let area = Mask::full(9, 3);
        let mut c = Canvas::new(&mut img, None);
        gradient(&mut c, &area, &colors, GradientShape::Linear { angle: 0.0 }, None, false);
        assert_eq!(img.get(0, 0), Some(Color::BLACK));
        assert_eq!(img.get(4, 1), Some(Color::rgb(128, 128, 128)));
        assert_eq!(img.get(8, 2), Some(Color::WHITE));
        assert!(img.color_counts().keys().all(|c| colors.contains(c)));
        let mut img2 = Image::new(9, 3);
        let mut c = Canvas::new(&mut img2, None);
        gradient(&mut c, &area, &colors, GradientShape::Linear { angle: 0.0 }, Some(DitherPattern::Bayer4), false);
        assert!(img2.color_counts().keys().all(|c| colors.contains(c)));
    }

    #[test]
    fn shading_marks_lit_and_dark_sides() {
        let mut img = square();
        let area = Mask::full(6, 6);
        let mut c = Canvas::new(&mut img, None);
        shade(&mut c, &area, LightDirection::Top, Some(ShadeColor::Fixed(Color::WHITE)), Some(ShadeColor::Fixed(Color::BLACK)), 1, 1);
        assert_eq!(img.get(2, 1), Some(Color::WHITE));
        assert_eq!(img.get(2, 4), Some(Color::BLACK));
        assert_eq!(img.get(2, 2), Some(Color::rgb(100, 100, 200)));
    }

    #[test]
    fn drop_shadow_goes_behind() {
        let mut img = square();
        let area = Mask::full(6, 6);
        let mut c = Canvas::new(&mut img, None);
        drop_shadow(&mut c, &area, 1, 1, Color::BLACK);
        assert_eq!(c.stats().changed, 7);
        assert_eq!(img.get(5, 5), Some(Color::BLACK));
        assert_eq!(img.get(2, 2), Some(Color::rgb(100, 100, 200)), "shape not overwritten");
    }

    #[test]
    fn cleanup_modes() {
        let mut img = Image::new(5, 5);
        img.set(0, 0, Color::BLACK);
        img.set(3, 3, Color::BLACK);
        img.set(4, 3, Color::BLACK);
        let area = Mask::full(5, 5);
        let mut c = Canvas::new(&mut img, None);
        cleanup(&mut c, &area, CleanupMode::Orphans);
        assert_eq!(c.stats().changed, 1);
        let mut noisy = Image::filled(3, 3, Color::WHITE);
        noisy.set(1, 1, Color::BLACK);
        let area = Mask::full(3, 3);
        let mut c = Canvas::new(&mut noisy, None);
        cleanup(&mut c, &area, CleanupMode::Noise);
        assert_eq!(noisy.get(1, 1), Some(Color::WHITE));
    }

    #[test]
    fn quantize_maps_to_palette() {
        let mut img = Image::filled(2, 1, Color::rgb(250, 5, 5));
        img.set(1, 0, Color::rgba(10, 10, 10, 128));
        let pal = Palette::new("p", vec![Color::rgb(255, 0, 0), Color::BLACK]);
        let area = Mask::full(2, 1);
        let mut c = Canvas::new(&mut img, None);
        quantize_to(&mut c, &area, &pal);
        assert_eq!(img.get(0, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(img.get(1, 0), Some(Color::rgba(0, 0, 0, 128)));
    }
}
