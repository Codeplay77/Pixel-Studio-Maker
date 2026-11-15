//! Brush strokes: the freehand path drawn by the pencil-like tools.
//!
//! A stroke is a polyline (optionally cleaned to be pixel-perfect), stamped with a square brush,
//! mirrored across one or two symmetry axes and, in wrap mode, folded back into the canvas so
//! tileable textures can be painted across the edges. Every covered pixel is visited exactly
//! once, then an ink decides its new color: plain paint, ordered dither, or a step along a color
//! ramp (shading).

use std::borrow::Cow;
use std::collections::BTreeSet;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Point};

use crate::effects::DitherPattern;
use crate::sdc::{DrawOp, MAX_POINTS};
use crate::{Canvas, DrawError, raster};

/// Most pixels one stroke may cover after the brush, the mirrors and the wrap.
pub const MAX_STROKE_PIXELS: usize = 4_000_000;

/// How a stroke changes the pixels it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StrokeMode {
    /// Pinta `color` em todos os pixels do traço.
    #[default]
    Paint,
    /// Pinta `color` onde o padrão ordenado manda (pela `density`) e `secondary` (se houver) no resto.
    Dither,
    /// Anda `step` posições na rampa de cores `ramp` (clareia ou escurece) em cada pixel tocado.
    Shade,
}

/// One mirror axis: `true` puts it on the sprite center, a number puts it at that pixel
/// coordinate (`.5` falls between two pixels), `false` turns it off.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Axis {
    Enabled(bool),
    At(f64),
}

impl JsonSchema for Axis {
    fn schema_name() -> Cow<'static, str> {
        "Axis".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Eixo de simetria: true = centro do sprite; número = coordenada do eixo em pixels (15 = passa pelo pixel 15; 15.5 = entre os pixels 15 e 16); false = desligado.",
            "anyOf": [{ "type": "boolean" }, { "type": "number" }]
        })
    }
}

/// Mirror symmetry of a stroke. With both axes the stroke is drawn four times.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Symmetry {
    /// Eixo vertical: espelha esquerda↔direita. `true` = centro do sprite ((largura-1)/2), ou a coordenada x do eixo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<Axis>,
    /// Eixo horizontal: espelha cima↔baixo. `true` = centro do sprite ((altura-1)/2), ou a coordenada y do eixo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<Axis>,
}

/// Twice the axis coordinate (so `.5` positions stay integers), or `None` when the axis is off.
/// `size` is the canvas side the default (centered) axis uses.
pub fn axis_twice(axis: Option<Axis>, size: u32, name: &str) -> Result<Option<i32>, DrawError> {
    match axis {
        None | Some(Axis::Enabled(false)) => Ok(None),
        Some(Axis::Enabled(true)) => Ok(Some(size as i32 - 1)),
        Some(Axis::At(v)) => {
            let twice = v * 2.0;
            if !twice.is_finite() || twice.abs() > 1_000_000.0 {
                return Err(DrawError::InvalidArgument(format!("eixo de simetria {name} inválido: {v}")));
            }
            if (twice - twice.round()).abs() > 1e-6 {
                return Err(DrawError::InvalidArgument(format!(
                    "eixo de simetria {name} = {v}: use um número inteiro (passa por um pixel) ou terminado em .5 (entre dois pixels)"
                )));
            }
            Ok(Some(twice.round() as i32))
        }
    }
}

/// Geometry of a stroke, independent of the ink.
#[derive(Debug, Clone, Copy)]
pub struct StrokeShape<'a> {
    pub points: &'a [Point],
    pub thickness: u32,
    pub pixel_perfect: bool,
    pub symmetry: Symmetry,
    pub wrap: bool,
}

