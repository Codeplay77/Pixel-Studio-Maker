//! Godot area tests: JSON commands in, real model data, pixels and files out.

use std::path::PathBuf;

use serde_json::{Value, json};
use sprite_core::Color;
use sprite_core::codec::decode_png;

use crate::*;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-godot-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn strict(dir: &std::path::Path) -> ExecContext {
    ExecContext {
        paths: PathPolicy { base: dir.to_path_buf(), allowed: vec![], allow_any: false },
        limits: Limits::default(),
        client: "teste".into(),
    }
}

fn session(ctx: &ExecContext) -> Session {
    create_project(
        &ProjectCreate {
            name: "godot".into(),
            width: 16,
            height: 16,
            sprite_name: Some("hero".into()),
            palette: vec![],
            palette_name: None,
            background: None,
        },
        ctx,
    )
    .unwrap()
}

fn run(s: &mut Session, ctx: &ExecContext, name: &str, args: Value) -> Outcome {
    let cmd = parse_command(name, args).unwrap_or_else(|e| panic!("{name}: {e}"));
    s.execute(cmd, ctx).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn run_err(s: &mut Session, ctx: &ExecContext, name: &str, args: Value) -> CommandError {
    match parse_command(name, args) {
        Err(e) => e,
        Ok(cmd) => s.execute(cmd, ctx).expect_err("devia falhar"),
    }
}

/// A 16x16 hero with a body and an `attack` animation (3 frames, East) copied from it.
fn hero(ctx: &ExecContext) -> (Session, Vec<u32>) {
    let mut s = session(ctx);
    run(
        &mut s,
        ctx,
        "draw_shape",
        json!({ "shape": "rect", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "color": "#3976d2", "fill": true }),
    );
    let base = s.doc.model.sprites[0].frames[0].id;
    let out = run(
        &mut s,
        ctx,
        "animation_create",
        json!({ "name": "attack", "directions": ["E"], "frames": 3, "copy_from": base, "loop_mode": "once" }),
    );
    let frames: Vec<u32> = out.data["tracks"][0]["frames"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
    (s, frames)
}

fn boxes(s: &Session) -> &Vec<sprite_animation::extras::godot::Hitbox> {
    &s.doc.model.sprites[0].extras.godot.hitboxes
}

#[test]
fn hitboxes_per_frame_and_per_track() {
    let ctx = ExecContext::local(std::env::temp_dir());
    let (mut s, f) = hero(&ctx);
    let made = run(
        &mut s,
        &ctx,
        "hitbox_create",
        json!({ "name": "golpe", "kind": "hitbox", "rect": { "x": 11, "y": 6, "w": 4, "h": 3 }, "frame": { "animation": "attack", "direction": "E", "index": 1 } }),
    );
    assert_eq!(made.data["hitbox"]["kind"], "hitbox");
    assert_eq!(made.data["frames"], json!([f[1]]));
    // A whole track at once, then switched off in the last frame.
    run(&mut s, &ctx, "hitbox_create", json!({ "name": "corpo", "kind": "hurtbox", "color": "#00ff00" }));
    let set = run(
        &mut s,
        &ctx,
        "hitbox_set",
        json!({ "hitbox": "corpo", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "animation": "attack", "direction": "E" }),
    );
    assert_eq!(set.data["frames"], json!(f));
    run(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "corpo", "clear": true, "frames": [f[2]] }));
    let corpo = &boxes(&s)[1];
    assert_eq!(corpo.at(f[0]), Some(sprite_core::Rect::new(5, 3, 6, 13)));
    assert_eq!(corpo.at(f[2]), None, "desligada no último frame");

    let list = run(&mut s, &ctx, "hitbox_list", json!({ "frame": f[1] }));
    assert_eq!(list.data["hitboxes"][0]["rect"], json!({ "x": 11, "y": 6, "w": 4, "h": 3 }));
    assert_eq!(list.data["hitboxes"][1]["color"], "#00ff00");
    let all = run(&mut s, &ctx, "hitbox_list", json!({ "hitbox": "golpe" }));
    assert_eq!(all.data["hitboxes"].as_array().unwrap().len(), 1);
    assert_eq!(all.data["hitboxes"][0]["rects"][f[1].to_string()]["w"], 4);

    // Out of the canvas: allowed, with a warning.
    let out = run(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "golpe", "rect": { "x": 14, "y": 6, "w": 6, "h": 3 }, "frame": f[1] }));
    assert!(out.warnings.iter().any(|w| w.contains("borda")), "{:?}", out.warnings);
    assert!(!run(&mut s, &ctx, "hitbox_list", json!({})).warnings.is_empty());

    // Rename, recolor, retype, delete.
    run(&mut s, &ctx, "hitbox_update", json!({ "hitbox": "golpe", "name": "espada", "kind": "custom", "color": "#ffcc00" }));
    assert_eq!(boxes(&s)[0].name, "espada");
    assert_eq!(boxes(&s)[0].kind, sprite_animation::extras::godot::HitboxKind::Custom);

    // Errors.
    assert_eq!(run_err(&mut s, &ctx, "hitbox_create", json!({ "name": "CORPO" })).code, ErrorCode::InvalidArgument);
    assert_eq!(
        run_err(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "nada", "clear": true, "all_frames": true })).code,
        ErrorCode::NotFound
    );
    assert!(run_err(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "corpo", "clear": true })).message.contains("quais frames"));
    assert!(
        run_err(
            &mut s,
            &ctx,
            "hitbox_set",
            json!({ "hitbox": "corpo", "clear": true, "rect": { "x": 0, "y": 0, "w": 1, "h": 1 }, "all_frames": true })
        )
        .message
        .contains("não os dois")
    );
    assert!(
        run_err(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "corpo", "rect": { "x": 0, "y": 0, "w": 0, "h": 1 }, "all_frames": true }))
            .message
            .contains("largura")
    );
    assert!(
        run_err(&mut s, &ctx, "hitbox_create", json!({ "name": "x", "rect": { "x": 0, "y": 0, "w": 2, "h": 2 } }))
            .message
            .contains("diga onde")
    );
    assert_eq!(
        run_err(&mut s, &ctx, "hitbox_set", json!({ "hitbox": "corpo", "clear": true, "animation": "attack", "direction": "N" })).code,
        ErrorCode::NotFound
    );

    // Undo brings the box back.
    run(&mut s, &ctx, "hitbox_update", json!({ "hitbox": "espada", "delete": true }));
    assert_eq!(boxes(&s).len(), 1);
    s.doc.undo().unwrap();
    assert_eq!(boxes(&s).len(), 2);
}

