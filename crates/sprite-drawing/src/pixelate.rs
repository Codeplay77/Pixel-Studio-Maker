//! Picture to pixel art: turns a photo, an illustration or an upscaled sprite into a small
//! static pixel-art image.
//!
//! Steps: background removal (flood fill from the borders), crop to the subject, block sampling,
//! tone adjustments, palette reduction (k-means in OKLab seeded by median cut, or a fixed
//! palette), optional dithering, cleanup of stray pixels and an optional outline.
//!
//! Plain image processing, fully deterministic: the same picture and options always give the
//! same pixels. Nothing here generates content; it only resamples the picture it is given.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::image::MAX_IMAGE_SIDE;
use sprite_core::palette::median_cut;
use sprite_core::{Color, Image, Mask, Rect};

use crate::effects::{self, CleanupMode, OutlineMode};
use crate::fill::Connectivity;
use crate::{Canvas, DrawError};

/// A picture to convert: straight RGBA, 8 bits per channel, row by row.
#[derive(Debug, Clone, Copy)]
pub struct Picture<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

impl Picture<'_> {
    fn pixel(&self, i: usize) -> [u8; 4] {
        let p = &self.rgba[i * 4..i * 4 + 4];
        [p[0], p[1], p[2], p[3]]
    }
}

/// How each pixel of the result is picked from its block of the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sampling {
    /// Média do grupo de cor mais comum no bloco: cores limpas e bordas nítidas.
    #[default]
    Dominant,
    /// Média do bloco inteiro: guarda detalhes finos, bordas mais suaves.
    Average,
    /// O pixel do centro do bloco: para imagens que já são pixel art.
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dither {
    /// Sem dithering: cada pixel recebe a cor mais próxima.
    #[default]
    None,
    /// Padrão ordenado (Bayer 4x4) entre as duas cores mais próximas: o dithering clássico de pixel art.
    Ordered,
    /// Difusão de erro (Floyd-Steinberg, atenuada): degradês mais suaves, mais ruído.
    FloydSteinberg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    /// Remove o fundo se as bordas forem quase todas de uma cor e a imagem não tiver transparência.
    #[default]
    Auto,
    /// Nunca remove.
    Keep,
    /// Sempre remove a cor de `background_color` (ou a mais comum nas bordas).
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cleanup {
    None,
    /// Apaga pixels soltos (sem vizinho visível).
    #[default]
    Light,
    /// Também troca pixels isolados pela cor dos 4 vizinhos (some com ruído, pode apagar brilhos de 1 px).
    Strong,
}

/// Options of the conversion. Every field has a default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PixelateOptions {
    /// Largura final em pixels. Só a largura (ou só a altura): a outra segue a proporção. As duas: a imagem cabe dentro, centralizada.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Altura final em pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Sem largura nem altura: o lado maior do resultado (padrão 64).
    pub max_side: u32,
    /// Para imagens que já são pixel art ampliada: quantos pixels da imagem formam um pixel do resultado. Ignora width/height/max_side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<u32>,
    /// Cores da paleta calculada (2 a 256, padrão 16).
    pub colors: u32,
    /// Paleta fixa: usa só estas cores, em vez de calcular uma.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<Color>,
    pub sampling: Sampling,
    pub dither: Dither,
    pub background: Background,
    /// Cor do fundo a remover (padrão: a mais comum nas bordas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,
    /// Tolerância da remoção de fundo: diferença máxima por canal, 0 a 255 (padrão 40).
    pub background_tolerance: u8,
    /// Recorte manual: só esta parte da imagem é convertida, em pixels da imagem original
    /// ({x, y, w, h}). A remoção de fundo e o recorte automático valem dentro dela.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Rect>,
    /// Recortar ao conteúdo visível antes de reduzir (padrão true).
    pub crop: bool,
    /// Cobertura mínima de um bloco, 0 a 255, para o pixel ficar visível (padrão 128).
    pub alpha_threshold: u8,
    /// Saturação: 1 = original, 1.3 = mais viva, 0 = cinza.
    pub saturation: f64,
    /// Contraste: 1 = original.
    pub contrast: f64,
    /// Brilho: -1 a 1, 0 = original.
    pub brightness: f64,
    pub cleanup: Cleanup,
    /// Contorno de 1 px em volta da silhueta (o resultado reserva 1 px de margem para ele).
    pub outline: bool,
    /// Cor do contorno (padrão: a cor mais escura da paleta, escurecida).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<Color>,
}

impl Default for PixelateOptions {
    fn default() -> Self {
        PixelateOptions {
            width: None,
            height: None,
            max_side: 64,
            cell: None,
            colors: 16,
            palette: Vec::new(),
            sampling: Sampling::default(),
            dither: Dither::default(),
            background: Background::default(),
            background_color: None,
            background_tolerance: 40,
            region: None,
            crop: true,
            alpha_threshold: 128,
            saturation: 1.0,
            contrast: 1.0,
            brightness: 0.0,
            cleanup: Cleanup::default(),
            outline: false,
            outline_color: None,
        }
    }
}

