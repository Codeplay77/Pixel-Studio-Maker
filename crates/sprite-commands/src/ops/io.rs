//! Palettes, import/export, references and snapshots.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_animation::sheet::plan;
use sprite_animation::{Animation, CycleInfo, Track};
use sprite_core::codec::encode_png;
use sprite_core::palette::median_cut;
use sprite_core::{Color, Image, Palette, PasteMode};
use sprite_drawing::Canvas;
use sprite_drawing::effects::quantize_to;
use sprite_project::Reference;
use sprite_project::export::{encode_gif, godot_sprite_frames, render_sheet, scale_image, sheet_items, sheet_metadata, track_frames};
use sprite_project::format::write_atomic;
use sprite_project::import::{decode_frames, slice_sheet};

use super::draw::ChangeSet;
use super::ensure_unique;
use super::structure::{append_frame, set_cel_image};
use crate::args::*;
use crate::target::{default_layer, frame_id, palette_colors, sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

fn structure_change(s: &Session, si: usize) -> Change {
    Change { sprite: s.doc.model.sprites[si].id, layer: None, frame: None, rect: None, kind: "structure".into() }
}

fn project_change(s: &Session) -> Vec<Change> {
    s.doc.model.sprites.iter().map(|sp| Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "structure".into() }).collect()
}

// ---- palettes -------------------------------------------------------------------------------

pub(crate) fn palette_create(s: &mut Session, a: PaletteCreate) -> Result<Outcome, CommandError> {
    ensure_unique("paleta", &a.name, s.doc.model.palettes.iter().map(|p| p.name.as_str()))?;
    if a.colors.is_empty() || a.colors.len() > 256 {
        return Err(CommandError::invalid("uma paleta tem de 1 a 256 cores"));
    }
    s.doc.model.palettes.push(Palette::new(a.name.clone(), a.colors.clone()));
    if a.activate {
        s.doc.model.active_palette = Some(a.name.clone());
    }
    Ok(Outcome {
        data: json!({ "palette": a.name, "colors": a.colors, "active": a.activate }),
        changes: project_change(s),
        ..Default::default()
    })
}

pub(crate) fn palette_update(s: &mut Session, a: PaletteUpdate) -> Result<Outcome, CommandError> {
    let name =
        a.palette.clone().or_else(|| s.doc.model.active_palette.clone()).ok_or_else(|| CommandError::not_found("nenhuma paleta ativa"))?;
    let index = s
        .doc
        .model
        .palettes
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case(&name))
        .ok_or_else(|| CommandError::not_found(format!("paleta \"{name}\" não existe")))?;
    if a.delete {
        let removed = s.doc.model.palettes.remove(index);
        if s.doc.model.active_palette.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&removed.name)) {
            s.doc.model.active_palette = s.doc.model.palettes.first().map(|p| p.name.clone());
        }
        for sp in &mut s.doc.model.sprites {
            if sp.palette.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&removed.name)) {
                sp.palette = None;
            }
        }
        return Ok(Outcome { data: json!({ "deleted": removed.name }), changes: project_change(s), ..Default::default() });
    }
    if let Some(n) = &a.rename {
        ensure_unique("paleta", n, s.doc.model.palettes.iter().enumerate().filter(|(i, _)| *i != index).map(|(_, p)| p.name.as_str()))?;
    }
    let old_name = s.doc.model.palettes[index].name.clone();
    {
        let p = &mut s.doc.model.palettes[index];
        if let Some(c) = a.colors {
            p.colors = c;
        }
        for c in a.add {
            if !p.colors.contains(&c) {
                p.colors.push(c);
            }
        }
        p.colors.retain(|c| !a.remove.contains(c));
        for set in a.set {
            if set.index >= p.colors.len() {
                return Err(CommandError::invalid(format!("índice {} fora da paleta ({} cores)", set.index, p.colors.len())));
            }
            p.colors[set.index] = set.color;
        }
        if p.colors.len() > 256 {
            return Err(CommandError::invalid("uma paleta tem no máximo 256 cores"));
        }
        if let Some(n) = &a.rename {
            p.name = n.clone();
        }
    }
    let new_name = s.doc.model.palettes[index].name.clone();
    if a.rename.is_some() {
        if s.doc.model.active_palette.as_deref() == Some(old_name.as_str()) {
            s.doc.model.active_palette = Some(new_name.clone());
        }
        for sp in &mut s.doc.model.sprites {
            if sp.palette.as_deref() == Some(old_name.as_str()) {
                sp.palette = Some(new_name.clone());
            }
        }
    }
    if a.activate {
        s.doc.model.active_palette = Some(new_name.clone());
    }
    let p = &s.doc.model.palettes[index];
    Ok(Outcome {
        data: json!({ "palette": p.name, "colors": p.colors, "active": s.doc.model.active_palette }),
        changes: project_change(s),
        ..Default::default()
    })
}

