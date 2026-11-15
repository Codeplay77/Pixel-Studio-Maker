//! Godot integration: hitboxes per frame, the animated scene export (`.tscn`) and export jobs
//! (saved exports that run on save and in batch).

use std::path::{Path, PathBuf};

use serde_json::json;
use sprite_animation::Sprite;
use sprite_animation::extras::godot::Hitbox;
use sprite_animation::sheet::plan;
use sprite_core::codec::encode_png;
use sprite_core::{Color, Image, ItemRef, Rect};
use sprite_project::export::{godot_sprite_frames, render_sheet, sheet_items};
use sprite_project::format::write_atomic;
use sprite_project::godot::{ExportJob, SceneOptions, godot_scene};

use super::ensure_unique;
use crate::args::*;
use crate::target::{sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Kind, Outcome, Session, parse_command, specs};

/// Largest coordinate or size accepted for a box (boxes may stick out of the canvas a bit).
const MAX_COORD: i64 = 16_384;

fn structural(s: &Session, si: usize, data: serde_json::Value) -> Outcome {
    let id = s.doc.model.sprites[si].id;
    Outcome {
        data,
        changes: vec![Change { sprite: id, layer: None, frame: None, rect: None, kind: "structure".into() }],
        ..Default::default()
    }
}

fn project_changes(s: &Session) -> Vec<Change> {
    s.doc.model.sprites.iter().map(|sp| Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "structure".into() }).collect()
}

// ---- hitboxes -------------------------------------------------------------------------------

fn hitbox_index(sp: &Sprite, r: &ItemRef) -> Result<usize, CommandError> {
    sp.extras
        .godot
        .hitboxes
        .iter()
        .position(|h| r.matches(h.id, &h.name))
        .ok_or_else(|| CommandError::not_found(format!("a caixa {r} não existe no sprite \"{}\"", sp.name)))
}

/// Frame ids chosen by `at`, without repeats, in the order given.
fn chosen_frames(sp: &Sprite, at: &HitboxFrames) -> Result<Vec<u32>, CommandError> {
    let mut out: Vec<u32> = Vec::new();
    let mut add = |f: u32| {
        if !out.contains(&f) {
            out.push(f);
        }
    };
    if let Some(f) = &at.frame {
        add(sp.resolve_frame(f)?);
    }
    for f in &at.frames {
        add(sp.resolve_frame(f)?);
    }
    if let Some(an) = &at.animation {
        let anim = sp.animation(an)?;
        match &at.direction {
            Some(d) => {
                let dir = sp.direction_name(d)?;
                let t = anim
                    .track(Some(&dir))
                    .ok_or_else(|| CommandError::not_found(format!("a animação \"{}\" não tem a direção {dir}", anim.name)))?;
                t.frames.iter().for_each(|f| add(*f));
            }
            None => anim.tracks.iter().flat_map(|t| t.frames.iter()).for_each(|f| add(*f)),
        }
    } else if at.direction.is_some() {
        return Err(CommandError::invalid("`direction` só vale junto com `animation`"));
    }
    if at.all_frames {
        sp.frames.iter().for_each(|f| add(f.id));
    }
    Ok(out)
}

fn check_rect(r: &Rect) -> Result<(), CommandError> {
    if r.w == 0 || r.h == 0 {
        return Err(CommandError::invalid("o retângulo precisa de largura e altura de pelo menos 1 px"));
    }
    if (r.x as i64).abs() > MAX_COORD || (r.y as i64).abs() > MAX_COORD || r.w as i64 > MAX_COORD || r.h as i64 > MAX_COORD {
        return Err(CommandError::new(ErrorCode::OutOfBounds, format!("retângulo {r:?} grande ou distante demais")));
    }
    Ok(())
}

/// Alert for a rectangle that is not fully inside the canvas (allowed, but usually a mistake).
fn outside_warning(sp: &Sprite, name: &str, r: &Rect) -> Option<String> {
    let canvas = Rect::sized(sp.width, sp.height);
    let inside = r.intersect(&canvas);
    if inside == *r {
        return None;
    }
    Some(if inside.is_empty() {
        format!("a caixa \"{name}\" ({},{} {}x{}) está fora do canvas {}x{}", r.x, r.y, r.w, r.h, sp.width, sp.height)
    } else {
        format!("a caixa \"{name}\" ({},{} {}x{}) passa da borda do canvas {}x{}", r.x, r.y, r.w, r.h, sp.width, sp.height)
    })
}