impl PixelateOptions {
    pub fn validate(&self) -> Result<(), DrawError> {
        let side = |name: &str, v: Option<u32>| match v {
            Some(0) => Err(invalid(format!("{name} precisa ser maior que zero"))),
            Some(v) if v > MAX_IMAGE_SIDE => Err(DrawError::TooLarge(format!("{name} {v} passa de {MAX_IMAGE_SIDE} px"))),
            _ => Ok(()),
        };
        side("width", self.width)?;
        side("height", self.height)?;
        side("max_side", Some(self.max_side))?;
        if self.cell == Some(0) {
            return Err(invalid("cell precisa ser maior que zero"));
        }
        if self.palette.is_empty() && !(2..=256).contains(&self.colors) {
            return Err(invalid("colors vai de 2 a 256"));
        }
        if self.palette.len() > 256 {
            return Err(invalid("uma paleta tem no máximo 256 cores"));
        }
        for (name, v, lo, hi) in
            [("saturation", self.saturation, 0.0, 4.0), ("contrast", self.contrast, 0.0, 4.0), ("brightness", self.brightness, -1.0, 1.0)]
        {
            if !(lo..=hi).contains(&v) {
                return Err(invalid(format!("{name} vai de {lo} a {hi}")));
            }
        }
        Ok(())
    }
}

/// What the conversion produced.
#[derive(Debug, Clone)]
pub struct Pixelated {
    pub image: Image,
    /// Colors of the result, darkest first. With a fixed palette: that palette (plus the outline color).
    pub palette: Vec<Color>,
    /// Background color that was removed, if any.
    pub background: Option<Color>,
    /// Part of the picture that was converted.
    pub source_rect: Rect,
    /// Picture pixels per result pixel (x, y).
    pub block: (f64, f64),
}

fn invalid(m: impl Into<String>) -> DrawError {
    DrawError::InvalidArgument(m.into())
}

/// Largest picture accepted, per side.
pub const MAX_PICTURE_SIDE: u32 = 16_384;

/// Converts a picture into pixel art.
pub fn pixelate(pic: Picture, opts: &PixelateOptions) -> Result<Pixelated, DrawError> {
    opts.validate()?;
    if pic.width == 0 || pic.height == 0 || pic.rgba.len() != pic.width as usize * pic.height as usize * 4 {
        return Err(invalid("imagem de origem inválida"));
    }
    if pic.width > MAX_PICTURE_SIDE || pic.height > MAX_PICTURE_SIDE {
        return Err(DrawError::TooLarge(format!("imagem {}x{} passa de {MAX_PICTURE_SIDE} px por lado", pic.width, pic.height)));
    }
    // Manual crop: work on that part only, as if it were the whole picture.
    let cropped: Vec<u8>;
    let (pic, origin) = match opts.region {
        Some(r) => {
            let r = r.intersect(&Rect::new(0, 0, pic.width, pic.height));
            if r.is_empty() {
                return Err(invalid("region fica fora da imagem"));
            }
            let row = pic.width as usize * 4;
            cropped = (r.y as usize..r.y as usize + r.h as usize)
                .flat_map(|y| &pic.rgba[y * row + r.x as usize * 4..y * row + (r.x as usize + r.w as usize) * 4])
                .copied()
                .collect();
            (Picture { width: r.w, height: r.h, rgba: &cropped }, (r.x, r.y))
        }
        None => (pic, (0, 0)),
    };
    let w = pic.width as usize;
    let lut = Lut::new();
    let mut alpha: Vec<u8> = pic.rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    let background = remove_background(pic, &mut alpha, opts);
    let region = content_region(pic, &alpha, opts)?;
    let layout = layout(region, opts)?;
    let (xs, ys) = (
        blocks(region.x as usize, region.w as usize, layout.content.0, opts.cell),
        blocks(region.y as usize, region.h as usize, layout.content.1, opts.cell),
    );
    let groups = (opts.sampling == Sampling::Dominant).then(|| {
        let k = if opts.palette.is_empty() { opts.colors } else { opts.palette.len() as u32 };
        Grouping::new(pic, &alpha, region, (k as usize * 2).clamp(8, 48), &lut)
    });

    // Sample: one optional color per result pixel of the content area.
    let threshold = opts.alpha_threshold.max(1) as f64 / 255.0;
    let mut cells: Vec<Option<Lab>> = Vec::with_capacity(xs.len() * ys.len());
    let mut group_acc = vec![(0u32, [0f64; 3]); groups.as_ref().map_or(0, |g| g.k)];
    for &(y0, y1) in &ys {
        for &(x0, x1) in &xs {
            let cell = match opts.sampling {
                Sampling::Nearest => {
                    let i = ((y0 + y1 - 1) / 2) * w + (x0 + x1 - 1) / 2;
                    (alpha[i] as f64 / 255.0 >= threshold).then(|| lut.lab(pic.pixel(i)))
                }
                Sampling::Average | Sampling::Dominant => {
                    let n = ((x1 - x0) * (y1 - y0)) as f64;
                    let mut sum_a = 0f64;
                    let mut sum = [0f64; 3];
                    for slot in group_acc.iter_mut() {
                        *slot = (0, [0.0; 3]);
                    }
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let i = y * w + x;
                            let a = alpha[i];
                            if a == 0 {
                                continue;
                            }
                            let p = pic.pixel(i);
                            let lin = lut.linear(p);
                            let wgt = a as f64 / 255.0;
                            sum_a += wgt;
                            for (s, l) in sum.iter_mut().zip(lin) {
                                *s += l * wgt;
                            }
                            if let Some(g) = &groups
                                && a >= 128
                            {
                                let slot = &mut group_acc[g.of(p)];
                                slot.0 += 1;
                                for (s, l) in slot.1.iter_mut().zip(lin) {
                                    *s += l;
                                }
                            }
                        }
                    }
                    if sum_a / n < threshold {
                        None
                    } else {
                        let best = group_acc.iter().enumerate().filter(|(_, g)| g.0 > 0).max_by_key(|(i, g)| (g.0, usize::MAX - i));
                        Some(match best {
                            Some((_, (count, s))) => linear_to_lab([s[0] / *count as f64, s[1] / *count as f64, s[2] / *count as f64]),
                            None => linear_to_lab([sum[0] / sum_a, sum[1] / sum_a, sum[2] / sum_a]),
                        })
                    }
                }
            };
            cells.push(cell.map(|c| adjust(c, opts)));
        }
    }
    if cells.iter().all(Option::is_none) {
        return Err(invalid("o resultado ficou vazio: diminua alpha_threshold ou não remova o fundo"));
    }

    // Palette: fixed, or k-means over the sampled colors.
    let fixed = !opts.palette.is_empty();
    let palette: Vec<Color> =
        if fixed { opts.palette.iter().map(|c| Color::rgb(c.r, c.g, c.b)).collect() } else { reduce(&cells, opts.colors as usize) };
    let pal_lab: Vec<Lab> = palette.iter().map(|c| lut.lab([c.r, c.g, c.b, 255])).collect();

    // Map (with dithering) into the canvas.
    let (cw, ch) = layout.content;
    let (ox, oy) = layout.offset;
    let mut image = Image::new(layout.canvas.0, layout.canvas.1);
    let indices = map_colors(&cells, cw, ch, (ox, oy), &pal_lab, opts.dither);
    for (k, idx) in indices.iter().enumerate() {
        if let Some(i) = idx {
            image.set((ox + k % cw) as i32, (oy + k / cw) as i32, palette[*i]);
        }
    }

    // Cleanup and outline.
    let full = Mask::full(image.width(), image.height());
    {
        let mut canvas = Canvas::new(&mut image, None);
        if opts.cleanup != Cleanup::None {
            effects::cleanup(&mut canvas, &full, CleanupMode::Orphans);
        }
        if opts.cleanup == Cleanup::Strong {
            effects::cleanup(&mut canvas, &full, CleanupMode::Noise);
        }
    }
    let mut used: Vec<Color> = if fixed {
        palette.clone()
    } else {
        let present = image.color_counts();
        palette.iter().copied().filter(|c| present.contains_key(c)).collect()
    };
    if opts.outline {
        let color = opts.outline_color.map(|c| Color::rgb(c.r, c.g, c.b)).unwrap_or_else(|| outline_color(&used, &lut));
        let mut canvas = Canvas::new(&mut image, None);
        effects::outline(&mut canvas, color, OutlineMode::Outside, Connectivity::Four, 1, &full);
        if !used.contains(&color) {
            used.push(color);
        }
    }
    if !fixed {
        used.sort_by(|a, b| lut.lab([a.r, a.g, a.b, 255]).l.total_cmp(&lut.lab([b.r, b.g, b.b, 255]).l).then(a.cmp(b)));
    }
    Ok(Pixelated {
        image,
        palette: used,
        background,
        source_rect: Rect::new(region.x + origin.0, region.y + origin.1, region.w, region.h),
        block: (region.w as f64 / cw as f64, region.h as f64 / ch as f64),
    })
}

