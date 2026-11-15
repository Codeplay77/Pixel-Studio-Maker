//! Structural commands: sprites, layers, frames, animations, directions, regions, anchors, poses.

use serde_json::json;
use sprite_animation::compose::layer_image;
use sprite_animation::{
    Anchor, Animation, Cel, CycleInfo, Direction, Frame, Layer, Pose, Region, RegionShape, Sprite, Track, standard_directions,
};
use sprite_core::compose::{LayerView, compose_onto};
use sprite_core::{Image, Mask, Point};
use sprite_project::Document;

use super::ensure_unique;
use crate::args::*;
use crate::target::{sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

fn structure_change(doc: &Document, si: usize) -> Change {
    Change { sprite: doc.model.sprites[si].id, layer: None, frame: None, rect: None, kind: "structure".into() }
}

fn structural(doc: &Document, si: usize, data: serde_json::Value) -> Outcome {
    Outcome { data, changes: vec![structure_change(doc, si)], ..Default::default() }
}

fn check_size(w: u32, h: u32, ctx: &ExecContext) -> Result<(), CommandError> {
    let max = ctx.limits.max_canvas_side;
    if w == 0 || h == 0 || w > max || h > max {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("tamanho {w}x{h} inválido: cada lado de 1 a {max} px (16 a 128 é o comum)"),
        ));
    }
    Ok(())
}

// ---- sprites --------------------------------------------------------------------------------

/// Adds a sprite to a document (also used by project_create in the runtime).
pub(crate) fn add_sprite(doc: &mut Document, name: &str, width: u32, height: u32) -> usize {
    let id = doc.alloc_id();
    let layer = doc.alloc_id();
    let frame = doc.alloc_id();
    let mut ids = [layer, frame].into_iter();
    let sprite = Sprite::new(id, name, width, height, &mut || ids.next().expect("two ids"));
    doc.model.sprites.push(sprite);
    doc.model.sprites.len() - 1
}

pub(crate) fn sprite_create(s: &mut Session, a: SpriteCreate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    check_size(a.width, a.height, ctx)?;
    ensure_unique("sprite", &a.name, s.doc.model.sprites.iter().map(|x| x.name.as_str()))?;
    let si = add_sprite(&mut s.doc, &a.name, a.width, a.height);
    let sp = sprite(&s.doc, si);
    Ok(structural(&s.doc, si, json!({ "sprite": sp.id, "name": sp.name, "layer": sp.layers[0].id, "frame": sp.frames[0].id })))
}

pub(crate) fn sprite_update(s: &mut Session, a: SpriteUpdate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    if let Some(name) = &a.name {
        ensure_unique("sprite", name, s.doc.model.sprites.iter().enumerate().filter(|(i, _)| *i != si).map(|(_, x)| x.name.as_str()))?;
        s.doc.model.sprites[si].name = name.clone();
    }
    if let Some(p) = &a.palette {
        if !s.doc.model.palettes.iter().any(|x| x.name.eq_ignore_ascii_case(p)) {
            return Err(CommandError::not_found(format!("paleta \"{p}\" não existe")));
        }
        s.doc.model.sprites[si].palette = Some(p.clone());
    }
    if let Some([w, h]) = a.size {
        check_size(w, h, ctx)?;
        let (ow, oh) = (s.doc.model.sprites[si].width, s.doc.model.sprites[si].height);
        let offset = match a.anchor {
            ResizeAnchor::TopLeft => Point::new(0, 0),
            ResizeAnchor::Center => Point::new((w as i32 - ow as i32) / 2, (h as i32 - oh as i32) / 2),
            ResizeAnchor::BottomCenter => Point::new((w as i32 - ow as i32) / 2, h as i32 - oh as i32),
        };
        let images: Vec<u32> = {
            let mut v: Vec<u32> = s.doc.model.sprites[si].cels.iter().map(|c| c.image).collect();
            v.sort();
            v.dedup();
            v
        };
        for id in images {
            let img = s.doc.image(id).cloned().ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
            let new = if a.scale_content { img.scale_nearest(w, h) } else { img.resize_canvas(w, h, offset) };
            s.doc.replace_image(id, new);
        }
        let sp = &mut s.doc.model.sprites[si];
        let map = |p: Point| -> Point {
            if a.scale_content {
                Point::new((p.x as i64 * w as i64 / ow as i64) as i32, (p.y as i64 * h as i64 / oh as i64) as i32)
            } else {
                p.offset(offset.x, offset.y)
            }
        };
        sp.origin = map(sp.origin);
        for an in &mut sp.anchors {
            an.default = an.default.map(map);
            for p in an.positions.values_mut() {
                *p = map(*p);
            }
        }
        if !a.scale_content {
            for r in &mut sp.regions {
                let shift = |shape: &mut RegionShape| match shape {
                    RegionShape::Rect { rect } => *rect = rect.translate(offset.x, offset.y),
                    RegionShape::Mask { at, .. } => *at = at.offset(offset.x, offset.y),
                    RegionShape::Polygon { points } => points.iter_mut().for_each(|p| *p = p.offset(offset.x, offset.y)),
                };
                if let Some(d) = &mut r.default {
                    shift(d);
                }
                r.shapes.values_mut().for_each(shift);
            }
        }
        sp.extras.godot.resize_canvas((ow, oh), (w, h), offset, a.scale_content);
        sp.width = w;
        sp.height = h;
    }
    if let Some(o) = a.origin {
        s.doc.model.sprites[si].origin = o;
    }
    let sp = sprite(&s.doc, si);
    let mut out = structural(
        &s.doc,
        si,
        json!({ "sprite": sp.id, "name": sp.name, "width": sp.width, "height": sp.height, "origin": sp.origin, "palette": sp.palette }),
    );
    if a.size.is_some() {
        out.changes.push(Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "pixels".into() });
    }
    Ok(out)
}

