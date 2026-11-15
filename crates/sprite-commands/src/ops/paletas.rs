//! Palette tools: palette files, palette variants (recolored previews and exports, never touching
//! the project's pixels), the layered import of Aseprite files and normal maps.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_animation::sheet::{SheetPlan, plan};
use sprite_animation::{Animation, Cel, CycleInfo, ImageStore, Layer, Region, RegionShape, Sprite, Track};
use sprite_core::codec::encode_png;
use sprite_core::{Color, Image, Palette, PasteMode, Point};
use sprite_drawing::normal_map::{NormalMapOptions, normal_map};
use sprite_project::aseprite::AseFile;
use sprite_project::export::{encode_gif, scale_image, sheet_items, sheet_metadata, track_frames};
use sprite_project::format::write_atomic;
use sprite_project::palette_file::{PaletteFileError, PaletteFormat, SwatchLayout, read_palette, swatch_image, write_palette};
use sprite_project::variants::{PaletteVariant, Recolor, RecolorStats};

use super::ensure_unique;
use super::structure::{add_sprite, append_frame};
use crate::args::*;
use crate::render::{auto_scale, check_scale, preview};
use crate::target::{frame_id, sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

#[cfg(test)]
mod tests;

// ---- helpers --------------------------------------------------------------------------------

fn project_change(s: &Session) -> Vec<Change> {
    s.doc.model.sprites.iter().map(|sp| Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "structure".into() }).collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.display().to_string())
}

fn file_error(path: &Path, e: PaletteFileError) -> CommandError {
    let err = CommandError::invalid(format!("{}: {e}", file_name(path)));
    match e {
        PaletteFileError::Line { line, .. } => err.with_details(json!({ "line": line })),
        PaletteFileError::Invalid(_) => err,
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, CommandError> {
    std::fs::read(path).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível ler {}: {e}", path.display())))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    write_atomic(path, bytes).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível gravar {}: {e}", path.display())))
}

fn png_bytes(img: &Image) -> Result<Vec<u8>, CommandError> {
    encode_png(img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))
}

fn check_export_scale(w: u32, h: u32, scale: u32) -> Result<u32, CommandError> {
    let s = scale.max(1);
    if s > 64 || w.saturating_mul(s) > 16_384 || h.saturating_mul(s) > 16_384 {
        return Err(CommandError::new(ErrorCode::LimitExceeded, "escala grande demais (máximo 16384 px por lado)"));
    }
    Ok(s)
}

/// Returns the image to the caller too, when it fits the preview limit.
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

fn sanitize(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if cleaned.is_empty() { "paleta".into() } else { cleaned }
}

/// `base`, or `base 2`, `base 3`... whichever is free (case-insensitive).
fn unique_name<'a>(base: &str, existing: impl Iterator<Item = &'a str> + Clone) -> String {
    let base = if base.trim().is_empty() { "sem nome" } else { base.trim() };
    let taken = |n: &str| existing.clone().any(|e| e.eq_ignore_ascii_case(n));
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).expect("a free name")
}

/// The palette a sprite uses: its own, else the project's active one.
fn sprite_palette_name(s: &Session, si: Option<usize>) -> Option<String> {
    si.and_then(|i| s.doc.model.sprites.get(i)).and_then(|sp| sp.palette.clone()).or_else(|| s.doc.model.active_palette.clone())
}

fn find_palette<'a>(s: &'a Session, name: &str) -> Result<&'a Palette, CommandError> {
    s.doc
        .model
        .palettes
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| CommandError::not_found(format!("paleta \"{name}\" não existe")))
}

fn find_variant<'a>(s: &'a Session, name: &str) -> Result<&'a PaletteVariant, CommandError> {
    s.doc.model.extras.paletas.variant(name).ok_or_else(|| {
        let known: Vec<&str> = s.doc.model.extras.paletas.variants.iter().map(|v| v.name.as_str()).collect();
        CommandError::not_found(format!(
            "variante de paleta \"{name}\" não existe{}",
            if known.is_empty() { " (o projeto não tem variantes)".to_string() } else { format!(" (existem: {})", known.join(", ")) }
        ))
    })
}