/// Every canvas pixel the stroke covers, each once, sorted by row then column. Without `wrap`,
/// pixels outside the canvas are dropped; with it they come back on the opposite side.
pub fn stroke_pixels(shape: &StrokeShape, width: u32, height: u32) -> Result<Vec<Point>, DrawError> {
    if shape.points.is_empty() {
        return Err(DrawError::InvalidArgument("o traço precisa de pelo menos 1 ponto".into()));
    }
    if shape.thickness == 0 || shape.thickness > 64 {
        return Err(DrawError::InvalidArgument(format!("espessura {} inválida (1 a 64)", shape.thickness)));
    }
    let ax = axis_twice(shape.symmetry.x, width, "x")?;
    let ay = axis_twice(shape.symmetry.y, height, "y")?;
    let mut path = raster::polyline(shape.points, false);
    if shape.pixel_perfect {
        path = raster::pixel_perfect(&path);
    }
    let copies = 1 + usize::from(ax.is_some()) + usize::from(ay.is_some()) + usize::from(ax.is_some() && ay.is_some());
    let estimate = path.len().saturating_mul((shape.thickness as usize).pow(2)).saturating_mul(copies);
    if estimate > MAX_STROKE_PIXELS {
        return Err(DrawError::TooLarge(format!("traço com {estimate} pixels (máximo {MAX_STROKE_PIXELS})")));
    }
    let brushed = raster::thicken(&path, shape.thickness);
    let (w, h) = (width as i32, height as i32);
    let mut out: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut push = |x: i32, y: i32| {
        let (x, y) = if shape.wrap && w > 0 && h > 0 { (x.rem_euclid(w), y.rem_euclid(h)) } else { (x, y) };
        if x >= 0 && y >= 0 && x < w && y < h {
            out.insert((y, x));
        }
    };
    for p in brushed {
        push(p.x, p.y);
        if let Some(a) = ax {
            push(a - p.x, p.y);
        }
        if let Some(b) = ay {
            push(p.x, b - p.y);
        }
        if let (Some(a), Some(b)) = (ax, ay) {
            push(a - p.x, b - p.y);
        }
    }
    Ok(out.into_iter().map(|(y, x)| Point::new(x, y)).collect())
}

/// What happens to each covered pixel.
#[derive(Debug, Clone)]
pub enum Ink {
    Paint(Color),
    Dither { color: Color, secondary: Option<Color>, pattern: DitherPattern, density: f64 },
    Shade { ramp: Vec<Color>, step: i32 },
}

/// The color a ramp step leads to, or `None` when `color` is not on the ramp (or transparent).
pub fn shade_step(ramp: &[Color], color: Color, step: i32) -> Option<Color> {
    if color.is_transparent() {
        return None;
    }
    let i = ramp.iter().position(|c| *c == color)? as i64;
    let j = (i + step as i64).clamp(0, ramp.len() as i64 - 1);
    Some(ramp[j as usize])
}

pub fn default_density() -> f64 {
    0.5
}

pub fn default_step() -> i32 {
    1
}

/// Runs a `stroke` SDC operation on the canvas. `palette` is the active palette (integer colors
/// and the default shading ramp).
pub fn apply_op(canvas: &mut Canvas, op: &DrawOp, palette: &[Color]) -> Result<(), DrawError> {
    let DrawOp::Stroke { points, color, thickness, pixel_perfect, symmetry, wrap, mode, pattern, density, secondary, step, ramp } = op
    else {
        return Err(DrawError::InvalidArgument("operação não é um traço".into()));
    };
    if points.len() > MAX_POINTS {
        return Err(DrawError::TooLarge(format!("{} pontos (máximo {MAX_POINTS})", points.len())));
    }
    let need_color = || -> Result<Color, DrawError> {
        color
            .as_ref()
            .ok_or_else(|| DrawError::InvalidArgument("o traço precisa de `color` nos modos paint e dither".into()))?
            .resolve(palette)
    };
    let ink = match mode {
        StrokeMode::Paint => Ink::Paint(need_color()?),
        StrokeMode::Dither => {
            if !density.is_finite() {
                return Err(DrawError::InvalidArgument("`density` precisa ser um número de 0 a 1".into()));
            }
            Ink::Dither {
                color: need_color()?,
                secondary: secondary.as_ref().map(|c| c.resolve(palette)).transpose()?,
                pattern: *pattern,
                density: *density,
            }
        }
        StrokeMode::Shade => {
            let ramp: Vec<Color> =
                if ramp.is_empty() { palette.to_vec() } else { ramp.iter().map(|c| c.resolve(palette)).collect::<Result<_, _>>()? };
            if ramp.len() < 2 {
                return Err(DrawError::InvalidArgument(
                    "o modo shade precisa de uma rampa com pelo menos 2 cores (`ramp` ou a paleta ativa)".into(),
                ));
            }
            Ink::Shade { ramp, step: *step }
        }
    };
    let shape =
        StrokeShape { points, thickness: *thickness, pixel_perfect: *pixel_perfect, symmetry: symmetry.unwrap_or_default(), wrap: *wrap };
    let pixels = stroke_pixels(&shape, canvas.width(), canvas.height())?;
    apply_stroke(canvas, &pixels, &ink);
    Ok(())
}