// ---- layers ---------------------------------------------------------------------------------

fn layer_json(l: &Layer) -> serde_json::Value {
    json!({ "id": l.id, "name": l.name, "visible": l.visible, "locked": l.locked, "opacity": l.opacity, "blend": l.blend, "reference": l.reference })
}

pub(crate) fn layer_create(s: &mut Session, a: LayerCreate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("layer", &a.name, sp.layers.iter().map(|l| l.name.as_str()))?;
    if sp.layers.len() >= ctx.limits.max_layers {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("máximo de {} layers", ctx.limits.max_layers)));
    }
    let index = match &a.above {
        Some(r) => sp.layer_index(sp.layer(r)?.id).expect("found") + 1,
        None => sp.layers.len(),
    };
    let id = s.doc.alloc_id();
    let layer = Layer { id, name: a.name, visible: true, locked: false, opacity: a.opacity, blend: a.blend, reference: a.reference };
    let data = layer_json(&layer);
    s.doc.model.sprites[si].layers.insert(index, layer);
    Ok(structural(&s.doc, si, json!({ "layer": data, "index": index })))
}

pub(crate) fn layer_delete(s: &mut Session, a: LayerRefArgs) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = &mut s.doc.model.sprites[si];
    let id = sp.layer(&a.layer)?.id;
    if sp.layers.len() == 1 {
        return Err(CommandError::invalid("o sprite precisa de pelo menos uma layer"));
    }
    sp.layers.retain(|l| l.id != id);
    sp.cels.retain(|c| c.layer != id);
    for r in &mut sp.regions {
        if r.layer == Some(id) {
            r.layer = None;
        }
    }
    Ok(structural(&s.doc, si, json!({ "deleted": id })))
}

pub(crate) fn layer_duplicate(s: &mut Session, a: LayerDuplicate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let src = sp.layer(&a.layer)?.clone();
    let name = a.name.unwrap_or_else(|| format!("{} cópia", src.name));
    ensure_unique("layer", &name, sp.layers.iter().map(|l| l.name.as_str()))?;
    if sp.layers.len() >= ctx.limits.max_layers {
        return Err(CommandError::new(ErrorCode::LimitExceeded, "limite de layers"));
    }
    let index = sp.layer_index(src.id).expect("exists") + 1;
    let cels: Vec<Cel> = sp.cels.iter().filter(|c| c.layer == src.id).cloned().collect();
    let id = s.doc.alloc_id();
    for c in cels {
        let img = s.doc.image(c.image).cloned().ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
        let image = s.doc.insert_image(img);
        s.doc.model.sprites[si].cels.push(Cel { layer: id, frame: c.frame, image, opacity: c.opacity });
    }
    let layer = Layer { id, name, ..src };
    let data = layer_json(&layer);
    s.doc.model.sprites[si].layers.insert(index, layer);
    Ok(structural(&s.doc, si, json!({ "layer": data, "index": index })))
}

pub(crate) fn layer_reorder(s: &mut Session, a: LayerReorder) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = &mut s.doc.model.sprites[si];
    let id = sp.layer(&a.layer)?.id;
    let from = sp.layer_index(id).expect("exists");
    let layer = sp.layers.remove(from);
    let to = if let Some(i) = a.index {
        i.min(sp.layers.len())
    } else if let Some(r) = &a.above {
        let other = sp.layer(r).map(|l| l.id);
        let other = other.inspect_err(|_| sp.layers.insert(from, layer.clone()))?;
        sp.layer_index(other).expect("exists") + 1
    } else if let Some(r) = &a.below {
        let other = sp.layer(r).map(|l| l.id);
        let other = other.inspect_err(|_| sp.layers.insert(from, layer.clone()))?;
        sp.layer_index(other).expect("exists")
    } else {
        sp.layers.insert(from, layer);
        return Err(CommandError::invalid("informe `index`, `above` ou `below`"));
    };
    sp.layers.insert(to, layer);
    let order: Vec<serde_json::Value> = sp.layers.iter().map(|l| json!({ "id": l.id, "name": l.name })).collect();
    Ok(structural(&s.doc, si, json!({ "layer": id, "index": to, "order_bottom_to_top": order })))
}

