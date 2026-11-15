//! Pixel and drawing commands. Every write ends up in [`apply_op`], which draws one SDC
//! operation on one cel through a clipped canvas.

use std::collections::BTreeMap;

use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame, layer_image};
use sprite_core::matrix::encode_region;
use sprite_core::{Color, DirtyRect, FrameRef, Image, ItemRef, Mask, PasteMode, Point, Rect, SourceRef};
use sprite_drawing::sdc::{self, ColorRef, DrawContext, DrawOp, PixelItem};
use sprite_drawing::transform::{Transform, transform_area};
use sprite_drawing::{Canvas, DrawError, DrawStats};
use sprite_project::Document;

use crate::args::*;
use crate::target::{check_point, frame_id, layer_id, palette_colors, region_mask, resolve, sprite, sprite_index, writable_cel};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

/// Pixel changes accumulated per (layer, frame).
#[derive(Default)]
pub(crate) struct ChangeSet {
    sprite: u32,
    map: BTreeMap<(u32, u32), DirtyRect>,
    pub changed: u32,
}

impl ChangeSet {
    pub fn new(sprite: u32) -> Self {
        ChangeSet { sprite, ..Default::default() }
    }

    /// Records a change on an image; linked cels (same image) are reported too.
    pub fn add(&mut self, doc: &Document, si: usize, image: u32, stats: &DrawStats) {
        self.changed += stats.changed;
        if stats.changed == 0 {
            return;
        }
        for cel in doc.model.sprites[si].cels.iter().filter(|c| c.image == image) {
            self.map.entry((cel.layer, cel.frame)).or_default().merge(&stats.dirty);
        }
    }

    pub fn changes(&self) -> Vec<Change> {
        self.map
            .iter()
            .map(|((layer, frame), d)| Change {
                sprite: self.sprite,
                layer: Some(*layer),
                frame: Some(*frame),
                rect: d.rect(),
                kind: "pixels".into(),
            })
            .collect()
    }

    pub fn bounds(&self) -> Option<Rect> {
        let mut all = DirtyRect::default();
        for d in self.map.values() {
            all.merge(d);
        }
        all.rect()
    }

    pub fn outcome(&self, extra: serde_json::Value) -> Outcome {
        let mut data = json!({ "changed_pixels": self.changed, "region": self.bounds() });
        if let (Some(obj), Some(more)) = (data.as_object_mut(), extra.as_object()) {
            for (k, v) in more {
                obj.insert(k.clone(), v.clone());
            }
        }
        Outcome { data, changes: self.changes(), ..Default::default() }
    }
}

fn load_source(doc: &Document, si: usize, layer: u32, frame: u32, from: &SourceRef) -> Result<Image, CommandError> {
    let s = sprite(doc, si);
    let f = match &from.frame {
        Some(r) => s.resolve_frame(r)?,
        None => frame,
    };
    if from.composite {
        return Ok(compose_frame(s, doc.images(), f, &ComposeOptions::default()));
    }
    let l = match &from.layer {
        Some(r) => s.layer(r)?.id,
        None => layer,
    };
    Ok(layer_image(s, doc.images(), l, f))
}

/// Draws one operation on the cel (layer, frame). Clip masks restrict where it paints.
/// One cel: sprite index, layer id, frame id.
#[derive(Clone, Copy)]
pub(crate) struct Cel {
    pub si: usize,
    pub layer: u32,
    pub frame: u32,
}

