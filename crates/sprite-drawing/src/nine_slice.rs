//! 9-slice (9-patch): corners stay intact, edges and center stretch or repeat to fill any size.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::Image;

use crate::DrawError;

/// Margins of a 9-slice, in pixels from each side of the sprite.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NineSlice {
    /// Largura do canto/borda esquerda.
    pub left: u32,
    /// Altura do canto/borda de cima.
    pub top: u32,
    /// Largura do canto/borda direita.
    pub right: u32,
    /// Altura do canto/borda de baixo.
    pub bottom: u32,
}

impl NineSlice {
    /// The center must keep at least one pixel in each direction.
    pub fn validate(&self, width: u32, height: u32) -> Result<(), DrawError> {
        if self.left.saturating_add(self.right) >= width {
            return Err(DrawError::InvalidArgument(format!(
                "left + right = {} precisa ser menor que a largura do sprite ({width}) para sobrar um centro",
                self.left.saturating_add(self.right)
            )));
        }
        if self.top.saturating_add(self.bottom) >= height {
            return Err(DrawError::InvalidArgument(format!(
                "top + bottom = {} precisa ser menor que a altura do sprite ({height}) para sobrar um centro",
                self.top.saturating_add(self.bottom)
            )));
        }
        Ok(())
    }

    pub fn scaled(&self, s: u32) -> NineSlice {
        NineSlice { left: self.left * s, top: self.top * s, right: self.right * s, bottom: self.bottom * s }
    }
}

/// How a middle part fills its space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SliceMode {
    /// Esticar (vizinho mais próximo).
    #[default]
    Stretch,
    /// Repetir (ladrilhar), cortando a última repetição.
    Tile,
    /// Repetir encaixando: um número inteiro de repetições, levemente esticadas para fechar o espaço.
    TileFit,
}

impl SliceMode {
    /// Value of `axis_stretch_horizontal/vertical` in Godot's NinePatchRect.
    pub fn godot_axis_stretch(self) -> u8 {
        match self {
            SliceMode::Stretch => 0,
            SliceMode::Tile => 1,
            SliceMode::TileFit => 2,
        }
    }
}

/// Maps position `u` of a middle part `target` pixels long to a source position in `0..source`.
fn map(u: u32, target: u32, source: u32, mode: SliceMode) -> u32 {
    let (u, t, s) = (u as u64, target as u64, source as u64);
    let v = match mode {
        SliceMode::Stretch => u * s / t,
        SliceMode::Tile => u % s,
        SliceMode::TileFit => {
            let n = ((t + s / 2) / s).max(1);
            let k = u * n / t;
            (u * n - k * t) * s / t
        }
    };
    v.min(s - 1) as u32
}

/// Modes per part and axis.
#[derive(Debug, Clone, Copy)]
struct Modes {
    /// Top and bottom edges, along x.
    edge_x: SliceMode,
    /// Left and right edges, along y.
    edge_y: SliceMode,
    center_x: SliceMode,
    center_y: SliceMode,
}

/// Source coordinate of target coordinate `t` along one axis (`lo`/`hi` are the margins).
fn axis(t: u32, total: u32, src: u32, lo: u32, hi: u32, mode: SliceMode) -> u32 {
    if t < lo {
        t
    } else if t >= total - hi {
        src - (total - t)
    } else {
        lo + map(t - lo, total - lo - hi, src - lo - hi, mode)
    }
}

fn render_modes(src: &Image, m: NineSlice, width: u32, height: u32, modes: Modes) -> Result<Image, DrawError> {
    m.validate(src.width(), src.height())?;
    if width < m.left + m.right || height < m.top + m.bottom {
        return Err(DrawError::InvalidArgument(format!(
            "tamanho {width}x{height} menor que os cantos ({}x{})",
            m.left + m.right,
            m.top + m.bottom
        )));
    }
    if width == 0 || height == 0 || width > crate::sdc::MAX_SHAPE_SIDE || height > crate::sdc::MAX_SHAPE_SIDE {
        return Err(DrawError::TooLarge(format!("tamanho {width}x{height} inválido (1 a {} px por lado)", crate::sdc::MAX_SHAPE_SIDE)));
    }
    let (sw, sh) = (src.width(), src.height());
    let mut out = Image::new(width, height);
    for y in 0..height {
        for x in 0..width {
            // Which band each axis falls in decides the mode: middle-middle is the center.
            let in_mid_x = x >= m.left && x < width - m.right;
            let in_mid_y = y >= m.top && y < height - m.bottom;
            let (mx, my) = match (in_mid_x, in_mid_y) {
                (true, true) => (modes.center_x, modes.center_y),
                _ => (modes.edge_x, modes.edge_y),
            };
            let sx = axis(x, width, sw, m.left, m.right, mx);
            let sy = axis(y, height, sh, m.top, m.bottom, my);
            out.set(x as i32, y as i32, src.get_or_clear(sx as i32, sy as i32));
        }
    }
    Ok(out)
}