fn sprite_images(s: &Session, si: usize) -> Vec<u32> {
    let sp = sprite(&s.doc, si);
    let mut ids: Vec<u32> = sp.cels.iter().filter(|c| sp.layers.iter().any(|l| l.id == c.layer && !l.locked)).map(|c| c.image).collect();
    ids.sort();
    ids.dedup();
    ids
}

fn edit_images(s: &mut Session, si: usize, f: impl Fn(&mut Canvas)) -> ChangeSet {
    let mut changes = ChangeSet::new(sprite(&s.doc, si).id);
    for id in sprite_images(s, si) {
        let stats = {
            let img = s.doc.image_mut(id).expect("cel image");
            let mut canvas = Canvas::new(img, None);
            f(&mut canvas);
            canvas.stats()
        };
        changes.add(&s.doc, si, id, &stats);
    }
    changes
}

pub(crate) fn palette_replace(s: &mut Session, a: PaletteReplace) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let palette = palette_colors(&s.doc, si);
    let from = a.from.resolve(&palette)?;
    let to = a.to.resolve(&palette)?;
    let tol = a.tolerance;
    let changes = edit_images(s, si, |c| {
        let area = c.area(None);
        sprite_drawing::fill::replace_color(c, from, to, tol, &area);
    });
    let mut updated = false;
    if a.update_palette {
        let name = sprite(&s.doc, si).palette.clone().or_else(|| s.doc.model.active_palette.clone());
        if let Some(p) = name.and_then(|n| s.doc.model.palettes.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&n))) {
            for c in &mut p.colors {
                if *c == from {
                    *c = to;
                    updated = true;
                }
            }
        }
    }
    let mut out = changes.outcome(json!({ "from": from, "to": to, "palette_updated": updated }));
    out.changes.push(structure_change(s, si));
    Ok(out)
}

pub(crate) fn palette_quantize(s: &mut Session, a: PaletteQuantize) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let target_palette = match (&a.palette, a.max_colors) {
        (Some(name), _) => {
            s.doc.model.palette(Some(name)).cloned().ok_or_else(|| CommandError::not_found(format!("paleta \"{name}\" não existe")))?
        }
        (None, Some(n)) => {
            if n == 0 || n > 256 {
                return Err(CommandError::invalid("max_colors de 1 a 256"));
            }
            let mut counts: BTreeMap<Color, u32> = BTreeMap::new();
            for id in sprite_images(s, si) {
                for (c, k) in s.doc.image(id).expect("cel image").color_counts() {
                    *counts.entry(Color::rgb(c.r, c.g, c.b)).or_insert(0) += k;
                }
            }
            Palette::new(a.save_as.clone().unwrap_or_else(|| "quantizada".into()), median_cut(&counts, n as usize))
        }
        (None, None) => return Err(CommandError::invalid("informe `max_colors` ou `palette`")),
    };
    let pal = target_palette.clone();
    let changes = edit_images(s, si, |c| {
        let area = c.area(None);
        quantize_to(c, &area, &pal);
    });
    if let Some(name) = &a.save_as {
        if let Some(existing) = s.doc.model.palettes.iter_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
            existing.colors = target_palette.colors.clone();
        } else {
            s.doc.model.palettes.push(Palette::new(name.clone(), target_palette.colors.clone()));
        }
        s.doc.model.active_palette = Some(name.clone());
    }
    let mut out = changes.outcome(json!({ "colors": target_palette.colors, "saved_as": a.save_as }));
    out.changes.push(structure_change(s, si));
    Ok(out)
}

// ---- import ---------------------------------------------------------------------------------

fn read_file(path: &Path) -> Result<Vec<u8>, CommandError> {
    std::fs::read(path).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível ler {}: {e}", path.display())))
}