pub(crate) fn layer_merge(s: &mut Session, a: LayerMerge) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let top = sp.layer(&a.layer)?.clone();
    let into = match &a.into {
        Some(r) => sp.layer(r)?.clone(),
        None => {
            let i = sp.layer_index(top.id).expect("exists");
            if i == 0 {
                return Err(CommandError::invalid("não há layer abaixo; informe `into`"));
            }
            sp.layers[i - 1].clone()
        }
    };
    if into.id == top.id {
        return Err(CommandError::invalid("não dá para fundir uma layer nela mesma"));
    }
    if into.locked {
        return Err(CommandError::new(ErrorCode::Locked, format!("a layer \"{}\" está bloqueada", into.name)));
    }
    let frames: Vec<u32> = sp.cels.iter().filter(|c| c.layer == top.id).map(|c| c.frame).collect();
    for f in frames {
        let sp = sprite(&s.doc, si);
        let cel = sp.cel(top.id, f).cloned().expect("listed");
        let src = layer_image(sp, s.doc.images(), top.id, f);
        let opacity = ((top.opacity as u32 * cel.opacity as u32 + 127) / 255) as u8;
        let dest = crate::target::writable_cel(&mut s.doc, si, into.id, f)?;
        let img = s.doc.image_mut(dest).expect("cel image");
        compose_onto(img, LayerView { image: &src, opacity, blend: top.blend, offset: Point::default() });
    }
    let sp = &mut s.doc.model.sprites[si];
    sp.layers.retain(|l| l.id != top.id);
    sp.cels.retain(|c| c.layer != top.id);
    let mut out = structural(&s.doc, si, json!({ "merged": top.id, "into": into.id }));
    out.changes.push(Change { sprite: s.doc.model.sprites[si].id, layer: Some(into.id), frame: None, rect: None, kind: "pixels".into() });
    Ok(out)
}

pub(crate) fn layer_properties(s: &mut Session, a: LayerProperties) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = &s.doc.model.sprites[si];
    let id = sp.layer(&a.layer)?.id;
    if let Some(n) = &a.name {
        ensure_unique("layer", n, sp.layers.iter().filter(|l| l.id != id).map(|l| l.name.as_str()))?;
    }
    let l = s.doc.model.sprites[si].layer_mut(id).expect("exists");
    if let Some(v) = a.name {
        l.name = v;
    }
    if let Some(v) = a.visible {
        l.visible = v;
    }
    if let Some(v) = a.locked {
        l.locked = v;
    }
    if let Some(v) = a.opacity {
        l.opacity = v;
    }
    if let Some(v) = a.blend {
        l.blend = v;
    }
    if let Some(v) = a.reference {
        l.reference = v;
    }
    let data = layer_json(l);
    Ok(structural(&s.doc, si, json!({ "layer": data })))
}

// ---- frames ---------------------------------------------------------------------------------

/// Appends a new empty frame to the timeline after `after` (or at the end).
fn new_frame(doc: &mut Document, si: usize, duration_ms: u32, after: Option<u32>) -> u32 {
    let id = doc.alloc_id();
    let sp = &mut doc.model.sprites[si];
    let pos = after.and_then(|a| sp.frame_index(a)).map(|i| i + 1).unwrap_or(sp.frames.len());
    sp.frames.insert(pos, Frame { id, duration_ms, tags: Vec::new() });
    id
}

/// Copies every cel of `from` into `to` (copy, mirrored copy, or linked).
fn copy_frame_content(doc: &mut Document, si: usize, from: u32, to: u32, mirror: bool, link: bool) -> Result<(), CommandError> {
    let cels: Vec<Cel> = doc.model.sprites[si].cels.iter().filter(|c| c.frame == from).cloned().collect();
    for c in cels {
        let image = if link && !mirror {
            c.image
        } else {
            let mut img = doc.image(c.image).cloned().ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
            if mirror {
                img = img.flip_horizontal();
            }
            doc.insert_image(img)
        };
        doc.model.sprites[si].cels.retain(|x| !(x.layer == c.layer && x.frame == to));
        doc.model.sprites[si].cels.push(Cel { layer: c.layer, frame: to, image, opacity: c.opacity });
    }
    // Anchors and region shapes follow the copy (mirrored when needed).
    let w = doc.model.sprites[si].width as i32;
    let sp = &mut doc.model.sprites[si];
    for an in &mut sp.anchors {
        if let Some(p) = an.positions.get(&from).copied() {
            an.positions.insert(to, if mirror { Point::new(w - 1 - p.x, p.y) } else { p });
        }
    }
    for r in &mut sp.regions {
        if let Some(shape) = r.shapes.get(&from).cloned() {
            let shape = if mirror {
                let mask = shape.to_mask(sp.width, sp.height);
                let mut flipped = Mask::new(sp.width, sp.height);
                for p in mask.points() {
                    flipped.set(w - 1 - p.x, p.y, true);
                }
                let (b, rows) = flipped.to_rows();
                b.map(|b| RegionShape::Mask { at: Point::new(b.x, b.y), rows }).unwrap_or(shape)
            } else {
                shape
            };
            r.shapes.insert(to, shape);
        }
    }
    let width = sp.width;
    sp.extras.godot.copy_frame(from, to, mirror.then_some(width));
    Ok(())
}

fn check_frames_limit(doc: &Document, si: usize, adding: usize, ctx: &ExecContext) -> Result<(), CommandError> {
    if doc.model.sprites[si].frames.len() + adding > ctx.limits.max_frames {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("máximo de {} frames por sprite", ctx.limits.max_frames)));
    }
    Ok(())
}

fn direction_arg(sp: &Sprite, d: Option<&str>) -> Result<Option<String>, CommandError> {
    d.map(|d| sp.direction_name(d)).transpose().map_err(Into::into)
}

