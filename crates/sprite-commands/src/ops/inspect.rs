//! Read-only commands: project info, previews, comparisons, inspection, lists, validation.

use std::collections::BTreeMap;

use serde_json::json;
use sprite_animation::Sprite;
use sprite_animation::analysis::{ValidateOptions, enclosed_transparent, frame_metrics, validate as run_validate};
use sprite_animation::compose::{ComposeOptions, compose_frame, layer_image, onion_skin};
use sprite_core::diff::{diff_pixels, diff_stats, diff_visual};
use sprite_core::{Color, FrameRef, Image};
use sprite_project::export::{encode_gif, track_frames};

use crate::args::*;
use crate::render::{self, auto_scale, check_scale, mark_point, outline_mask, overlay_color, preview, side_by_side};
use crate::target::{frame_id, palette_colors, resolve, sprite, sprite_index};
use crate::{CommandError, ExecContext, Outcome, RenderedImage, Session};

fn sprite_summary(sp: &Sprite, full: bool) -> serde_json::Value {
    let animations: Vec<serde_json::Value> = sp
        .animations
        .iter()
        .map(|a| {
            json!({
                "id": a.id, "name": a.name, "loop_mode": a.loop_mode,
                "tracks": a.tracks.iter().map(|t| if full {
                    json!({ "direction": t.direction, "frames": t.frames })
                } else {
                    json!({ "direction": t.direction, "frames": t.frames.len() })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    let used_dirs: Vec<&str> = sp.directions.iter().filter(|d| d.custom).map(|d| d.name.as_str()).collect();
    let mut v = json!({
        "id": sp.id, "name": sp.name, "width": sp.width, "height": sp.height, "origin": sp.origin,
        "palette": sp.palette,
        "layers": sp.layers.iter().rev().map(|l| json!({ "id": l.id, "name": l.name, "visible": l.visible, "locked": l.locked, "opacity": l.opacity, "reference": l.reference })).collect::<Vec<_>>(),
        "frame_count": sp.frames.len(),
        "animations": animations,
        "custom_directions": used_dirs,
        "regions": sp.regions.iter().map(|r| json!({ "id": r.id, "name": r.name, "part": r.part })).collect::<Vec<_>>(),
        "anchors": sp.anchors.iter().map(|a| json!({ "id": a.id, "name": a.name, "kind": a.kind })).collect::<Vec<_>>(),
        "poses": sp.poses.len(),
        "extras": sp.extras,
    });
    if full {
        v["frames"] =
            json!(sp.frames.iter().map(|f| json!({ "id": f.id, "duration_ms": f.duration_ms, "tags": f.tags })).collect::<Vec<_>>());
        v["cels"] = json!(sp.cels.iter().map(|c| json!([c.layer, c.frame, c.image])).collect::<Vec<_>>());
        v["directions"] = serde_json::to_value(&sp.directions).unwrap_or_default();
    } else {
        v["frames"] = json!(sp.frames.iter().map(|f| f.id).collect::<Vec<_>>());
    }
    v
}

pub(crate) fn project_info(s: &mut Session, a: ProjectInfo) -> Result<Outcome, CommandError> {
    let m = &s.doc.model;
    let (undo, redo) = s.doc.history();
    let mut data = json!({
        "name": m.name,
        "uid": m.uid,
        "path": s.path.as_ref().map(|p| p.display().to_string()),
        "dirty": s.dirty(),
        "revision": s.doc.revision(),
        "sprites": m.sprites.iter().map(|sp| sprite_summary(sp, a.full)).collect::<Vec<_>>(),
        "palettes": m.palettes.iter().map(|p| json!({ "name": p.name, "colors": p.colors })).collect::<Vec<_>>(),
        "active_palette": m.active_palette,
        "references": m.references.iter().map(|r| json!({ "id": r.id, "name": r.name, "sprite": r.sprite, "visible": r.visible })).collect::<Vec<_>>(),
        "snapshots": s.doc.snapshots.iter().map(|x| json!({ "id": x.id, "name": x.name })).collect::<Vec<_>>(),
        "can_undo": undo.last().map(|x| x.1.clone()),
        "can_redo": redo.first().map(|x| x.1.clone()),
        "transaction": s.doc.transaction_id(),
    });
    if a.full {
        data["history"] =
            json!({ "undo": undo.iter().map(|x| &x.1).collect::<Vec<_>>(), "redo": redo.iter().map(|x| &x.1).collect::<Vec<_>>() });
        data["metadata"] = json!(m.metadata);
    }
    Ok(Outcome::data(data))
}

pub(crate) fn validate(s: &mut Session, a: ProjectValidate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let mut opts = ValidateOptions::for_sprite(sp);
    if a.palette_check {
        let p = palette_colors(&s.doc, si);
        if !p.is_empty() {
            opts.palette = Some(p);
        }
    }
    if let Some(j) = a.max_jump {
        opts.max_jump = j;
    }
    if let Some(v) = a.max_change {
        if !v.is_finite() || !(0.0..=2.0).contains(&v) {
            return Err(CommandError::new(crate::ErrorCode::InvalidArgument, "max_change deve estar entre 0 e 2"));
        }
        opts.max_change = v;
    }
    if let Some(v) = a.ground_tolerance {
        if v < 0 {
            return Err(CommandError::new(crate::ErrorCode::InvalidArgument, "ground_tolerance deve ser nao negativa"));
        }
        opts.ground_tolerance = v;
    }
    if !opts.max_jump.is_finite() || opts.max_jump < 0.0 {
        return Err(CommandError::new(crate::ErrorCode::InvalidArgument, "max_jump deve ser nao negativo"));
    }
    let issues = run_validate(sp, s.doc.images(), &opts);
    let warnings = issues.iter().filter(|i| i.severity == sprite_animation::analysis::Severity::Warning).count();
    Ok(Outcome::data(json!({
        "sprite": sp.name,
        "warnings": warnings,
        "infos": issues.len() - warnings,
        "issues": issues,
        "note": "verificações automáticas não garantem qualidade estética; inspecione as imagens",
    })))
}

pub(crate) fn snapshot_list(s: &mut Session) -> Result<Outcome, CommandError> {
    Ok(Outcome::data(
        json!({ "snapshots": s.doc.snapshots.iter().map(|x| json!({ "id": x.id, "name": x.name, "created": x.created })).collect::<Vec<_>>() }),
    ))
}

fn list_frame(sp: &Sprite, f: Option<&FrameRef>) -> Result<Option<u32>, CommandError> {
    f.map(|f| sp.resolve_frame(f)).transpose().map_err(Into::into)
}

pub(crate) fn region_list(s: &mut Session, a: ListArgs) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = list_frame(sp, a.frame.as_ref())?;
    let regions: Vec<serde_json::Value> = sp
        .regions
        .iter()
        .map(|r| {
            let shape = match frame {
                Some(f) => r.shape(f),
                None => r.default.as_ref(),
            };
            json!({
                "id": r.id, "name": r.name, "part": r.part, "layer": r.layer,
                "bounds": shape.and_then(|sh| sh.bounds(sp.width, sp.height)),
                "pixels": shape.map(|sh| sh.to_mask(sp.width, sp.height).count()),
                "frames_with_own_shape": r.shapes.keys().collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(Outcome::data(json!({ "frame": frame, "regions": regions })))
}

pub(crate) fn anchor_list(s: &mut Session, a: ListArgs) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = list_frame(sp, a.frame.as_ref())?;
    let anchors: Vec<serde_json::Value> = sp
        .anchors
        .iter()
        .map(|an| match frame {
            Some(f) => json!({ "id": an.id, "name": an.name, "kind": an.kind, "position": an.at(f) }),
            None => json!({ "id": an.id, "name": an.name, "kind": an.kind, "default": an.default, "positions": an.positions }),
        })
        .collect();
    Ok(Outcome::data(json!({ "frame": frame, "origin": sp.origin, "anchors": anchors })))
}

pub(crate) fn pose_list(s: &mut Session, a: ListArgs) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = list_frame(sp, a.frame.as_ref())?;
    let poses: Vec<&sprite_animation::Pose> = sp.poses.iter().filter(|p| frame.is_none() || p.frame == frame).collect();
    Ok(Outcome::data(json!({ "poses": poses })))
}

pub(crate) fn animation_info(s: &mut Session, a: AnimationInfo) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let Some(r) = &a.animation else {
        let list: Vec<serde_json::Value> = sp
            .animations
            .iter()
            .map(|an| {
                json!({
                    "id": an.id, "name": an.name, "loop_mode": an.loop_mode, "tags": an.tags,
                    "tracks": an.tracks.iter().map(|t| json!({
                        "direction": t.direction, "frames": t.frames.len(),
                        "total_ms": t.frames.iter().map(|f| sp.frame(*f).map(|x| x.duration_ms).unwrap_or(0)).sum::<u32>(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        return Ok(Outcome::data(json!({ "animations": list })));
    };
    let an = sp.animation(r)?;
    let opts = ComposeOptions::default();
    let tracks: Vec<serde_json::Value> = an
        .tracks
        .iter()
        .map(|t| {
            let mut prev_anchor: BTreeMap<String, sprite_core::Point> = BTreeMap::new();
            let frames: Vec<serde_json::Value> = t
                .frames
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let fr = sp.frame(*f);
                    let mut v =
                        json!({ "index": i, "frame": f, "duration_ms": fr.map(|x| x.duration_ms), "tags": fr.map(|x| x.tags.clone()) });
                    if a.metrics {
                        let img = compose_frame(sp, s.doc.images(), *f, &opts);
                        let m = frame_metrics(&img);
                        let anchors: BTreeMap<String, serde_json::Value> = sp
                            .anchors
                            .iter()
                            .filter_map(|anc| {
                                anc.at(*f).map(|p| {
                                    let moved = prev_anchor.get(&anc.name).map(|q| [p.x - q.x, p.y - q.y]);
                                    prev_anchor.insert(anc.name.clone(), p);
                                    (anc.name.clone(), json!({ "at": p, "moved": moved }))
                                })
                            })
                            .collect();
                        v["bounds"] = json!(m.bounds);
                        v["ground_y"] = json!(m.ground_y);
                        v["center"] = json!(m.center);
                        v["opaque_pixels"] = json!(m.opaque_pixels);
                        v["anchors"] = json!(anchors);
                    }
                    v
                })
                .collect();
            json!({ "direction": t.direction, "sequence": an.sequence(t), "frames": frames })
        })
        .collect();
    let poses: Vec<&sprite_animation::Pose> = sp.poses.iter().filter(|p| p.animation == Some(an.id)).collect();
    Ok(Outcome::data(json!({
        "id": an.id, "name": an.name, "loop_mode": an.loop_mode, "tags": an.tags, "cycle": an.cycle,
        "tracks": tracks, "poses": poses,
    })))
}

pub(crate) fn animation_preview(s: &mut Session, a: AnimationPreview, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frames = track_frames(sp, s.doc.images(), &a.animation, a.direction.as_deref())?;
    let an = sp.animation(&a.animation)?;
    let dir = a.direction.as_deref().map(|d| sp.direction_name(d)).transpose()?;
    let track = an.track(dir.as_deref()).expect("track_frames checked it");
    let max = ctx.limits.max_preview_side;
    let mut out = Outcome::default();
    match a.mode {
        PreviewMode::Gif => {
            let scale = check_scale(sp.width, sp.height, a.scale.unwrap_or_else(|| auto_scale(sp.width, sp.height, max).min(8)), max)?;
            let list: Vec<(Image, u32)> = frames.iter().map(|(_, img, ms)| (img.clone(), *ms)).collect();
            let data = encode_gif(&list, scale, an.loop_mode != sprite_animation::LoopMode::Once)?;
            out.images.push(RenderedImage {
                label: format!("{} gif", an.name),
                mime: "image/gif".into(),
                data,
                width: sp.width * scale,
                height: sp.height * scale,
            });
        }
        PreviewMode::Strip => {
            let n = track.frames.len().max(1) as u32;
            let fit = (max / (sp.width * n + 4 * n)).max(1);
            let scale =
                check_scale(sp.width * n, sp.height, a.scale.unwrap_or_else(|| auto_scale(sp.width, sp.height, max).min(fit).min(8)), max)?;
            let cells: Vec<Image> = track
                .frames
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let img = if a.onion {
                        onion_skin(sp, s.doc.images(), &track.frames, i, 1, 0)
                    } else {
                        compose_frame(sp, s.doc.images(), track.frames[i], &ComposeOptions::default())
                    };
                    preview(&img, scale, Background::Checker, false)
                })
                .collect();
            let strip = side_by_side(&cells, 4);
            out.images.push(render::png(format!("{} {}", an.name, dir.as_deref().unwrap_or("")), &strip)?);
        }
    }
    out.data = json!({
        "animation": an.name, "direction": dir, "loop_mode": an.loop_mode,
        "frames": track.frames, "durations": track.frames.iter().map(|f| sp.frame(*f).map(|x| x.duration_ms).unwrap_or(0)).collect::<Vec<_>>(),
        "playback_order": frames.iter().map(|f| f.0).collect::<Vec<_>>(),
    });
    Ok(out)
}

/// Encodes the change from `a` to `b` as a patch matrix ('.' = keep, '~' = clear).
fn encode_patch(a: &Image, b: &Image, palette: &[Color]) -> Option<serde_json::Value> {
    let changes = diff_pixels(a, b);
    let bounds = sprite_core::geom::DirtyRect::default();
    let mut bounds = bounds;
    changes.iter().for_each(|c| bounds.add(c.at));
    let rect = bounds.rect()?;
    let alphabet: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ#$%&*+-=?@^<>()[]{}/|!:;,_".chars().collect();
    let mut table: Vec<(Color, char)> =
        palette.iter().filter(|c| !c.is_transparent()).copied().take(alphabet.len()).zip(alphabet.iter().copied()).collect();
    let mut next = table.len();
    let mut legend: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    legend.insert(".".into(), serde_json::Value::Null);
    let changed: std::collections::BTreeSet<(i32, i32)> = changes.iter().map(|c| (c.at.x, c.at.y)).collect();
    let mut rows = Vec::new();
    for y in rect.y..rect.bottom() {
        let mut row = String::new();
        for x in rect.x..rect.right() {
            if !changed.contains(&(x, y)) {
                row.push('.');
                continue;
            }
            let c = b.get_or_clear(x, y);
            if c.is_transparent() {
                row.push('~');
                legend.insert("~".into(), json!("clear"));
                continue;
            }
            let ch = match table.iter().find(|(t, _)| *t == c) {
                Some((_, ch)) => *ch,
                None => {
                    let ch = *alphabet.get(next)?;
                    next += 1;
                    table.push((c, ch));
                    ch
                }
            };
            legend.insert(ch.to_string(), json!(c));
            row.push(ch);
        }
        rows.push(row);
    }
    Some(json!({ "at": [rect.x, rect.y], "legend": legend, "rows": rows }))
}

fn frame_image(s: &Session, si: usize, frame: u32, layer: Option<u32>) -> Image {
    let sp = sprite(&s.doc, si);
    match layer {
        Some(l) => layer_image(sp, s.doc.images(), l, frame),
        None => compose_frame(sp, s.doc.images(), frame, &ComposeOptions::default()),
    }
}

pub(crate) fn frame_compare(s: &mut Session, a: FrameCompare, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let fa = sp.resolve_frame(&a.a)?;
    let fb = sp.resolve_frame(&a.b)?;
    let layer = a.layer.as_ref().map(|l| sp.layer(l).map(|x| x.id)).transpose()?;
    let (ia, ib) = (frame_image(s, si, fa, layer), frame_image(s, si, fb, layer));
    let stats = diff_stats(&ia, &ib);
    let ma = frame_metrics(&ia);
    let mb = frame_metrics(&ib);
    let mut data = json!({
        "a": fa, "b": fb, "changed": stats.changed, "ratio": stats.ratio, "bounds": stats.bounds,
        "added": stats.added, "removed": stats.removed,
        "identical": stats.changed == 0,
        "a_bounds": ma.bounds, "b_bounds": mb.bounds,
        "center_shift": match (ma.center, mb.center) { (Some(p), Some(q)) => Some([q[0] - p[0], q[1] - p[1]]), _ => None },
    });
    let anchor_changes: Vec<_> = sp
        .anchors
        .iter()
        .filter_map(|anchor| {
            let (a, b) = (anchor.at(fa)?, anchor.at(fb)?);
            Some(json!({ "name": anchor.name, "a": a, "b": b, "delta": [b.x - a.x, b.y - a.y] }))
        })
        .collect();
    data["anchor_changes"] = json!(anchor_changes);
    if !sp.regions.is_empty() {
        let changes = diff_pixels(&ia, &ib);
        let regions: Vec<_> = sp
            .regions
            .iter()
            .filter_map(|region| {
                let before = region.shape(fa).map(|shape| shape.to_mask(sp.width, sp.height));
                let after = region.shape(fb).map(|shape| shape.to_mask(sp.width, sp.height));
                if before.is_none() && after.is_none() {
                    return None;
                }
                let count = changes
                    .iter()
                    .filter(|c| {
                        before.as_ref().is_some_and(|m| m.get(c.at.x, c.at.y)) || after.as_ref().is_some_and(|m| m.get(c.at.x, c.at.y))
                    })
                    .count();
                Some(json!({ "name": region.name, "changed": count }))
            })
            .collect();
        data["region_changes"] = json!(regions);
    }
    let mut out = Outcome::default();
    if matches!(a.output, CompareOutput::Full) {
        let palette = palette_colors(&s.doc, si);
        data["patch_a_to_b"] = encode_patch(&ia, &ib, &palette).unwrap_or(serde_json::Value::Null);
        let max = ctx.limits.max_preview_side;
        let scale = check_scale(sp.width * 3, sp.height, a.scale.unwrap_or_else(|| auto_scale(sp.width * 3, sp.height, max).max(1)), max)?;
        let strip = side_by_side(
            &[
                preview(&ia, scale, Background::Checker, false),
                preview(&ib, scale, Background::Checker, false),
                preview(&diff_visual(&ia, &ib), scale, Background::Black, false),
            ],
            6,
        );
        out.images
            .push(render::png(format!("frame {fa} | frame {fb} | diferenças (verde=novo, vermelho=apagado, amarelo=recolorido)"), &strip)?);
    }
    out.data = data;
    Ok(out)
}

pub(crate) fn frame_preview(s: &mut Session, a: FramePreview, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = frame_id(sp, a.frame.as_ref())?;
    let mut img = if let Some(onion) = &a.onion {
        // Find the frame's track to know its neighbors.
        let track = sp.animations.iter().flat_map(|an| an.tracks.iter()).find(|t| t.frames.contains(&frame));
        match track {
            Some(t) => {
                let pos = t.frames.iter().position(|f| *f == frame).expect("contains");
                onion_skin(sp, s.doc.images(), &t.frames, pos, onion.before, onion.after)
            }
            None => {
                let ids: Vec<u32> = sp.frames.iter().map(|f| f.id).collect();
                let pos = ids.iter().position(|f| *f == frame).expect("exists");
                onion_skin(sp, s.doc.images(), &ids, pos, onion.before, onion.after)
            }
        }
    } else if !a.layers.is_empty() {
        let only: Vec<u32> = a.layers.iter().map(|l| sp.layer(l).map(|x| x.id)).collect::<Result<_, _>>()?;
        compose_frame(sp, s.doc.images(), frame, &ComposeOptions { layers: Some(only), ..Default::default() })
    } else {
        compose_frame(sp, s.doc.images(), frame, &ComposeOptions::default())
    };
    let rect = a.rect.map(|r| r.intersect(&img.bounds())).unwrap_or(img.bounds());
    if rect.is_empty() {
        return Err(CommandError::new(crate::ErrorCode::OutOfBounds, "a região não cruza o sprite"));
    }
    if rect != img.bounds() {
        img = img.crop(rect);
    }
    let max = ctx.limits.max_preview_side;
    let scale = check_scale(img.width(), img.height(), a.scale.unwrap_or_else(|| auto_scale(img.width(), img.height(), max)), max)?;
    let mut out_img = preview(&img, scale, a.background, a.grid);
    let mut legend = Vec::new();
    if a.regions {
        for (i, r) in sp.regions.iter().enumerate() {
            if let Some(sh) = r.shape(frame) {
                let color = r.color.unwrap_or(overlay_color(i));
                let mask = sh.to_mask(sp.width, sp.height).translate(-rect.x, -rect.y);
                outline_mask(&mut out_img, &mask, scale, color);
                legend.push(json!({ "region": r.name, "color": color }));
            }
        }
    }
    if a.anchors {
        for (i, an) in sp.anchors.iter().enumerate() {
            if let Some(p) = an.at(frame) {
                let color = an.color.unwrap_or(overlay_color(i + 3));
                mark_point(&mut out_img, p.x - rect.x, p.y - rect.y, scale, color);
                legend.push(json!({ "anchor": an.name, "at": p, "color": color }));
            }
        }
    }
    if a.tiles {
        super::tiles::preview_overlay(sp, &mut out_img, rect, scale, &mut legend);
    }
    if a.hitboxes {
        super::godot::draw_hitboxes(&mut out_img, sp, frame, rect, scale, &mut legend);
    }
    if a.nine_slice {
        super::texto::nine_slice_overlay(sp, rect, scale, &mut out_img, &mut legend);
    }
    let m = frame_metrics(&img);
    let mut out = Outcome::default();
    out.images.push(render::png(format!("frame {frame} ({}x{}, escala {scale}x)", img.width(), img.height()), &out_img)?);
    if a.repeat {
        let (w, h) = (img.width() * 3, img.height() * 3);
        let repeat_scale = check_scale(w, h, auto_scale(w, h, max), max)?;
        let mut repeated = Image::new(w, h);
        for y in 0..3 {
            for x in 0..3 {
                repeated.paste(
                    &img,
                    sprite_core::Point::new((x * img.width()) as i32, (y * img.height()) as i32),
                    sprite_core::image::PasteMode::Replace,
                );
            }
        }
        out.images.push(render::png("repeticao 3x3", &preview(&repeated, repeat_scale, a.background, false))?);
    }
    out.data = json!({
        "frame": frame, "rect": rect, "scale": scale, "image_size": [out_img.width(), out_img.height()],
        "bounds": m.bounds, "opaque_pixels": m.opaque_pixels, "colors": m.color_count, "overlays": legend,
        "usage": sp.frame_usage(frame).iter().map(|(a, d, i)| json!({ "animation": a, "direction": d, "index": i })).collect::<Vec<_>>(),
    });
    Ok(out)
}

pub(crate) fn frame_inspect(s: &mut Session, a: FrameInspect) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let sp = sprite(&s.doc, r.si);
    let img = if a.layer_only {
        layer_image(sp, s.doc.images(), r.layer, r.frame)
    } else {
        compose_frame(sp, s.doc.images(), r.frame, &ComposeOptions::default())
    };
    let m = frame_metrics(&img);
    let palette = palette_colors(&s.doc, r.si);
    let mut colors: Vec<(Color, u32)> = img.color_counts().into_iter().collect();
    colors.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    // Bounded comparisons keep inspection useful for imported photographs with many colors.
    let mut near_colors = Vec::new();
    let near_distance = i32::from(a.near_color_distance.unwrap_or(12));
    let luminance = |c: Color| {
        let linear = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    };
    // Only opaque colors: contrast of translucent pixels depends on the background.
    let lumas: Vec<_> = colors.iter().filter(|(c, _)| c.a == 255).map(|(c, _)| luminance(*c)).collect();
    let luminance_range = lumas.iter().copied().reduce(f64::min).zip(lumas.iter().copied().reduce(f64::max));
    for (i, (a, _)) in colors.iter().take(64).enumerate() {
        for (b, _) in colors.iter().take(64).skip(i + 1) {
            let d = (a.r as i32 - b.r as i32).pow(2)
                + (a.g as i32 - b.g as i32).pow(2)
                + (a.b as i32 - b.b as i32).pow(2)
                + (a.a as i32 - b.a as i32).pow(2);
            if d > 0 && d <= near_distance * near_distance {
                near_colors.push(json!({ "a": a, "b": b, "distance_rgba": (d as f64).sqrt() }));
            }
        }
    }
    near_colors.truncate(32);
    let horizontal_seam =
        (0..img.height()).filter(|&y| img.get_or_clear(0, y as i32) != img.get_or_clear(img.width() as i32 - 1, y as i32)).count();
    let vertical_seam =
        (0..img.width()).filter(|&x| img.get_or_clear(x as i32, 0) != img.get_or_clear(x as i32, img.height() as i32 - 1)).count();
    let margins = m.bounds.map(
        |b| json!({"left": b.x, "top": b.y, "right": sp.width as i64 - b.right() as i64, "bottom": sp.height as i64 - b.bottom() as i64}),
    );
    let colors: Vec<serde_json::Value> =
        colors.iter().map(|(c, n)| json!({ "color": c, "pixels": n, "palette_index": palette.iter().position(|p| p == c) })).collect();
    let off_palette = if palette.is_empty() { 0 } else { colors.iter().filter(|c| c["palette_index"].is_null()).count() };
    let holes = enclosed_transparent(&img);
    let regions: Vec<serde_json::Value> = sp
        .regions
        .iter()
        .filter_map(|reg| reg.shape(r.frame).map(|sh| json!({ "name": reg.name, "bounds": sh.bounds(sp.width, sp.height) })))
        .collect();
    let anchors: Vec<serde_json::Value> =
        sp.anchors.iter().filter_map(|an| an.at(r.frame).map(|p| json!({ "name": an.name, "at": p }))).collect();
    let cels: Vec<serde_json::Value> = sp
        .cels
        .iter()
        .filter(|c| c.frame == r.frame)
        .map(|c| json!({ "layer": c.layer, "image": c.image, "linked_to": sp.cels.iter().filter(|x| x.image == c.image && x.frame != r.frame).map(|x| x.frame).collect::<Vec<_>>() }))
        .collect();
    Ok(Outcome::data(json!({
        "frame": r.frame,
        "layer": if a.layer_only { Some(r.layer) } else { None },
        "size": [sp.width, sp.height],
        "bounds": m.bounds, "opaque_pixels": m.opaque_pixels, "translucent_pixels": m.translucent_pixels,
        "ground_y": m.ground_y, "center": m.center,
        "holes": { "count": holes.count(), "bounds": holes.bounds() },
        "visual_review": {
            "visible_fraction": m.opaque_pixels as f64 / (sp.width as f64 * sp.height as f64),
            "margins": margins,
            "near_colors": near_colors,
            "near_color_distance": near_distance,
            "near_colors_sample_limit": 64,
            "opaque_luminance_range": luminance_range.map(|(min, max)| [min, max]),
            "tile_edge_mismatches": { "left_right": horizontal_seam, "top_bottom": vertical_seam },
            "note": "As bordas sao uma heuristica, nao uma prova de continuidade; inspecione a repeticao visualmente. Cores proximas usam distancia RGBA, nao perceptual."
        },
        "colors": colors, "off_palette_colors": off_palette,
        "anchors": anchors, "regions": regions, "cels": cels,
        "pivot": sp.pivot(r.frame),
    })))
}

#[cfg(test)]
mod review_tests {
    use super::*;

    #[test]
    fn patch_uses_distinct_symbols_with_transparent_palette_entry() {
        let a = Image::new(2, 1);
        let mut b = Image::new(2, 1);
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        b.set(0, 0, red);
        b.set(1, 0, blue);
        let patch = encode_patch(&a, &b, &[Color::TRANSPARENT, red]).unwrap();
        let row = patch["rows"][0].as_str().unwrap();
        let mut chars = row.chars();
        let first = chars.next().unwrap().to_string();
        let second = chars.next().unwrap().to_string();
        assert_ne!(first, second);
        assert_eq!(patch["legend"][first], json!(red));
        assert_eq!(patch["legend"][second], json!(blue));
    }
}