/// Renders `src` stretched to `width`×`height`: corners intact, edges by `edges`, center by `center`.
pub fn render(src: &Image, m: NineSlice, width: u32, height: u32, edges: SliceMode, center: SliceMode) -> Result<Image, DrawError> {
    render_modes(src, m, width, height, Modes { edge_x: edges, edge_y: edges, center_x: center, center_y: center })
}

/// Renders like Godot's NinePatchRect: one mode per axis, shared by the edges and the center.
pub fn render_axes(
    src: &Image,
    m: NineSlice,
    width: u32,
    height: u32,
    horizontal: SliceMode,
    vertical: SliceMode,
) -> Result<Image, DrawError> {
    render_modes(src, m, width, height, Modes { edge_x: horizontal, edge_y: vertical, center_x: horizontal, center_y: vertical })
}

#[cfg(test)]
mod tests {
    use sprite_core::Color;

    use super::*;

    /// 6x6 source: every pixel a distinct color, so we can trace where each one goes.
    fn source() -> Image {
        let mut img = Image::new(6, 6);
        for y in 0..6 {
            for x in 0..6 {
                img.set(x, y, Color::rgb(x as u8 * 40, y as u8 * 40, 100));
            }
        }
        img
    }

    fn src_at(img: &Image, x: i32, y: i32) -> (i32, i32) {
        let c = img.get(x, y).unwrap();
        (c.r as i32 / 40, c.g as i32 / 40)
    }

    const M: NineSlice = NineSlice { left: 2, top: 1, right: 1, bottom: 2 };

    #[test]
    fn corners_stay_intact() {
        let out = render(&source(), M, 15, 11, SliceMode::Stretch, SliceMode::Stretch).unwrap();
        // Top-left corner 2x1, top-right 1x1, bottom-left 2x2, bottom-right 1x2.
        assert_eq!(src_at(&out, 0, 0), (0, 0));
        assert_eq!(src_at(&out, 1, 0), (1, 0));
        assert_eq!(src_at(&out, 14, 0), (5, 0));
        assert_eq!(src_at(&out, 0, 9), (0, 4));
        assert_eq!(src_at(&out, 1, 10), (1, 5));
        assert_eq!(src_at(&out, 14, 10), (5, 5));
    }

    #[test]
    fn edges_stretch_or_tile() {
        // Middle columns: source 2..5 (3 px) → target 2..14 (12 px).
        let out = render(&source(), M, 15, 11, SliceMode::Stretch, SliceMode::Stretch).unwrap();
        let top: Vec<i32> = (2..14).map(|x| src_at(&out, x, 0).0).collect();
        assert_eq!(top, [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]);
        let tiled = render(&source(), M, 15, 11, SliceMode::Tile, SliceMode::Stretch).unwrap();
        let top: Vec<i32> = (2..14).map(|x| src_at(&tiled, x, 0).0).collect();
        assert_eq!(top, [2, 3, 4, 2, 3, 4, 2, 3, 4, 2, 3, 4]);
        // Left edge (rows 1..9 of the target) repeats source rows 1..4 vertically.
        let left: Vec<i32> = (1..9).map(|y| src_at(&tiled, 0, y).1).collect();
        assert_eq!(left, [1, 2, 3, 1, 2, 3, 1, 2]);
        // The center keeps its own mode.
        assert_eq!(src_at(&tiled, 2, 1), (2, 1));
        assert_eq!(src_at(&tiled, 13, 8), (4, 3));
        // Tile-fit: 12 px / 3 px = 4 whole repetitions.
        let fit = render(&source(), M, 15, 11, SliceMode::TileFit, SliceMode::TileFit).unwrap();
        let top: Vec<i32> = (2..14).map(|x| src_at(&fit, x, 0).0).collect();
        assert_eq!(top, [2, 3, 4, 2, 3, 4, 2, 3, 4, 2, 3, 4]);
        let fit = render(&source(), M, 13, 11, SliceMode::TileFit, SliceMode::TileFit).unwrap();
        let top: Vec<i32> = (2..12).map(|x| src_at(&fit, x, 0).0).collect();
        assert_eq!(top, [2, 2, 3, 4, 2, 3, 4, 2, 3, 4], "10 px = 3 repetições esticadas");
    }

    #[test]
    fn godot_axes_and_invalid_margins() {
        let out = render_axes(&source(), M, 9, 9, SliceMode::Tile, SliceMode::Stretch).unwrap();
        assert_eq!(src_at(&out, 5, 0), (2, 0), "horizontal repete");
        assert_eq!(src_at(&out, 0, 6), (0, 3), "vertical estica");
        assert!(NineSlice { left: 3, top: 0, right: 3, bottom: 0 }.validate(6, 6).is_err());
        assert!(NineSlice { left: 0, top: 4, right: 0, bottom: 2 }.validate(6, 6).is_err());
        assert!(NineSlice { left: 2, top: 2, right: 3, bottom: 3 }.validate(6, 6).is_ok());
        assert!(render(&source(), M, 2, 11, SliceMode::Stretch, SliceMode::Stretch).is_err());
        assert_eq!(SliceMode::TileFit.godot_axis_stretch(), 2);
        // Same size as the source: identical.
        assert_eq!(render(&source(), M, 6, 6, SliceMode::Stretch, SliceMode::Tile).unwrap(), source());
    }
}