pub(crate) fn frame_create(s: &mut Session, a: FrameCreate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    if a.count == 0 || a.count > 512 {
        return Err(CommandError::invalid("count de 1 a 512"));
    }
    if a.duration_ms == 0 || a.duration_ms > 60_000 {
        return Err(CommandError::invalid("duração de 1 a 60000 ms"));
    }
    check_frames_limit(&s.doc, si, a.count as usize, ctx)?;
    let sp = sprite(&s.doc, si);
    let copy_from = a.copy_from.as_ref().map(|f| sp.resolve_frame(f)).transpose()?;
    let track = match &a.animation {
        Some(anim) => {
            let ai = sp.animation_index(anim)?;
            let dir = direction_arg(sp, a.direction.as_deref())?;
            if sp.animations[ai].track(dir.as_deref()).is_none() {
                return Err(CommandError::not_found(format!(
                    "a animação \"{}\" não tem trilha {}",
                    sp.animations[ai].name,
                    dir.as_deref().unwrap_or("sem direção")
                )));
            }
            Some((ai, dir))
        }
        None => None,
    };
    // Insert in the timeline right after the track's last frame (keeps tracks grouped).
    let mut after = track.as_ref().and_then(|(ai, dir)| sp.animations[*ai].track(dir.as_deref()).and_then(|t| t.frames.last().copied()));
    let mut created = Vec::new();
    for _ in 0..a.count {
        let id = new_frame(&mut s.doc, si, a.duration_ms, after);
        if let Some(src) = copy_from {
            copy_frame_content(&mut s.doc, si, src, id, false, false)?;
        }
        after = Some(id);
        created.push(id);
    }
    if let Some((ai, dir)) = track {
        let t = s.doc.model.sprites[si].animations[ai].track_mut(dir.as_deref()).expect("checked");
        let at = a.index.unwrap_or(t.frames.len()).min(t.frames.len());
        for (k, id) in created.iter().enumerate() {
            t.frames.insert(at + k, *id);
        }
    }
    Ok(structural(&s.doc, si, json!({ "frames": created })))
}

pub(crate) fn frame_duplicate(s: &mut Session, a: FrameDuplicate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    if a.count == 0 || a.count > 256 {
        return Err(CommandError::invalid("count de 1 a 256"));
    }
    check_frames_limit(&s.doc, si, a.count as usize, ctx)?;
    let sp = sprite(&s.doc, si);
    let src = sp.resolve_frame(&a.frame)?;
    let duration = sp.frame(src).map(|f| f.duration_ms).unwrap_or(100);
    let mut after = src;
    let mut created = Vec::new();
    for _ in 0..a.count {
        let id = new_frame(&mut s.doc, si, duration, Some(after));
        copy_frame_content(&mut s.doc, si, src, id, a.mirror, a.link)?;
        after = id;
        created.push(id);
    }
    // In every track holding the source, the copies follow it.
    for anim in &mut s.doc.model.sprites[si].animations {
        for t in &mut anim.tracks {
            if let Some(pos) = t.frames.iter().position(|f| *f == src) {
                for (k, id) in created.iter().enumerate() {
                    t.frames.insert(pos + 1 + k, *id);
                }
            }
        }
    }
    Ok(structural(&s.doc, si, json!({ "source": src, "frames": created, "linked": a.link && !a.mirror })))
}

pub(crate) fn frame_delete(s: &mut Session, a: FrameDelete) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let ids: Vec<u32> = a.frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?;
    remove_frames(&mut s.doc, si, &ids)?;
    Ok(structural(&s.doc, si, json!({ "deleted": ids })))
}

fn remove_frames(doc: &mut Document, si: usize, ids: &[u32]) -> Result<(), CommandError> {
    let sp = &mut doc.model.sprites[si];
    if sp.frames.iter().all(|f| ids.contains(&f.id)) {
        return Err(CommandError::invalid("o sprite precisa de pelo menos um frame"));
    }
    sp.frames.retain(|f| !ids.contains(&f.id));
    sp.cels.retain(|c| !ids.contains(&c.frame));
    for an in &mut sp.animations {
        for t in &mut an.tracks {
            t.frames.retain(|f| !ids.contains(f));
        }
        an.cycle.key_frames.retain(|f| !ids.contains(f));
        an.cycle.contact_frames.retain(|f| !ids.contains(f));
    }
    for an in &mut sp.anchors {
        an.positions.retain(|f, _| !ids.contains(f));
    }
    for r in &mut sp.regions {
        r.shapes.retain(|f, _| !ids.contains(f));
    }
    sp.extras.godot.remove_frames(ids);
    for p in &mut sp.poses {
        if p.frame.is_some_and(|f| ids.contains(&f)) {
            p.frame = None;
        }
    }
    Ok(())
}

pub(crate) fn frame_move(s: &mut Session, a: FrameMove) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let id = sp.resolve_frame(&a.frame)?;
    match &a.animation {
        Some(anim) => {
            let ai = sp.animation_index(anim)?;
            let dir = direction_arg(sp, a.direction.as_deref())?;
            let t = s.doc.model.sprites[si].animations[ai]
                .track_mut(dir.as_deref())
                .ok_or_else(|| CommandError::not_found("trilha não existe"))?;
            let from = t
                .frames
                .iter()
                .position(|f| *f == id)
                .ok_or_else(|| CommandError::not_found(format!("o frame {id} não está nessa trilha")))?;
            let f = t.frames.remove(from);
            let to = a.index.min(t.frames.len());
            t.frames.insert(to, f);
            let order = t.frames.clone();
            Ok(structural(&s.doc, si, json!({ "frame": id, "index": to, "track": order })))
        }
        None => {
            let sp = &mut s.doc.model.sprites[si];
            let from = sp.frame_index(id).expect("resolved");
            let f = sp.frames.remove(from);
            let to = a.index.min(sp.frames.len());
            sp.frames.insert(to, f);
            Ok(structural(&s.doc, si, json!({ "frame": id, "index": to })))
        }
    }
}