#[test]
fn duplicated_frames_carry_their_boxes() {
    let ctx = ExecContext::local(std::env::temp_dir());
    let (mut s, f) = hero(&ctx);
    run(&mut s, &ctx, "hitbox_create", json!({ "name": "golpe", "rect": { "x": 10, "y": 6, "w": 4, "h": 3 }, "frame": f[1] }));
    let dup = run(&mut s, &ctx, "frame_duplicate", json!({ "frame": f[1], "count": 2 }));
    let copies: Vec<u32> = dup.data["frames"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
    for c in &copies {
        assert_eq!(boxes(&s)[0].at(*c), Some(sprite_core::Rect::new(10, 6, 4, 3)));
    }
    let mirrored = run(&mut s, &ctx, "frame_duplicate", json!({ "frame": f[1], "mirror": true }));
    let m = mirrored.data["frames"][0].as_u64().unwrap() as u32;
    assert_eq!(boxes(&s)[0].at(m), Some(sprite_core::Rect::new(2, 6, 4, 3)), "16 - 10 - 4 = 2");
    // New animation copied from a frame with a box.
    let made = run(&mut s, &ctx, "animation_create", json!({ "name": "hold", "frames": 2, "copy_from": f[1] }));
    for v in made.data["tracks"][0]["frames"].as_array().unwrap() {
        assert!(boxes(&s)[0].at(v.as_u64().unwrap() as u32).is_some());
    }
    // A mirrored direction copied from E.
    run(
        &mut s,
        &ctx,
        "animation_set_direction",
        json!({ "animation": "attack", "direction": "W", "copy_from": { "direction": "E", "mirror": true } }),
    );
    let w = s.doc.model.sprites[0].animations[0].track(Some("W")).unwrap().frames.clone();
    assert_eq!(boxes(&s)[0].at(w[1]), Some(sprite_core::Rect::new(2, 6, 4, 3)));
    assert_eq!(boxes(&s)[0].at(w[0]), None);
    // Deleted frames drop their rectangles.
    run(&mut s, &ctx, "frame_delete", json!({ "frames": copies }));
    assert!(copies.iter().all(|c| boxes(&s)[0].at(*c).is_none()));
    // Canvas changes move the boxes with the pixels.
    run(&mut s, &ctx, "sprite_update", json!({ "size": [20, 20], "anchor": "top_left" }));
    assert_eq!(boxes(&s)[0].at(f[1]), Some(sprite_core::Rect::new(10, 6, 4, 3)));
    run(&mut s, &ctx, "sprite_update", json!({ "size": [40, 40], "scale_content": true }));
    assert_eq!(boxes(&s)[0].at(f[1]), Some(sprite_core::Rect::new(20, 12, 8, 6)));
}

#[test]
fn hitboxes_survive_save_and_open() {
    let dir = temp_dir("persist");
    let ctx = ExecContext::local(&dir);
    let (mut s, f) = hero(&ctx);
    run(
        &mut s,
        &ctx,
        "hitbox_create",
        json!({ "name": "corpo", "kind": "hurtbox", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "all_frames": true }),
    );
    run(
        &mut s,
        &ctx,
        "export_job_set",
        json!({ "name": "sheet", "command": "export_spritesheet", "args": { "path": "out/sheet.png" }, "on_save": true }),
    );
    let path = dir.join("hero.psmproj");
    sprite_project::format::save(&s.doc, &path).unwrap();
    let back = sprite_project::format::load(&path).unwrap();
    assert_eq!(back.model, s.doc.model);
    let hb = &back.model.sprites[0].extras.godot.hitboxes[0];
    assert_eq!(hb.kind, sprite_animation::extras::godot::HitboxKind::Hurtbox);
    assert_eq!(hb.at(f[2]), Some(sprite_core::Rect::new(5, 3, 6, 13)));
    assert_eq!(back.model.extras.godot.export_jobs[0].command, "export_spritesheet");
    // Projects without Godot data keep their manifest free of it.
    let plain = session(&ctx);
    let json = serde_json::to_string(&plain.doc.model).unwrap();
    assert!(!json.contains("extras") && !json.contains("hitboxes") && !json.contains("export_jobs"), "{json}");
}

#[test]
fn frame_preview_draws_the_boxes() {
    let ctx = ExecContext::local(std::env::temp_dir());
    let (mut s, f) = hero(&ctx);
    run(
        &mut s,
        &ctx,
        "hitbox_create",
        json!({ "name": "golpe", "color": "#ff00ff", "rect": { "x": 11, "y": 6, "w": 4, "h": 3 }, "frame": f[1] }),
    );
    let plain = run(&mut s, &ctx, "frame_preview", json!({ "frame": f[1], "scale": 4, "background": "none" }));
    let with = run(&mut s, &ctx, "frame_preview", json!({ "frame": f[1], "scale": 4, "background": "none", "hitboxes": true }));
    let a = decode_png(&plain.images[0].data).unwrap();
    let b = decode_png(&with.images[0].data).unwrap();
    // Top-left corner of the box (11, 6) at 4x is (44, 24): outline color; inside: tinted.
    assert_eq!(b.get(44, 24), Some(Color::rgb(255, 0, 255)));
    assert_eq!(b.get(59, 35), Some(Color::rgb(255, 0, 255)), "bottom-right corner");
    assert_eq!(a.get(44, 24), Some(Color::TRANSPARENT));
    assert_ne!(b.get(50, 29), a.get(50, 29), "light fill inside");
    assert_eq!(b.get(40, 24), a.get(40, 24), "outside untouched");
    assert_eq!(with.data["overlays"][0]["hitbox"], "golpe");
    // Other frames: no box, nothing drawn. A cropped view shifts the box.
    let other = run(&mut s, &ctx, "frame_preview", json!({ "frame": f[0], "scale": 4, "background": "none", "hitboxes": true }));
    assert_eq!(other.data["overlays"], json!([]));
    let crop = run(
        &mut s,
        &ctx,
        "frame_preview",
        json!({ "frame": f[1], "scale": 2, "background": "none", "hitboxes": true, "rect": { "x": 8, "y": 4, "w": 8, "h": 8 } }),
    );
    let c = decode_png(&crop.images[0].data).unwrap();
    assert_eq!(c.get(6, 4), Some(Color::rgb(255, 0, 255)), "(11-8)*2, (6-4)*2");
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn export_godot_scene_writes_a_consistent_scene() {
    let dir = temp_dir("scene");
    let ctx = strict(&dir);
    let (mut s, f) = hero(&ctx);
    run(&mut s, &ctx, "hitbox_create", json!({ "name": "golpe", "rect": { "x": 11, "y": 6, "w": 4, "h": 3 }, "frame": f[1] }));
    run(
        &mut s,
        &ctx,
        "hitbox_create",
        json!({ "name": "corpo", "kind": "hurtbox", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "all_frames": true }),
    );
    run(&mut s, &ctx, "anchor_create", json!({ "name": "mao", "kind": "attachment", "position": [12, 8] }));
    run(&mut s, &ctx, "animation_set_timing", json!({ "animation": "attack", "durations": [100, 150, 50] }));
    let out = run(&mut s, &ctx, "export_godot_scene", json!({ "path": "godot/hero", "res_dir": "res://chars/hero", "scale": 2 }));
    assert!(dir.join("godot/hero.tscn").exists() && dir.join("godot/hero.tres").exists() && dir.join("godot/hero.png").exists());
    assert_eq!(out.data["res"]["sprite_frames"], "res://chars/hero/hero.tres");
    assert_eq!(out.images.len(), 1, "sheet preview");
    let tres = read(dir.join("godot/hero.tres"));
    assert!(tres.contains("path=\"res://chars/hero/hero.png\""));
    assert!(tres.contains("&\"attack_E\""));
    let tscn = read(dir.join("godot/hero.tscn"));
    // Resources: 1 ext + 2 shapes + 1 animation + library.
    assert!(tscn.starts_with("[gd_scene load_steps=6 format=3]"), "{tscn}");
    assert!(tscn.contains("[ext_resource type=\"SpriteFrames\" path=\"res://chars/hero/hero.tres\" id=\"1_frames\"]"));
    assert!(tscn.contains("[node name=\"hero\" type=\"Node2D\"]"));
    // Pivot = default origin (8, 15), scaled 2x.
    assert!(tscn.contains("centered = false\noffset = Vector2(-16, -30)"));
    assert!(tscn.contains("resource_name = \"attack_E\"\nlength = 0.3\nloop_mode = 0"));
    assert!(tscn.contains("\"times\": PackedFloat32Array(0, 0.1, 0.25)"));
    // golpe: center (13, 7.5) - pivot (8, 15) = (5, -7.5), x2.
    assert!(tscn.contains("\"values\": [Vector2(10, -15)]"), "{tscn}");
    assert!(tscn.contains("\"values\": [true, false, true]"));
    assert!(tscn.contains("NodePath(\"mao:position\")"));
    assert!(tscn.contains("[node name=\"mao\" type=\"Marker2D\" parent=\".\"]"));
    assert!(tscn.contains("groups=[\"hurtbox\"]"));
    // Every referenced id is declared once.
    let declared: Vec<&str> = tscn
        .lines()
        .filter(|l| l.starts_with("[sub_resource") || l.starts_with("[ext_resource"))
        .map(|l| l.split("id=\"").nth(1).unwrap().split('"').next().unwrap())
        .collect();
    assert_eq!(declared.len(), 5);
    for part in tscn.split("SubResource(\"").skip(1).chain(tscn.split("ExtResource(\"").skip(1)) {
        assert!(declared.contains(&part.split('"').next().unwrap()));
    }
    let names: Vec<&str> = out.data["animations"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["attack_E"]);
    // Inside a Godot project the res:// paths follow the folder.
    std::fs::create_dir_all(dir.join("jogo/chars")).unwrap();
    std::fs::write(dir.join("jogo/project.godot"), "config_version=5\n").unwrap();
    let inside = run(&mut s, &ctx, "export_godot_scene", json!({ "path": "jogo/chars/hero.tscn" }));
    assert_eq!(inside.data["res"]["scene"], "res://chars/hero.tscn");
    assert_eq!(inside.data["godot_project"], dir.join("jogo").display().to_string());
    assert!(read(dir.join("jogo/chars/hero.tscn")).contains("path=\"res://chars/hero.tres\""));
    assert!(read(dir.join("jogo/chars/hero.tres")).contains("path=\"res://chars/hero.png\""));
    let root = run(&mut s, &ctx, "export_godot_scene", json!({ "path": "jogo/hero.tscn" }));
    assert_eq!(root.data["res"]["sprite_frames"], "res://hero.tres");
    // Wrong extension and path policy.
    assert!(run_err(&mut s, &ctx, "export_godot_scene", json!({ "path": "hero.tres" })).message.contains(".tscn"));
    assert_eq!(run_err(&mut s, &ctx, "export_godot_scene", json!({ "path": "../fora.tscn" })).code, ErrorCode::PermissionDenied);
}

#[test]
fn export_jobs_validate_run_relative_to_the_project_and_report_failures() {
    let dir = temp_dir("jobs");
    std::fs::create_dir_all(dir.join("projeto")).unwrap();
    let ctx = strict(&dir);
    let (mut s, _) = hero(&ctx);
    // Only read-only export commands, with valid arguments.
    assert!(
        run_err(&mut s, &ctx, "export_job_set", json!({ "name": "x", "command": "pixel_set", "args": {} })).message.contains("exportação")
    );
    assert!(run_err(&mut s, &ctx, "export_job_set", json!({ "name": "x", "command": "export_jobs_run" })).message.contains("exportação"));
    assert_eq!(
        run_err(&mut s, &ctx, "export_job_set", json!({ "name": "x", "command": "export_png", "args": { "scale": 2 } })).code,
        ErrorCode::InvalidArgument
    );
    assert!(run_err(&mut s, &ctx, "export_job_set", json!({ "name": "x" })).message.contains("command"));

    run(
        &mut s,
        &ctx,
        "export_job_set",
        json!({ "name": "cena", "command": "export_godot_scene", "args": { "path": "godot/hero.tscn" }, "on_save": true }),
    );
    run(&mut s, &ctx, "export_job_set", json!({ "name": "png", "command": "export_png", "args": { "path": "png/hero.png", "scale": 2 } }));
    run(
        &mut s,
        &ctx,
        "export_job_set",
        json!({ "name": "gif", "command": "export_gif", "args": { "path": "hero.gif", "animation": "nao_existe" } }),
    );
    // Update keeps what is not given.
    let upd = run(&mut s, &ctx, "export_job_set", json!({ "name": "PNG", "on_save": true }));
    assert_eq!(upd.data["created"], false);
    assert_eq!(upd.data["job"]["args"]["scale"], 2);
    let list = run(&mut s, &ctx, "export_job_list", json!({}));
    assert_eq!(list.data["on_save"], json!(["cena", "png"]));
    assert!(!list.warnings.is_empty(), "projeto ainda sem arquivo");

    // The project file lives in dir/projeto: relative paths start there.
    s.path = Some(dir.join("projeto/hero.psmproj"));
    let r = run(&mut s, &ctx, "export_jobs_run", json!({ "on_save": true }));
    assert_eq!(r.data["ran"], 2);
    assert_eq!(r.data["failed"], 0);
    assert!(dir.join("projeto/godot/hero.tscn").exists());
    assert!(dir.join("projeto/png/hero.png").exists());
    assert!(!dir.join("projeto/hero.gif").exists(), "only on_save jobs");
    assert_eq!(r.data["base_dir"], dir.join("projeto").display().to_string());
    // All jobs: one fails, the others still run, and the failure is reported.
    let all = run(&mut s, &ctx, "export_jobs_run", json!({}));
    assert_eq!(all.data["ran"], 3);
    assert_eq!(all.data["failed"], 1);
    assert_eq!(all.data["results"][2]["ok"], false);
    assert!(all.data["results"][2]["error"]["message"].as_str().unwrap().contains("nao_existe"));
    assert!(all.warnings.iter().any(|w| w.contains("\"gif\" falhou")));
    // Named jobs, in the given order; unknown names fail before anything runs.
    let named = run(&mut s, &ctx, "export_jobs_run", json!({ "jobs": ["png"] }));
    assert_eq!(named.data["results"][0]["name"], "png");
    assert_eq!(run_err(&mut s, &ctx, "export_jobs_run", json!({ "jobs": ["png", "nada"] })).code, ErrorCode::NotFound);
    // Paths still obey the client's policy.
    run(&mut s, &ctx, "export_job_set", json!({ "name": "fora", "command": "export_png", "args": { "path": "../../fora.png" } }));
    let escaped = run(&mut s, &ctx, "export_jobs_run", json!({ "jobs": ["fora"] }));
    assert_eq!(escaped.data["results"][0]["error"]["code"], "PERMISSION_DENIED");
    // Delete.
    run(&mut s, &ctx, "export_job_delete", json!({ "name": "fora" }));
    assert_eq!(s.doc.model.extras.godot.export_jobs.len(), 3);
    assert_eq!(run_err(&mut s, &ctx, "export_job_delete", json!({ "name": "fora" })).code, ErrorCode::NotFound);
    // Nothing to run is not an error.
    let mut empty = session(&ctx);
    let none = run(&mut empty, &ctx, "export_jobs_run", json!({}));
    assert_eq!(none.data["ran"], 0);
}
