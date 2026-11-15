//! Drawing-tool commands: magic wand and lasso selections, reading only the selected pixels,
//! and the masked variants of `pixel_clear` and `draw_transform` used by the editor's selection
//! actions (delete, cut, move, flip, rotate).

use std::collections::BTreeMap;

use serde_json::json;
use sprite_animation::compose::layer_image;
use sprite_core::matrix::encode_region;
use sprite_core::{Color, Image, Mask, Point};
use sprite_drawing::sdc::{ColorRef, DrawOp};
use sprite_drawing::select::{self, SelectMode, SelectionMask};

use super::draw::{Cel, ChangeSet, apply_op, apply_transform};
use crate::args::*;
use crate::target::{check_point, palette_colors, region_mask, resolve, sprite, sprite_index};
use crate::{CommandError, Outcome, Session};

/// The JSON form of a selection mask: `bounds`, `rows` and how many pixels are selected.
fn mask_json(mask: &Mask) -> serde_json::Value {
    let text = SelectionMask::from_mask(mask);
    json!({ "bounds": text.bounds, "rows": text.rows, "count": mask.count() })
}

fn decode(mask: &SelectionMask, w: u32, h: u32) -> Result<Mask, CommandError> {
    Ok(mask.to_mask(w, h)?)
}

/// Combines a freshly computed selection with the optional base and answers with the result.
fn answer(found: &Mask, base: Option<&SelectionMask>, mode: SelectMode, extra: serde_json::Value) -> Result<Outcome, CommandError> {
    let base = base.map(|b| decode(b, found.width(), found.height())).transpose()?;
    let result = select::combine(base.as_ref(), found, mode);
    let mut data = mask_json(&result);
    if let (Some(obj), Some(more)) = (data.as_object_mut(), extra.as_object()) {
        obj.insert("mode".into(), json!(mode));
        obj.insert("found".into(), json!(found.count()));
        for (k, v) in more {
            obj.insert(k.clone(), v.clone());
        }
    }
    let mut out = Outcome::data(data);
    if result.is_empty() {
        out.warnings.push("a seleção ficou vazia".into());
    }
    Ok(out)
}

fn source_image(s: &mut Session, target: &Target, composite: bool) -> Result<(usize, u32, Image), CommandError> {
    let r = resolve(&s.doc, target)?;
    let sp = sprite(&s.doc, r.si);
    let img = if composite {
        let id = sp.id;
        s.composite(id, r.frame).ok_or_else(|| CommandError::not_found(format!("frame {} não existe", r.frame)))?
    } else {
        layer_image(sp, s.doc.images(), r.layer, r.frame)
    };
    Ok((r.si, r.frame, img))
}

pub(crate) fn select_color(s: &mut Session, a: SelectColor) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    check_point(sprite(&s.doc, r.si), a.x, a.y)?;
    let (_, frame, img) = source_image(s, &a.target, a.composite)?;
    let start = Point::new(a.x, a.y);
    let found = select::magic_wand(&img, start, a.contiguous, a.tolerance, a.connectivity);
    let picked = img.get_or_clear(a.x, a.y);
    answer(
        &found,
        a.base.as_ref(),
        a.mode,
        json!({ "picked": picked, "frame": frame, "layer": if a.composite { None } else { Some(r.layer) } }),
    )
}

pub(crate) fn select_polygon(s: &mut Session, a: SelectPolygon) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let found = select::lasso(sp.width, sp.height, &a.points)?;
    answer(&found, a.base.as_ref(), a.mode, json!({ "points": a.points.len() }))
}