// ---- animations -----------------------------------------------------------------------------

fn expand_directions(sp: &Sprite, list: &[String]) -> Result<Vec<Option<String>>, CommandError> {
    if list.is_empty() {
        return Ok(vec![None]);
    }
    let mut out = Vec::new();
    for d in list {
        if d.eq_ignore_ascii_case("all") || d == "8" {
            for std in standard_directions() {
                if !out.contains(&Some(std.name.clone())) {
                    out.push(Some(std.name));
                }
            }
        } else {
            let n = sp.direction_name(d)?;
            if !out.contains(&Some(n.clone())) {
                out.push(Some(n));
            }
        }
    }
    Ok(out)
}

pub(crate) fn animation_create(s: &mut Session, a: AnimationCreate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("animação", &a.name, sp.animations.iter().map(|x| x.name.as_str()))?;
    if a.frames == 0 || a.frames > 256 {
        return Err(CommandError::invalid("frames por trilha de 1 a 256"));
    }
    let dirs = expand_directions(sp, &a.directions)?;
    let copy_from = a.copy_from.as_ref().map(|f| sp.resolve_frame(f)).transpose()?;
    let existing: Vec<u32> = a.use_frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?;
    let to_create = if existing.is_empty() { dirs.len() * a.frames as usize } else { (dirs.len() - 1) * a.frames as usize };
    check_frames_limit(&s.doc, si, to_create, ctx)?;
    let mut tracks = Vec::new();
    for (i, dir) in dirs.iter().enumerate() {
        let frames = if i == 0 && !existing.is_empty() {
            existing.clone()
        } else {
            let mut ids = Vec::new();
            for _ in 0..a.frames {
                let id = new_frame(&mut s.doc, si, a.duration_ms, None);
                if let Some(src) = copy_from {
                    copy_frame_content(&mut s.doc, si, src, id, false, false)?;
                }
                ids.push(id);
            }
            ids
        };
        tracks.push(Track { direction: dir.clone(), frames });
    }
    let id = s.doc.alloc_id();
    let anim = Animation { id, name: a.name, loop_mode: a.loop_mode, tags: a.tags, tracks, cycle: CycleInfo::default() };
    let data = json!({
        "animation": anim.id, "name": anim.name, "loop_mode": anim.loop_mode,
        "tracks": anim.tracks.iter().map(|t| json!({ "direction": t.direction, "frames": t.frames })).collect::<Vec<_>>(),
    });
    s.doc.model.sprites[si].animations.push(anim);
    Ok(structural(&s.doc, si, data))
}

pub(crate) fn animation_delete(s: &mut Session, a: AnimationDelete) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let ai = sp.animation_index(&a.animation)?;
    let anim = sp.animations[ai].clone();
    s.doc.model.sprites[si].animations.remove(ai);
    let mut deleted = Vec::new();
    if a.delete_frames {
        let sp = sprite(&s.doc, si);
        let still_used: std::collections::BTreeSet<u32> =
            sp.animations.iter().flat_map(|x| x.tracks.iter().flat_map(|t| t.frames.iter().copied())).collect();
        let mut orphan: Vec<u32> = anim.tracks.iter().flat_map(|t| t.frames.iter().copied()).filter(|f| !still_used.contains(f)).collect();
        orphan.sort();
        orphan.dedup();
        if orphan.len() == sp.frames.len() {
            // Keep one empty frame so the sprite stays valid.
            let keep = new_frame(&mut s.doc, si, 100, None);
            let _ = keep;
        }
        remove_frames(&mut s.doc, si, &orphan)?;
        deleted = orphan;
    }
    Ok(structural(&s.doc, si, json!({ "deleted": anim.name, "deleted_frames": deleted })))
}

pub(crate) fn animation_update(s: &mut Session, a: AnimationUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let ai = sp.animation_index(&a.animation)?;
    if let Some(n) = &a.name {
        ensure_unique("animação", n, sp.animations.iter().enumerate().filter(|(i, _)| *i != ai).map(|(_, x)| x.name.as_str()))?;
    }
    if let Some(c) = &a.cycle {
        for f in c.key_frames.iter().chain(c.contact_frames.iter()) {
            if sp.frame(*f).is_none() {
                return Err(CommandError::not_found(format!("frame {f} não existe")));
            }
        }
    }
    let anim = &mut s.doc.model.sprites[si].animations[ai];
    if let Some(n) = a.name {
        anim.name = n;
    }
    if let Some(t) = a.tags {
        anim.tags = t;
    }
    if let Some(c) = a.cycle {
        anim.cycle = c;
    }
    let data = json!({ "animation": anim.id, "name": anim.name, "tags": anim.tags, "cycle": anim.cycle });
    Ok(structural(&s.doc, si, data))
}