/// Box-filtered reduction of a picture to fit `max_side` (for showing the original next to the
/// result). Pictures already small enough are copied as they are.
pub fn thumbnail(pic: Picture, max_side: u32) -> Result<Image, DrawError> {
    let (w, h) = (pic.width, pic.height);
    if w == 0 || h == 0 || pic.rgba.len() != (w * h * 4) as usize {
        return Err(invalid("imagem de origem inválida"));
    }
    let scale = (max_side.max(1) as f64 / w.max(h) as f64).min(1.0);
    let (tw, th) = (((w as f64 * scale).round() as u32).max(1), ((h as f64 * scale).round() as u32).max(1));
    let (xs, ys) = (blocks(0, w as usize, tw as usize, None), blocks(0, h as usize, th as usize, None));
    let mut out = Image::new(tw, th);
    for (ty, &(y0, y1)) in ys.iter().enumerate() {
        for (tx, &(x0, x1)) in xs.iter().enumerate() {
            let mut sum = [0u64; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = pic.pixel(y * w as usize + x);
                    for (s, v) in sum.iter_mut().zip(&p[..3]) {
                        *s += *v as u64 * p[3] as u64;
                    }
                    sum[3] += p[3] as u64;
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as u64;
            if sum[3] > 0 {
                let ch = |c: usize| ((sum[c] + sum[3] / 2) / sum[3]) as u8;
                out.set(tx as i32, ty as i32, Color::rgba(ch(0), ch(1), ch(2), ((sum[3] + n / 2) / n) as u8));
            }
        }
    }
    Ok(out)
}

// ---- background and region ---------------------------------------------------------------------

fn max_delta(p: [u8; 4], c: Color) -> u8 {
    p[0].abs_diff(c.r).max(p[1].abs_diff(c.g)).max(p[2].abs_diff(c.b))
}

/// Flood-fills the background from the borders, making it transparent. Returns its color.
fn remove_background(pic: Picture, alpha: &mut [u8], opts: &PixelateOptions) -> Option<Color> {
    if opts.background == Background::Keep {
        return None;
    }
    let (w, h) = (pic.width as usize, pic.height as usize);
    let auto = opts.background == Background::Auto;
    if auto && opts.background_color.is_none() {
        // Pictures that already have transparency keep it as their background.
        let translucent = alpha.iter().filter(|a| **a < 250).count();
        if translucent * 200 > alpha.len() {
            return None;
        }
    }
    let target = match opts.background_color {
        Some(c) => Color::rgb(c.r, c.g, c.b),
        None => border_color(pic, opts.background_tolerance, auto)?,
    };
    let tol = opts.background_tolerance;
    let mut removed = vec![false; w * h];
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    let push = |i: usize, seen: &mut [bool], stack: &mut Vec<usize>| {
        if !seen[i] {
            seen[i] = true;
            stack.push(i);
        }
    };
    for x in 0..w {
        push(x, &mut seen, &mut stack);
        push((h - 1) * w + x, &mut seen, &mut stack);
    }
    for y in 0..h {
        push(y * w, &mut seen, &mut stack);
        push(y * w + w - 1, &mut seen, &mut stack);
    }
    while let Some(i) = stack.pop() {
        if alpha[i] != 0 && max_delta(pic.pixel(i), target) > tol {
            continue;
        }
        removed[i] = true;
        let (x, y) = (i % w, i / w);
        if x > 0 {
            push(i - 1, &mut seen, &mut stack);
        }
        if x + 1 < w {
            push(i + 1, &mut seen, &mut stack);
        }
        if y > 0 {
            push(i - w, &mut seen, &mut stack);
        }
        if y + 1 < h {
            push(i + w, &mut seen, &mut stack);
        }
    }
    // Antialiased edge: pixels touching the removed area and still close to the background color.
    let halo = tol.saturating_add(tol / 2);
    let mut edge = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if removed[i] || alpha[i] == 0 || max_delta(pic.pixel(i), target) > halo {
                continue;
            }
            let touches = (y.saturating_sub(1)..=(y + 1).min(h - 1))
                .any(|ny| (x.saturating_sub(1)..=(x + 1).min(w - 1)).any(|nx| removed[ny * w + nx]));
            if touches {
                edge.push(i);
            }
        }
    }
    let mut any = false;
    for (i, r) in removed.iter().enumerate() {
        if *r {
            any |= alpha[i] != 0;
            alpha[i] = 0;
        }
    }
    for i in edge {
        alpha[i] = 0;
    }
    any.then_some(target)
}