pub(crate) fn apply_op(
    doc: &mut Document,
    cel: Cel,
    op: &DrawOp,
    clip: Option<&Mask>,
    palette: &[Color],
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let Cel { si, layer, frame } = cel;
    let prepared = super::texto::prepare_op(doc, op)?;
    let op: &DrawOp = &prepared;
    let source = match op {
        DrawOp::Copy { from, .. } => Some((from.clone(), load_source(doc, si, layer, frame, from)?)),
        _ => None,
    };
    let image = writable_cel(doc, si, layer, frame)?;
    let loader = |r: &SourceRef| -> Result<Image, DrawError> {
        source.as_ref().filter(|(s, _)| s == r).map(|(_, i)| i.clone()).ok_or_else(|| DrawError::Source("fonte não carregada".into()))
    };
    let stats = {
        let img = doc.image_mut(image).ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
        let ctx = DrawContext { palette, source: Some(&loader) };
        let mut canvas = Canvas::new(img, clip);
        sdc::apply(&mut canvas, op, &ctx)?;
        canvas.stats()
    };
    changes.add(doc, si, image, &stats);
    Ok(())
}

/// Applies a transform to the visible pixels of `area` on one cel (no clipping of the result).
pub(crate) fn apply_transform(
    doc: &mut Document,
    si: usize,
    layer: u32,
    frame: u32,
    area: &Mask,
    t: Transform,
    changes: &mut ChangeSet,
) -> Result<(), CommandError> {
    let image = writable_cel(doc, si, layer, frame)?;
    let stats = {
        let img = doc.image_mut(image).ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
        let mut canvas = Canvas::new(img, None);
        transform_area(&mut canvas, area, t)?;
        canvas.stats()
    };
    changes.add(doc, si, image, &stats);
    Ok(())
}

fn single(s: &mut Session, target: &Target, op: DrawOp, clip_region: Option<&ItemRef>) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, target)?;
    let palette = palette_colors(&s.doc, r.si);
    let clip = clip_region.map(|reg| region_mask(&s.doc, r.si, reg, r.frame)).transpose()?;
    let mut changes = ChangeSet::new(sprite(&s.doc, r.si).id);
    apply_op(&mut s.doc, Cel { si: r.si, layer: r.layer, frame: r.frame }, &op, clip.as_ref(), &palette, &mut changes)?;
    Ok(changes.outcome(json!({ "layer": r.layer, "frame": r.frame })))
}

pub(crate) fn pixel_get(s: &mut Session, a: PixelGet) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    check_point(sp, a.x, a.y)?;
    let img = if a.composite {
        compose_frame(sp, s.doc.images(), r.frame, &ComposeOptions::default())
    } else {
        layer_image(sp, s.doc.images(), r.layer, r.frame)
    };
    let c = img.get_or_clear(a.x, a.y);
    let palette = palette_colors(&s.doc, r.si);
    Ok(Outcome::data(json!({
        "x": a.x, "y": a.y, "color": c, "transparent": c.is_transparent(),
        "palette_index": palette.iter().position(|p| *p == c),
        "layer": if a.composite { None } else { Some(r.layer) }, "frame": r.frame,
    })))
}

pub(crate) fn pixel_set(s: &mut Session, a: PixelSet) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    check_point(sprite(&s.doc, r.si), a.x, a.y)?;
    single(s, &a.target, DrawOp::Pixel { x: a.x, y: a.y, color: a.color }, None)
}

pub(crate) fn pixel_set_batch(s: &mut Session, a: PixelSetBatch) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let outside = a
        .pixels
        .iter()
        .filter(|p| {
            let (x, y) = match p {
                PixelItem::At(x, y) | PixelItem::WithColor(x, y, _) => (*x, *y),
            };
            x < 0 || y < 0 || x >= sp.width as i32 || y >= sp.height as i32
        })
        .count();
    let mut out = single(s, &a.target, DrawOp::Pixels { pixels: a.pixels, color: a.color }, None)?;
    if outside > 0 {
        out.warnings.push(format!("{outside} pixels fora do canvas foram ignorados"));
    }
    Ok(out)
}

pub(crate) fn pixel_clear(s: &mut Session, a: PixelClear) -> Result<Outcome, CommandError> {
    if a.mask.is_some() {
        return super::ferramentas::pixel_clear_masked(s, a);
    }
    let op = match (a.at, a.rect) {
        (Some(p), _) => DrawOp::Pixel { x: p.x, y: p.y, color: ColorRef::Value("clear".into()) },
        (None, rect) => DrawOp::Clear { rect },
    };
    single(s, &a.target, op, None)
}