pub(crate) fn animation_set_direction(s: &mut Session, a: AnimationSetDirection, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let ai = sp.animation_index(&a.animation)?;
    let dir = sp.direction_name(&a.direction)?;
    if let Some(r) = &a.reference {
        let rn = sp.direction_name(r)?;
        let d = s.doc.model.sprites[si].directions.iter_mut().find(|d| d.name == dir).expect("exists");
        d.reference = Some(rn);
    }
    let sp = sprite(&s.doc, si);
    if a.remove {
        let anim = &mut s.doc.model.sprites[si].animations[ai];
        let before = anim.tracks.len();
        anim.tracks.retain(|t| t.direction.as_deref() != Some(dir.as_str()));
        if anim.tracks.len() == before {
            return Err(CommandError::not_found(format!("a animação não tem a direção {dir}")));
        }
        return Ok(structural(&s.doc, si, json!({ "removed": dir })));
    }
    let frames: Vec<u32> = if !a.frames.is_empty() {
        a.frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?
    } else if let Some(n) = a.create {
        if n == 0 || n > 256 {
            return Err(CommandError::invalid("create de 1 a 256"));
        }
        check_frames_limit(&s.doc, si, n as usize, ctx)?;
        let duration = sp.animations[ai]
            .tracks
            .first()
            .and_then(|t| t.frames.first())
            .and_then(|f| sp.frame(*f))
            .map(|f| f.duration_ms)
            .unwrap_or(100);
        (0..n).map(|_| new_frame(&mut s.doc, si, duration, None)).collect()
    } else if let Some(copy) = &a.copy_from {
        let src_dir = sp.direction_name(&copy.direction)?;
        let src = sp.animations[ai]
            .track(Some(&src_dir))
            .ok_or_else(|| CommandError::not_found(format!("a animação não tem a direção {src_dir}")))?
            .frames
            .clone();
        check_frames_limit(&s.doc, si, src.len(), ctx)?;
        let mut out = Vec::new();
        for f in src {
            let duration = sprite(&s.doc, si).frame(f).map(|x| x.duration_ms).unwrap_or(100);
            let id = new_frame(&mut s.doc, si, duration, None);
            copy_frame_content(&mut s.doc, si, f, id, copy.mirror, copy.link)?;
            out.push(id);
        }
        out
    } else if a.reference.is_some() {
        return Ok(structural(&s.doc, si, json!({ "direction": dir, "reference": a.reference })));
    } else {
        return Err(CommandError::invalid("informe `frames`, `create`, `copy_from`, `reference` ou `remove`"));
    };
    let anim = &mut s.doc.model.sprites[si].animations[ai];
    match anim.tracks.iter_mut().find(|t| t.direction.as_deref() == Some(dir.as_str())) {
        Some(t) => t.frames = frames.clone(),
        None => anim.tracks.push(Track { direction: Some(dir.clone()), frames: frames.clone() }),
    }
    let anim_name = anim.name.clone();
    Ok(structural(&s.doc, si, json!({ "animation": anim_name, "direction": dir, "frames": frames })))
}

pub(crate) fn animation_set_timing(s: &mut Session, a: AnimationSetTiming) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let ai = sp.animation_index(&a.animation)?;
    let dir = a.direction.as_deref().map(|d| sp.direction_name(d)).transpose()?;
    let tracks: Vec<Vec<u32>> =
        sp.animations[ai].tracks.iter().filter(|t| dir.is_none() || t.direction == dir).map(|t| t.frames.clone()).collect();
    if dir.is_some() && tracks.is_empty() {
        return Err(CommandError::not_found("trilha não existe"));
    }
    let uniform = match (a.duration_ms, a.fps) {
        (Some(ms), _) => Some(ms),
        (None, Some(fps)) if fps > 0.0 => Some((1000.0 / fps).round() as u32),
        (None, Some(_)) => return Err(CommandError::invalid("fps precisa ser maior que zero")),
        _ => None,
    };
    for d in uniform.iter().chain(a.durations.iter()) {
        if *d == 0 || *d > 60_000 {
            return Err(CommandError::invalid("duração de 1 a 60000 ms"));
        }
    }
    let sp = &mut s.doc.model.sprites[si];
    for frames in &tracks {
        if !a.durations.is_empty() && a.durations.len() != frames.len() {
            return Err(CommandError::invalid(format!(
                "`durations` tem {} valores, a trilha tem {} frames",
                a.durations.len(),
                frames.len()
            )));
        }
        for (i, f) in frames.iter().enumerate() {
            let fr = sp.frame_mut(*f).expect("track frame exists");
            if let Some(ms) = uniform {
                fr.duration_ms = ms;
            }
            if !a.durations.is_empty() {
                fr.duration_ms = a.durations[i];
            }
        }
    }
    if let Some(m) = a.loop_mode {
        sp.animations[ai].loop_mode = m;
    }
    let anim = &sp.animations[ai];
    let summary: Vec<serde_json::Value> = anim
        .tracks
        .iter()
        .map(|t| json!({ "direction": t.direction, "durations": t.frames.iter().map(|f| sp.frame(*f).map(|x| x.duration_ms).unwrap_or(0)).collect::<Vec<_>>() }))
        .collect();
    let data = json!({ "animation": anim.name, "loop_mode": anim.loop_mode, "tracks": summary });
    Ok(structural(&s.doc, si, data))
}