fn hitbox_json(h: &Hitbox) -> serde_json::Value {
    json!({ "id": h.id, "name": h.name, "kind": h.kind, "color": h.display_color(), "frames": h.frames.len() })
}

pub(crate) fn hitbox_create(s: &mut Session, a: HitboxCreate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let name = a.name.trim().to_string();
    ensure_unique("caixa", &name, sp.extras.godot.hitboxes.iter().map(|h| h.name.as_str()))?;
    let mut frames = std::collections::BTreeMap::new();
    let mut warnings = Vec::new();
    match (&a.rect, a.at.is_empty()) {
        (Some(r), false) => {
            check_rect(r)?;
            for f in chosen_frames(sp, &a.at)? {
                frames.insert(f, *r);
            }
            warnings.extend(outside_warning(sp, &name, r));
        }
        (Some(_), true) => return Err(CommandError::invalid("com `rect`, diga onde: `frame`, `frames`, `animation` ou `all_frames`")),
        (None, false) => return Err(CommandError::invalid("informe `rect` para posicionar a caixa nesses frames")),
        (None, true) => {}
    }
    let id = s.doc.alloc_id();
    let hb = Hitbox { id, name, kind: a.kind, color: a.color, frames };
    let data = json!({ "hitbox": hitbox_json(&hb), "frames": hb.frames.keys().collect::<Vec<_>>() });
    s.doc.model.sprites[si].extras.godot.hitboxes.push(hb);
    let mut out = structural(s, si, data);
    out.warnings = warnings;
    Ok(out)
}

pub(crate) fn hitbox_update(s: &mut Session, a: HitboxUpdate) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let hi = hitbox_index(sp, &a.hitbox)?;
    if a.delete {
        let removed = s.doc.model.sprites[si].extras.godot.hitboxes.remove(hi);
        return Ok(structural(s, si, json!({ "deleted": removed.name, "id": removed.id })));
    }
    if let Some(n) = &a.name {
        let others = sp.extras.godot.hitboxes.iter().enumerate().filter(|(i, _)| *i != hi).map(|(_, h)| h.name.as_str());
        ensure_unique("caixa", n.trim(), others)?;
    }
    let hb = &mut s.doc.model.sprites[si].extras.godot.hitboxes[hi];
    if let Some(n) = a.name {
        hb.name = n.trim().to_string();
    }
    if let Some(k) = a.kind {
        hb.kind = k;
    }
    if let Some(c) = a.color {
        hb.color = Some(c);
    }
    let data = json!({ "hitbox": hitbox_json(hb) });
    Ok(structural(s, si, data))
}

pub(crate) fn hitbox_set(s: &mut Session, a: HitboxSet) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let hi = hitbox_index(sp, &a.hitbox)?;
    match (&a.rect, a.clear) {
        (Some(_), true) => return Err(CommandError::invalid("use `rect` ou `clear`, não os dois")),
        (None, false) => return Err(CommandError::invalid("informe `rect` (ligar/mover a caixa) ou `clear: true` (desligar)")),
        (Some(r), false) => check_rect(r)?,
        (None, true) => {}
    }
    if a.at.is_empty() {
        return Err(CommandError::invalid("diga em quais frames: `frame`, `frames`, `animation` (+ `direction`) ou `all_frames`"));
    }
    let frames = chosen_frames(sp, &a.at)?;
    let name = sp.extras.godot.hitboxes[hi].name.clone();
    let warnings: Vec<String> = a.rect.as_ref().and_then(|r| outside_warning(sp, &name, r)).into_iter().collect();
    let hb = &mut s.doc.model.sprites[si].extras.godot.hitboxes[hi];
    for f in &frames {
        match a.rect {
            Some(r) => {
                hb.frames.insert(*f, r);
            }
            None => {
                hb.frames.remove(f);
            }
        }
    }
    let data = json!({ "hitbox": hitbox_json(hb), "frames": frames, "rect": a.rect, "cleared": a.clear });
    let mut out = structural(s, si, data);
    out.warnings = warnings;
    Ok(out)
}