pub(crate) fn import_image(s: &mut Session, a: ImportImage, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let path = ctx.paths.resolve(&a.path)?;
    // Optional conversion to pixel art first (static pictures only).
    let mut converted: Option<(serde_json::Value, Vec<Color>, Vec<String>)> = None;
    let mut flattened = false;
    let mut frames: Vec<(Image, u32)> = match &a.pixelate {
        Some(opts) => {
            if a.slice.is_some() {
                return Err(CommandError::invalid(
                    "`pixelate` não combina com `slice`: converta a imagem inteira ou corte a sheet sem converter",
                ));
            }
            let opts = match a.mode {
                ImportMode::NewLayer | ImportMode::Frames => {
                    let sp = sprite(&s.doc, sprite_index(&s.doc, a.target.sprite.as_ref())?);
                    crate::picture::fit_options(opts, sp.width, sp.height)
                }
                _ => opts.clone(),
            };
            let (r, source, warnings) = crate::picture::convert(&path, &opts)?;
            converted = Some((crate::picture::summary(&r, &source), r.palette.clone(), warnings));
            vec![(r.image, 100)]
        }
        None => {
            let bytes = read_file(&path)?;
            if sprite_project::aseprite::is_aseprite(&bytes) || sprite_project::aseprite::has_aseprite_extension(&path) {
                // Aseprite: layers, frames, tags and palette as a new sprite; flattened otherwise.
                let file = sprite_project::aseprite::parse(&bytes)?;
                if matches!(a.mode, ImportMode::NewSprite) && a.slice.is_none() {
                    return super::paletas::import_aseprite_sprite(s, &a, file, &path, ctx);
                }
                flattened = true;
                file.composites().into_iter().map(|f| (f.image, f.duration_ms)).collect()
            } else {
                decode_frames(&bytes)?.into_iter().map(|f| (f.image, f.duration_ms)).collect()
            }
        }
    };
    if let Some(slice) = &a.slice {
        let first = frames.first().map(|f| f.0.clone()).ok_or_else(|| CommandError::invalid("imagem vazia"))?;
        frames = slice_sheet(&first, slice)?.into_iter().map(|i| (i, 100)).collect();
    }
    let (fw, fh) = (frames[0].0.width(), frames[0].0.height());
    let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "importado".into());
    let name = a.name.clone().unwrap_or(stem);
    let mut out = match a.mode {
        ImportMode::Reference => {
            let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
            let image = s.doc.insert_image(frames.remove(0).0);
            let id = s.doc.alloc_id();
            let sprite_id = sprite(&s.doc, si).id;
            s.doc.model.references.push(Reference {
                id,
                name: name.clone(),
                image,
                sprite: Some(sprite_id),
                position: a.at,
                scale: 1,
                opacity: 128,
                visible: true,
            });
            Outcome {
                data: json!({ "reference": id, "name": name, "size": [fw, fh] }),
                changes: vec![structure_change(s, si)],
                ..Default::default()
            }
        }
        ImportMode::NewSprite => {
            if fw > ctx.limits.max_canvas_side || fh > ctx.limits.max_canvas_side {
                return Err(CommandError::new(
                    ErrorCode::LimitExceeded,
                    format!("imagem {fw}x{fh} maior que o limite de {} px por lado", ctx.limits.max_canvas_side),
                ));
            }
            ensure_unique("sprite", &name, s.doc.model.sprites.iter().map(|x| x.name.as_str()))?;
            let si = super::structure::add_sprite(&mut s.doc, &name, fw, fh);
            let layer = sprite(&s.doc, si).layers[0].id;
            let first_frame = sprite(&s.doc, si).frames[0].id;
            let mut ids = Vec::new();
            for (i, (img, ms)) in frames.into_iter().enumerate() {
                let f = if i == 0 { first_frame } else { append_frame(&mut s.doc, si, ms) };
                if let Some(fr) = s.doc.model.sprites[si].frame_mut(f) {
                    fr.duration_ms = ms;
                }
                set_cel_image(&mut s.doc, si, layer, f, img)?;
                ids.push(f);
            }
            let anim = make_animation(s, si, a.animation.as_deref(), a.direction.as_deref(), &ids)?;
            let sp = sprite(&s.doc, si);
            Outcome {
                data: json!({ "sprite": sp.id, "name": sp.name, "size": [fw, fh], "layer": layer, "frames": ids, "animation": anim }),
                changes: vec![structure_change(s, si)],
                ..Default::default()
            }
        }
        ImportMode::NewLayer | ImportMode::Frames => {
            let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
            let (w, h) = (sprite(&s.doc, si).width, sprite(&s.doc, si).height);
            let layer = if matches!(a.mode, ImportMode::NewLayer) {
                ensure_unique("layer", &name, sprite(&s.doc, si).layers.iter().map(|l| l.name.as_str()))?;
                let id = s.doc.alloc_id();
                s.doc.model.sprites[si].layers.push(sprite_animation::Layer {
                    id,
                    name: name.clone(),
                    visible: true,
                    locked: false,
                    opacity: 255,
                    blend: Default::default(),
                    reference: false,
                });
                id
            } else {
                default_layer(sprite(&s.doc, si)).ok_or_else(|| CommandError::not_found("o sprite não tem layers"))?
            };
            let mut ids = Vec::new();
            let existing: Vec<u32> = sprite(&s.doc, si).frames.iter().map(|f| f.id).collect();
            for (i, (img, ms)) in frames.into_iter().enumerate() {
                let f = if matches!(a.mode, ImportMode::NewLayer) && i < existing.len() {
                    existing[i]
                } else {
                    append_frame(&mut s.doc, si, ms)
                };
                let mut canvas = Image::new(w, h);
                canvas.paste(&img, a.at, PasteMode::Replace);
                set_cel_image(&mut s.doc, si, layer, f, canvas)?;
                ids.push(f);
            }
            let anim = make_animation(s, si, a.animation.as_deref(), a.direction.as_deref(), &ids)?;
            let mut warnings = Vec::new();
            if fw != w || fh != h {
                warnings.push(format!("imagem {fw}x{fh} colocada num sprite {w}x{h} (recortada ou com sobra)"));
            }
            Outcome {
                data: json!({ "layer": layer, "frames": ids, "animation": anim }),
                changes: vec![structure_change(s, si)],
                warnings,
                ..Default::default()
            }
        }
    };
    if let Some((summary, colors, warnings)) = converted {
        out.warnings.extend(warnings);
        // Keep the colors of the conversion as a palette (not activated: indices in use stay valid).
        let base = format!("{name} (pixel art)");
        let mut palette = base.clone();
        let mut n = 2;
        while s.doc.model.palettes.iter().any(|p| p.name.eq_ignore_ascii_case(&palette)) {
            palette = format!("{base} {n}");
            n += 1;
        }
        if !colors.is_empty() {
            s.doc.model.palettes.push(Palette::new(palette.clone(), colors));
            out.data["palette"] = json!(palette);
        }
        out.data["pixelate"] = summary;
    }
    if flattened {
        out.warnings.push("arquivo do Aseprite importado achatado: a imagem composta de cada frame (layers só no modo new_sprite)".into());
    }
    Ok(out)
}