/// The color swap of a saved variant, plus notes when its base palette changed size.
fn variant_recolor(s: &Session, name: &str) -> Result<(PaletteVariant, Recolor, Vec<String>), CommandError> {
    let v = find_variant(s, name)?.clone();
    let base = find_palette(s, &v.base).map_err(|_| {
        CommandError::not_found(format!(
            "a paleta base \"{}\" da variante \"{}\" não existe mais; mude a base com palette_variant_set",
            v.base, v.name
        ))
    })?;
    let mut notes = Vec::new();
    if base.colors.len() != v.colors.len() {
        notes.push(format!(
            "a paleta \"{}\" tem {} cores e a variante \"{}\" tem {}: só as {} primeiras foram trocadas",
            base.name,
            base.colors.len(),
            v.name,
            v.colors.len(),
            base.colors.len().min(v.colors.len())
        ));
    }
    let r = Recolor::new(&base.colors, &v.colors);
    Ok((v, r, notes))
}

fn unmatched_json(stats: &RecolorStats) -> serde_json::Value {
    let mut list: Vec<(&Color, &u64)> = stats.unmatched.iter().collect();
    list.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    json!(list.iter().take(16).map(|(c, n)| json!({ "color": c, "pixels": n })).collect::<Vec<_>>())
}

/// Like `sprite_project::export::render_sheet`, with every composed frame passed through `map`
/// first (once per frame), so the background is never touched.
fn render_sheet_mapped(
    sp: &Sprite,
    images: &ImageStore,
    plan: &SheetPlan,
    scale: u32,
    background: Option<Color>,
    map: &mut dyn FnMut(Image) -> Image,
) -> Image {
    let s = scale.max(1);
    let mut sheet = Image::filled(plan.width * s, plan.height * s, background.unwrap_or(Color::TRANSPARENT));
    let mut cache: BTreeMap<u32, Image> = BTreeMap::new();
    for p in &plan.placements {
        let frame = cache.entry(p.frame).or_insert_with(|| map(compose_frame(sp, images, p.frame, &ComposeOptions::default())));
        let part = scale_image(&frame.crop(p.source), s);
        sheet.paste(&part, Point::new(p.dest.x * s as i32, p.dest.y * s as i32), PasteMode::Over);
    }
    sheet
}

// ---- palette files --------------------------------------------------------------------------

pub(crate) fn palette_import(s: &mut Session, a: PaletteImport, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let path = ctx.paths.resolve(&a.path)?;
    let format = PaletteFormat::from_path(&path).map_err(|e| file_error(&path, e))?;
    let file = read_palette(format, &read_file(&path)?).map_err(|e| file_error(&path, e))?;
    let explicit = a.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let existing = explicit.and_then(|n| s.doc.model.palettes.iter().position(|p| p.name.eq_ignore_ascii_case(n)));
    let (name, replaced) = match (explicit, existing) {
        (Some(_), Some(i)) if a.replace => {
            s.doc.model.palettes[i].colors = file.colors.clone();
            (s.doc.model.palettes[i].name.clone(), true)
        }
        (Some(n), Some(_)) => {
            return Err(CommandError::invalid(format!(
                "já existe uma paleta chamada \"{n}\"; use replace: true para trocar as cores dela"
            )));
        }
        (Some(n), None) => {
            s.doc.model.palettes.push(Palette::new(n, file.colors.clone()));
            (n.to_string(), false)
        }
        (None, _) => {
            let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            let base = file.name.clone().unwrap_or(stem);
            let name = unique_name(&base, s.doc.model.palettes.iter().map(|p| p.name.as_str()));
            s.doc.model.palettes.push(Palette::new(name.clone(), file.colors.clone()));
            (name, false)
        }
    };
    if a.activate {
        s.doc.model.active_palette = Some(name.clone());
    }
    Ok(Outcome {
        data: json!({
            "palette": name, "format": format, "count": file.colors.len(), "colors": file.colors,
            "replaced": replaced, "active": s.doc.model.active_palette,
        }),
        warnings: file.warnings,
        changes: project_change(s),
        ..Default::default()
    })
}