pub(crate) fn direction_create(s: &mut Session, a: DirectionCreate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("direção", &a.name, sp.directions.iter().map(|d| d.name.as_str()))?;
    let mirror_of = a.mirror_of.as_deref().map(|d| sp.direction_name(d)).transpose()?;
    let reference = a.reference.as_deref().map(|d| sp.direction_name(d)).transpose()?;
    let d = Direction { name: a.name.clone(), label: a.label.unwrap_or(a.name), angle: a.angle, mirror_of, reference, custom: true };
    let data = serde_json::to_value(&d).unwrap_or_default();
    s.doc.model.sprites[si].directions.push(d);
    Ok(structural(&s.doc, si, json!({ "direction": data })))
}

pub(crate) fn direction_update(s: &mut Session, a: DirectionUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let name = sp.direction_name(&a.direction)?;
    let opt = |v: &Option<String>| -> Result<Option<Option<String>>, CommandError> {
        match v.as_deref() {
            None => Ok(None),
            Some("") => Ok(Some(None)),
            Some(d) => Ok(Some(Some(sp.direction_name(d)?))),
        }
    };
    let mirror = opt(&a.mirror_of)?;
    let reference = opt(&a.reference)?;
    if a.delete {
        let d = sp.direction(&name).expect("exists");
        if !d.custom {
            return Err(CommandError::invalid("só direções personalizadas podem ser apagadas"));
        }
        if sp.animations.iter().any(|an| an.tracks.iter().any(|t| t.direction.as_deref() == Some(name.as_str()))) {
            return Err(CommandError::invalid("a direção ainda tem trilhas; remova-as antes"));
        }
        s.doc.model.sprites[si].directions.retain(|d| d.name != name);
        return Ok(structural(&s.doc, si, json!({ "deleted": name })));
    }
    let d = s.doc.model.sprites[si].directions.iter_mut().find(|d| d.name == name).expect("exists");
    if let Some(l) = a.label {
        d.label = l;
    }
    if let Some(an) = a.angle {
        d.angle = Some(an);
    }
    if let Some(m) = mirror {
        d.mirror_of = m;
    }
    if let Some(r) = reference {
        d.reference = r;
    }
    let data = serde_json::to_value(&*d).unwrap_or_default();
    Ok(structural(&s.doc, si, json!({ "direction": data })))
}

// ---- regions, anchors, poses ---------------------------------------------------------------

fn opaque_shape(doc: &Document, si: usize, layer: u32, frame: u32) -> Result<RegionShape, CommandError> {
    let img = layer_image(sprite(doc, si), doc.images(), layer, frame);
    let mask = Mask::from_opaque(&img);
    let (b, rows) = mask.to_rows();
    let b = b.ok_or_else(|| CommandError::invalid("a layer está vazia nesse frame; não há pixels para formar a região"))?;
    Ok(RegionShape::Mask { at: Point::new(b.x, b.y), rows })
}

pub(crate) fn region_create(s: &mut Session, a: RegionCreate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("região", &a.name, sp.regions.iter().map(|r| r.name.as_str()))?;
    let layer = a.layer.as_ref().map(|l| sp.layer(l).map(|x| x.id)).transpose()?;
    let shape = match (a.shape, &a.from_frame) {
        (Some(sh), _) => sh,
        (None, Some(f)) => {
            let layer = layer.ok_or_else(|| CommandError::invalid("`from_frame` precisa de `layer`"))?;
            let fid = sp.resolve_frame(f)?;
            opaque_shape(&s.doc, si, layer, fid)?
        }
        (None, None) => return Err(CommandError::invalid("informe `shape` ou `layer` + `from_frame`")),
    };
    let (w, h) = (sp.width, sp.height);
    let id = s.doc.alloc_id();
    let r = Region { id, name: a.name, part: a.part, layer, default: Some(shape), shapes: Default::default(), color: a.color };
    let bounds = r.default.as_ref().and_then(|sh| sh.bounds(w, h));
    let data = json!({ "region": id, "name": r.name, "bounds": bounds });
    s.doc.model.sprites[si].regions.push(r);
    Ok(structural(&s.doc, si, data))
}

pub(crate) fn region_update(s: &mut Session, a: RegionUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let id = sp.region(&a.region)?.id;
    if a.delete {
        s.doc.model.sprites[si].regions.retain(|r| r.id != id);
        return Ok(structural(&s.doc, si, json!({ "deleted": id })));
    }
    if let Some(n) = &a.name {
        ensure_unique("região", n, sp.regions.iter().filter(|r| r.id != id).map(|r| r.name.as_str()))?;
    }
    let layer = a.layer.as_ref().map(|l| sp.layer(l).map(|x| x.id)).transpose()?;
    let shapes: Vec<(u32, RegionShape)> =
        a.frame_shapes.iter().map(|fs| sp.resolve_frame(&fs.frame).map(|f| (f, fs.shape.clone()))).collect::<Result<_, _>>()?;
    let clear: Vec<u32> = a.clear_frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?;
    let r = s.doc.model.sprites[si].regions.iter_mut().find(|r| r.id == id).expect("exists");
    if let Some(n) = a.name {
        r.name = n;
    }
    if let Some(p) = a.part {
        r.part = Some(p);
    }
    if layer.is_some() {
        r.layer = layer;
    }
    if let Some(sh) = a.shape {
        r.default = Some(sh);
    }
    for (f, sh) in shapes {
        r.shapes.insert(f, sh);
    }
    for f in clear {
        r.shapes.remove(&f);
    }
    let data = serde_json::to_value(&*r).unwrap_or_default();
    Ok(structural(&s.doc, si, json!({ "region": data })))
}