/// The selected, visible pixels as a matrix `pixel_write_region` can paste: text (`rows` +
/// `legend`) when the colors fit one char each, numeric (`pixels` + `palette`) otherwise. In
/// both forms an unselected or transparent pixel keeps the destination pixel.
pub(crate) fn selection_read(s: &mut Session, a: SelectionRead) -> Result<Outcome, CommandError> {
    let (si, frame, img) = source_image(s, &a.target, a.composite)?;
    let (w, h) = (img.width(), img.height());
    let mut area = match &a.mask {
        Some(m) => decode(m, w, h)?,
        None => Mask::full(w, h),
    };
    if let Some(rect) = a.rect {
        area = area.intersect(&Mask::from_rect(w, h, rect));
    }
    let Some(bounds) = area.bounds() else {
        let mut out = Outcome::data(json!({ "rect": null, "rows": [], "legend": {}, "count": 0, "frame": frame }));
        out.warnings.push("a seleção está vazia".into());
        return Ok(out);
    };
    let mut picked = Image::new(w, h);
    let mut count = 0u32;
    for p in area.points() {
        let c = img.get_or_clear(p.x, p.y);
        if !c.is_transparent() {
            picked.set(p.x, p.y, c);
            count += 1;
        }
    }
    let palette = palette_colors(&s.doc, si);
    let mut data = match encode_region(&picked, bounds, &palette, false) {
        Some(m) => {
            // `.` stands for "transparent" in a read; here it must mean "keep" when pasted.
            let legend: BTreeMap<String, String> = m.legend.into_iter().filter(|(k, _)| k != ".").collect();
            json!({ "rect": m.rect, "legend": legend, "rows": m.rows })
        }
        None => {
            let mut colors: Vec<Color> = Vec::new();
            let pixels: Vec<Vec<Option<u32>>> = (bounds.y..bounds.bottom())
                .map(|y| {
                    (bounds.x..bounds.right())
                        .map(|x| {
                            let c = picked.get_or_clear(x, y);
                            if c.is_transparent() {
                                return None;
                            }
                            let i = colors.iter().position(|k| *k == c).unwrap_or_else(|| {
                                colors.push(c);
                                colors.len() - 1
                            });
                            Some(i as u32)
                        })
                        .collect()
                })
                .collect();
            json!({ "rect": bounds, "pixels": pixels, "palette": colors })
        }
    };
    if let Some(obj) = data.as_object_mut() {
        obj.insert("count".into(), json!(count));
        obj.insert("frame".into(), json!(frame));
        obj.insert("selection".into(), mask_json(&area));
    }
    Ok(Outcome::data(data))
}

/// `pixel_clear` limited to a selection mask.
pub(crate) fn pixel_clear_masked(s: &mut Session, a: PixelClear) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let (w, h, id) = (sp.width, sp.height, sp.id);
    let mask = match &a.mask {
        Some(m) => decode(m, w, h)?,
        None => Mask::full(w, h),
    };
    let op = match (a.at, a.rect) {
        (Some(p), _) => DrawOp::Pixel { x: p.x, y: p.y, color: ColorRef::Value("clear".into()) },
        (None, rect) => DrawOp::Clear { rect },
    };
    let palette = palette_colors(&s.doc, r.si);
    let mut changes = ChangeSet::new(id);
    apply_op(&mut s.doc, Cel { si: r.si, layer: r.layer, frame: r.frame }, &op, Some(&mask), &palette, &mut changes)?;
    Ok(changes.outcome(json!({ "layer": r.layer, "frame": r.frame })))
}

/// `draw_transform` limited to a selection mask; answers with the transformed selection too.
pub(crate) fn draw_transform_masked(s: &mut Session, a: DrawTransform) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let (w, h, id) = (sp.width, sp.height, sp.id);
    let mut area = match &a.mask {
        Some(m) => decode(m, w, h)?,
        None => Mask::full(w, h),
    };
    if let Some(reg) = &a.region {
        area = area.intersect(&region_mask(&s.doc, r.si, reg, r.frame)?);
    }
    if let Some(rect) = a.rect {
        area = area.intersect(&Mask::from_rect(w, h, rect));
    }
    let mut changes = ChangeSet::new(id);
    apply_transform(&mut s.doc, r.si, r.layer, r.frame, &area, a.transform, &mut changes)?;
    let moved = select::transform_mask(&area, a.transform)?;
    Ok(changes.outcome(json!({ "layer": r.layer, "frame": r.frame, "selection": mask_json(&moved) })))
}

#[cfg(test)]
#[path = "ferramentas_tests.rs"]
mod tests;