pub(crate) fn palette_export(s: &mut Session, a: PaletteExport, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let path = ctx.paths.resolve(&a.path)?;
    let format = PaletteFormat::from_path(&path).map_err(|e| file_error(&path, e))?;
    if a.swatch == 0 || a.swatch > 256 {
        return Err(CommandError::invalid("swatch de 1 a 256 px"));
    }
    let (name, colors, kind) = match &a.variant {
        Some(v) => {
            let v = find_variant(s, v)?;
            (v.name.clone(), v.colors.clone(), "variant")
        }
        None => {
            let name = a
                .palette
                .clone()
                .or_else(|| s.doc.model.active_palette.clone())
                .ok_or_else(|| CommandError::not_found("nenhuma paleta ativa; informe `palette`"))?;
            let p = find_palette(s, &name)?;
            (p.name.clone(), p.colors.clone(), "palette")
        }
    };
    let layout = SwatchLayout { swatch: a.swatch, columns: a.columns };
    let (bytes, notes) = write_palette(format, &name, &colors, layout).map_err(|e| file_error(&path, e))?;
    write(&path, &bytes)?;
    let mut data = json!({ "path": path.display().to_string(), "format": format, "count": colors.len() });
    data[kind] = json!(name);
    let out = Outcome { data, warnings: notes, ..Default::default() };
    if format.is_image() {
        let img = swatch_image(&colors, layout).map_err(|e| file_error(&path, e))?;
        return Ok(with_preview(out, "paleta", &img, ctx));
    }
    Ok(out)
}

// ---- palette variants -----------------------------------------------------------------------

pub(crate) fn palette_variant_set(s: &mut Session, a: PaletteVariantSet) -> Result<Outcome, CommandError> {
    let name = a.name.trim();
    if name.is_empty() {
        return Err(CommandError::invalid("a variante precisa de um nome"));
    }
    let index = s.doc.model.extras.paletas.variant_index(name);
    let mut warnings = Vec::new();
    let mut variant = match index {
        Some(i) => s.doc.model.extras.paletas.variants[i].clone(),
        None => {
            if s.doc.model.extras.paletas.variants.len() >= 256 {
                return Err(CommandError::new(ErrorCode::LimitExceeded, "máximo de 256 variantes de paleta por projeto"));
            }
            let base =
                a.base.clone().or_else(|| sprite_palette_name(s, (!s.doc.model.sprites.is_empty()).then_some(0))).ok_or_else(|| {
                    CommandError::not_found("o projeto não tem paleta para servir de base; crie uma com palette_create ou palette_import")
                })?;
            let base = find_palette(s, &base)?;
            PaletteVariant { name: name.to_string(), base: base.name.clone(), colors: base.colors.clone() }
        }
    };
    if index.is_some()
        && let Some(b) = &a.base
        && !b.eq_ignore_ascii_case(&variant.base)
    {
        let base = find_palette(s, b)?;
        if base.colors.len() != variant.colors.len() && a.colors.is_none() {
            return Err(CommandError::invalid(format!(
                "a paleta \"{}\" tem {} cores e a variante {}; passe também `colors` com {} cores",
                base.name,
                base.colors.len(),
                variant.colors.len(),
                base.colors.len()
            )));
        }
        variant.base = base.name.clone();
    }
    let base = find_palette(s, &variant.base)
        .map_err(|_| CommandError::not_found(format!("a paleta base \"{}\" não existe mais; informe outra em `base`", variant.base)))?
        .clone();
    let n = base.colors.len();
    if let Some(colors) = a.colors {
        if colors.len() != n {
            return Err(CommandError::invalid(format!(
                "a variante precisa de {n} cores, a mesma quantidade da paleta \"{}\" (vieram {})",
                base.name,
                colors.len()
            )));
        }
        variant.colors = colors;
    } else if variant.colors.len() != n {
        warnings.push(format!(
            "a paleta \"{}\" mudou de tamanho ({} → {n} cores): a variante foi ajustada com as cores da base",
            base.name,
            variant.colors.len()
        ));
        variant.colors.truncate(n);
        variant.colors.extend_from_slice(&base.colors[variant.colors.len()..]);
    }
    for set in &a.set {
        if set.index >= n {
            return Err(CommandError::invalid(format!("índice {} fora da paleta \"{}\" ({n} cores)", set.index, base.name)));
        }
        variant.colors[set.index] = set.color;
    }
    for swap in &a.replace {
        let indices: Vec<usize> = base.colors.iter().enumerate().filter(|(_, c)| **c == swap.from).map(|(i, _)| i).collect();
        if indices.is_empty() {
            return Err(CommandError::invalid(format!("a cor {} não está na paleta base \"{}\"", swap.from.to_hex(), base.name)));
        }
        for i in indices {
            variant.colors[i] = swap.to;
        }
    }
    if let Some(new_name) = &a.rename {
        let others =
            s.doc.model.extras.paletas.variants.iter().enumerate().filter(|(i, _)| Some(*i) != index).map(|(_, v)| v.name.as_str());
        ensure_unique("variante", new_name.trim(), others)?;
        variant.name = new_name.trim().to_string();
    }
    let changed: Vec<usize> = (0..n).filter(|i| variant.colors[*i] != base.colors[*i]).collect();
    let data = json!({
        "variant": variant.name, "base": variant.base, "colors": variant.colors, "created": index.is_none(), "changed_indices": changed,
    });
    match index {
        Some(i) => s.doc.model.extras.paletas.variants[i] = variant,
        None => s.doc.model.extras.paletas.variants.push(variant),
    }
    Ok(Outcome { data, warnings, changes: project_change(s), ..Default::default() })
}