/// Most common color along the borders (opaque pixels). With `strict`, only when it covers at
/// least half of the border.
fn border_color(pic: Picture, tol: u8, strict: bool) -> Option<Color> {
    let (w, h) = (pic.width as usize, pic.height as usize);
    let mut border: Vec<[u8; 4]> = Vec::new();
    for x in 0..w {
        border.push(pic.pixel(x));
        border.push(pic.pixel((h - 1) * w + x));
    }
    for y in 1..h.saturating_sub(1) {
        border.push(pic.pixel(y * w));
        border.push(pic.pixel(y * w + w - 1));
    }
    border.retain(|p| p[3] >= 128);
    if border.is_empty() {
        return None;
    }
    let mut bins: BTreeMap<u16, (u32, [u64; 3])> = BTreeMap::new();
    for p in &border {
        let key = ((p[0] as u16 >> 4) << 8) | ((p[1] as u16 >> 4) << 4) | (p[2] as u16 >> 4);
        let e = bins.entry(key).or_insert((0, [0; 3]));
        e.0 += 1;
        for (s, v) in e.1.iter_mut().zip(&p[..3]) {
            *s += *v as u64;
        }
    }
    let (_, (n, sum)) = bins.iter().max_by_key(|(k, (n, _))| (*n, u16::MAX - **k))?;
    let mean = Color::rgb((sum[0] / *n as u64) as u8, (sum[1] / *n as u64) as u8, (sum[2] / *n as u64) as u8);
    let share = border.iter().filter(|p| max_delta(**p, mean) <= tol).count();
    (!strict || share * 2 >= border.len()).then_some(mean)
}