fn make_animation(
    s: &mut Session,
    si: usize,
    name: Option<&str>,
    direction: Option<&str>,
    frames: &[u32],
) -> Result<Option<u32>, CommandError> {
    let Some(name) = name else { return Ok(None) };
    let sp = sprite(&s.doc, si);
    let dir = direction.map(|d| sp.direction_name(d)).transpose()?;
    if let Some(ai) = sp.animations.iter().position(|an| an.name.eq_ignore_ascii_case(name)) {
        let anim = &mut s.doc.model.sprites[si].animations[ai];
        match anim.tracks.iter_mut().find(|t| t.direction == dir) {
            Some(t) => t.frames.extend_from_slice(frames),
            None => anim.tracks.push(Track { direction: dir, frames: frames.to_vec() }),
        }
        return Ok(Some(anim.id));
    }
    let id = s.doc.alloc_id();
    s.doc.model.sprites[si].animations.push(Animation {
        id,
        name: name.into(),
        loop_mode: Default::default(),
        tags: Vec::new(),
        tracks: vec![Track { direction: dir, frames: frames.to_vec() }],
        cycle: CycleInfo::default(),
    });
    Ok(Some(id))
}

// ---- export ---------------------------------------------------------------------------------

fn write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    write_atomic(path, bytes).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível gravar {}: {e}", path.display())))
}

fn check_export_scale(w: u32, h: u32, scale: u32) -> Result<u32, CommandError> {
    let s = scale.max(1);
    if s > 64 || w.saturating_mul(s) > 16_384 || h.saturating_mul(s) > 16_384 {
        return Err(CommandError::new(ErrorCode::LimitExceeded, "escala grande demais (máximo 16384 px por lado)"));
    }
    Ok(s)
}

