//! Normal maps for 2D lighting (Godot's `CanvasTexture`): a height field from the sprite
//! (distance to the transparent edge, brightness, or both), Sobel gradients and normals encoded
//! as RGB. A flat area is (128, 128, 255); alpha is copied from the sprite.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image};

/// Como calcular a altura de cada pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HeightMode {
    /// Distância até a borda transparente, arredondada por `bevel` px: o sprite vira uma
    /// "almofada" com as bordas inclinadas para fora.
    #[default]
    Bevel,
    /// Brilho de cada pixel (cores claras ficam mais altas); bordas transparentes não inclinam.
    Luminance,
    /// Média das duas: bordas arredondadas e detalhes internos pelo brilho.
    Mix,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NormalMapOptions {
    pub mode: HeightMode,
    /// Bevel width in pixels (1..=64).
    pub bevel: u32,
    /// Slope multiplier (0 = flat).
    pub strength: f64,
    /// DirectX convention (green = down). Default OpenGL (green = up), the one Godot uses.
    pub flip_y: bool,
}

impl Default for NormalMapOptions {
    fn default() -> Self {
        NormalMapOptions { mode: HeightMode::Bevel, bevel: 2, strength: 1.0, flip_y: false }
    }
}

/// Squared Euclidean distance transform of one line (Felzenszwalb & Huttenlocher).
fn distance_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    if n == 0 {
        return d;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        let mut s;
        loop {
            let p = v[k];
            s = ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * q as f64 - 2.0 * p as f64);
            if s <= z[k] {
                k -= 1;
            } else {
                break;
            }
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f64 - p as f64).powi(2) + f[p];
    }
    d
}

/// For each visible pixel, the distance (pixel centers) to the nearest transparent pixel or to
/// the outside of the image; 0 for transparent pixels. Exact Euclidean distance.
pub fn edge_distance(img: &Image) -> Vec<f64> {
    const FAR: f64 = 1e12;
    // One transparent pixel of padding all around: the outside counts as transparent.
    let (w, h) = (img.width() as usize + 2, img.height() as usize + 2);
    let mut grid = vec![0.0f64; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            if !img.get_or_clear(x as i32 - 1, y as i32 - 1).is_transparent() {
                grid[y * w + x] = FAR;
            }
        }
    }
    let mut column = vec![0.0; h];
    for x in 0..w {
        for y in 0..h {
            column[y] = grid[y * w + x];
        }
        for (y, v) in distance_1d(&column).into_iter().enumerate() {
            grid[y * w + x] = v;
        }
    }
    for y in 0..h {
        let row = distance_1d(&grid[y * w..(y + 1) * w]);
        grid[y * w..(y + 1) * w].copy_from_slice(&row);
    }
    let mut out = vec![0.0; img.width() as usize * img.height() as usize];
    for y in 0..img.height() as usize {
        for x in 0..img.width() as usize {
            out[y * img.width() as usize + x] = grid[(y + 1) * w + x + 1].sqrt();
        }
    }
    out
}

fn luminance(c: Color) -> f64 {
    (299 * c.r as u32 + 587 * c.g as u32 + 114 * c.b as u32) as f64 / (1000.0 * 255.0)
}

/// Height of every pixel, 0..=1 (row-major). Transparent pixels are 0.
pub fn height_field(img: &Image, mode: HeightMode, bevel: u32) -> Vec<f64> {
    let bevel = bevel.clamp(1, 64) as f64;
    let dist = if mode == HeightMode::Luminance { Vec::new() } else { edge_distance(img) };
    img.pixels()
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if c.is_transparent() {
                return 0.0;
            }
            // Rounded profile: steep at the edge, flat inside.
            let rounded = || {
                let t = dist[i].min(bevel) / bevel;
                (1.0 - (1.0 - t) * (1.0 - t)).sqrt()
            };
            match mode {
                HeightMode::Bevel => rounded(),
                HeightMode::Luminance => luminance(*c),
                HeightMode::Mix => 0.5 * rounded() + 0.5 * luminance(*c),
            }
        })
        .collect()
}

fn encode(v: f64) -> u8 {
    ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8
}