/// The part of the picture to convert: the visible content (when cropping), aligned to the cell
/// grid in cell mode.
fn content_region(pic: Picture, alpha: &[u8], opts: &PixelateOptions) -> Result<Rect, DrawError> {
    let (w, h) = (pic.width as usize, pic.height as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if alpha[y * w + x] >= 32 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 == usize::MAX {
        return Err(invalid("a imagem não tem pixels visíveis (o fundo removido cobriu tudo? use background \"keep\")"));
    }
    if !opts.crop {
        return Ok(Rect::new(0, 0, pic.width, pic.height));
    }
    if let Some(cell) = opts.cell {
        let c = cell as usize;
        x0 = x0 / c * c;
        y0 = y0 / c * c;
        x1 = x1.div_ceil(c) * c;
        y1 = y1.div_ceil(c) * c;
        x1 = x1.min(w);
        y1 = y1.min(h);
    }
    Ok(Rect::new(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32))
}

struct Layout {
    canvas: (u32, u32),
    content: (usize, usize),
    offset: (usize, usize),
}

fn layout(region: Rect, opts: &PixelateOptions) -> Result<Layout, DrawError> {
    let m = if opts.outline { 2 } else { 0 };
    let (rw, rh) = (region.w as f64, region.h as f64);
    let fit = |v: f64, limit: u32| (v.round() as u32).clamp(1, limit.max(1));
    let (cw, ch, cnv_w, cnv_h) = if let Some(cell) = opts.cell {
        let cw = ((region.w + cell / 2) / cell).max(1);
        let ch = ((region.h + cell / 2) / cell).max(1);
        (cw, ch, cw + m, ch + m)
    } else {
        match (opts.width, opts.height) {
            (Some(tw), Some(th)) => {
                let (aw, ah) = (tw.saturating_sub(m).max(1), th.saturating_sub(m).max(1));
                let s = (aw as f64 / rw).min(ah as f64 / rh);
                (fit(rw * s, aw), fit(rh * s, ah), tw.max(m + 1), th.max(m + 1))
            }
            (Some(tw), None) => {
                let cw = tw.saturating_sub(m).max(1);
                let ch = fit(cw as f64 * rh / rw, MAX_IMAGE_SIDE);
                (cw, ch, cw + m, ch + m)
            }
            (None, Some(th)) => {
                let ch = th.saturating_sub(m).max(1);
                let cw = fit(ch as f64 * rw / rh, MAX_IMAGE_SIDE);
                (cw, ch, cw + m, ch + m)
            }
            (None, None) => {
                let side = opts.max_side.saturating_sub(m).max(1);
                let (cw, ch) = if rw >= rh { (side, fit(side as f64 * rh / rw, side)) } else { (fit(side as f64 * rw / rh, side), side) };
                (cw, ch, cw + m, ch + m)
            }
        }
    };
    if cnv_w > MAX_IMAGE_SIDE || cnv_h > MAX_IMAGE_SIDE {
        return Err(DrawError::TooLarge(format!("resultado {cnv_w}x{cnv_h} passa de {MAX_IMAGE_SIDE} px por lado")));
    }
    Ok(Layout {
        canvas: (cnv_w, cnv_h),
        content: (cw as usize, ch as usize),
        offset: (((cnv_w - cw) / 2) as usize, ((cnv_h - ch) / 2) as usize),
    })
}

/// Splits `len` picture pixels starting at `start` into `n` blocks (exact `cell`-sized blocks in
/// cell mode). Every block has at least one pixel.
fn blocks(start: usize, len: usize, n: usize, cell: Option<u32>) -> Vec<(usize, usize)> {
    let end = start + len;
    (0..n)
        .map(|i| {
            let (a, b) = match cell {
                Some(c) => (start + i * c as usize, start + (i + 1) * c as usize),
                None => {
                    let s = len as f64 / n as f64;
                    (start + (i as f64 * s).floor() as usize, start + ((i + 1) as f64 * s).floor() as usize)
                }
            };
            let a = a.min(end - 1);
            (a, b.clamp(a + 1, end))
        })
        .collect()
}

// ---- color ----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Lab {
    l: f64,
    a: f64,
    b: f64,
}

impl Lab {
    fn dist2(self, o: Lab) -> f64 {
        let (dl, da, db) = (self.l - o.l, self.a - o.a, self.b - o.b);
        dl * dl + da * da + db * db
    }
}

/// sRGB to linear lookup.
struct Lut {
    linear: [f64; 256],
}