pub(crate) fn palette_variant_delete(s: &mut Session, a: PaletteVariantDelete) -> Result<Outcome, CommandError> {
    let name = find_variant(s, &a.name)?.name.clone();
    s.doc.model.extras.paletas.variants.retain(|v| v.name != name);
    Ok(Outcome { data: json!({ "deleted": name }), changes: project_change(s), ..Default::default() })
}

pub(crate) fn palette_variant_list(s: &mut Session, a: PaletteVariantList) -> Result<Outcome, CommandError> {
    let list: Vec<serde_json::Value> = s
        .doc
        .model
        .extras
        .paletas
        .variants
        .iter()
        .filter(|v| a.base.as_ref().is_none_or(|b| b.eq_ignore_ascii_case(&v.base)))
        .map(|v| {
            let base = s.doc.model.palettes.iter().find(|p| p.name.eq_ignore_ascii_case(&v.base));
            let problem = match base {
                None => Some(format!("a paleta base \"{}\" não existe mais", v.base)),
                Some(p) if p.colors.len() != v.colors.len() => {
                    Some(format!("a paleta base tem {} cores e a variante {}", p.colors.len(), v.colors.len()))
                }
                Some(_) => None,
            };
            let changed = base.map(|p| p.colors.iter().zip(&v.colors).filter(|(a, b)| a != b).count());
            json!({ "name": v.name, "base": v.base, "colors": v.colors, "changed": changed, "valid": problem.is_none(), "problem": problem })
        })
        .collect();
    Ok(Outcome::data(json!({ "variants": list, "count": list.len() })))
}