pub(crate) fn export_png(s: &mut Session, a: ExportPng, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = frame_id(sp, a.frame.as_ref())?;
    let layers =
        if a.layers.is_empty() { None } else { Some(a.layers.iter().map(|l| sp.layer(l).map(|x| x.id)).collect::<Result<Vec<_>, _>>()?) };
    let img = compose_frame(sp, s.doc.images(), frame, &ComposeOptions { layers, ..Default::default() });
    let scale = check_export_scale(img.width(), img.height(), a.scale)?;
    let out = scale_image(&img, scale);
    let path = ctx.paths.resolve(&a.path)?;
    write(&path, &encode_png(&out).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?)?;
    Ok(Outcome::data(json!({ "path": path.display().to_string(), "size": [out.width(), out.height()], "frame": frame })))
}

fn with_extension(path: &Path, ext: &str) -> PathBuf {
    path.with_extension(ext)
}

pub(crate) fn export_spritesheet(s: &mut Session, a: ExportSpritesheet, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let items = sheet_items(sp, s.doc.images(), &a.selection)?;
    let p = plan(&items, sp.width, sp.height, &a.sheet);
    let scale = check_export_scale(p.width, p.height, a.scale)?;
    let img = render_sheet(sp, s.doc.images(), &p, scale, a.background);
    let path = ctx.paths.resolve(&a.path)?;
    write(&path, &encode_png(&img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?)?;
    let mut data = json!({ "path": path.display().to_string(), "size": [img.width(), img.height()], "frames": p.placements.len(), "cell": [p.cell_width * scale, p.cell_height * scale] });
    if a.json {
        let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let meta = sheet_metadata(sp, &p, &file, scale);
        let json_path = with_extension(&path, "json");
        write(&json_path, serde_json::to_string_pretty(&meta).unwrap_or_default().as_bytes())?;
        data["json"] = json!(json_path.display().to_string());
    }
    Ok(with_preview(Outcome::data(data), "sprite sheet", &img, ctx))
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if cleaned.is_empty() { "sprite".into() } else { cleaned }
}

pub(crate) fn export_frames(s: &mut Session, a: ExportFrames, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let items = sheet_items(sp, s.doc.images(), &a.selection)?;
    let dir = ctx.paths.resolve(&a.directory)?;
    std::fs::create_dir_all(&dir).map_err(|e| CommandError::new(ErrorCode::Io, e.to_string()))?;
    let prefix = sanitize(&a.prefix.clone().unwrap_or_else(|| sp.name.clone()));
    let scale = check_export_scale(sp.width, sp.height, a.scale)?;
    let mut files = Vec::new();
    for it in &items {
        let img = scale_image(&compose_frame(sp, s.doc.images(), it.frame, &ComposeOptions::default()), scale);
        let name = match (&it.animation, &it.direction) {
            (Some(an), Some(d)) => format!("{prefix}_{}_{}_{:02}.png", sanitize(an), sanitize(d), it.index),
            (Some(an), None) => format!("{prefix}_{}_{:02}.png", sanitize(an), it.index),
            _ => format!("{prefix}_{:03}.png", it.index),
        };
        let path = dir.join(&name);
        write(&path, &encode_png(&img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?)?;
        files.push(name);
    }
    Ok(Outcome::data(json!({ "directory": dir.display().to_string(), "files": files })))
}

pub(crate) fn export_gif(s: &mut Session, a: ExportGif, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frames = track_frames(sp, s.doc.images(), &a.animation, a.direction.as_deref())?;
    let scale = check_export_scale(sp.width, sp.height, a.scale)?;
    let looping = sp.animation(&a.animation)?.loop_mode != sprite_animation::LoopMode::Once;
    let list: Vec<(Image, u32)> = frames.into_iter().map(|(_, i, ms)| (i, ms)).collect();
    let bytes = encode_gif(&list, scale, looping)?;
    let path = ctx.paths.resolve(&a.path)?;
    write(&path, &bytes)?;
    Ok(Outcome::data(json!({ "path": path.display().to_string(), "frames": list.len(), "size": [sp.width * scale, sp.height * scale] })))
}

pub(crate) fn export_godot(s: &mut Session, a: ExportGodot, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let tres = ctx.paths.resolve(&a.path)?;
    let png_path = with_extension(&tres, "png");
    let png_name = png_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let res_path = a.res_path.clone().unwrap_or_else(|| format!("res://{png_name}"));
    let items = sheet_items(sp, s.doc.images(), &a.selection)?;
    let p = plan(&items, sp.width, sp.height, &a.sheet);
    let scale = check_export_scale(p.width, p.height, a.scale)?;
    let img = render_sheet(sp, s.doc.images(), &p, scale, None);
    write(&png_path, &encode_png(&img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?)?;
    let text = godot_sprite_frames(sp, &p, &res_path, scale);
    write(&tres, text.as_bytes())?;
    let meta = sheet_metadata(sp, &p, &png_name, scale);
    let json_path = with_extension(&tres, "json");
    write(&json_path, serde_json::to_string_pretty(&meta).unwrap_or_default().as_bytes())?;
    let animations: Vec<String> = sp
        .animations
        .iter()
        .flat_map(|an| {
            an.tracks.iter().map(move |t| match &t.direction {
                Some(d) => format!("{}_{d}", an.name),
                None => an.name.clone(),
            })
        })
        .collect();
    Ok(with_preview(
        Outcome::data(json!({
            "tres": tres.display().to_string(), "png": png_path.display().to_string(), "json": json_path.display().to_string(),
            "texture_res_path": res_path, "animations": animations, "sheet_size": [img.width(), img.height()],
        })),
        "sprite sheet do Godot",
        &img,
        ctx,
    ))
}

// ---- references & snapshots -----------------------------------------------------------------

pub(crate) fn reference_add(s: &mut Session, a: ReferenceAdd, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let path = ctx.paths.resolve(&a.path)?;
    let frames = decode_frames(&read_file(&path)?)?;
    let img = frames.into_iter().next().map(|f| f.image).ok_or_else(|| CommandError::invalid("imagem vazia"))?;
    let size = [img.width(), img.height()];
    let image = s.doc.insert_image(img);
    let id = s.doc.alloc_id();
    let name = a.name.unwrap_or_else(|| path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "referência".into()));
    let sprite_id = sprite(&s.doc, si).id;
    s.doc.model.references.push(Reference {
        id,
        name: name.clone(),
        image,
        sprite: Some(sprite_id),
        position: a.position,
        scale: a.scale.clamp(1, 32),
        opacity: a.opacity,
        visible: true,
    });
    Ok(Outcome {
        data: json!({ "reference": id, "name": name, "size": size }),
        changes: vec![structure_change(s, si)],
        ..Default::default()
    })
}

pub(crate) fn reference_update(s: &mut Session, a: ReferenceUpdate) -> Result<Outcome, CommandError> {
    let idx = s
        .doc
        .model
        .references
        .iter()
        .position(|r| a.reference.matches(r.id, &r.name))
        .ok_or_else(|| CommandError::not_found(format!("referência {} não existe", a.reference)))?;
    if a.delete {
        let r = s.doc.model.references.remove(idx);
        return Ok(Outcome { data: json!({ "deleted": r.id }), changes: project_change(s), ..Default::default() });
    }
    let r = &mut s.doc.model.references[idx];
    if let Some(p) = a.position {
        r.position = p;
    }
    if let Some(sc) = a.scale {
        r.scale = sc.clamp(1, 32);
    }
    if let Some(o) = a.opacity {
        r.opacity = o;
    }
    if let Some(v) = a.visible {
        r.visible = v;
    }
    let data = serde_json::to_value(&*r).unwrap_or_default();
    Ok(Outcome { data, changes: project_change(s), ..Default::default() })
}

pub(crate) fn snapshot_create(s: &mut Session, a: SnapshotCreate) -> Result<Outcome, CommandError> {
    if s.doc.snapshots.len() >= 64 {
        return Err(CommandError::new(ErrorCode::LimitExceeded, "máximo de 64 snapshots por projeto"));
    }
    let id = s.doc.create_snapshot(a.name.clone());
    // Snapshots live outside the undo history; touch metadata so the project counts as changed.
    s.doc.model.metadata.insert("last_snapshot".into(), json!(id));
    Ok(Outcome::data(json!({ "snapshot": id, "name": a.name })))
}

pub(crate) fn snapshot_restore(s: &mut Session, a: SnapshotRestore) -> Result<Outcome, CommandError> {
    s.doc.restore_snapshot(a.id)?;
    Ok(Outcome { data: json!({ "restored": a.id }), changes: project_change(s), ..Default::default() })
}

/// Returns the exported image to the caller too (when it fits the preview limit), so agents can
/// look at the result without opening the file.
fn with_preview(mut out: Outcome, label: &str, img: &Image, ctx: &ExecContext) -> Outcome {
    let max = ctx.limits.max_preview_side;
    if img.width() <= max
        && img.height() <= max
        && let Ok(png) = crate::render::png(label, img)
    {
        out.images.push(png);
    }
    out
}