pub(crate) fn pixel_read_region(s: &mut Session, a: PixelReadRegion) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let img = if a.composite {
        compose_frame(sp, s.doc.images(), r.frame, &ComposeOptions::default())
    } else {
        layer_image(sp, s.doc.images(), r.layer, r.frame)
    };
    let mut rect = a.rect.unwrap_or(img.bounds()).intersect(&img.bounds());
    if a.trim {
        match img.crop(rect).opaque_bounds() {
            Some(b) => rect = b.translate(rect.x, rect.y),
            None => {
                return Ok(Outcome::data(json!({ "rect": null, "rows": [], "legend": {}, "note": "região vazia (toda transparente)" })));
            }
        }
    }
    if rect.is_empty() {
        return Err(CommandError::new(ErrorCode::OutOfBounds, "a região não cruza o sprite"));
    }
    let palette = palette_colors(&s.doc, r.si);
    match encode_region(&img, rect, &palette, a.rle) {
        Some(m) => Ok(Outcome::data(json!({
            "rect": m.rect, "legend": m.legend, "rows": m.rows, "rle": a.rle,
            "layer": if a.composite { None } else { Some(r.layer) }, "frame": r.frame,
        }))),
        None => {
            // Too many colors for one-char codes: list visible pixels instead.
            let pixels: Vec<serde_json::Value> =
                rect.points().filter_map(|p| img.get(p.x, p.y).filter(|c| !c.is_transparent()).map(|c| json!([p.x, p.y, c]))).collect();
            Ok(Outcome::data(
                json!({ "rect": rect, "pixels": pixels, "note": "cores demais para a matriz textual; lista [x,y,cor] dos pixels visíveis" }),
            ))
        }
    }
}

fn matrix_op(at: Point, d: MatrixData) -> DrawOp {
    DrawOp::Matrix { at, rows: d.rows, legend: d.legend, rle: d.rle, pixels: d.pixels, palette: d.palette }
}

pub(crate) fn pixel_write_region(s: &mut Session, a: PixelWriteRegion) -> Result<Outcome, CommandError> {
    if a.data.rows.is_empty() && a.data.pixels.is_empty() {
        return Err(CommandError::invalid("informe `rows` (+ `legend`) ou `pixels`"));
    }
    single(s, &a.target, matrix_op(a.at, a.data), None)
}

pub(crate) fn apply_patch(s: &mut Session, target: Target, patch: Patch) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &target)?;
    let palette = palette_colors(&s.doc, r.si);
    let mut changes = ChangeSet::new(sprite(&s.doc, r.si).id);
    let has_matrix = !patch.matrix.rows.is_empty() || !patch.matrix.pixels.is_empty();
    if !has_matrix && patch.changes.is_empty() {
        return Err(CommandError::invalid("patch vazio: informe uma matriz (`rows`/`pixels` + `at`) e/ou `changes`"));
    }
    if has_matrix {
        let at = patch.at.ok_or_else(|| CommandError::invalid("patch com matriz precisa de `at`"))?;
        apply_op(&mut s.doc, Cel { si: r.si, layer: r.layer, frame: r.frame }, &matrix_op(at, patch.matrix), None, &palette, &mut changes)?;
    }
    if !patch.changes.is_empty() {
        apply_op(
            &mut s.doc,
            Cel { si: r.si, layer: r.layer, frame: r.frame },
            &DrawOp::Pixels { pixels: patch.changes, color: None },
            None,
            &palette,
            &mut changes,
        )?;
    }
    Ok(changes.outcome(json!({ "layer": r.layer, "frame": r.frame })))
}