pub(crate) fn palette_variant_preview(s: &mut Session, a: PaletteVariantPreview, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let frame = frame_id(sprite(&s.doc, si), a.frame.as_ref())?;
    let (label, base_name, recolor, mut warnings) = match (&a.variant, &a.colors) {
        (Some(v), _) => {
            let (v, r, notes) = variant_recolor(s, v)?;
            (v.name, v.base, r, notes)
        }
        (None, Some(colors)) => {
            let base =
                a.base.clone().or_else(|| sprite_palette_name(s, Some(si))).ok_or_else(|| CommandError::not_found("informe `base`"))?;
            let base = find_palette(s, &base)?;
            if colors.len() != base.colors.len() {
                return Err(CommandError::invalid(format!(
                    "`colors` precisa de {} cores, a mesma quantidade da paleta \"{}\" (vieram {})",
                    base.colors.len(),
                    base.name,
                    colors.len()
                )));
            }
            ("teste".to_string(), base.name.clone(), Recolor::new(&base.colors, colors), Vec::new())
        }
        (None, None) => return Err(CommandError::invalid("informe `variant` (uma variante salva) ou `colors` (cores de teste)")),
    };
    let sprite_id = sprite(&s.doc, si).id;
    let original = s.composite(sprite_id, frame).ok_or_else(|| CommandError::not_found(format!("frame {frame} não existe")))?;
    let (recolored, stats) = recolor.apply(&original);
    if !stats.unmatched.is_empty() {
        warnings.push(format!("{} cor(es) do frame não estão na paleta \"{base_name}\" e ficaram iguais", stats.unmatched.len()));
    }
    let shown = if a.compare {
        let (w, h) = (original.width(), original.height());
        let mut both = Image::new(w * 2 + 1, h);
        both.paste(&original, Point::default(), PasteMode::Replace);
        both.paste(&recolored, Point::new(w as i32 + 1, 0), PasteMode::Replace);
        both
    } else {
        recolored
    };
    let max = ctx.limits.max_preview_side;
    let scale = match a.scale {
        Some(sc) => check_scale(shown.width(), shown.height(), sc, max)?,
        None => auto_scale(shown.width(), shown.height(), max),
    };
    let img = preview(&shown, scale, a.background, false);
    let mut out = Outcome {
        data: json!({
            "variant": label, "base": base_name, "frame": frame, "changed_pixels": stats.changed,
            "unmatched_colors": unmatched_json(&stats), "scale": scale, "compare": a.compare,
        }),
        warnings,
        ..Default::default()
    };
    out.images.push(crate::render::png(format!("variante {label}"), &img)?);
    Ok(out)
}