/// The normal map of an image (same size). Transparent pixels stay transparent; visible ones
/// get the normal in RGB and their own alpha.
pub fn normal_map(img: &Image, opts: &NormalMapOptions) -> Image {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let heights = height_field(img, opts.mode, opts.bevel);
    let solid = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && !img.get_or_clear(x, y).is_transparent();
    let mut out = Image::new(img.width(), img.height());
    for y in 0..h {
        for x in 0..w {
            let c = img.get_or_clear(x, y);
            if c.is_transparent() {
                continue;
            }
            let center = heights[(y * w + x) as usize];
            // Outside and transparent neighbors: height 0 (edges slope outwards), except in
            // luminance mode, where they repeat the center (no slope from transparency).
            let s = |dx: i32, dy: i32| {
                let (nx, ny) = (x + dx, y + dy);
                if solid(nx, ny) {
                    heights[(ny * w + nx) as usize]
                } else if opts.mode == HeightMode::Luminance {
                    center
                } else {
                    0.0
                }
            };
            let gx = (s(1, -1) + 2.0 * s(1, 0) + s(1, 1)) - (s(-1, -1) + 2.0 * s(-1, 0) + s(-1, 1));
            let gy = (s(-1, 1) + 2.0 * s(0, 1) + s(1, 1)) - (s(-1, -1) + 2.0 * s(0, -1) + s(1, -1));
            let strength = opts.strength.max(0.0);
            // Normal in image space (y down), from the height slope per pixel.
            let (nx, ny, nz) = (-gx / 8.0 * strength, -gy / 8.0 * strength, 1.0);
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let (nx, ny, nz) = (nx / len, ny / len, nz / len);
            // OpenGL: green grows upwards (the image y axis flipped).
            let green = if opts.flip_y { ny } else { -ny };
            out.set(x, y, Color::rgba(encode(nx), encode(green), encode(nz), c.a));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: u32, at: i32, side: i32, color: Color) -> Image {
        let mut img = Image::new(size, size);
        for y in at..at + side {
            for x in at..at + side {
                img.set(x, y, color);
            }
        }
        img
    }

    const FLAT: Color = Color::rgb(128, 128, 255);

    #[test]
    fn distance_to_the_edge_is_euclidean() {
        let img = square(9, 1, 7, Color::WHITE);
        let d = edge_distance(&img);
        let at = |x: usize, y: usize| d[y * 9 + x];
        assert_eq!(at(0, 0), 0.0);
        assert_eq!(at(1, 4), 1.0);
        assert_eq!(at(4, 4), 4.0);
        assert_eq!(at(2, 2), 2.0);
        // The image border counts as transparent too.
        let full = Image::filled(3, 3, Color::WHITE);
        assert_eq!(edge_distance(&full)[4], 2.0);
    }

    #[test]
    fn flat_inside_and_edges_slope_outwards() {
        let img = square(16, 2, 12, Color::rgb(57, 118, 210));
        let n = normal_map(&img, &NormalMapOptions::default());
        assert_eq!(n.get(8, 8), Some(FLAT), "far from the edges");
        assert_eq!(n.get(0, 0), Some(Color::TRANSPARENT));
        let left = n.get(2, 8).unwrap();
        assert!(left.r < 128 && left.g == 128 && left.b < 255, "left edge faces left: {left:?}");
        let right = n.get(13, 8).unwrap();
        assert!(right.r > 128 && right.g == 128, "right edge faces right: {right:?}");
        let top = n.get(8, 2).unwrap();
        assert!(top.g > 128 && top.r == 128, "top edge faces up (OpenGL): {top:?}");
        let bottom = n.get(8, 13).unwrap();
        assert!(bottom.g < 128, "bottom edge faces down: {bottom:?}");
        // DirectX flips green only.
        let dx = normal_map(&img, &NormalMapOptions { flip_y: true, ..Default::default() });
        assert_eq!(dx.get(8, 2).unwrap().g, 255 - top.g);
        assert_eq!(dx.get(2, 8), Some(left));
        assert_eq!(dx.get(8, 8), Some(FLAT));
        // More strength, more slope; zero strength, flat.
        let strong = normal_map(&img, &NormalMapOptions { strength: 3.0, ..Default::default() });
        assert!(strong.get(2, 8).unwrap().r < left.r);
        let none = normal_map(&img, &NormalMapOptions { strength: 0.0, ..Default::default() });
        assert!(none.pixels().iter().all(|p| p.is_transparent() || *p == FLAT));
    }

    #[test]
    fn luminance_mode_and_alpha() {
        // A uniform color is flat in luminance mode, edges included.
        let img = square(8, 1, 6, Color::rgba(200, 50, 50, 180));
        let n = normal_map(&img, &NormalMapOptions { mode: HeightMode::Luminance, ..Default::default() });
        assert_eq!(n.get(1, 1), Some(Color::rgba(128, 128, 255, 180)), "alpha copied");
        assert!(n.pixels().iter().all(|p| p.is_transparent() || p.to_array()[..3] == [128, 128, 255]));
        // A bright ridge in the middle: the slopes beside it face away from it.
        let mut img = Image::filled(9, 3, Color::rgb(40, 40, 40));
        for y in 0..3 {
            img.set(4, y, Color::WHITE);
        }
        let n = normal_map(&img, &NormalMapOptions { mode: HeightMode::Luminance, ..Default::default() });
        assert!(n.get(3, 1).unwrap().r < 128, "rising to the right: faces left");
        assert!(n.get(5, 1).unwrap().r > 128, "falling to the right: faces right");
        assert_eq!(n.get(0, 1), Some(FLAT));
        // Mix keeps the bevel at the edges.
        let m = normal_map(&square(16, 2, 12, Color::WHITE), &NormalMapOptions { mode: HeightMode::Mix, ..Default::default() });
        assert!(m.get(2, 8).unwrap().r < 128);
        assert_eq!(m.get(8, 8), Some(FLAT));
    }
}