/// Runs batch operations with per-op targets. Shared by draw_batch and frame_edit.
pub(crate) fn run_batch(
    s: &mut Session,
    target: &Target,
    ops: &[BatchOp],
    clip: Option<&ItemRef>,
    policy: BatchPolicy,
    ctx: &ExecContext,
) -> Result<(ChangeSet, Vec<serde_json::Value>), CommandError> {
    if ops.len() > ctx.limits.max_batch_ops {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("{} operações; o máximo por lote é {}", ops.len(), ctx.limits.max_batch_ops),
        ));
    }
    let base = resolve(&s.doc, target)?;
    let palette = palette_colors(&s.doc, base.si);
    let mut changes = ChangeSet::new(sprite(&s.doc, base.si).id);
    let mut skipped = Vec::new();
    fn attempt(
        doc: &mut Document,
        base: &crate::target::Resolved,
        bop: &BatchOp,
        clip: Option<&ItemRef>,
        palette: &[Color],
        changes: &mut ChangeSet,
    ) -> Result<(), CommandError> {
        let sp = sprite(doc, base.si);
        let layer = match &bop.layer {
            Some(l) => layer_id(sp, Some(l))?,
            None => base.layer,
        };
        let frame = match &bop.frame {
            Some(f) => frame_id(sp, Some(f))?,
            None => base.frame,
        };
        let region = bop.clip.as_ref().or(clip);
        let mask = region.map(|reg| region_mask(doc, base.si, reg, frame)).transpose()?;
        apply_op(doc, Cel { si: base.si, layer, frame }, &bop.op, mask.as_ref(), palette, changes)
    }
    for (i, bop) in ops.iter().enumerate() {
        match policy {
            BatchPolicy::AllOrNothing => {
                attempt(&mut s.doc, &base, bop, clip, &palette, &mut changes).map_err(|e| {
                    let mut e2 = CommandError::new(e.code, format!("operação {i} ({}): {}", bop.op.name(), e.message));
                    e2.details = Some(json!({ "failed_index": i, "op": bop.op.name(), "rolled_back": true }));
                    e2
                })?;
            }
            BatchPolicy::SkipInvalid => {
                s.doc.savepoint();
                match attempt(&mut s.doc, &base, bop, clip, &palette, &mut changes) {
                    Ok(()) => s.doc.release_savepoint(),
                    Err(e) => {
                        s.doc.rollback_savepoint();
                        skipped.push(json!({ "index": i, "op": bop.op.name(), "error": e.message, "code": e.code }));
                    }
                }
            }
        }
    }
    Ok((changes, skipped))
}

pub(crate) fn draw_batch(s: &mut Session, a: DrawBatch, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    if a.ops.is_empty() {
        return Err(CommandError::invalid("lote vazio"));
    }
    let (changes, skipped) = run_batch(s, &a.target, &a.ops, a.clip.as_ref(), a.policy, ctx)?;
    let mut out = changes.outcome(json!({ "ops": a.ops.len(), "applied": a.ops.len() - skipped.len(), "skipped": skipped }));
    out.warnings.extend(super::texto::batch_warnings(&s.doc, &a.ops));
    if !skipped.is_empty() {
        out.warnings.push(format!("{} operações inválidas foram puladas", skipped.len()));
    }
    Ok(out)
}

pub(crate) fn draw_line(s: &mut Session, a: DrawLine) -> Result<Outcome, CommandError> {
    single(s, &a.target, DrawOp::Line { from: a.from, to: a.to, color: a.color, thickness: a.thickness }, None)
}

pub(crate) fn draw_shape(s: &mut Session, a: DrawShape) -> Result<Outcome, CommandError> {
    let op = match a.shape {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            let r = a.rect.ok_or_else(|| CommandError::invalid("retângulo/elipse precisa de `rect` {x,y,w,h}"))?;
            if matches!(a.shape, ShapeKind::Rect) {
                DrawOp::Rect { x: r.x, y: r.y, w: r.w, h: r.h, color: a.color, fill: a.fill, thickness: a.thickness }
            } else {
                DrawOp::Ellipse { x: r.x, y: r.y, w: r.w, h: r.h, color: a.color, fill: a.fill, thickness: a.thickness }
            }
        }
        ShapeKind::Circle => DrawOp::Circle {
            center: a.center.ok_or_else(|| CommandError::invalid("círculo precisa de `center`"))?,
            radius: a.radius.ok_or_else(|| CommandError::invalid("círculo precisa de `radius`"))?,
            color: a.color,
            fill: a.fill,
        },
    };
    single(s, &a.target, op, None)
}

