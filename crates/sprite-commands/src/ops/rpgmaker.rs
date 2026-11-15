//! RPG Maker commands: templates (a sprite with the exact size and named blocks of an RPG Maker
//! file) and exports (character sheets, face sheets, side-view battlers). The layouts come from
//! `sprite_project::rpgmaker`; this module resolves sprites, animations and frames, composes them
//! and writes the files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::json;
use sprite_animation::rpgmaker::{RpgMakerEngine, RpgMakerInfo, RpgMakerKind};
use sprite_animation::{Animation, Region, RegionShape, Sprite, Track};
use sprite_core::codec::encode_png;
use sprite_core::{Image, ItemRef, PasteMode, Point, Rect};
use sprite_project::format::write_atomic;
use sprite_project::rpgmaker::{
    CHARACTER_DIRECTIONS, RpgMakerError, SV_CELL, SV_MOTIONS, TEXT_COLORS, TemplateOptions, battler_cell, character_cell,
    character_file_name, check_cell, face_cell, motion_index, place_centered, place_on_baseline, template_layout, text_color_sample,
};

use super::ensure_unique;
use super::structure::add_sprite;
use crate::args::*;
use crate::target::{default_layer, sprite, sprite_index, writable_cel};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

fn layout_error(e: RpgMakerError) -> CommandError {
    match e {
        RpgMakerError::Unsupported(m) => CommandError::new(ErrorCode::Unsupported, m),
        RpgMakerError::Invalid(m) => CommandError::invalid(m),
    }
}

fn structure_change(s: &Session, si: usize) -> Change {
    Change { sprite: s.doc.model.sprites[si].id, layer: None, frame: None, rect: None, kind: "structure".into() }
}

// ---- template -------------------------------------------------------------------------------

/// A sprite name not used yet: `base`, `base 2`, `base 3`...
fn free_sprite_name(s: &Session, base: &str) -> String {
    let taken = |n: &str| s.doc.model.sprites.iter().any(|sp| sp.name.eq_ignore_ascii_case(n));
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base} {i}")).find(|n| !taken(n)).expect("some free name")
}