pub(crate) fn hitbox_list(s: &mut Session, a: HitboxList) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = a.frame.as_ref().map(|f| sp.resolve_frame(f)).transpose()?;
    let only = a.hitbox.as_ref().map(|r| hitbox_index(sp, r)).transpose()?;
    let mut warnings = Vec::new();
    let boxes: Vec<serde_json::Value> = sp
        .extras
        .godot
        .hitboxes
        .iter()
        .enumerate()
        .filter(|(i, _)| only.is_none_or(|o| o == *i))
        .map(|(_, h)| {
            let outside = h.frames.iter().filter(|(_, r)| outside_warning(sp, &h.name, r).is_some()).map(|(f, _)| *f).collect::<Vec<_>>();
            if !outside.is_empty() {
                warnings.push(format!("a caixa \"{}\" sai do canvas nos frames {outside:?}", h.name));
            }
            let mut v = hitbox_json(h);
            match frame {
                Some(f) => v["rect"] = json!(h.at(f)),
                None => v["rects"] = json!(h.frames),
            }
            v
        })
        .collect();
    let mut out = Outcome::data(json!({ "sprite": sp.name, "frame": frame, "hitboxes": boxes, "canvas": [sp.width, sp.height] }));
    out.warnings = warnings;
    Ok(out)
}

/// Draws the boxes of `frame` on a preview of the sprite area `view` enlarged `scale` times:
/// a light fill and an outline in each box's color.
pub(crate) fn draw_hitboxes(out: &mut Image, sp: &Sprite, frame: u32, view: Rect, scale: u32, legend: &mut Vec<serde_json::Value>) {
    let k = scale as i32;
    let thick = (k / 4).clamp(1, 3);
    for hb in &sp.extras.godot.hitboxes {
        let Some(r) = hb.at(frame) else { continue };
        let c = hb.display_color();
        let (x0, y0) = ((r.x - view.x) * k, (r.y - view.y) * k);
        let (x1, y1) = (x0 + r.w as i32 * k, y0 + r.h as i32 * k);
        let fill = Color::rgba(c.r, c.g, c.b, 56);
        for y in y0.max(0)..y1.min(out.height() as i32) {
            for x in x0.max(0)..x1.min(out.width() as i32) {
                let border = x < x0 + thick || x >= x1 - thick || y < y0 + thick || y >= y1 - thick;
                let px = if border { Color::rgb(c.r, c.g, c.b) } else { fill.over(out.get_or_clear(x, y)) };
                out.set(x, y, px);
            }
        }
        legend.push(json!({ "hitbox": hb.name, "kind": hb.kind, "rect": r, "color": c }));
    }
}

// ---- Godot scene ----------------------------------------------------------------------------

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

/// The Godot project holding `file` (nearest folder above it with `project.godot`) and the
/// `res://` path of the file's folder in it.
fn godot_project_root(file: &Path) -> Option<(PathBuf, String)> {
    let dir = file.parent()?;
    let root = dir.ancestors().find(|d| d.join("project.godot").is_file())?;
    let parts: Vec<String> = dir.strip_prefix(root).ok()?.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    Some((root.to_path_buf(), if parts.is_empty() { "res://".into() } else { format!("res://{}/", parts.join("/")) }))
}