pub(crate) fn draw_polygon(s: &mut Session, a: DrawPolygon) -> Result<Outcome, CommandError> {
    single(s, &a.target, DrawOp::Polygon { points: a.points, color: a.color, fill: a.fill }, None)
}

pub(crate) fn draw_fill(s: &mut Session, a: DrawFill) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    check_point(sprite(&s.doc, r.si), a.x, a.y)?;
    single(
        s,
        &a.target,
        DrawOp::Fill { x: a.x, y: a.y, color: a.color, contiguous: a.contiguous, connectivity: a.connectivity, tolerance: a.tolerance },
        None,
    )
}

pub(crate) fn draw_outline(s: &mut Session, a: DrawOutline) -> Result<Outcome, CommandError> {
    single(
        s,
        &a.target,
        DrawOp::Outline { color: a.color, mode: a.mode, connectivity: a.connectivity, thickness: a.thickness, rect: a.rect },
        None,
    )
}

pub(crate) fn draw_dither(s: &mut Session, a: DrawDither) -> Result<Outcome, CommandError> {
    single(
        s,
        &a.target,
        DrawOp::Dither {
            color: a.color,
            background: a.background,
            density: a.density,
            pattern: a.pattern,
            rect: a.rect,
            only_opaque: a.only_opaque,
        },
        None,
    )
}

pub(crate) fn draw_gradient(s: &mut Session, a: DrawGradient) -> Result<Outcome, CommandError> {
    single(
        s,
        &a.target,
        DrawOp::Gradient { colors: a.colors, shape: a.shape, dither: a.dither, rect: a.rect, only_opaque: a.only_opaque },
        None,
    )
}

pub(crate) fn draw_shade(s: &mut Session, a: DrawShade) -> Result<Outcome, CommandError> {
    let op = DrawOp::Shade {
        light: a.light,
        highlight: a.highlight,
        shadow: a.shadow,
        highlight_amount: a.highlight_amount,
        shadow_amount: a.shadow_amount,
        highlight_depth: a.highlight_depth,
        shadow_depth: a.shadow_depth,
        hue_shift: a.hue_shift,
        rect: a.rect,
    };
    single(s, &a.target, op, a.clip.as_ref())
}

pub(crate) fn draw_transform(s: &mut Session, a: DrawTransform) -> Result<Outcome, CommandError> {
    if a.mask.is_some() {
        return super::ferramentas::draw_transform_masked(s, a);
    }
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let (w, h) = (sp.width, sp.height);
    let mut area = match &a.region {
        Some(reg) => region_mask(&s.doc, r.si, reg, r.frame)?,
        None => Mask::full(w, h),
    };
    if let Some(rect) = a.rect {
        area = area.intersect(&Mask::from_rect(w, h, rect));
    }
    let mut changes = ChangeSet::new(sp.id);
    apply_transform(&mut s.doc, r.si, r.layer, r.frame, &area, a.transform, &mut changes)?;
    Ok(changes.outcome(json!({ "layer": r.layer, "frame": r.frame })))
}