pub(crate) fn export_palette_variants(s: &mut Session, a: ExportPaletteVariants, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let names: Vec<String> = if a.variants.is_empty() {
        s.doc.model.extras.paletas.variants.iter().map(|v| v.name.clone()).collect()
    } else {
        a.variants.iter().map(|n| find_variant(s, n).map(|v| v.name.clone())).collect::<Result<_, _>>()?
    };
    if names.is_empty() && !a.include_original {
        return Err(CommandError::invalid("o projeto não tem variantes de paleta; crie uma com palette_variant_set"));
    }
    let mut warnings = Vec::new();
    // (name in the file, recoloring; None = the original colors)
    let mut jobs: Vec<(String, Option<Recolor>)> = Vec::new();
    if a.include_original {
        let base = names
            .first()
            .and_then(|n| s.doc.model.extras.paletas.variant(n))
            .map(|v| v.base.clone())
            .or_else(|| sprite_palette_name(s, Some(si)))
            .unwrap_or_else(|| "original".into());
        jobs.push((base, None));
    }
    for n in &names {
        let (v, r, notes) = variant_recolor(s, n)?;
        warnings.extend(notes);
        jobs.push((v.name, Some(r)));
    }
    let dir = ctx.paths.resolve(&a.directory)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível criar {}: {e}", dir.display())))?;
    let sp = sprite(&s.doc, si);
    let images = s.doc.images();
    let prefix = sanitize(&a.prefix.clone().unwrap_or_else(|| sp.name.clone()));
    let ext = match a.format {
        VariantExportFormat::Png | VariantExportFormat::Spritesheet => "png",
        VariantExportFormat::Gif => "gif",
    };
    let mut used = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    let mut first_image: Option<Image> = None;
    for (label, recolor) in &jobs {
        let mut stem = format!("{prefix}_{}", sanitize(label));
        let mut k = 2;
        while !used.insert(stem.to_lowercase()) {
            stem = format!("{prefix}_{}_{k}", sanitize(label));
            k += 1;
        }
        let path = dir.join(format!("{stem}.{ext}"));
        let mut stats = RecolorStats::default();
        let mut map = |img: Image| match recolor {
            Some(r) => {
                let (out, st) = r.apply(&img);
                stats.merge(&st);
                out
            }
            None => img,
        };
        let mut entry = json!({ "variant": label, "original": recolor.is_none() });
        match a.format {
            VariantExportFormat::Png => {
                let frame = frame_id(sp, a.frame.as_ref())?;
                let scale = check_export_scale(sp.width, sp.height, a.scale)?;
                let img = scale_image(&map(compose_frame(sp, images, frame, &ComposeOptions::default())), scale);
                write(&path, &png_bytes(&img)?)?;
                entry["frame"] = json!(frame);
                first_image.get_or_insert(img);
            }
            VariantExportFormat::Spritesheet => {
                let items = sheet_items(sp, images, &a.selection)?;
                let p = plan(&items, sp.width, sp.height, &a.sheet);
                let scale = check_export_scale(p.width, p.height, a.scale)?;
                let img = render_sheet_mapped(sp, images, &p, scale, a.background, &mut map);
                write(&path, &png_bytes(&img)?)?;
                if a.json {
                    let meta = sheet_metadata(sp, &p, &file_name(&path), scale);
                    let json_path = path.with_extension("json");
                    write(&json_path, serde_json::to_string_pretty(&meta).unwrap_or_default().as_bytes())?;
                    entry["json"] = json!(json_path.display().to_string());
                }
                entry["size"] = json!([img.width(), img.height()]);
                first_image.get_or_insert(img);
            }
            VariantExportFormat::Gif => {
                let anim = a.animation.as_ref().ok_or_else(|| CommandError::invalid("o formato gif precisa de `animation`"))?;
                let frames = track_frames(sp, images, anim, a.direction.as_deref())?;
                let scale = check_export_scale(sp.width, sp.height, a.scale)?;
                let looping = sp.animation(anim)?.loop_mode != sprite_animation::LoopMode::Once;
                let list: Vec<(Image, u32)> = frames.into_iter().map(|(_, img, ms)| (map(img), ms)).collect();
                write(&path, &encode_gif(&list, scale, looping)?)?;
                entry["frames"] = json!(list.len());
            }
        }
        if recolor.is_some() && stats.changed == 0 {
            warnings
                .push(format!("a variante \"{label}\" não mudou nenhum pixel: as cores da paleta base não aparecem no que foi exportado"));
        }
        entry["path"] = json!(path.display().to_string());
        entry["changed_pixels"] = json!(stats.changed);
        files.push(entry);
    }
    let out = Outcome {
        data: json!({ "directory": dir.display().to_string(), "format": a.format, "files": files }),
        warnings,
        ..Default::default()
    };
    Ok(match first_image {
        Some(img) => with_preview(out, "primeira variante exportada", &img, ctx),
        None => out,
    })
}

// ---- normal map -----------------------------------------------------------------------------

pub(crate) fn export_normal_map(s: &mut Session, a: ExportNormalMap, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    if a.bevel == 0 || a.bevel > 64 {
        return Err(CommandError::invalid("bevel de 1 a 64 px"));
    }
    if !a.strength.is_finite() || a.strength < 0.0 || a.strength > 100.0 {
        return Err(CommandError::invalid("strength de 0 a 100"));
    }
    let opts = NormalMapOptions { mode: a.mode, bevel: a.bevel, strength: a.strength, flip_y: a.flip_y };
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let images = s.doc.images();
    let path = ctx.paths.resolve(&a.path)?;
    let (img, mut data) = match &a.sheet {
        Some(sheet) => {
            let items = sheet_items(sp, images, &a.selection)?;
            let p = plan(&items, sp.width, sp.height, sheet);
            let scale = check_export_scale(p.width, p.height, a.scale)?;
            let img = render_sheet_mapped(sp, images, &p, scale, None, &mut |f: Image| normal_map(&f, &opts));
            let data = json!({ "layout": "sheet", "frames": p.placements.len(), "cell": [p.cell_width * scale, p.cell_height * scale] });
            (img, data)
        }
        None => {
            let frame = frame_id(sp, a.frame.as_ref())?;
            let scale = check_export_scale(sp.width, sp.height, a.scale)?;
            let img = scale_image(&normal_map(&compose_frame(sp, images, frame, &ComposeOptions::default()), &opts), scale);
            (img, json!({ "layout": "frame", "frame": frame }))
        }
    };
    write(&path, &png_bytes(&img)?)?;
    data["path"] = json!(path.display().to_string());
    data["size"] = json!([img.width(), img.height()]);
    data["mode"] = json!(a.mode);
    data["convention"] = json!(if a.flip_y { "directx" } else { "opengl" });
    Ok(with_preview(Outcome::data(data), "normal map", &img, ctx))
}