pub(crate) fn template(s: &mut Session, a: RpgMakerTemplate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let layout = template_layout(a.kind, a.engine, TemplateOptions { single: a.single, cell: a.cell }).map_err(layout_error)?;
    let mut warnings = Vec::new();
    if a.single && a.kind != RpgMakerKind::Character {
        warnings.push("`single` só vale para character; ignorado".to_string());
    }
    if a.cell.is_some() && !matches!(a.kind, RpgMakerKind::Character | RpgMakerKind::SvBattler) {
        warnings.push(format!("`cell` só vale para character e sv_battler; {} tem tamanho fixo", a.kind.id()));
    }
    if a.fill_text_colors && a.kind != RpgMakerKind::Window {
        warnings.push("`fill_text_colors` só vale para window; ignorado".to_string());
    }
    let (w, h) = (layout.width, layout.height);
    let max = ctx.limits.max_canvas_side;
    if w > max || h > max {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("o modelo {} teria {w}x{h} px, acima do limite de {max} px por lado; use uma célula menor", a.kind.id()),
        ));
    }
    let created = a.sprite.is_none();
    let si = match &a.sprite {
        Some(r) => {
            let si = sprite_index(&s.doc, Some(r))?;
            let sp = sprite(&s.doc, si);
            if (sp.width, sp.height) != (w, h) {
                return Err(CommandError::invalid(format!(
                    "o sprite \"{}\" tem {}x{} px, mas {} do {} precisa de exatamente {w}x{h} px; omita `sprite` para criar um sprite novo ou ajuste o tamanho com sprite_update",
                    sp.name,
                    sp.width,
                    sp.height,
                    a.kind.id(),
                    a.engine.label()
                )));
            }
            if let Some(name) = &a.name {
                ensure_unique(
                    "sprite",
                    name,
                    s.doc.model.sprites.iter().enumerate().filter(|(i, _)| *i != si).map(|(_, x)| x.name.as_str()),
                )?;
                s.doc.model.sprites[si].name = name.trim().to_string();
            }
            si
        }
        None => {
            let name = match &a.name {
                Some(n) => {
                    ensure_unique("sprite", n, s.doc.model.sprites.iter().map(|x| x.name.as_str()))?;
                    n.trim().to_string()
                }
                None => free_sprite_name(s, a.kind.default_name()),
            };
            add_sprite(&mut s.doc, &name, w, h)
        }
    };

    // Regions: applying the template again replaces the blocks with the same names.
    let replaced = {
        let sp = &mut s.doc.model.sprites[si];
        let before = sp.regions.len();
        sp.regions.retain(|r| !layout.regions.iter().any(|t| t.name.eq_ignore_ascii_case(&r.name)));
        before - sp.regions.len()
    };
    for t in &layout.regions {
        let id = s.doc.alloc_id();
        s.doc.model.sprites[si].regions.push(Region {
            id,
            name: t.name.clone(),
            part: Some(t.part.clone()),
            layer: None,
            default: Some(RegionShape::Rect { rect: t.rect }),
            shapes: Default::default(),
            color: Some(t.color),
        });
    }
    s.doc.model.sprites[si].extras.rpgmaker =
        Some(RpgMakerInfo { kind: a.kind, engine: a.engine, tile: layout.tile, cell: layout.cell, single: layout.single });

    let mut changes = vec![structure_change(s, si)];
    let mut painted = false;
    if a.fill_text_colors && a.kind == RpgMakerKind::Window {
        let sp = sprite(&s.doc, si);
        let sprite_id = sp.id;
        let layer = default_layer(sp).ok_or_else(|| CommandError::not_found("o sprite não tem layers"))?;
        let frame = sp.frames.first().map(|f| f.id).ok_or_else(|| CommandError::not_found("o sprite não tem frames"))?;
        let image = writable_cel(&mut s.doc, si, layer, frame)?;
        let img = s.doc.image_mut(image).ok_or_else(|| CommandError::new(ErrorCode::Internal, "cel sem imagem"))?;
        let mut area: Option<Rect> = None;
        for (n, color) in TEXT_COLORS.iter().enumerate() {
            let r = text_color_sample(a.engine, n);
            for p in r.points() {
                img.set(p.x, p.y, *color);
            }
            area = Some(area.map_or(r, |x| x.union(&r)));
        }
        changes.push(Change { sprite: sprite_id, layer: Some(layer), frame: Some(frame), rect: area, kind: "pixels".into() });
        painted = true;
    }

    let sp = sprite(&s.doc, si);
    let regions: Vec<serde_json::Value> = layout
        .regions
        .iter()
        .map(|r| json!({ "name": r.name, "part": r.part, "rect": { "x": r.rect.x, "y": r.rect.y, "w": r.rect.w, "h": r.rect.h } }))
        .collect();
    let mut data = json!({
        "sprite": sp.id,
        "name": sp.name,
        "created": created,
        "kind": a.kind,
        "engine": a.engine,
        "tile": layout.tile,
        "size": [w, h],
        "grid": layout.grid,
        "regions": regions,
    });
    if let Some(cell) = layout.cell {
        data["cell"] = json!(cell);
    }
    if layout.single {
        data["single"] = json!(true);
    }
    if replaced > 0 {
        data["replaced_regions"] = json!(replaced);
    }
    if painted {
        data["text_colors"] = json!(TEXT_COLORS.iter().map(|c| c.to_hex()).collect::<Vec<_>>());
    }
    Ok(Outcome { data, changes, warnings, ..Default::default() })
}

// ---- shared export helpers ------------------------------------------------------------------

fn scale_arg(scale: u32) -> Result<u32, CommandError> {
    if scale == 0 || scale > 16 {
        return Err(CommandError::invalid("`scale` de 1 a 16"));
    }
    Ok(scale)
}

fn frames_arg(frames: &[usize], what: &str) -> Result<(), CommandError> {
    if !frames.is_empty() && frames.len() != 3 {
        return Err(CommandError::invalid(format!(
            "{what}: `frames` precisa de exatamente 3 índices (colunas 0, 1 e 2), recebeu {}",
            frames.len()
        )));
    }
    Ok(())
}