pub(crate) fn export_godot_scene(s: &mut Session, a: ExportGodotScene, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let mut tscn = ctx.paths.resolve(&a.path)?;
    match tscn.extension().and_then(|e| e.to_str()) {
        None => {
            tscn.set_extension("tscn");
        }
        Some(e) if e.eq_ignore_ascii_case("tscn") => {}
        Some(e) => return Err(CommandError::invalid(format!("a cena precisa ser um arquivo .tscn (recebi .{e})"))),
    }
    let (tres, png) = (tscn.with_extension("tres"), tscn.with_extension("png"));
    let file = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut warnings = Vec::new();
    // Inside a Godot project (a folder with project.godot above the scene), the res:// path
    // follows the folder; otherwise the files are expected at the project root.
    let godot_root = godot_project_root(&tscn);
    let mut res_dir = match (&a.res_dir, &godot_root) {
        (Some(d), _) => d.clone(),
        (None, Some((_, rel))) => rel.clone(),
        (None, None) => "res://".into(),
    }
    .replace('\\', "/");
    if !res_dir.starts_with("res://") {
        warnings.push(format!("res_dir \"{res_dir}\" não começa com res://; o Godot pode não achar os arquivos"));
    }
    if !res_dir.ends_with('/') {
        res_dir.push('/');
    }
    let (png_res, tres_res) = (format!("{res_dir}{}", file(&png)), format!("{res_dir}{}", file(&tres)));

    let items = sheet_items(sp, s.doc.images(), &a.selection)?;
    let p = plan(&items, sp.width, sp.height, &a.sheet);
    let scale = check_export_scale(p.width, p.height, a.scale)?;
    let img = render_sheet(sp, s.doc.images(), &p, scale, None);
    write(&png, &encode_png(&img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?)?;
    write(&tres, godot_sprite_frames(sp, &p, &png_res, scale).as_bytes())?;
    let (text, info) = godot_scene(sp, &p, &SceneOptions { frames_res_path: tres_res.clone(), scale, autoplay: a.autoplay });
    write(&tscn, text.as_bytes())?;
    for hb in &sp.extras.godot.hitboxes {
        if hb.frames.is_empty() {
            warnings.push(format!("a caixa \"{}\" não tem retângulo em nenhum frame: fica sempre desligada", hb.name));
        }
    }
    let animations: Vec<serde_json::Value> = info
        .animations
        .iter()
        .map(|an| json!({ "name": an.name, "frames": an.frames, "length_ms": an.length_ms, "loop_mode": an.loop_mode }))
        .collect();
    let mut out = Outcome::data(json!({
        "tscn": tscn.display().to_string(), "tres": tres.display().to_string(), "png": png.display().to_string(),
        "res": { "scene": format!("{res_dir}{}", file(&tscn)), "sprite_frames": tres_res, "texture": png_res },
        "godot_project": godot_root.as_ref().map(|(root, _)| root.display().to_string()),
        "root": info.root, "offset": info.offset, "animations": animations,
        "hitboxes": info.hitboxes, "markers": info.markers, "sheet_size": [img.width(), img.height()],
    }));
    out.warnings = warnings;
    if img.width() <= ctx.limits.max_preview_side
        && img.height() <= ctx.limits.max_preview_side
        && let Ok(png) = crate::render::png("sprite sheet da cena Godot", &img)
    {
        out.images.push(png);
    }
    Ok(out)
}

// ---- export jobs ----------------------------------------------------------------------------

/// Commands a job may run: read-only exports, not the job commands themselves.
fn check_job_command(name: &str) -> Result<(), CommandError> {
    let exports =
        || specs().into_iter().filter(|s| s.name.starts_with("export_") && !s.name.starts_with("export_job") && s.kind == Kind::Read);
    if exports().any(|s| s.name == name) {
        return Ok(());
    }
    Err(CommandError::invalid(format!(
        "\"{name}\" não é um comando de exportação; use um destes: {}",
        exports().map(|s| s.name).collect::<Vec<_>>().join(", ")
    )))
}

fn check_job(job: &ExportJob) -> Result<(), CommandError> {
    check_job_command(&job.command)?;
    parse_command(&job.command, serde_json::Value::Object(job.args.clone()))
        .map(|_| ())
        .map_err(|e| CommandError::invalid(format!("tarefa \"{}\": {}", job.name, e.message)))
}

pub(crate) fn export_job_set(s: &mut Session, a: ExportJobSet) -> Result<Outcome, CommandError> {
    let name = a.name.trim().to_string();
    if name.is_empty() {
        return Err(CommandError::invalid("a tarefa precisa de um nome"));
    }
    let jobs = &s.doc.model.extras.godot.export_jobs;
    let existing = jobs.iter().position(|j| j.name.eq_ignore_ascii_case(&name));
    let job = match existing {
        Some(i) => {
            let mut job = jobs[i].clone();
            if let Some(c) = a.command {
                job.command = c;
            }
            if let Some(args) = a.args {
                job.args = args;
            }
            if let Some(o) = a.on_save {
                job.on_save = o;
            }
            job
        }
        None => ExportJob {
            name,
            command: a.command.ok_or_else(|| CommandError::invalid("informe `command` para criar a tarefa"))?,
            args: a.args.unwrap_or_default(),
            on_save: a.on_save.unwrap_or(false),
        },
    };
    check_job(&job)?;
    let data = json!({ "job": job, "created": existing.is_none() });
    let jobs = &mut s.doc.model.extras.godot.export_jobs;
    match existing {
        Some(i) => jobs[i] = job,
        None => jobs.push(job),
    }
    Ok(Outcome { data, changes: project_changes(s), ..Default::default() })
}

pub(crate) fn export_job_delete(s: &mut Session, a: ExportJobDelete) -> Result<Outcome, CommandError> {
    let jobs = &mut s.doc.model.extras.godot.export_jobs;
    let i = jobs
        .iter()
        .position(|j| j.name.eq_ignore_ascii_case(a.name.trim()))
        .ok_or_else(|| CommandError::not_found(format!("a tarefa \"{}\" não existe", a.name)))?;
    let removed = jobs.remove(i);
    Ok(Outcome { data: json!({ "deleted": removed.name }), changes: project_changes(s), ..Default::default() })
}

/// Context for running jobs: relative paths start at the project file's folder (when it is
/// saved and that folder is allowed for this client).
fn job_context(s: &Session, ctx: &ExecContext) -> (ExecContext, Option<PathBuf>, Option<String>) {
    let mut c = ctx.clone();
    let Some(dir) = s.path.as_ref().and_then(|p| p.parent()).filter(|d| !d.as_os_str().is_empty()) else {
        return (c, None, None);
    };
    let dir = if dir.is_absolute() { dir.to_path_buf() } else { ctx.paths.base.join(dir) };
    match ctx.paths.resolve(&dir.to_string_lossy()) {
        Ok(dir) => {
            c.paths.allowed.push(ctx.paths.base.clone());
            c.paths.base = dir.clone();
            (c, Some(dir), None)
        }
        Err(e) => (
            c,
            None,
            Some(format!(
                "a pasta do projeto não é permitida para este cliente; caminhos relativos partem da pasta de trabalho ({})",
                e.message
            )),
        ),
    }
}

pub(crate) fn export_job_list(s: &mut Session, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let (_, dir, note) = job_context(s, ctx);
    let jobs = &s.doc.model.extras.godot.export_jobs;
    let mut out = Outcome::data(json!({
        "jobs": jobs,
        "on_save": jobs.iter().filter(|j| j.on_save).map(|j| j.name.clone()).collect::<Vec<_>>(),
        "base_dir": dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| ctx.paths.base.display().to_string()),
    }));
    if s.path.is_none() && !jobs.is_empty() {
        out.warnings.push("o projeto ainda não foi salvo: caminhos relativos partem da pasta de trabalho".into());
    }
    out.warnings.extend(note);
    Ok(out)
}