fn frame_points(sp: &Sprite, list: &[FramePoint]) -> Result<Vec<(u32, Point)>, CommandError> {
    list.iter().map(|fp| sp.resolve_frame(&fp.frame).map(|f| (f, fp.position)).map_err(Into::into)).collect()
}

pub(crate) fn anchor_create(s: &mut Session, a: AnchorCreate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("anchor", &a.name, sp.anchors.iter().map(|x| x.name.as_str()))?;
    if a.position.is_none() && a.positions.is_empty() {
        return Err(CommandError::invalid("informe `position` e/ou `positions`"));
    }
    let positions = frame_points(sp, &a.positions)?;
    let id = s.doc.alloc_id();
    let anchor = Anchor { id, name: a.name, kind: a.kind, default: a.position, positions: positions.into_iter().collect(), color: a.color };
    let data = serde_json::to_value(&anchor).unwrap_or_default();
    s.doc.model.sprites[si].anchors.push(anchor);
    Ok(structural(&s.doc, si, json!({ "anchor": data })))
}

pub(crate) fn anchor_update(s: &mut Session, a: AnchorUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let id = sp.anchor(&a.anchor)?.id;
    if a.delete {
        s.doc.model.sprites[si].anchors.retain(|x| x.id != id);
        return Ok(structural(&s.doc, si, json!({ "deleted": id })));
    }
    if let Some(n) = &a.name {
        ensure_unique("anchor", n, sp.anchors.iter().filter(|x| x.id != id).map(|x| x.name.as_str()))?;
    }
    let positions = frame_points(sp, &a.positions)?;
    let clear: Vec<u32> = a.clear_frames.iter().map(|f| sp.resolve_frame(f)).collect::<Result<_, _>>()?;
    let an = s.doc.model.sprites[si].anchors.iter_mut().find(|x| x.id == id).expect("exists");
    if let Some(n) = a.name {
        an.name = n;
    }
    if let Some(k) = a.kind {
        an.kind = k;
    }
    if let Some(p) = a.position {
        an.default = Some(p);
    }
    for (f, p) in positions {
        an.positions.insert(f, p);
    }
    for f in clear {
        an.positions.remove(&f);
    }
    let data = serde_json::to_value(&*an).unwrap_or_default();
    Ok(structural(&s.doc, si, json!({ "anchor": data })))
}

pub(crate) fn pose_create(s: &mut Session, a: PoseCreate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    ensure_unique("pose", &a.name, sp.poses.iter().map(|x| x.name.as_str()))?;
    let animation = a.animation.as_ref().map(|r| sp.animation(r).map(|x| x.id)).transpose()?;
    let direction = a.direction.as_deref().map(|d| sp.direction_name(d)).transpose()?;
    let frame = a.frame.as_ref().map(|f| sp.resolve_frame(f)).transpose()?;
    let mut anchors = a.anchors.clone();
    if anchors.is_empty()
        && a.capture_anchors
        && let Some(f) = frame
    {
        for an in &sp.anchors {
            if let Some(p) = an.at(f) {
                anchors.insert(an.name.clone(), p);
            }
        }
    }
    let id = s.doc.alloc_id();
    let pose = Pose { id, name: a.name, kind: a.kind, animation, direction, frame, anchors, ground_y: a.ground_y, notes: a.notes };
    let data = serde_json::to_value(&pose).unwrap_or_default();
    s.doc.model.sprites[si].poses.push(pose);
    Ok(structural(&s.doc, si, json!({ "pose": data })))
}

pub(crate) fn pose_update(s: &mut Session, a: PoseUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let id = sp
        .poses
        .iter()
        .find(|p| a.pose.matches(p.id, &p.name))
        .map(|p| p.id)
        .ok_or_else(|| CommandError::not_found(format!("pose {} não existe", a.pose)))?;
    if a.delete {
        s.doc.model.sprites[si].poses.retain(|p| p.id != id);
        return Ok(structural(&s.doc, si, json!({ "deleted": id })));
    }
    if let Some(n) = &a.name {
        ensure_unique("pose", n, sp.poses.iter().filter(|p| p.id != id).map(|p| p.name.as_str()))?;
    }
    let frame = a.frame.as_ref().map(|f| sp.resolve_frame(f)).transpose()?;
    let p = s.doc.model.sprites[si].poses.iter_mut().find(|p| p.id == id).expect("exists");
    if let Some(n) = a.name {
        p.name = n;
    }
    if let Some(k) = a.kind {
        p.kind = k;
    }
    if frame.is_some() {
        p.frame = frame;
    }
    for (k, v) in a.anchors {
        p.anchors.insert(k, v);
    }
    if a.ground_y.is_some() {
        p.ground_y = a.ground_y;
    }
    if let Some(n) = a.notes {
        p.notes = n;
    }
    let data = serde_json::to_value(&*p).unwrap_or_default();
    Ok(structural(&s.doc, si, json!({ "pose": data })))
}

/// Used by import: puts an image into a (layer, frame) cel, replacing what was there.
pub(crate) fn set_cel_image(doc: &mut Document, si: usize, layer: u32, frame: u32, img: Image) -> Result<(), CommandError> {
    let id = crate::target::writable_cel(doc, si, layer, frame)?;
    doc.replace_image(id, img);
    Ok(())
}

pub(crate) fn append_frame(doc: &mut Document, si: usize, duration: u32) -> u32 {
    new_frame(doc, si, duration, None)
}