fn sheet_size(w: u32, h: u32) -> Result<(), CommandError> {
    Image::check_size(w, h).map_err(|_| {
        CommandError::new(
            ErrorCode::LimitExceeded,
            format!("a folha teria {w}x{h} px; o máximo é {} px por lado", sprite_core::image::MAX_IMAGE_SIDE),
        )
    })
}

/// The 3 frame ids of a track for columns 0, 1, 2. Without explicit indices: the first three,
/// repeating the last one (with a warning) when the track is shorter.
fn pick_frames(track: &Track, wanted: &[usize], what: &str, warnings: &mut Vec<String>) -> Result<[u32; 3], CommandError> {
    let f = &track.frames;
    if wanted.len() == 3 {
        let mut out = [0; 3];
        for (k, i) in wanted.iter().enumerate() {
            out[k] = *f
                .get(*i)
                .ok_or_else(|| CommandError::invalid(format!("{what}: a trilha tem {} frame(s); o índice {i} não existe", f.len())))?;
        }
        return Ok(out);
    }
    let n = f.len();
    if n < 3 {
        warnings.push(format!("{what}: a trilha tem só {n} frame(s); o último foi repetido nas colunas que faltavam"));
    }
    Ok([f[0], f[1.min(n - 1)], f[2.min(n - 1)]])
}

fn direction_track<'a>(anim: &'a Animation, direction: &str) -> Option<&'a Track> {
    anim.tracks.iter().find(|t| !t.frames.is_empty() && t.direction.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(direction)))
}

fn plain_track(anim: &Animation) -> Option<&Track> {
    anim.tracks.iter().find(|t| !t.frames.is_empty() && t.direction.is_none())
}

fn check_fits(sp: &Sprite, scale: u32, [cw, ch]: [u32; 2], hint: &str) -> Result<(), CommandError> {
    let (w, h) = (sp.width * scale, sp.height * scale);
    if w > cw || h > ch {
        let scaled = if scale > 1 { format!(" (na escala {scale}: {w}x{h})") } else { String::new() };
        return Err(CommandError::invalid(format!(
            "o sprite \"{}\" tem {}x{} px{scaled} e não cabe na célula de {cw}x{ch} px; {hint}",
            sp.name, sp.width, sp.height
        )));
    }
    Ok(())
}

/// Resolves the output path, forcing the `.png` extension and creating missing folders.
fn png_path(raw: &str, ctx: &ExecContext) -> Result<PathBuf, CommandError> {
    let mut path = ctx.paths.resolve(raw)?;
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        let mut name = path.file_name().map(|n| n.to_os_string()).ok_or_else(|| CommandError::invalid("informe o nome do arquivo .png"))?;
        name.push(".png");
        path.set_file_name(name);
    }
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)
            .map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível criar a pasta {}: {e}", dir.display())))?;
    }
    Ok(path)
}

