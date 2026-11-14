//! Sprite Drawing Commands (SDC), schema version 1.
//!
//! An SDC operation is a JSON object with an `"op"` field. A batch is an ordered list of them,
//! applied atomically by the command layer. Colors are hex strings (`"#3976d2"`), `"clear"`, or
//! integers indexing the active palette. Full reference: `docs/sdc.md`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::matrix::{self, Cell, Grid};
use sprite_core::{Color, Image, PasteMode, Point, Rect, SourceRef};

use crate::effects::{self, CleanupMode, DitherPattern, GradientShape, LightDirection, OutlineMode, ShadeColor};
use crate::fill::{self, Connectivity};
use crate::raster;
use crate::transform::{self, Transform};
use crate::{Canvas, DrawError};

pub const SDC_VERSION: u32 = 1;

/// Largest shape box or rectangle side accepted by one operation.
pub const MAX_SHAPE_SIDE: u32 = 4096;
/// Most points/pixels accepted in one list.
pub const MAX_POINTS: usize = 200_000;

/// A color: hex string, `"clear"`/`"transparent"`, or an index into the active palette.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ColorRef {
    /// Index in the active palette (0-based).
    Index(u32),
    /// `"#rrggbb"`, `"#rrggbbaa"`, `"#rgb"`, or `"clear"`.
    Value(String),
}

impl ColorRef {
    pub fn resolve(&self, palette: &[Color]) -> Result<Color, DrawError> {
        match self {
            ColorRef::Index(i) => palette
                .get(*i as usize)
                .copied()
                .ok_or_else(|| DrawError::Color(format!("índice {i} fora da paleta ativa ({} cores)", palette.len()))),
            ColorRef::Value(v) => Color::parse(v).map_err(|e| DrawError::Color(e.to_string())),
        }
    }
}

impl From<Color> for ColorRef {
    fn from(c: Color) -> Self {
        ColorRef::Value(c.to_hex())
    }
}

/// One entry of a pixel list: `[x, y]` (uses the operation's `color`) or `[x, y, color]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PixelItem {
    WithColor(i32, i32, ColorRef),
    At(i32, i32),
}

fn one() -> u32 {
    1
}
fn yes() -> bool {
    true
}