pub(crate) fn export_jobs_run(s: &mut Session, a: ExportJobsRun, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let all = s.doc.model.extras.godot.export_jobs.clone();
    let jobs: Vec<ExportJob> = if a.jobs.is_empty() {
        all.into_iter().filter(|j| !a.on_save || j.on_save).collect()
    } else {
        a.jobs
            .iter()
            .map(|n| match all.iter().find(|j| j.name.eq_ignore_ascii_case(n.trim())) {
                None => Err(CommandError::not_found(format!("a tarefa \"{n}\" não existe"))),
                Some(j) if a.on_save && !j.on_save => {
                    Err(CommandError::invalid(format!("a tarefa \"{n}\" não está marcada para rodar ao salvar")))
                }
                Some(j) => Ok(j.clone()),
            })
            .collect::<Result<_, _>>()?
    };
    let (job_ctx, dir, note) = job_context(s, ctx);
    let mut results = Vec::new();
    let mut warnings: Vec<String> = note.into_iter().collect();
    for job in &jobs {
        let run = check_job_command(&job.command)
            .and_then(|_| parse_command(&job.command, serde_json::Value::Object(job.args.clone())))
            .and_then(|cmd| super::run(s, cmd, &job_ctx));
        match run {
            Ok(r) => {
                warnings.extend(r.warnings.iter().map(|w| format!("{}: {w}", job.name)));
                results.push(json!({ "name": job.name, "command": job.command, "ok": true, "data": r.data }));
            }
            Err(e) => {
                warnings.push(format!("a exportação \"{}\" falhou: {}", job.name, e.message));
                results.push(
                    json!({ "name": job.name, "command": job.command, "ok": false, "error": { "code": e.code, "message": e.message } }),
                );
            }
        }
    }
    let failed = results.iter().filter(|r| r["ok"] == false).count();
    let mut out = Outcome::data(json!({
        "ran": results.len(), "failed": failed, "results": results,
        "base_dir": dir.as_ref().map(|d| d.display().to_string()).unwrap_or_else(|| ctx.paths.base.display().to_string()),
    }));
    if jobs.is_empty() {
        out.data["note"] = json!(if a.on_save {
            "nenhuma tarefa marcada para rodar ao salvar"
        } else {
            "o projeto não tem tarefas de exportação; crie com export_job_set"
        });
    }
    out.warnings = warnings;
    Ok(out)
}

#[cfg(test)]
mod tests;