fn write_png(path: &Path, img: &Image) -> Result<(), CommandError> {
    let bytes = encode_png(img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
    write_atomic(path, &bytes).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível gravar {}: {e}", path.display())))
}

/// Adds the written image to the outcome when it fits the preview limit.
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

fn rect_json(r: Rect) -> serde_json::Value {
    json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

/// One cell to fill: which frame, where, and whether to flip the finished cell.
struct Cell {
    si: usize,
    frame: u32,
    rect: Rect,
    mirror: bool,
}

/// Composes every cell (frame on the baseline, pivot at the bottom center) into a sheet. Returns
/// the sheet and the number of visible pixels cut off at the cell borders.
fn draw_cells(s: &mut Session, cells: &[Cell], width: u32, height: u32, scale: u32, cell: [u32; 2]) -> (Image, u32) {
    let mut sheet = Image::new(width, height);
    let mut lost = 0;
    for c in cells {
        let sprite_id = s.doc.model.sprites[c.si].id;
        let pivot = s.doc.model.sprites[c.si].pivot(c.frame);
        let Some(img) = s.composite(sprite_id, c.frame) else { continue };
        let (mut placed, cut) = place_on_baseline(&img, pivot, scale, cell);
        if c.mirror {
            placed = placed.flip_horizontal();
        }
        lost += cut;
        sheet.paste(&placed, Point::new(c.rect.x, c.rect.y), PasteMode::Replace);
    }
    (sheet, lost)
}

// ---- character sheet ------------------------------------------------------------------------

pub(crate) fn export_character(s: &mut Session, a: ExportRpgMakerCharacter, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let n = a.characters.len();
    if n == 0 || n > 8 {
        return Err(CommandError::invalid(format!("`characters` precisa de 1 a 8 personagens, recebeu {n}")));
    }
    if a.single && n != 1 {
        return Err(CommandError::invalid(format!(
            "`single` (folha $ de um personagem só) aceita exatamente 1 personagem, recebeu {n}; tire `single` para a folha comum de até 8"
        )));
    }
    let scale = scale_arg(a.scale)?;
    let tile = a.engine.tile();
    let mut warnings = Vec::new();

    // Sprites of each character, and the cell size.
    let mut sprites = Vec::with_capacity(n);
    for item in &a.characters {
        sprites.push(sprite_index(&s.doc, item.sprite.as_ref().or(a.target.sprite.as_ref()))?);
    }
    let cell = match a.cell {
        Some(c) => check_cell(c).map_err(layout_error)?,
        None if a.single => {
            let sp = sprite(&s.doc, sprites[0]);
            [tile.max(sp.width * scale), tile.max(sp.height * scale)]
        }
        None => [tile, tile],
    };
    if !a.single {
        for &si in &sprites {
            check_fits(
                sprite(&s.doc, si),
                scale,
                cell,
                "use `single: true` (folha $ de um personagem, célula de qualquer tamanho), uma `cell` maior ou um sprite menor",
            )?;
        }
    }

    // Plan: 4 direction rows × 3 columns per character.
    let mut cells = Vec::new();
    let mut rows = Vec::new();
    for (k, item) in a.characters.iter().enumerate() {
        let si = sprites[k];
        let what = format!("personagem {k}");
        frames_arg(&item.frames, &what)?;
        let sp = sprite(&s.doc, si);
        let anim = sp.animation(&item.animation)?;
        let mut filled = 0;
        for (row, (dir, label)) in CHARACTER_DIRECTIONS.iter().enumerate() {
            let opposite = match *dir {
                "W" => Some("E"),
                "E" => Some("W"),
                _ => None,
            };
            let (track, mirror, source) = if let Some(t) = direction_track(anim, dir) {
                (Some(t), false, dir.to_string())
            } else if let Some(t) = opposite.filter(|_| a.mirror_missing).and_then(|o| direction_track(anim, o)) {
                (Some(t), true, format!("{} espelhado", opposite.unwrap_or_default()))
            } else if let Some(t) = plain_track(anim) {
                (Some(t), false, "trilha sem direção".to_string())
            } else {
                (None, false, "vazio".to_string())
            };
            let Some(track) = track else {
                let hint =
                    if opposite.is_some() && !a.mirror_missing { " (use `mirror_missing` para espelhar a direção oposta)" } else { "" };
                warnings.push(format!("{what}: a animação \"{}\" não tem a direção {dir} ({label}); a linha ficou vazia{hint}", anim.name));
                rows.push(json!({ "character": k, "direction": dir, "source": source, "frames": [] }));
                continue;
            };
            let frames = pick_frames(track, &item.frames, &format!("{what}, direção {dir}"), &mut warnings)?;
            for (pattern, frame) in frames.iter().enumerate() {
                cells.push(Cell { si, frame: *frame, rect: character_cell(k, row, pattern, cell, a.single), mirror });
            }
            filled += 1;
            rows.push(json!({
                "character": k, "direction": dir, "source": source, "frames": frames,
                "cell": rect_json(character_cell(k, row, 0, cell, a.single)),
            }));
        }
        if filled == 0 {
            return Err(CommandError::invalid(format!(
                "{what}: a animação \"{}\" não tem trilhas nas direções S, W, E ou N (nem uma trilha sem direção); crie-as com animation_set_direction",
                anim.name
            )));
        }
    }

    let (cols, rows_n) = if a.single { (3, 4) } else { (12, 8) };
    let (width, height) = (cols * cell[0], rows_n * cell[1]);
    sheet_size(width, height)?;
    let (sheet, lost) = draw_cells(s, &cells, width, height, scale, cell);
    if lost > 0 {
        warnings.push(format!(
            "{lost} pixel(s) visíveis ficaram fora das células e foram cortados; confira o pivô do sprite ou use uma célula maior"
        ));
    }

    let path = png_path(&a.path, ctx)?;
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let (prefixed, changed) = character_file_name(&file, a.single, a.object);
    if changed {
        warnings.push(format!("o prefixo do nome foi ajustado para combinar com `single`/`object`: {prefixed}"));
    }
    let path = path.with_file_name(&prefixed);
    write_png(&path, &sheet)?;
    let out = Outcome {
        data: json!({
            "path": path.display().to_string(),
            "file": prefixed,
            "engine": a.engine,
            "size": [width, height],
            "cell": cell,
            "characters": n,
            "single": a.single,
            "object": a.object,
            "rows": rows,
        }),
        warnings,
        ..Default::default()
    };
    Ok(with_preview(out, "folha de personagem do RPG Maker", &sheet, ctx))
}

// ---- faces ----------------------------------------------------------------------------------

pub(crate) fn export_faces(s: &mut Session, a: ExportRpgMakerFaces, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let n = a.faces.len();
    if n == 0 || n > 8 {
        return Err(CommandError::invalid(format!("`faces` precisa de 1 a 8 faces, recebeu {n}")));
    }
    let scale = scale_arg(a.scale)?;
    let size = 3 * a.engine.tile();
    let mut slots: BTreeMap<usize, (usize, u32)> = BTreeMap::new();
    for (k, item) in a.faces.iter().enumerate() {
        let index = item.index.unwrap_or(k);
        if index > 7 {
            return Err(CommandError::new(ErrorCode::OutOfBounds, format!("face {k}: `index` {index} fora da grade (0 a 7)")));
        }
        let si = sprite_index(&s.doc, item.sprite.as_ref().or(a.target.sprite.as_ref()))?;
        let sp = sprite(&s.doc, si);
        check_fits(sp, scale, [size, size], "use uma `scale` menor ou recorte o sprite")?;
        let frame = sp.resolve_frame(&item.frame)?;
        if slots.insert(index, (si, frame)).is_some() {
            return Err(CommandError::invalid(format!("duas faces na posição {index}; informe `index` diferentes")));
        }
    }
    let (width, height) = (4 * size, 2 * size);
    let mut sheet = Image::new(width, height);
    let mut placed = Vec::new();
    let mut lost = 0;
    for (index, (si, frame)) in &slots {
        let sprite_id = s.doc.model.sprites[*si].id;
        let Some(img) = s.composite(sprite_id, *frame) else { continue };
        let (face, cut) = place_centered(&img, scale, [size, size]);
        lost += cut;
        let r = face_cell(*index, size);
        sheet.paste(&face, Point::new(r.x, r.y), PasteMode::Replace);
        placed.push(json!({ "index": index, "sprite": sprite_id, "frame": frame, "cell": rect_json(r) }));
    }
    let mut warnings = Vec::new();
    if lost > 0 {
        warnings.push(format!("{lost} pixel(s) ficaram fora das células e foram cortados"));
    }
    let path = png_path(&a.path, ctx)?;
    write_png(&path, &sheet)?;
    let out = Outcome {
        data: json!({ "path": path.display().to_string(), "engine": a.engine, "size": [width, height], "cell": [size, size], "faces": placed }),
        warnings,
        ..Default::default()
    };
    Ok(with_preview(out, "faces do RPG Maker", &sheet, ctx))
}

// ---- side-view battler ----------------------------------------------------------------------

pub(crate) fn export_battler(s: &mut Session, a: ExportRpgMakerBattler, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    if a.engine == RpgMakerEngine::Vxace {
        return Err(CommandError::new(
            ErrorCode::Unsupported,
            "o RPG Maker VX Ace não tem battlers de visão lateral; use engine \"mz\" ou \"mv\"",
        ));
    }
    if a.motions.is_empty() {
        return Err(CommandError::invalid("informe pelo menos um movimento em `motions` (ex.: {\"walk\": \"andar\", \"wait\": \"idle\"})"));
    }
    frames_arg(&a.frames, "battler")?;
    let scale = scale_arg(a.scale)?;
    let cell = check_cell(a.cell.unwrap_or(SV_CELL)).map_err(layout_error)?;
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    check_fits(sp, scale, cell, "informe uma `cell` maior (o RPG Maker divide a imagem em 9×6 células de qualquer tamanho)")?;

    let mut by_index: BTreeMap<usize, (&String, &RpgMakerMotionSource)> = BTreeMap::new();
    for (key, src) in &a.motions {
        let i = motion_index(key).ok_or_else(|| {
            CommandError::invalid(format!(
                "movimento desconhecido: \"{key}\"; use {}",
                SV_MOTIONS.iter().map(|m| m.0).collect::<Vec<_>>().join(", ")
            ))
        })?;
        if let Some((other, _)) = by_index.insert(i, (key, src)) {
            return Err(CommandError::invalid(format!("\"{other}\" e \"{key}\" são o mesmo movimento ({})", SV_MOTIONS[i].0)));
        }
    }

    let mut warnings = Vec::new();
    let mut cells = Vec::new();
    let mut motions = Vec::new();
    for (i, (key, src)) in &by_index {
        let (anim_ref, direction, frames): (&ItemRef, Option<&String>, &[usize]) = match src {
            RpgMakerMotionSource::Animation(r) => (r, a.direction.as_ref(), &a.frames),
            RpgMakerMotionSource::Detailed { animation, direction, frames } => {
                (animation, direction.as_ref().or(a.direction.as_ref()), if frames.is_empty() { &a.frames } else { frames })
            }
        };
        let what = format!("movimento {}", SV_MOTIONS[*i].0);
        frames_arg(frames, &what)?;
        let anim = sp.animation(anim_ref)?;
        let track = match direction {
            Some(d) => {
                let d = sp.direction_name(d)?;
                direction_track(anim, &d)
                    .ok_or_else(|| CommandError::not_found(format!("{what}: a animação \"{}\" não tem trilha na direção {d}", anim.name)))?
            }
            None => anim
                .tracks
                .iter()
                .find(|t| !t.frames.is_empty())
                .ok_or_else(|| CommandError::invalid(format!("{what}: a animação \"{}\" não tem frames", anim.name)))?,
        };
        let ids = pick_frames(track, frames, &what, &mut warnings)?;
        for (pattern, frame) in ids.iter().enumerate() {
            cells.push(Cell { si, frame: *frame, rect: battler_cell(*i, pattern, cell), mirror: false });
        }
        motions.push(json!({
            "motion": SV_MOTIONS[*i].0, "index": i, "key": key, "animation": anim.name,
            "direction": track.direction, "frames": ids, "cell": rect_json(battler_cell(*i, 0, cell)),
        }));
    }
    let missing: Vec<&str> = SV_MOTIONS.iter().enumerate().filter(|(i, _)| !by_index.contains_key(i)).map(|(_, m)| m.0).collect();
    if !missing.is_empty() {
        warnings.push(format!("movimentos sem animação (ficaram vazios): {}", missing.join(", ")));
    }

    let (width, height) = (9 * cell[0], 6 * cell[1]);
    sheet_size(width, height)?;
    let (sheet, lost) = draw_cells(s, &cells, width, height, scale, cell);
    if lost > 0 {
        warnings.push(format!("{lost} pixel(s) visíveis ficaram fora das células e foram cortados; confira o pivô do sprite"));
    }
    let path = png_path(&a.path, ctx)?;
    write_png(&path, &sheet)?;
    let out = Outcome {
        data: json!({
            "path": path.display().to_string(), "engine": a.engine, "size": [width, height], "cell": cell,
            "motions": motions, "empty": missing,
        }),
        warnings,
        ..Default::default()
    };
    Ok(with_preview(out, "battler lateral do RPG Maker", &sheet, ctx))
}

#[cfg(test)]
mod tests;
