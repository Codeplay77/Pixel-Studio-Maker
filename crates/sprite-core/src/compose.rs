use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::geom::Point;
use crate::image::Image;

/// Layer blend modes. All math is integer, so composition is bit-exact on every machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Add,
    Subtract,
}

impl BlendMode {
    fn channel(self, backdrop: u8, source: u8) -> u8 {
        let (b, s) = (backdrop as u32, source as u32);
        let v = match self {
            BlendMode::Normal => s,
            BlendMode::Multiply => (b * s + 127) / 255,
            BlendMode::Screen => b + s - (b * s + 127) / 255,
            BlendMode::Overlay => {
                if b < 128 {
                    (2 * b * s + 127) / 255
                } else {
                    255 - (2 * (255 - b) * (255 - s) + 127) / 255
                }
            }
            BlendMode::Darken => b.min(s),
            BlendMode::Lighten => b.max(s),
            BlendMode::Add => (b + s).min(255),
            BlendMode::Subtract => b.saturating_sub(s),
        };
        v.min(255) as u8
    }

    /// Composites `src` onto `dst` with this mode (W3C compositing: blend, then source-over).
    pub fn apply(self, src: Color, dst: Color) -> Color {
        if self == BlendMode::Normal || dst.a == 0 || src.a == 0 {
            return src.over(dst);
        }
        let da = dst.a as u32;
        let mix = |s: u8, d: u8| {
            let blended = self.channel(d, s) as u32;
            ((s as u32 * (255 - da) + blended * da + 127) / 255) as u8
        };
        let mixed = Color::rgba(mix(src.r, dst.r), mix(src.g, dst.g), mix(src.b, dst.b), src.a);
        mixed.over(dst)
    }
}

/// One layer as seen by the compositor.
#[derive(Clone, Copy)]
pub struct LayerView<'a> {
    pub image: &'a Image,
    pub opacity: u8,
    pub blend: BlendMode,
    pub offset: Point,
}

impl<'a> LayerView<'a> {
    pub fn new(image: &'a Image) -> Self {
        LayerView { image, opacity: 255, blend: BlendMode::Normal, offset: Point::default() }
    }
}

/// Composites layers bottom to top onto a transparent `width`×`height` canvas.
pub fn compose<'a>(width: u32, height: u32, layers: impl IntoIterator<Item = LayerView<'a>>) -> Image {
    let mut out = Image::new(width, height);
    for layer in layers {
        compose_onto(&mut out, layer);
    }
    out
}

/// Composites one layer onto an existing image.
pub fn compose_onto(out: &mut Image, layer: LayerView<'_>) {
    if layer.opacity == 0 {
        return;
    }
    let src = layer.image;
    for sy in 0..src.height() as i32 {
        for sx in 0..src.width() as i32 {
            let (x, y) = (sx + layer.offset.x, sy + layer.offset.y);
            if !out.in_bounds(x, y) {
                continue;
            }
            let s = src.get_or_clear(sx, sy).with_opacity(layer.opacity);
            if s.is_transparent() {
                continue;
            }
            let d = out.get_or_clear(x, y);
            out.set(x, y, layer.blend.apply(s, d));
        }
    }
}

/// Draws a checkerboard (two colors, `cell` px squares) under an image — for previews only.
pub fn on_checkerboard(image: &Image, cell: u32, light: Color, dark: Color) -> Image {
    let mut out = Image::new(image.width(), image.height());
    let cell = cell.max(1) as i32;
    for y in 0..image.height() as i32 {
        for x in 0..image.width() as i32 {
            let bg = if ((x / cell) + (y / cell)) % 2 == 0 { light } else { dark };
            out.set(x, y, image.get_or_clear(x, y).over(bg));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_stack_in_order() {
        let mut bottom = Image::filled(2, 1, Color::rgb(255, 0, 0));
        bottom.set(1, 0, Color::TRANSPARENT);
        let top = Image::filled(1, 1, Color::rgb(0, 0, 255));
        let out = compose(2, 1, [LayerView::new(&bottom), LayerView { offset: Point::new(1, 0), ..LayerView::new(&top) }]);
        assert_eq!(out.get(0, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(out.get(1, 0), Some(Color::rgb(0, 0, 255)));
    }

    #[test]
    fn opacity_and_hidden() {
        let base = Image::filled(1, 1, Color::WHITE);
        let ink = Image::filled(1, 1, Color::BLACK);
        let half = compose(1, 1, [LayerView::new(&base), LayerView { opacity: 128, ..LayerView::new(&ink) }]);
        assert_eq!(half.get(0, 0), Some(Color::rgb(127, 127, 127)));
        let hidden = compose(1, 1, [LayerView::new(&base), LayerView { opacity: 0, ..LayerView::new(&ink) }]);
        assert_eq!(hidden.get(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn blend_modes() {
        let d = Color::rgb(200, 100, 50);
        let s = Color::rgb(100, 100, 100);
        assert_eq!(BlendMode::Multiply.apply(s, d), Color::rgb(78, 39, 20));
        assert_eq!(BlendMode::Darken.apply(s, d), Color::rgb(100, 100, 50));
        assert_eq!(BlendMode::Add.apply(s, d), Color::rgb(255, 200, 150));
        // Blending onto nothing behaves like normal.
        assert_eq!(BlendMode::Multiply.apply(s, Color::TRANSPARENT), s);
    }
}