/// The distinct (layer, frame) cels a color scope covers.
fn scope_cels(doc: &Document, si: usize, layer: u32, frame: u32, scope: ColorScope) -> Vec<(u32, u32)> {
    let sp = sprite(doc, si);
    let mut out: Vec<(u32, u32)> = match scope {
        ColorScope::Cel => vec![(layer, frame)],
        ColorScope::Layer => sp.cels.iter().filter(|c| c.layer == layer).map(|c| (c.layer, c.frame)).collect(),
        ColorScope::Frame => sp.cels.iter().filter(|c| c.frame == frame).map(|c| (c.layer, c.frame)).collect(),
        ColorScope::Sprite => sp.cels.iter().map(|c| (c.layer, c.frame)).collect(),
    };
    // Linked cels share pixels; edit each image once.
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|(l, f)| sp.cel(*l, *f).map(|c| seen.insert(c.image)).unwrap_or(true));
    out.retain(|(l, _)| sp.layers.iter().any(|x| x.id == *l && !x.locked));
    out
}

pub(crate) fn draw_replace_color(s: &mut Session, a: DrawReplaceColor) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let palette = palette_colors(&s.doc, r.si);
    let mut changes = ChangeSet::new(sprite(&s.doc, r.si).id);
    let op = DrawOp::ReplaceColor { from: a.from, to: a.to, tolerance: a.tolerance, rect: None };
    for (layer, frame) in scope_cels(&s.doc, r.si, r.layer, r.frame, a.scope) {
        apply_op(&mut s.doc, Cel { si: r.si, layer, frame }, &op, None, &palette, &mut changes)?;
    }
    Ok(changes.outcome(json!({ "scope": a.scope })))
}

pub(crate) fn frame_edit(s: &mut Session, a: FrameEdit, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let fid = sprite(&s.doc, si).resolve_frame(&a.frame)?;
    let sprite_id = sprite(&s.doc, si).id;
    let mut structural = false;
    {
        let sp = &mut s.doc.model.sprites[si];
        let f = sp.frame_mut(fid).ok_or_else(|| CommandError::not_found(format!("frame {fid}")))?;
        if let Some(d) = a.duration_ms {
            if d == 0 || d > 60_000 {
                return Err(CommandError::invalid("duração precisa ficar entre 1 e 60000 ms"));
            }
            f.duration_ms = d;
            structural = true;
        }
        if let Some(tags) = a.tags {
            f.tags = tags;
            structural = true;
        }
    }
    if a.unlink {
        let cels: Vec<(usize, u32)> =
            sprite(&s.doc, si).cels.iter().enumerate().filter(|(_, c)| c.frame == fid).map(|(i, c)| (i, c.image)).collect();
        for (i, image) in cels {
            if sprite(&s.doc, si).cels.iter().filter(|c| c.image == image).count() > 1 {
                let copy = s.doc.image(image).cloned().ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
                let new_id = s.doc.insert_image(copy);
                s.doc.model.sprites[si].cels[i].image = new_id;
                structural = true;
            }
        }
    }
    let mut out = if a.ops.is_empty() {
        Outcome::data(json!({ "frame": fid }))
    } else {
        let target = Target { sprite: a.target.sprite.clone(), layer: a.layer.clone(), frame: Some(FrameRef::Id(fid)) };
        let (changes, skipped) = run_batch(s, &target, &a.ops, None, BatchPolicy::AllOrNothing, ctx)?;
        changes.outcome(json!({ "frame": fid, "skipped": skipped }))
    };
    if structural {
        out.changes.push(Change { sprite: sprite_id, layer: None, frame: Some(fid), rect: None, kind: "structure".into() });
    }
    Ok(out)
}