impl Lut {
    fn new() -> Self {
        let mut linear = [0f64; 256];
        for (i, v) in linear.iter_mut().enumerate() {
            let c = i as f64 / 255.0;
            *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        Lut { linear }
    }

    fn linear(&self, p: [u8; 4]) -> [f64; 3] {
        [self.linear[p[0] as usize], self.linear[p[1] as usize], self.linear[p[2] as usize]]
    }

    fn lab(&self, p: [u8; 4]) -> Lab {
        linear_to_lab(self.linear(p))
    }
}

fn linear_to_lab([r, g, b]: [f64; 3]) -> Lab {
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    Lab {
        l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

fn lab_to_color(c: Lab) -> Color {
    let l = (c.l + 0.396_337_777_4 * c.a + 0.215_803_757_3 * c.b).powi(3);
    let m = (c.l - 0.105_561_345_8 * c.a - 0.063_854_172_8 * c.b).powi(3);
    let s = (c.l - 0.089_484_177_5 * c.a - 1.291_485_548 * c.b).powi(3);
    let lin = [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ];
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round().clamp(0.0, 255.0) as u8
    };
    Color::rgb(enc(lin[0]), enc(lin[1]), enc(lin[2]))
}

fn adjust(c: Lab, opts: &PixelateOptions) -> Lab {
    if opts.contrast == 1.0 && opts.brightness == 0.0 && opts.saturation == 1.0 {
        return c;
    }
    Lab {
        l: ((c.l - 0.5) * opts.contrast + 0.5 + opts.brightness * 0.5).clamp(0.0, 1.0),
        a: c.a * opts.saturation,
        b: c.b * opts.saturation,
    }
}

fn nearest(p: Lab, pal: &[Lab]) -> usize {
    let mut best = (f64::MAX, 0);
    for (i, c) in pal.iter().enumerate() {
        let d = p.dist2(*c);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

fn nearest_two(p: Lab, pal: &[Lab]) -> (usize, usize) {
    let (mut a, mut b) = ((f64::MAX, 0), (f64::MAX, 0));
    for (i, c) in pal.iter().enumerate() {
        let d = p.dist2(*c);
        if d < a.0 {
            b = a;
            a = (d, i);
        } else if d < b.0 {
            b = (d, i);
        }
    }
    (a.1, if b.0 == f64::MAX { a.1 } else { b.1 })
}

/// Weighted k-means (Lloyd) in OKLab. Empty clusters keep their center.
fn kmeans(points: &[(Lab, f64)], mut centers: Vec<Lab>, iterations: usize) -> Vec<Lab> {
    let mut assign = vec![usize::MAX; points.len()];
    for _ in 0..iterations {
        let mut changed = false;
        for (i, (p, _)) in points.iter().enumerate() {
            let n = nearest(*p, &centers);
            if assign[i] != n {
                assign[i] = n;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        let mut acc = vec![(0f64, 0f64, 0f64, 0f64); centers.len()];
        for (i, (p, wgt)) in points.iter().enumerate() {
            let e = &mut acc[assign[i]];
            e.0 += p.l * wgt;
            e.1 += p.a * wgt;
            e.2 += p.b * wgt;
            e.3 += wgt;
        }
        for (c, e) in centers.iter_mut().zip(acc) {
            if e.3 > 0.0 {
                *c = Lab { l: e.0 / e.3, a: e.1 / e.3, b: e.2 / e.3 };
            }
        }
    }
    centers
}

/// Weighted color set from Lab samples, keyed by their sRGB color (deterministic order).
fn color_set<'a>(samples: impl Iterator<Item = &'a Lab>) -> BTreeMap<Color, (u32, Lab)> {
    let mut set: BTreeMap<Color, (u32, Lab)> = BTreeMap::new();
    for c in samples {
        let e = set.entry(lab_to_color(*c)).or_insert((0, *c));
        e.0 += 1;
    }
    set
}

/// Median cut seeds, refined by k-means; returns up to `k` distinct colors.
fn palette_from(set: &BTreeMap<Color, (u32, Lab)>, k: usize) -> Vec<Lab> {
    let counts: BTreeMap<Color, u32> = set.iter().map(|(c, (n, _))| (*c, *n)).collect();
    let seeds = median_cut(&counts, k);
    if counts.len() <= k {
        return seeds.iter().map(|c| set.get(c).map_or_else(|| Lut::new().lab([c.r, c.g, c.b, 255]), |e| e.1)).collect();
    }
    let lut = Lut::new();
    let init: Vec<Lab> = seeds.iter().map(|c| lut.lab([c.r, c.g, c.b, 255])).collect();
    // At most ~65k points: enough to place the centers, cheap on big results.
    let step = set.len().div_ceil(65_536).max(1);
    let points: Vec<(Lab, f64)> = set.values().step_by(step).map(|(n, lab)| (*lab, *n as f64)).collect();
    kmeans(&points, init, 24)
}

/// The palette of an automatic conversion: `k` colors (or fewer) from the sampled cells.
fn reduce(cells: &[Option<Lab>], k: usize) -> Vec<Color> {
    let set = color_set(cells.iter().flatten());
    let mut colors: Vec<Color> = palette_from(&set, k).into_iter().map(lab_to_color).collect();
    colors.sort();
    colors.dedup();
    colors
}

/// Coarse color groups of the picture, used by the dominant sampling to decide which pixels of a
/// block belong together.
struct Grouping {
    k: usize,
    lut: Vec<u8>,
}

impl Grouping {
    fn new(pic: Picture, alpha: &[u8], region: Rect, k: usize, lut: &Lut) -> Grouping {
        let w = pic.width as usize;
        let area = region.w as usize * region.h as usize;
        let step = ((area as f64 / 65_536.0).sqrt().ceil() as usize).max(1);
        let mut samples = Vec::new();
        for y in (region.y as usize..region.y as usize + region.h as usize).step_by(step) {
            for x in (region.x as usize..region.x as usize + region.w as usize).step_by(step) {
                let i = y * w + x;
                if alpha[i] >= 128 {
                    samples.push(lut.lab(pic.pixel(i)));
                }
            }
        }
        let centers = palette_from(&color_set(samples.iter()), k);
        let centers = if centers.is_empty() { vec![Lab { l: 0.0, a: 0.0, b: 0.0 }] } else { centers };
        // 5 bits per channel → nearest group.
        let table = (0..32768u32)
            .map(|i| {
                let ch = |s: u32| (((i >> s) & 31) << 3 | 4) as u8;
                nearest(lut.lab([ch(10), ch(5), ch(0), 255]), &centers) as u8
            })
            .collect();
        Grouping { k: centers.len(), lut: table }
    }

    fn of(&self, p: [u8; 4]) -> usize {
        self.lut[((p[0] as usize >> 3) << 10) | ((p[1] as usize >> 3) << 5) | (p[2] as usize >> 3)] as usize
    }
}

const BAYER4: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

/// Palette index for every cell (row by row), with the chosen dithering.
fn map_colors(cells: &[Option<Lab>], cw: usize, ch: usize, offset: (usize, usize), pal: &[Lab], dither: Dither) -> Vec<Option<usize>> {
    match dither {
        Dither::None => cells.iter().map(|c| c.map(|c| nearest(c, pal))).collect(),
        Dither::Ordered => cells
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let c = (*c)?;
                let (i1, i2) = nearest_two(c, pal);
                if i1 == i2 {
                    return Some(i1);
                }
                let (p1, p2) = (pal[i1], pal[i2]);
                let seg = Lab { l: p2.l - p1.l, a: p2.a - p1.a, b: p2.b - p1.b };
                let len2 = seg.l * seg.l + seg.a * seg.a + seg.b * seg.b;
                let t = if len2 > 0.0 { ((c.l - p1.l) * seg.l + (c.a - p1.a) * seg.a + (c.b - p1.b) * seg.b) / len2 } else { 0.0 };
                let (x, y) = (offset.0 + k % cw, offset.1 + k / cw);
                let threshold = (BAYER4[(y & 3) * 4 + (x & 3)] as f64 + 0.5) / 16.0;
                Some(if t.clamp(0.0, 1.0) > threshold { i2 } else { i1 })
            })
            .collect(),
        Dither::FloydSteinberg => {
            const DAMP: f64 = 0.8;
            let mut work: Vec<Option<Lab>> = cells.to_vec();
            let mut out = vec![None; cells.len()];
            for y in 0..ch {
                let forward = y % 2 == 0;
                for step in 0..cw {
                    let x = if forward { step } else { cw - 1 - step };
                    let k = y * cw + x;
                    let Some(c) = work[k] else { continue };
                    let i = nearest(c, pal);
                    out[k] = Some(i);
                    let err = Lab { l: (c.l - pal[i].l) * DAMP, a: (c.a - pal[i].a) * DAMP, b: (c.b - pal[i].b) * DAMP };
                    let dir: isize = if forward { 1 } else { -1 };
                    for (dx, dy, wgt) in [(dir, 0, 7.0), (-dir, 1, 3.0), (0, 1, 5.0), (dir, 1, 1.0)] {
                        let (nx, ny) = (x as isize + dx, y + dy);
                        if nx < 0 || nx >= cw as isize || ny >= ch {
                            continue;
                        }
                        if let Some(n) = work[ny * cw + nx as usize].as_mut() {
                            let f = wgt / 16.0;
                            n.l += err.l * f;
                            n.a += err.a * f;
                            n.b += err.b * f;
                        }
                    }
                }
            }
            out
        }
    }
}

fn outline_color(colors: &[Color], lut: &Lut) -> Color {
    let darkest = colors.iter().map(|c| lut.lab([c.r, c.g, c.b, 255])).min_by(|a, b| a.l.total_cmp(&b.l));
    match darkest {
        Some(d) => lab_to_color(Lab { l: d.l * 0.45, a: d.a * 0.6, b: d.b * 0.6 }),
        None => Color::BLACK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(img: &Image) -> Vec<u8> {
        img.to_rgba()
    }

    fn pic<'a>(img: &Image, bytes: &'a [u8]) -> Picture<'a> {
        Picture { width: img.width(), height: img.height(), rgba: bytes }
    }

    fn sprite() -> Image {
        // 8x6 sprite: dark outline, two body colors and a highlight, on transparency.
        let rows = ["..kkkk..", ".kbbbbk.", "kbbwbbbk", "kbbbbrrk", ".kbbbbk.", "..kkkk.."];
        let mut img = Image::new(8, 6);
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let c = match ch {
                    'k' => Color::rgb(20, 18, 30),
                    'b' => Color::rgb(57, 118, 210),
                    'w' => Color::rgb(240, 240, 245),
                    'r' => Color::rgb(220, 60, 50),
                    _ => continue,
                };
                img.set(x as i32, y as i32, c);
            }
        }
        img
    }