// ---- Aseprite import ------------------------------------------------------------------------

/// `import_image` of an Aseprite file in `new_sprite` mode: one sprite with the same layers,
/// frames (durations), cels (positions, linked cels), one animation per tag, the file's palette
/// (active and used by the sprite) and slices as regions.
pub(crate) fn import_aseprite_sprite(
    s: &mut Session,
    a: &ImportImage,
    file: AseFile,
    path: &Path,
    ctx: &ExecContext,
) -> Result<Outcome, CommandError> {
    let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "importado".into());
    let name = a.name.clone().unwrap_or(stem);
    let (w, h) = (file.width, file.height);
    let limits = &ctx.limits;
    if w > limits.max_canvas_side || h > limits.max_canvas_side {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("imagem {w}x{h} maior que o limite de {} px por lado", limits.max_canvas_side),
        ));
    }
    if file.frames.len() > limits.max_frames {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("{} frames; o limite é {}", file.frames.len(), limits.max_frames)));
    }
    let (flat, flat_warnings) = file.flat_layers();
    if flat.len() > limits.max_layers {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("{} layers; o limite é {}", flat.len(), limits.max_layers)));
    }
    let mut warnings = file.warnings.clone();
    warnings.extend(flat_warnings);
    ensure_unique("sprite", &name, s.doc.model.sprites.iter().map(|x| x.name.as_str()))?;
    let si = add_sprite(&mut s.doc, &name, w, h);

    // Layers, bottom to top (the default layer is replaced).
    if flat.is_empty() {
        warnings.push("o arquivo não tem layers de imagem".into());
    } else {
        let mut layers: Vec<Layer> = Vec::new();
        for fl in &flat {
            let name = unique_name(if fl.name.trim().is_empty() { "Layer" } else { &fl.name }, layers.iter().map(|l| l.name.as_str()));
            let id = s.doc.alloc_id();
            layers.push(Layer {
                id,
                name,
                visible: fl.visible,
                locked: false,
                opacity: fl.opacity,
                blend: fl.blend,
                reference: fl.reference,
            });
        }
        s.doc.model.sprites[si].layers = layers;
    }
    let layer_ids: Vec<u32> = s.doc.model.sprites[si].layers.iter().map(|l| l.id).collect();

    // Frames with their durations.
    let mut frame_ids = vec![s.doc.model.sprites[si].frames[0].id];
    s.doc.model.sprites[si].frames[0].duration_ms = file.frames[0].duration_ms;
    for f in &file.frames[1..] {
        frame_ids.push(append_frame(&mut s.doc, si, f.duration_ms));
    }

    // Cels: one image per source cel; linked cels share it.
    let mut stored: BTreeMap<(usize, usize), u32> = BTreeMap::new();
    let (mut cels, mut linked) = (0usize, 0usize);
    for (fi, frame_id) in frame_ids.iter().copied().enumerate() {
        for (li, fl) in flat.iter().enumerate() {
            let Some(cel) = file.cel(fi, fl.index) else { continue };
            let Some((src_frame, src)) = file.source_cel(fi, fl.index) else { continue };
            let image = match stored.get(&(src_frame, fl.index)) {
                Some(id) => {
                    linked += 1;
                    *id
                }
                None => {
                    let Some(img) = file.cel_image(src) else { continue };
                    let id = s.doc.insert_image(img);
                    stored.insert((src_frame, fl.index), id);
                    id
                }
            };
            s.doc.model.sprites[si].cels.push(Cel { layer: layer_ids[li], frame: frame_id, image, opacity: cel.opacity });
            cels += 1;
        }
    }

    // Tags become animations.
    let mut animations = Vec::new();
    for tag in &file.tags {
        let (mode, _) = tag.loop_mode();
        if tag.repeat > 1 || (tag.repeat == 1 && tag.direction != 0) {
            warnings.push(format!("a tag \"{}\" repete {} vez(es) no Aseprite; aqui ela fica em repetição contínua", tag.name, tag.repeat));
        }
        let anim_name = unique_name(&tag.name, s.doc.model.sprites[si].animations.iter().map(|x| x.name.as_str()));
        let frames: Vec<u32> = tag.frames().into_iter().map(|i| frame_ids[i]).collect();
        let id = s.doc.alloc_id();
        animations.push(json!({ "id": id, "name": anim_name, "loop_mode": mode, "frames": frames }));
        s.doc.model.sprites[si].animations.push(Animation {
            id,
            name: anim_name,
            loop_mode: mode,
            tags: Vec::new(),
            tracks: vec![Track { direction: None, frames }],
            cycle: CycleInfo::default(),
        });
    }
    if let Some(anim_name) = &a.animation {
        let sp = sprite(&s.doc, si);
        let direction = a.direction.as_deref().map(|d| sp.direction_name(d)).transpose()?;
        ensure_unique("animação", anim_name, sp.animations.iter().map(|x| x.name.as_str()))?;
        let id = s.doc.alloc_id();
        animations.push(json!({ "id": id, "name": anim_name, "loop_mode": "loop", "frames": frame_ids }));
        s.doc.model.sprites[si].animations.push(Animation {
            id,
            name: anim_name.clone(),
            loop_mode: Default::default(),
            tags: Vec::new(),
            tracks: vec![Track { direction, frames: frame_ids.clone() }],
            cycle: CycleInfo::default(),
        });
    }

    // The file's palette becomes the sprite's palette (and the active one).
    let mut palette = None;
    if !file.palette.is_empty() {
        let mut colors = file.palette.clone();
        if colors.len() > 256 {
            warnings.push(format!("a paleta do arquivo tem {} cores: só as 256 primeiras foram guardadas", colors.len()));
            colors.truncate(256);
        }
        let pname = unique_name(&name, s.doc.model.palettes.iter().map(|p| p.name.as_str()));
        s.doc.model.palettes.push(Palette::new(pname.clone(), colors));
        s.doc.model.sprites[si].palette = Some(pname.clone());
        s.doc.model.active_palette = Some(pname.clone());
        palette = Some(pname);
    }

    // Slices become regions (a rectangle per frame where it moves).
    let mut regions = Vec::new();
    for slice in &file.slices {
        let Some(first) = slice.key_at(0) else { continue };
        let mut shapes = BTreeMap::new();
        for (fi, fid) in frame_ids.iter().enumerate() {
            if let Some(k) = slice.key_at(fi)
                && k.rect != first.rect
            {
                shapes.insert(*fid, RegionShape::Rect { rect: k.rect });
            }
        }
        if slice.keys.iter().any(|k| k.center.is_some()) {
            warnings.push(format!("o 9-slice da slice \"{}\" não foi importado (só o retângulo)", slice.name));
        }
        let rname = unique_name(&slice.name, s.doc.model.sprites[si].regions.iter().map(|r| r.name.as_str()));
        let id = s.doc.alloc_id();
        regions.push(rname.clone());
        s.doc.model.sprites[si].regions.push(Region {
            id,
            name: rname,
            part: None,
            layer: None,
            default: Some(RegionShape::Rect { rect: first.rect }),
            shapes,
            color: None,
        });
    }

    let sp = sprite(&s.doc, si);
    Ok(Outcome {
        data: json!({
            "sprite": sp.id, "name": sp.name, "size": [w, h], "format": "aseprite",
            "layers": sp.layers.iter().map(|l| json!({ "id": l.id, "name": l.name, "visible": l.visible })).collect::<Vec<_>>(),
            "frames": frame_ids, "cels": cels, "linked_cels": linked, "animations": animations,
            "palette": palette, "regions": regions,
        }),
        changes: vec![Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "structure".into() }],
        warnings,
        ..Default::default()
    })
}