/// Copies the pixels of a region from one frame into others.
pub(crate) fn region_copy(s: &mut Session, a: RegionCopy) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let region = sp.region(&a.region)?.clone();
    let from = sp.resolve_frame(&a.from_frame)?;
    let to: Vec<u32> = a.to_frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?;
    let layers: Vec<u32> = if !a.layers.is_empty() {
        a.layers.iter().map(|l| sp.layer(l).map(|x| x.id)).collect::<Result<_, _>>()?
    } else if let Some(l) = region.layer {
        vec![l]
    } else {
        sp.layers.iter().filter(|l| !l.reference).map(|l| l.id).collect()
    };
    let (w, h) = (sp.width, sp.height);
    let src_mask = region
        .shape(from)
        .ok_or_else(|| CommandError::invalid(format!("a região \"{}\" não tem forma no frame {from}", region.name)))?
        .to_mask(w, h);
    let bounds = src_mask.bounds().ok_or_else(|| CommandError::invalid("a região está vazia nesse frame"))?;
    let mut changes = ChangeSet::new(sp.id);
    for layer in layers {
        let src = layer_image(sprite(&s.doc, si), s.doc.images(), layer, from);
        // Masked copy of the region (transparent outside it).
        let mut part = Image::new(w, h);
        for p in src_mask.points() {
            part.set(p.x, p.y, src.get_or_clear(p.x, p.y));
        }
        let mut part = part.crop(bounds);
        if a.flip_h {
            part = part.flip_horizontal();
        }
        for &f in &to {
            let image = writable_cel(&mut s.doc, si, layer, f)?;
            let dest_mask = region.shape(f).map(|sh| sh.to_mask(w, h)).unwrap_or_else(|| src_mask.clone());
            let stats = {
                let img = s.doc.image_mut(image).ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
                let mut canvas = Canvas::new(img, None);
                if a.clear_destination {
                    for p in dest_mask.points() {
                        canvas.put(p.x, p.y, Color::TRANSPARENT);
                    }
                }
                let at = Point::new(bounds.x + a.offset.x, bounds.y + a.offset.y);
                for y in 0..part.height() as i32 {
                    for x in 0..part.width() as i32 {
                        let c = part.get_or_clear(x, y);
                        let (dx, dy) = (at.x + x, at.y + y);
                        let new = match a.mode {
                            PasteMode::Replace => c,
                            PasteMode::Over => c.over(canvas.get(dx, dy)),
                            PasteMode::SkipTransparent => {
                                if c.is_transparent() {
                                    continue;
                                }
                                c
                            }
                        };
                        canvas.put(dx, dy, new);
                    }
                }
                canvas.stats()
            };
            changes.add(&s.doc, si, image, &stats);
        }
    }
    Ok(changes.outcome(json!({ "region": region.name, "from": from, "to": to })))
}

pub(crate) fn region_transform(s: &mut Session, a: RegionTransform) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let region = sp.region(&a.region)?.clone();
    let frames: Vec<u32> = if a.frames.is_empty() {
        vec![frame_id(sp, None)?]
    } else {
        a.frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?
    };
    let layers: Vec<u32> = if !a.layers.is_empty() {
        a.layers.iter().map(|l| sp.layer(l).map(|x| x.id)).collect::<Result<_, _>>()?
    } else if let Some(l) = region.layer {
        vec![l]
    } else {
        sp.layers.iter().filter(|l| !l.reference).map(|l| l.id).collect()
    };
    let (w, h) = (sp.width, sp.height);
    let mut changes = ChangeSet::new(sp.id);
    for &f in &frames {
        let mask = region
            .shape(f)
            .ok_or_else(|| CommandError::invalid(format!("a região \"{}\" não tem forma no frame {f}", region.name)))?
            .to_mask(w, h);
        for &layer in &layers {
            apply_transform(&mut s.doc, si, layer, f, &mask, a.transform, &mut changes)?;
        }
        // A moved part keeps its region attached.
        if let Transform::Move { dx, dy, wrap: false } = a.transform {
            let moved = mask.translate(dx, dy);
            let (bounds, rows) = moved.to_rows();
            if let Some(b) = bounds {
                let reg = s.doc.model.sprites[si].regions.iter_mut().find(|r| r.id == region.id).expect("region exists");
                reg.shapes.insert(f, sprite_animation::RegionShape::Mask { at: Point::new(b.x, b.y), rows });
            }
        }
    }
    let mut out = changes.outcome(json!({ "region": region.name, "frames": frames }));
    out.changes.push(Change { sprite: sprite(&s.doc, si).id, layer: None, frame: None, rect: None, kind: "structure".into() });
    Ok(out)
}