    fn opts() -> PixelateOptions {
        PixelateOptions { cleanup: Cleanup::None, ..Default::default() }
    }

    #[test]
    fn recovers_upscaled_pixel_art_exactly() {
        let small = sprite();
        let big = small.scale_nearest(40, 30);
        let bytes = picture(&big);
        for sampling in [Sampling::Dominant, Sampling::Average, Sampling::Nearest] {
            let r = pixelate(pic(&big, &bytes), &PixelateOptions { cell: Some(5), sampling, ..opts() }).unwrap();
            assert_eq!(r.image, small, "{sampling:?}");
            assert_eq!(r.palette.len(), 4);
            assert_eq!(r.block, (5.0, 5.0));
        }
        // Without `cell`: the same sprite, by target size.
        let r = pixelate(pic(&big, &bytes), &PixelateOptions { max_side: 8, ..opts() }).unwrap();
        assert_eq!(r.image, small);
    }

    #[test]
    fn removes_a_solid_background_and_crops_to_the_subject() {
        let mut img = Image::filled(60, 40, Color::rgb(250, 250, 250));
        for y in 10..30 {
            for x in 20..40 {
                img.set(x, y, Color::rgb(200, 40, 40));
            }
        }
        for y in 14..18 {
            for x in 24..28 {
                img.set(x, y, Color::rgb(30, 30, 140));
            }
        }
        let bytes = picture(&img);
        let r = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 10, ..opts() }).unwrap();
        assert_eq!(r.background, Some(Color::rgb(250, 250, 250)));
        assert_eq!(r.source_rect, Rect::new(20, 10, 20, 20));
        assert_eq!((r.image.width(), r.image.height()), (10, 10));
        assert_eq!(r.image.opaque_count(), 100);
        assert!(!r.palette.contains(&Color::rgb(250, 250, 250)));
        assert_eq!(r.image.get(2, 2), Some(Color::rgb(30, 30, 140)));
        assert_eq!(r.image.get(8, 8), Some(Color::rgb(200, 40, 40)));
        // Keeping the background: the whole picture, opaque.
        let kept = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 12, background: Background::Keep, ..opts() }).unwrap();
        assert_eq!(kept.background, None);
        assert_eq!((kept.image.width(), kept.image.height()), (12, 8));
        assert_eq!(kept.image.opaque_count(), 96);
    }

    #[test]
    fn auto_background_leaves_varied_borders_alone() {
        let mut img = Image::new(32, 32);
        for y in 0..32 {
            for x in 0..32 {
                img.set(x, y, Color::rgb((x * 8) as u8, (y * 8) as u8, 128));
            }
        }
        let bytes = picture(&img);
        let r = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 16, colors: 6, ..opts() }).unwrap();
        assert_eq!(r.background, None);
        assert_eq!(r.image.opaque_count(), 256);
        assert!(r.palette.len() <= 6 && r.palette.len() >= 4, "{:?}", r.palette);
        let again = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 16, colors: 6, ..opts() }).unwrap();
        assert_eq!(r.image, again.image, "deterministic");
        // Fixed palette: only those colors appear.
        let fixed = vec![Color::rgb(0, 0, 0), Color::rgb(255, 255, 255), Color::rgb(255, 0, 0)];
        let f = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 16, palette: fixed.clone(), ..opts() }).unwrap();
        assert!(f.image.color_counts().keys().all(|c| fixed.contains(c)));
        assert_eq!(f.palette, fixed);
    }

    #[test]
    fn ordered_dither_mixes_the_two_nearest_colors() {
        let img = Image::filled(16, 16, Color::rgb(119, 119, 119));
        let bytes = picture(&img);
        let pal = vec![Color::BLACK, Color::WHITE];
        let base = PixelateOptions { max_side: 16, palette: pal, background: Background::Keep, ..opts() };
        let flat = pixelate(pic(&img, &bytes), &base).unwrap();
        assert_eq!(flat.image.color_counts().len(), 1);
        let dithered = pixelate(pic(&img, &bytes), &PixelateOptions { dither: Dither::Ordered, ..base.clone() }).unwrap();
        let counts = dithered.image.color_counts();
        assert_eq!(counts.len(), 2);
        let fs = pixelate(pic(&img, &bytes), &PixelateOptions { dither: Dither::FloydSteinberg, ..base }).unwrap();
        assert_eq!(fs.image.color_counts().len(), 2);
    }

    #[test]
    fn outline_and_proportional_sizes() {
        let mut img = Image::filled(40, 20, Color::WHITE);
        for y in 4..16 {
            for x in 4..36 {
                img.set(x, y, Color::rgb(60, 160, 60));
            }
        }
        let bytes = picture(&img);
        let r = pixelate(pic(&img, &bytes), &PixelateOptions { width: Some(18), outline: true, ..opts() }).unwrap();
        // Content 16x6 plus 1 px of margin on each side for the outline.
        assert_eq!((r.image.width(), r.image.height()), (18, 8));
        let edge = r.image.get(0, 3).unwrap();
        assert!(!edge.is_transparent());
        assert_ne!(edge, Color::rgb(60, 160, 60));
        assert_eq!(r.image.get(0, 0), Some(Color::TRANSPARENT), "4-connected outline leaves the corners");
        assert!(r.palette.contains(&edge));
        let tall = pixelate(pic(&img, &bytes), &PixelateOptions { height: Some(3), ..opts() }).unwrap();
        assert_eq!((tall.image.width(), tall.image.height()), (8, 3));
    }

    #[test]
    fn manual_region_converts_only_that_part() {
        let mut img = Image::filled(80, 40, Color::WHITE);
        for y in 10..30 {
            for x in 5..25 {
                img.set(x, y, Color::rgb(200, 30, 30));
            }
            for x in 50..70 {
                img.set(x, y, Color::rgb(30, 30, 200));
            }
        }
        let bytes = picture(&img);
        let r = pixelate(pic(&img, &bytes), &PixelateOptions { max_side: 8, region: Some(Rect::new(40, 0, 40, 40)), ..opts() }).unwrap();
        assert_eq!(r.background, Some(Color::WHITE), "background removed from the region borders");
        assert_eq!(r.source_rect, Rect::new(50, 10, 20, 20), "in picture coordinates");
        assert_eq!(r.palette, vec![Color::rgb(30, 30, 200)]);
        assert_eq!(r.image.opaque_count(), 64);
        let outside = pixelate(pic(&img, &bytes), &PixelateOptions { region: Some(Rect::new(100, 0, 10, 10)), ..opts() });
        assert!(outside.is_err());
    }

    #[test]
    fn errors_are_clear() {
        let img = Image::filled(8, 8, Color::WHITE);
        let bytes = picture(&img);
        assert!(pixelate(pic(&img, &bytes), &PixelateOptions { colors: 1, ..opts() }).is_err());
        assert!(pixelate(pic(&img, &bytes), &PixelateOptions { cell: Some(0), ..opts() }).is_err());
        let all_bg = pixelate(pic(&img, &bytes), &PixelateOptions { background: Background::Remove, ..opts() });
        assert!(all_bg.is_err(), "removing the only color leaves nothing");
        let bad = Picture { width: 3, height: 3, rgba: &[0; 5] };
        assert!(pixelate(bad, &opts()).is_err());
    }

    #[test]
    fn thumbnail_fits() {
        let img = sprite().scale_nearest(80, 60);
        let bytes = picture(&img);
        let t = thumbnail(pic(&img, &bytes), 16).unwrap();
        assert_eq!((t.width(), t.height()), (16, 12));
        let same = thumbnail(pic(&img, &bytes), 500).unwrap();
        assert_eq!(same, img);
    }
}