/// One drawing operation. Coordinates are pixels, (0,0) top-left. Every shape is clipped to the
/// canvas (and to the active selection/region when the command layer sets one).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DrawOp {
    /// Um pixel.
    Pixel { x: i32, y: i32, color: ColorRef },
    /// Lista de pixels: `[[x,y], ...]` com `color`, ou `[[x,y,cor], ...]`.
    Pixels {
        pixels: Vec<PixelItem>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<ColorRef>,
    },
    /// Matriz de pixels a partir de `at`. Forma textual: `rows` + `legend` (char → cor, `null` =
    /// manter, `"clear"` = apagar; `.` mantém por padrão), opcionalmente com `rle: true` ("3k2.").
    /// Forma numérica: `pixels` com índices de `palette` (ou da paleta ativa) e `null` = manter.
    Matrix {
        #[serde(alias = "position")]
        at: Point,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        rows: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        legend: BTreeMap<String, Option<ColorRef>>,
        #[serde(default)]
        rle: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pixels: Vec<Vec<Option<u32>>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        palette: Vec<Color>,
    },
    /// Linha reta (Bresenham), espessura em px.
    Line {
        from: Point,
        to: Point,
        color: ColorRef,
        #[serde(default = "one")]
        thickness: u32,
    },
    /// Linhas ligando os pontos. `pixel_perfect` remove cantos em "L" (traço à mão).
    Polyline {
        points: Vec<Point>,
        color: ColorRef,
        #[serde(default)]
        closed: bool,
        #[serde(default = "one")]
        thickness: u32,
        #[serde(default)]
        pixel_perfect: bool,
    },
    /// Retângulo (contorno, ou cheio com `fill: true`).
    Rect {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: ColorRef,
        #[serde(default)]
        fill: bool,
        #[serde(default = "one")]
        thickness: u32,
    },
    /// Elipse inscrita no retângulo x,y,w,h (simétrica, sem antialiasing).
    Ellipse {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        color: ColorRef,
        #[serde(default)]
        fill: bool,
        #[serde(default = "one")]
        thickness: u32,
    },
    /// Círculo com centro e raio.
    Circle {
        center: Point,
        radius: u32,
        color: ColorRef,
        #[serde(default)]
        fill: bool,
    },
    /// Polígono pelos vértices (cheio por padrão; `fill: false` desenha só as arestas).
    Polygon {
        points: Vec<Point>,
        color: ColorRef,
        #[serde(default = "yes")]
        fill: bool,
    },
    /// Curva: 3 pontos = Bézier quadrática, 4 = cúbica, mais = spline passando por todos.
    Curve {
        points: Vec<Point>,
        color: ColorRef,
        #[serde(default = "one")]
        thickness: u32,
    },
    /// Balde de tinta a partir de (x,y). `contiguous: false` troca a cor em toda a área.
    Fill {
        x: i32,
        y: i32,
        color: ColorRef,
        #[serde(default = "yes")]
        contiguous: bool,
        #[serde(default)]
        connectivity: Connectivity,
        #[serde(default)]
        tolerance: u8,
    },
    /// Troca uma cor por outra (com tolerância por canal 0-255).
    ReplaceColor {
        from: ColorRef,
        to: ColorRef,
        #[serde(default)]
        tolerance: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Apaga (deixa transparente) o retângulo, ou tudo se `rect` faltar.
    Clear {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Contorno em volta das formas visíveis (`outside`) ou na borda delas (`inside`).
    Outline {
        color: ColorRef,
        #[serde(default)]
        mode: OutlineMode,
        #[serde(default)]
        connectivity: Connectivity,
        #[serde(default = "one")]
        thickness: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Dithering ordenado: `density` 0..1 de `color`; o resto recebe `background` ou fica igual.
    Dither {
        color: ColorRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background: Option<ColorRef>,
        density: f64,
        #[serde(default)]
        pattern: DitherPattern,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
        #[serde(default)]
        only_opaque: bool,
    },
    /// Degradê discreto com exatamente as cores dadas (faixas), opcionalmente com dithering.
    Gradient {
        colors: Vec<ColorRef>,
        shape: GradientShape,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dither: Option<DitherPattern>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
        #[serde(default)]
        only_opaque: bool,
    },
    /// Sombreamento pela direção da luz: bordas viradas para a luz ganham `highlight`, as do lado
    /// oposto ganham `shadow`. Sem cor explícita, usa a própria cor clareada/escurecida por
    /// `highlight_amount`/`shadow_amount` (0..1) com desvio de matiz.
    Shade {
        #[serde(default)]
        light: LightDirection,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        highlight: Option<ColorRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shadow: Option<ColorRef>,
        #[serde(default = "default_highlight_amount")]
        highlight_amount: f64,
        #[serde(default = "default_shadow_amount")]
        shadow_amount: f64,
        #[serde(default = "one")]
        highlight_depth: u32,
        #[serde(default = "one")]
        shadow_depth: u32,
        #[serde(default = "yes")]
        hue_shift: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Sombra projetada: a silhueta deslocada (dx,dy), pintada só onde está transparente.
    DropShadow {
        dx: i32,
        dy: i32,
        color: ColorRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Padrão repetido (`rows` + `legend`, como em `matrix`) ladrilhado na área.
    Pattern {
        rows: Vec<String>,
        legend: BTreeMap<String, Option<ColorRef>>,
        #[serde(default)]
        offset: Point,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
        #[serde(default)]
        only_opaque: bool,
    },
    /// Transforma os pixels visíveis da área: flip_h, flip_v, rotate, move, scale.
    Transform {
        transform: Transform,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Copia um retângulo de outra layer/frame (ou do composto) para `to`.
    Copy {
        #[serde(default)]
        from: SourceRef,
        rect: Rect,
        to: Point,
        #[serde(default = "skip_transparent")]
        mode: PasteMode,
        #[serde(default)]
        flip_h: bool,
    },
    /// Limpeza: `orphans` remove pixels soltos; `noise` corrige pixels isolados de outra cor.
    Cleanup {
        #[serde(default)]
        mode: CleanupMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Leva cada pixel visível à cor mais próxima de `colors` (ou da paleta ativa).
    Quantize {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        colors: Vec<ColorRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
    /// Ajusta matiz (graus), saturação e valor (-1..1) dos pixels visíveis.
    Adjust {
        #[serde(default)]
        hue: f64,
        #[serde(default)]
        saturation: f64,
        #[serde(default)]
        value: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rect: Option<Rect>,
    },
}

fn default_highlight_amount() -> f64 {
    0.3
}
fn default_shadow_amount() -> f64 {
    0.3
}
fn skip_transparent() -> PasteMode {
    PasteMode::SkipTransparent
}

impl DrawOp {
    pub fn name(&self) -> &'static str {
        match self {
            DrawOp::Pixel { .. } => "pixel",
            DrawOp::Pixels { .. } => "pixels",
            DrawOp::Matrix { .. } => "matrix",
            DrawOp::Line { .. } => "line",
            DrawOp::Polyline { .. } => "polyline",
            DrawOp::Rect { .. } => "rect",
            DrawOp::Ellipse { .. } => "ellipse",
            DrawOp::Circle { .. } => "circle",
            DrawOp::Polygon { .. } => "polygon",
            DrawOp::Curve { .. } => "curve",
            DrawOp::Fill { .. } => "fill",
            DrawOp::ReplaceColor { .. } => "replace_color",
            DrawOp::Clear { .. } => "clear",
            DrawOp::Outline { .. } => "outline",
            DrawOp::Dither { .. } => "dither",
            DrawOp::Gradient { .. } => "gradient",
            DrawOp::Shade { .. } => "shade",
            DrawOp::DropShadow { .. } => "drop_shadow",
            DrawOp::Pattern { .. } => "pattern",
            DrawOp::Transform { .. } => "transform",
            DrawOp::Copy { .. } => "copy",
            DrawOp::Cleanup { .. } => "cleanup",
            DrawOp::Quantize { .. } => "quantize",
            DrawOp::Adjust { .. } => "adjust",
        }
    }

    /// Which colors this operation references (for palette checks).
    pub fn color_refs(&self) -> Vec<&ColorRef> {
        match self {
            DrawOp::Pixel { color, .. }
            | DrawOp::Line { color, .. }
            | DrawOp::Polyline { color, .. }
            | DrawOp::Rect { color, .. }
            | DrawOp::Ellipse { color, .. }
            | DrawOp::Circle { color, .. }
            | DrawOp::Polygon { color, .. }
            | DrawOp::Curve { color, .. }
            | DrawOp::Fill { color, .. }
            | DrawOp::Outline { color, .. }
            | DrawOp::DropShadow { color, .. } => vec![color],
            DrawOp::Pixels { pixels, color } => {
                let mut v: Vec<&ColorRef> = color.iter().collect();
                v.extend(pixels.iter().filter_map(|p| match p {
                    PixelItem::WithColor(_, _, c) => Some(c),
                    PixelItem::At(..) => None,
                }));
                v
            }
            DrawOp::ReplaceColor { from, to, .. } => vec![from, to],
            DrawOp::Dither { color, background, .. } => std::iter::once(color).chain(background.iter()).collect(),
            DrawOp::Gradient { colors, .. } | DrawOp::Quantize { colors, .. } => colors.iter().collect(),
            DrawOp::Shade { highlight, shadow, .. } => highlight.iter().chain(shadow.iter()).collect(),
            DrawOp::Matrix { legend, .. } | DrawOp::Pattern { legend, .. } => legend.values().flatten().collect(),
            _ => Vec::new(),
        }
    }
}

/// What an operation can read besides its own canvas.
/// Loads the pixels a `copy` operation reads (another layer, frame or the composite).
pub type SourceLoader<'a> = &'a dyn Fn(&SourceRef) -> Result<Image, DrawError>;

pub struct DrawContext<'a> {
    /// The active palette, for integer color references.
    pub palette: &'a [Color],
    /// Loads pixels for `copy` (other layers/frames). `None` means copying is unavailable.
    pub source: Option<SourceLoader<'a>>,
}

fn check_side(name: &str, v: u32) -> Result<(), DrawError> {
    if v > MAX_SHAPE_SIDE {
        return Err(DrawError::TooLarge(format!("{name} = {v} (máximo {MAX_SHAPE_SIDE})")));
    }
    Ok(())
}

fn check_points(n: usize) -> Result<(), DrawError> {
    if n > MAX_POINTS {
        return Err(DrawError::TooLarge(format!("{n} pontos (máximo {MAX_POINTS})")));
    }
    Ok(())
}

fn check_thickness(t: u32) -> Result<(), DrawError> {
    if t == 0 || t > 64 {
        return Err(DrawError::InvalidArgument(format!("espessura {t} inválida (1 a 64)")));
    }
    Ok(())
}

fn legend_table(legend: &BTreeMap<String, Option<ColorRef>>, palette: &[Color]) -> Result<BTreeMap<char, Cell>, DrawError> {
    let mut resolved: BTreeMap<String, Option<String>> = BTreeMap::new();
    for (k, v) in legend {
        let value = match v {
            None => None,
            Some(ColorRef::Value(s)) if s.eq_ignore_ascii_case("keep") => None,
            Some(ColorRef::Value(s)) if ["clear", "erase", "transparent"].contains(&s.to_ascii_lowercase().as_str()) => {
                Some("clear".to_string())
            }
            Some(c) => Some(c.resolve(palette)?.to_hex()),
        };
        resolved.insert(k.clone(), value);
    }
    Ok(matrix::parse_legend(&resolved)?)
}

/// Decodes the cells of a `matrix` operation (text or numeric form).
pub fn matrix_grid(
    rows: &[String],
    legend: &BTreeMap<String, Option<ColorRef>>,
    rle: bool,
    pixels: &[Vec<Option<u32>>],
    palette: &[Color],
    active: &[Color],
) -> Result<Grid, DrawError> {
    if !rows.is_empty() && !pixels.is_empty() {
        return Err(DrawError::InvalidArgument("use `rows` (texto) ou `pixels` (números), não os dois".into()));
    }
    if !pixels.is_empty() {
        let pal = if palette.is_empty() { active } else { palette };
        let total: usize = pixels.iter().map(|r| r.len()).sum();
        check_points(total)?;
        let mut out = Vec::with_capacity(pixels.len());
        for row in pixels {
            let mut cells = Vec::with_capacity(row.len());
            for v in row {
                cells.push(match v {
                    None => Cell::Keep,
                    Some(i) => {
                        let c = pal
                            .get(*i as usize)
                            .copied()
                            .ok_or_else(|| DrawError::Color(format!("índice {i} fora da paleta da matriz ({} cores)", pal.len())))?;
                        if c.is_transparent() { Cell::Clear } else { Cell::Paint(c) }
                    }
                });
            }
            out.push(cells);
        }
        return Ok(Grid::from_rows(out));
    }
    let pal = if palette.is_empty() { active } else { palette };
    let table = legend_table(legend, pal)?;
    Ok(matrix::decode_rows(rows, &table, rle)?)
}

fn brush(points: Vec<Point>, thickness: u32) -> Vec<Point> {
    raster::thicken(&points, thickness)
}

/// Applies one operation to the canvas.
pub fn apply(canvas: &mut Canvas, op: &DrawOp, ctx: &DrawContext) -> Result<(), DrawError> {
    let pal = ctx.palette;
    match op {
        DrawOp::Pixel { x, y, color } => {
            let c = color.resolve(pal)?;
            canvas.put(*x, *y, c);
        }
        DrawOp::Pixels { pixels, color } => {
            check_points(pixels.len())?;
            let default = color.as_ref().map(|c| c.resolve(pal)).transpose()?;
            for item in pixels {
                match item {
                    PixelItem::WithColor(x, y, c) => {
                        let c = c.resolve(pal)?;
                        canvas.put(*x, *y, c);
                    }
                    PixelItem::At(x, y) => {
                        let c = default
                            .ok_or_else(|| DrawError::InvalidArgument("pixel [x,y] sem cor: informe `color` ou use [x,y,cor]".into()))?;
                        canvas.put(*x, *y, c);
                    }
                }
            }
        }
        DrawOp::Matrix { at, rows, legend, rle, pixels, palette } => {
            let grid = matrix_grid(rows, legend, *rle, pixels, palette, pal)?;
            for y in 0..grid.height {
                for x in 0..grid.width {
                    let cell = grid.get(x, y);
                    if cell != Cell::Keep {
                        let (px, py) = (at.x + x as i32, at.y + y as i32);
                        let existing = canvas.get(px, py);
                        canvas.put(px, py, cell.resolve(existing));
                    }
                }
            }
        }
        DrawOp::Line { from, to, color, thickness } => {
            check_thickness(*thickness)?;
            let c = color.resolve(pal)?;
            let pts = raster::line(*from, *to);
            check_points(pts.len())?;
            canvas.put_all(brush(pts, *thickness), c);
        }
        DrawOp::Polyline { points, color, closed, thickness, pixel_perfect } => {
            check_thickness(*thickness)?;
            check_points(points.len())?;
            let c = color.resolve(pal)?;
            let mut pts = raster::polyline(points, *closed);
            if *pixel_perfect {
                pts = raster::pixel_perfect(&pts);
            }
            check_points(pts.len())?;
            canvas.put_all(brush(pts, *thickness), c);
        }
        DrawOp::Rect { x, y, w, h, color, fill, thickness } => {
            check_side("w", *w)?;
            check_side("h", *h)?;
            check_thickness(*thickness)?;
            let c = color.resolve(pal)?;
            let r = Rect::new(*x, *y, *w, *h).intersect(&Rect::new(-1, -1, canvas.width() + 2, canvas.height() + 2));
            let r_full = Rect::new(*x, *y, *w, *h);
            let pts = if *fill { raster::rect_fill(r) } else { raster::rect_outline(r_full, *thickness) };
            canvas.put_all(pts, c);
        }
        DrawOp::Ellipse { x, y, w, h, color, fill, thickness } => {
            check_side("w", *w)?;
            check_side("h", *h)?;
            check_thickness(*thickness)?;
            let c = color.resolve(pal)?;
            let r = Rect::new(*x, *y, *w, *h);
            let pts = if *fill { raster::ellipse_fill(r) } else { raster::ellipse_outline(r, *thickness) };
            canvas.put_all(pts, c);
        }
        DrawOp::Circle { center, radius, color, fill } => {
            check_side("radius", radius.saturating_mul(2))?;
            let c = color.resolve(pal)?;
            let r = raster::circle_box(*center, *radius);
            let pts = if *fill { raster::ellipse_fill(r) } else { raster::ellipse_outline(r, 1) };
            canvas.put_all(pts, c);
        }
        DrawOp::Polygon { points, color, fill } => {
            check_points(points.len())?;
            if points.len() < 2 {
                return Err(DrawError::InvalidArgument("polígono precisa de pelo menos 2 pontos".into()));
            }
            let span = |f: fn(&Point) -> i32| {
                let (lo, hi) = points.iter().map(f).fold((i32::MAX, i32::MIN), |(a, b), v| (a.min(v), b.max(v)));
                (hi - lo) as u32
            };
            check_side("largura do polígono", span(|p| p.x))?;
            check_side("altura do polígono", span(|p| p.y))?;
            let c = color.resolve(pal)?;
            let pts = if *fill { raster::polygon_fill(points) } else { raster::polyline(points, true) };
            canvas.put_all(pts, c);
        }
        DrawOp::Curve { points, color, thickness } => {
            check_thickness(*thickness)?;
            if points.len() > 1000 {
                return Err(DrawError::TooLarge("curva com mais de 1000 pontos de controle".into()));
            }
            let c = color.resolve(pal)?;
            canvas.put_all(brush(raster::curve(points), *thickness), c);
        }
        DrawOp::Fill { x, y, color, contiguous, connectivity, tolerance } => {
            let c = color.resolve(pal)?;
            let area = canvas.area(None);
            fill::flood_fill(canvas, Point::new(*x, *y), c, *connectivity, *tolerance, *contiguous, &area);
        }
        DrawOp::ReplaceColor { from, to, tolerance, rect } => {
            let (f, t) = (from.resolve(pal)?, to.resolve(pal)?);
            let area = canvas.area(*rect);
            fill::replace_color(canvas, f, t, *tolerance, &area);
        }
        DrawOp::Clear { rect } => {
            let area = canvas.area(*rect);
            let pts: Vec<Point> = area.points().collect();
            canvas.put_all(pts, Color::TRANSPARENT);
        }
        DrawOp::Outline { color, mode, connectivity, thickness, rect } => {
            check_thickness(*thickness)?;
            let c = color.resolve(pal)?;
            let area = canvas.area(*rect);
            effects::outline(canvas, c, *mode, *connectivity, *thickness, &area);
        }
        DrawOp::Dither { color, background, density, pattern, rect, only_opaque } => {
            let c = color.resolve(pal)?;
            let bg = background.as_ref().map(|b| b.resolve(pal)).transpose()?;
            let area = canvas.area(*rect);
            effects::dither(canvas, &area, c, bg, *density, *pattern, *only_opaque);
        }
        DrawOp::Gradient { colors, shape, dither, rect, only_opaque } => {
            if colors.is_empty() || colors.len() > 64 {
                return Err(DrawError::InvalidArgument("degradê precisa de 1 a 64 cores".into()));
            }
            let cs = colors.iter().map(|c| c.resolve(pal)).collect::<Result<Vec<_>, _>>()?;
            let area = canvas.area(*rect);
            effects::gradient(canvas, &area, &cs, *shape, *dither, *only_opaque);
        }
        DrawOp::Shade { light, highlight, shadow, highlight_amount, shadow_amount, highlight_depth, shadow_depth, hue_shift, rect } => {
            let hl = match highlight {
                Some(c) => ShadeColor::Fixed(c.resolve(pal)?),
                None => ShadeColor::Relative(highlight_amount.abs(), *hue_shift),
            };
            let sh = match shadow {
                Some(c) => ShadeColor::Fixed(c.resolve(pal)?),
                None => ShadeColor::Relative(-shadow_amount.abs(), *hue_shift),
            };
            let area = canvas.area(*rect);
            effects::shade(canvas, &area, *light, Some(hl), Some(sh), (*highlight_depth).min(64), (*shadow_depth).min(64));
        }
        DrawOp::DropShadow { dx, dy, color, rect } => {
            let c = color.resolve(pal)?;
            let area = canvas.area(*rect);
            effects::drop_shadow(canvas, &area, *dx, *dy, c);
        }
        DrawOp::Pattern { rows, legend, offset, rect, only_opaque } => {
            let table = legend_table(legend, pal)?;
            let grid = matrix::decode_rows(rows, &table, false)?;
            let area = canvas.area(*rect);
            effects::pattern(canvas, &area, &grid, *offset, *only_opaque);
        }
        DrawOp::Transform { transform, rect } => {
            let area = canvas.area(*rect);
            transform::transform_area(canvas, &area, *transform)?;
        }
        DrawOp::Copy { from, rect, to, mode, flip_h } => {
            check_side("w", rect.w)?;
            check_side("h", rect.h)?;
            let loader = ctx.source.ok_or_else(|| DrawError::Source("cópia entre layers/frames indisponível aqui".into()))?;
            let src = loader(from)?;
            let mut part = src.crop(*rect);
            if *flip_h {
                part = part.flip_horizontal();
            }
            for y in 0..part.height() as i32 {
                for x in 0..part.width() as i32 {
                    let s = part.get_or_clear(x, y);
                    let (dx, dy) = (to.x + x, to.y + y);
                    let new = match mode {
                        PasteMode::Replace => s,
                        PasteMode::Over => s.over(canvas.get(dx, dy)),
                        PasteMode::SkipTransparent => {
                            if s.is_transparent() {
                                continue;
                            }
                            s
                        }
                    };
                    canvas.put(dx, dy, new);
                }
            }
        }
        DrawOp::Cleanup { mode, rect } => {
            let area = canvas.area(*rect);
            effects::cleanup(canvas, &area, *mode);
        }
        DrawOp::Quantize { colors, rect } => {
            let cs = if colors.is_empty() { pal.to_vec() } else { colors.iter().map(|c| c.resolve(pal)).collect::<Result<Vec<_>, _>>()? };
            if cs.is_empty() {
                return Err(DrawError::InvalidArgument("quantize precisa de cores (ou de uma paleta ativa)".into()));
            }
            let area = canvas.area(*rect);
            effects::quantize_to(canvas, &area, &sprite_core::Palette::new("quantize", cs));
        }
        DrawOp::Adjust { hue, saturation, value, rect } => {
            let area = canvas.area(*rect);
            effects::adjust_hsv(canvas, &area, *hue, *saturation, *value);
        }
    }
    Ok(())
}

/// Applies a list of operations to one image. Stops at the first error (the caller decides
/// whether to roll back). Returns the merged statistics.
pub fn apply_all(
    image: &mut Image,
    clip: Option<&sprite_core::Mask>,
    ops: &[DrawOp],
    ctx: &DrawContext,
) -> Result<crate::DrawStats, (usize, DrawError)> {
    let mut canvas = Canvas::new(image, clip);
    for (i, op) in ops.iter().enumerate() {
        apply(&mut canvas, op, ctx).map_err(|e| (i, e))?;
    }
    Ok(canvas.stats())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ops(json: &str) -> Vec<DrawOp> {
        serde_json::from_str(json).expect("SDC válido")
    }

    fn ctx() -> DrawContext<'static> {
        static PAL: [Color; 3] = [Color::BLACK, Color::rgb(57, 118, 210), Color::rgb(147, 197, 253)];
        DrawContext { palette: &PAL, source: None }
    }

    #[test]
    fn spec_matrix_example_numeric() {
        // The example from the project spec, numeric form.
        let list = ops(r##"[{"op":"matrix","position":[10,10],"palette":["#000000","#3976D2","#93C5FD"],
            "pixels":[[null,0,0,null],[0,1,1,0],[0,1,2,0],[null,0,0,null]]}]"##);
        let mut img = Image::filled(32, 32, Color::WHITE);
        let stats = apply_all(&mut img, None, &list, &ctx()).unwrap();
        assert_eq!(stats.changed, 12);
        assert_eq!(img.get(10, 10), Some(Color::WHITE), "null keeps");
        assert_eq!(img.get(11, 10), Some(Color::BLACK));
        assert_eq!(img.get(12, 12), Some(Color::rgb(0x93, 0xc5, 0xfd)));
        assert_eq!(stats.rect(), Some(Rect::new(10, 10, 4, 4)));
    }

    #[test]
    fn text_matrix_with_palette_indices_and_clear() {
        let list = ops(r##"[{"op":"matrix","at":[0,0],"legend":{"k":0,"b":1,"x":"clear"},"rows":["kbk","x.x"]}]"##);
        let mut img = Image::filled(3, 2, Color::WHITE);
        apply_all(&mut img, None, &list, &ctx()).unwrap();
        assert_eq!(img.get(1, 0), Some(Color::rgb(57, 118, 210)));
        assert_eq!(img.get(0, 1), Some(Color::TRANSPARENT));
        assert_eq!(img.get(1, 1), Some(Color::WHITE));
    }

    #[test]
    fn every_op_parses_and_runs() {
        let list = ops(r##"[
            {"op":"pixel","x":1,"y":1,"color":"#ff0000"},
            {"op":"pixels","pixels":[[2,2],[3,3,"#00ff00"]],"color":0},
            {"op":"line","from":[0,0],"to":[15,15],"color":1,"thickness":2},
            {"op":"polyline","points":[[0,15],[5,10],[10,15]],"color":2,"pixel_perfect":true},
            {"op":"rect","x":1,"y":1,"w":6,"h":4,"color":0},
            {"op":"rect","x":8,"y":1,"w":6,"h":4,"color":0,"fill":true},
            {"op":"ellipse","x":2,"y":6,"w":7,"h":5,"color":1,"fill":true},
            {"op":"circle","center":[12,12],"radius":3,"color":2},
            {"op":"polygon","points":[[1,20],[8,20],[4,27]],"color":0},
            {"op":"curve","points":[[0,30],[10,20],[20,30]],"color":1},
            {"op":"fill","x":30,"y":30,"color":"#222222"},
            {"op":"replace_color","from":"#222222","to":"#333333"},
            {"op":"outline","color":0,"mode":"outside","connectivity":"8"},
            {"op":"dither","color":"#ffffff","density":0.25,"rect":{"x":0,"y":0,"w":8,"h":8},"only_opaque":true},
            {"op":"gradient","colors":[0,1,2],"shape":{"type":"linear","angle":90},"dither":"bayer4","rect":{"x":20,"y":0,"w":10,"h":10}},
            {"op":"shade","light":"top_left","rect":{"x":0,"y":0,"w":16,"h":16}},
            {"op":"drop_shadow","dx":1,"dy":1,"color":"#00000080"},
            {"op":"pattern","rows":["ab","ba"],"legend":{"a":0,"b":null},"rect":{"x":24,"y":24,"w":4,"h":4}},
            {"op":"transform","transform":{"kind":"flip_h"},"rect":{"x":0,"y":0,"w":16,"h":16}},
            {"op":"transform","transform":{"kind":"rotate","quarter_turns":1},"rect":{"x":20,"y":0,"w":10,"h":10}},
            {"op":"transform","transform":{"kind":"move","dx":1,"dy":0}},
            {"op":"transform","transform":{"kind":"scale","sx":1,"sy":1}},
            {"op":"cleanup","mode":"orphans"},
            {"op":"quantize"},
            {"op":"adjust","hue":10,"value":-0.1},
            {"op":"clear","rect":{"x":0,"y":0,"w":2,"h":2}}
        ]"##);
        let mut img = Image::new(32, 32);
        let stats = apply_all(&mut img, None, &list, &ctx()).unwrap();
        assert!(stats.changed > 100);
        assert!(!img.is_blank());
    }

    #[test]
    fn errors_report_the_failing_index() {
        let list = ops(r##"[{"op":"pixel","x":0,"y":0,"color":"#fff"},{"op":"pixel","x":0,"y":0,"color":9}]"##);
        let mut img = Image::new(4, 4);
        let (index, err) = apply_all(&mut img, None, &list, &ctx()).unwrap_err();
        assert_eq!(index, 1);
        assert!(matches!(err, DrawError::Color(_)));
        let bad = ops(r##"[{"op":"rect","x":0,"y":0,"w":99999,"h":2,"color":0}]"##);
        assert!(matches!(apply_all(&mut img, None, &bad, &ctx()).unwrap_err().1, DrawError::TooLarge(_)));
    }

    #[test]
    fn copy_uses_source_loader() {
        let mut other = Image::new(4, 4);
        other.set(0, 0, Color::BLACK);
        other.set(1, 0, Color::WHITE);
        let loader = move |_: &SourceRef| Ok(other.clone());
        let ctx = DrawContext { palette: &[], source: Some(&loader) };
        let list = ops(r##"[{"op":"copy","rect":{"x":0,"y":0,"w":2,"h":1},"to":[2,3],"flip_h":true}]"##);
        let mut img = Image::new(4, 4);
        apply_all(&mut img, None, &list, &ctx).unwrap();
        assert_eq!(img.get(2, 3), Some(Color::WHITE));
        assert_eq!(img.get(3, 3), Some(Color::BLACK));
    }

    #[test]
    fn schema_is_generated() {
        let schema = schemars::schema_for!(Vec<DrawOp>);
        let text = serde_json::to_string(&schema).unwrap();
        assert!(text.contains("matrix") && text.contains("drop_shadow"));
    }
}