/// Applies `ink` to every pixel of the stroke (each pixel exactly once).
pub fn apply_stroke(canvas: &mut Canvas, pixels: &[Point], ink: &Ink) {
    match ink {
        Ink::Paint(c) => {
            for p in pixels {
                canvas.put(p.x, p.y, *c);
            }
        }
        Ink::Dither { color, secondary, pattern, density } => {
            let density = density.clamp(0.0, 1.0);
            for p in pixels {
                if density > pattern.threshold(p.x, p.y) {
                    canvas.put(p.x, p.y, *color);
                } else if let Some(s) = secondary {
                    canvas.put(p.x, p.y, *s);
                }
            }
        }
        Ink::Shade { ramp, step } => {
            // Read every source color before writing, so the result never depends on order.
            let changes: Vec<(Point, Color)> =
                pixels.iter().filter_map(|p| shade_step(ramp, canvas.get(p.x, p.y), *step).map(|c| (*p, c))).collect();
            for (p, c) in changes {
                canvas.put(p.x, p.y, c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Image;

    fn pts(list: &[(i32, i32)]) -> Vec<Point> {
        list.iter().map(|(x, y)| Point::new(*x, *y)).collect()
    }

    fn shape(points: &[Point]) -> StrokeShape<'_> {
        StrokeShape { points, thickness: 1, pixel_perfect: false, symmetry: Symmetry::default(), wrap: false }
    }

    #[test]
    fn mirrors_on_both_axes_with_half_pixel_center() {
        let points = pts(&[(1, 1)]);
        let sym = Symmetry { x: Some(Axis::Enabled(true)), y: Some(Axis::Enabled(true)) };
        let out = stroke_pixels(&StrokeShape { symmetry: sym, ..shape(&points) }, 8, 6).unwrap();
        // Axis x = 3.5 (between 3 and 4), axis y = 2.5.
        assert_eq!(out, pts(&[(1, 1), (6, 1), (1, 4), (6, 4)]));
    }

    #[test]
    fn axis_through_a_pixel_and_invalid_axis() {
        let points = pts(&[(2, 0), (4, 0)]);
        let sym = Symmetry { x: Some(Axis::At(4.0)), y: None };
        let out = stroke_pixels(&StrokeShape { symmetry: sym, ..shape(&points) }, 10, 1).unwrap();
        assert_eq!(out, pts(&[(2, 0), (3, 0), (4, 0), (5, 0), (6, 0)]));
        let bad = Symmetry { x: Some(Axis::At(4.25)), y: None };
        assert!(stroke_pixels(&StrokeShape { symmetry: bad, ..shape(&points) }, 10, 1).is_err());
    }

    #[test]
    fn wrap_folds_pixels_back() {
        let points = pts(&[(-1, 2), (1, 2)]);
        assert_eq!(stroke_pixels(&shape(&points), 4, 4).unwrap(), pts(&[(0, 2), (1, 2)]));
        let out = stroke_pixels(&StrokeShape { wrap: true, ..shape(&points) }, 4, 4).unwrap();
        assert_eq!(out, pts(&[(0, 2), (1, 2), (3, 2)]));
    }

    #[test]
    fn documented_examples_parse_and_run() {
        let doc = include_str!("../../../docs/sdc.md");
        let start = doc.find("## Traço de pincel").expect("seção do stroke");
        let block = &doc[start..];
        let json = &block[block.find("```json").unwrap() + 7..];
        let json = &json[..json.find("```").unwrap()];
        let palette = [Color::BLACK, Color::rgb(40, 40, 40), Color::rgb(80, 80, 80), Color::rgb(120, 120, 120)];
        let mut img = Image::filled(32, 32, palette[2]);
        let mut count = 0;
        // The block lists several JSON objects one after the other.
        for op in serde_json::Deserializer::from_str(json).into_iter::<DrawOp>() {
            let op = op.unwrap_or_else(|e| panic!("exemplo inválido: {e}"));
            let mut canvas = Canvas::new(&mut img, None);
            apply_op(&mut canvas, &op, &palette).unwrap();
            assert!(canvas.stats().changed > 0, "{op:?}");
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn shade_moves_once_along_the_ramp() {
        let ramp = [Color::rgb(10, 10, 10), Color::rgb(20, 20, 20), Color::rgb(30, 30, 30)];
        assert_eq!(shade_step(&ramp, ramp[0], 1), Some(ramp[1]));
        assert_eq!(shade_step(&ramp, ramp[2], 1), Some(ramp[2]), "clamps at the end");
        assert_eq!(shade_step(&ramp, ramp[1], -5), Some(ramp[0]));
        assert_eq!(shade_step(&ramp, Color::WHITE, 1), None);
        let mut img = Image::filled(3, 1, ramp[0]);
        let mut canvas = Canvas::new(&mut img, None);
        // A stroke that goes back and forth still touches each pixel once.
        let points = pts(&[(0, 0), (2, 0), (0, 0)]);
        let pixels = stroke_pixels(&shape(&points), 3, 1).unwrap();
        apply_stroke(&mut canvas, &pixels, &Ink::Shade { ramp: ramp.to_vec(), step: 1 });
        assert_eq!(img.pixels(), &[ramp[1]; 3]);
    }
}
