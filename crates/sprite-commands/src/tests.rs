//! Command-level tests: JSON in, real pixels out.

use serde_json::{Value, json};
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_core::Color;

use super::*;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-cmd-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn session(w: u32, h: u32) -> (Session, ExecContext) {
    let ctx = ExecContext::local(std::env::temp_dir());
    let s = create_project(
        &ProjectCreate {
            name: "teste".into(),
            width: w,
            height: h,
            sprite_name: Some("hero".into()),
            palette: vec![],
            palette_name: None,
            background: None,
        },
        &ctx,
    )
    .unwrap();
    (s, ctx)
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

fn composite(s: &Session, frame: u32) -> sprite_core::Image {
    let sp = &s.doc.model.sprites[0];
    compose_frame(sp, s.doc.images(), frame, &ComposeOptions::default())
}

fn first_frame(s: &Session) -> u32 {
    s.doc.model.sprites[0].frames[0].id
}

#[test]
fn every_spec_name_parses() {
    for spec in specs() {
        match parse_command(spec.name, json!({})) {
            Ok(cmd) => assert_eq!(cmd.name(), spec.name),
            Err(e) => {
                assert_eq!(e.code, ErrorCode::InvalidArgument, "{}: {}", spec.name, e.message);
                assert!(!e.message.contains("unknown variant"), "{}: nome do enum difere do spec", spec.name);
            }
        }
        let schema = serde_json::to_value((spec.schema)()).unwrap();
        assert_eq!(schema["type"], "object", "{} schema", spec.name);
        assert!(!serde_json::to_string(&schema).unwrap().contains("$ref"), "{} schema is inline", spec.name);
    }
    assert_eq!(parse_command("nao_existe", json!({})).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn draw_batch_is_atomic_and_undoable() {
    let (mut s, ctx) = session(16, 16);
    let r0 = s.doc.revision();
    let out = run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 2, "y": 2, "w": 6, "h": 6, "color": "#3976d2", "fill": true },
            { "op": "outline", "color": "#000000" },
            { "op": "pixel", "x": 4, "y": 4, "color": "#ffffff" }
        ]}),
    );
    assert_eq!(out.revision, r0 + 1);
    assert!(out.data["changed_pixels"].as_u64().unwrap() > 36);
    assert_eq!(out.changes[0].kind, "pixels");
    let img = composite(&s, first_frame(&s));
    assert_eq!(img.get(4, 4), Some(Color::WHITE));
    assert_eq!(img.get(1, 2), Some(Color::BLACK));

    // A failing op rolls the whole batch back.
    let before = composite(&s, first_frame(&s));
    let err = run_err(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "fill", "x": 0, "y": 0, "color": "#ff0000" },
            { "op": "pixel", "x": 0, "y": 0, "color": 99 }
        ]}),
    );
    assert_eq!(err.details.as_ref().unwrap()["failed_index"], 1);
    assert_eq!(composite(&s, first_frame(&s)), before);
    assert_eq!(s.doc.revision(), r0 + 1);

    // skip_invalid keeps the good ops.
    let out = run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "policy": "skip_invalid", "ops": [
            { "op": "pixel", "x": 0, "y": 0, "color": "#ff0000" },
            { "op": "pixel", "x": 1, "y": 0, "color": 99 }
        ]}),
    );
    assert_eq!(out.data["skipped"][0]["index"], 1);
    assert_eq!(composite(&s, first_frame(&s)).get(0, 0), Some(Color::rgb(255, 0, 0)));

    let undo_target = s.doc.revision();
    assert!(s.doc.undo().unwrap().is_some());
    assert_eq!(composite(&s, first_frame(&s)), before);
    assert!(s.doc.redo().unwrap().is_some());
    assert!(s.doc.revision() > undo_target);
}

#[test]
fn read_write_region_roundtrip() {
    let (mut s, ctx) = session(8, 8);
    run(
        &mut s,
        &ctx,
        "pixel_write_region",
        json!({ "at": [1, 1], "legend": { "k": "#1a1c2c", "b": "#3976d2", "w": "#ffffff" }, "rows": [".kk.", "kbbk", "kbwk", ".kk."] }),
    );
    let read = run(&mut s, &ctx, "pixel_read_region", json!({ "trim": true }));
    assert_eq!(read.data["rect"], json!({ "x": 1, "y": 1, "w": 4, "h": 4 }));
    let rows: Vec<String> = serde_json::from_value(read.data["rows"].clone()).unwrap();
    assert_eq!(rows.len(), 4);
    // Write the read data elsewhere: identical pixels.
    let legend = read.data["legend"].clone();
    run(&mut s, &ctx, "layer_create", json!({ "name": "copia" }));
    run(&mut s, &ctx, "pixel_write_region", json!({ "layer": "copia", "at": [1, 1], "legend": legend, "rows": rows }));
    let sp = &s.doc.model.sprites[0];
    let f = sp.frames[0].id;
    let a = sprite_animation::compose::layer_image(sp, s.doc.images(), sp.layers[0].id, f);
    let b = sprite_animation::compose::layer_image(sp, s.doc.images(), sp.layers[1].id, f);
    assert_eq!(a, b);
    let px = run(&mut s, &ctx, "pixel_get", json!({ "x": 3, "y": 3, "composite": true }));
    assert_eq!(px.data["color"], "#ffffff");
    assert_eq!(run_err(&mut s, &ctx, "pixel_get", json!({ "x": 30, "y": 3 })).code, ErrorCode::OutOfBounds);
}

#[test]
fn palette_indices_and_patch() {
    let (mut s, ctx) = session(8, 8);
    run(&mut s, &ctx, "palette_create", json!({ "name": "p", "colors": ["#000000", "#ff0000", "#00ff00"] }));
    run(&mut s, &ctx, "pixel_set_batch", json!({ "pixels": [[0, 0], [1, 0]], "color": 1 }));
    run(
        &mut s,
        &ctx,
        "pixel_apply_patch",
        json!({ "patch": { "at": [0, 0], "legend": { "g": 2, "x": "clear" }, "rows": ["gx"], "changes": [[5, 5, 0]] } }),
    );
    let img = composite(&s, first_frame(&s));
    assert_eq!(img.get(0, 0), Some(Color::rgb(0, 255, 0)));
    assert_eq!(img.get(1, 0), Some(Color::TRANSPARENT));
    assert_eq!(img.get(5, 5), Some(Color::BLACK));
    let val = run(&mut s, &ctx, "project_validate", json!({}));
    assert_eq!(val.data["warnings"], 0);
    run(&mut s, &ctx, "pixel_set", json!({ "x": 7, "y": 7, "color": "#123456" }));
    let val = run(&mut s, &ctx, "project_validate", json!({}));
    assert!(val.data["issues"].as_array().unwrap().iter().any(|i| i["code"] == "off_palette"));
}

#[test]
fn animation_with_directions_and_real_pose_changes() {
    let (mut s, ctx) = session(16, 16);
    run(
        &mut s,
        &ctx,
        "draw_shape",
        json!({ "shape": "rect", "rect": { "x": 6, "y": 4, "w": 4, "h": 10 }, "color": "#3976d2", "fill": true }),
    );
    let base = first_frame(&s);
    let out = run(
        &mut s,
        &ctx,
        "animation_create",
        json!({ "name": "walk", "directions": ["E", "W"], "frames": 4, "copy_from": base, "duration_ms": 120 }),
    );
    let east: Vec<u32> = serde_json::from_value(out.data["tracks"][0]["frames"].clone()).unwrap();
    assert_eq!(east.len(), 4);
    // Redraw a leg differently in frame 2 (index 1) of the east track.
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "frame": { "animation": "walk", "direction": "E", "index": 1 }, "ops": [
            { "op": "line", "from": [7, 14], "to": [5, 15], "color": "#000000" }
        ]}),
    );
    let cmp = run(&mut s, &ctx, "frame_compare", json!({ "a": east[0], "b": east[1] }));
    assert!(cmp.data["changed"].as_u64().unwrap() > 0);
    assert!(cmp.data["patch_a_to_b"]["rows"].is_array());
    assert_eq!(cmp.images.len(), 1);
    assert_eq!(&cmp.images[0].data[1..4], b"PNG");
    // Mirror east into NE as an optional starting point.
    let out = run(
        &mut s,
        &ctx,
        "animation_set_direction",
        json!({ "animation": "walk", "direction": "NE", "copy_from": { "direction": "E", "mirror": true } }),
    );
    assert_eq!(out.data["frames"].as_array().unwrap().len(), 4);
    let info = run(&mut s, &ctx, "animation_info", json!({ "animation": "walk", "metrics": true }));
    assert_eq!(info.data["tracks"].as_array().unwrap().len(), 3);
    run(
        &mut s,
        &ctx,
        "animation_set_timing",
        json!({ "animation": "walk", "direction": "E", "durations": [100, 150, 100, 150], "loop_mode": "ping_pong" }),
    );
    let strip = run(&mut s, &ctx, "animation_preview", json!({ "animation": "walk", "direction": "E" }));
    assert_eq!(strip.images[0].mime, "image/png");
    assert_eq!(strip.data["playback_order"].as_array().unwrap().len(), 6);
    let gif = run(&mut s, &ctx, "animation_preview", json!({ "animation": "walk", "direction": "E", "mode": "gif" }));
    assert_eq!(&gif.images[0].data[0..3], b"GIF");
}

#[test]
fn regions_anchors_poses() {
    let (mut s, ctx) = session(16, 16);
    run(&mut s, &ctx, "layer_create", json!({ "name": "braco" }));
    run(
        &mut s,
        &ctx,
        "draw_shape",
        json!({ "layer": "braco", "shape": "rect", "rect": { "x": 10, "y": 6, "w": 2, "h": 5 }, "color": "#e0a272", "fill": true }),
    );
    let f0 = first_frame(&s);
    run(&mut s, &ctx, "region_create", json!({ "name": "braco_dir", "part": "arm_right", "layer": "braco", "from_frame": f0 }));
    run(&mut s, &ctx, "anchor_create", json!({ "name": "ombro_dir", "kind": "joint", "position": [10, 6] }));
    let dup = run(&mut s, &ctx, "frame_duplicate", json!({ "frame": f0 }));
    let f1 = dup.data["frames"][0].as_u64().unwrap() as u32;
    // Move the arm forward in the copy; the region follows.
    run(
        &mut s,
        &ctx,
        "region_transform",
        json!({ "region": "braco_dir", "frames": [f1], "transform": { "kind": "move", "dx": 2, "dy": -1 } }),
    );
    let img = composite(&s, f1);
    assert_eq!(img.get(12, 5).unwrap().a, 255);
    assert!(img.get(10, 10).unwrap().is_transparent());
    let regions = run(&mut s, &ctx, "region_list", json!({ "frame": f1 }));
    assert_eq!(regions.data["regions"][0]["bounds"], json!({ "x": 12, "y": 5, "w": 2, "h": 5 }));
    run(&mut s, &ctx, "anchor_update", json!({ "anchor": "ombro_dir", "positions": [{ "frame": f1, "position": [12, 5] }] }));
    run(&mut s, &ctx, "pose_create", json!({ "name": "passo", "kind": "contact", "frame": f1, "notes": "braço à frente" }));
    let poses = run(&mut s, &ctx, "pose_list", json!({}));
    assert_eq!(poses.data["poses"][0]["anchors"]["ombro_dir"], json!([12, 5]));
    // Copy the region back to frame 0 with clear_destination.
    run(&mut s, &ctx, "region_copy", json!({ "region": "braco_dir", "from_frame": f1, "to_frames": [f0], "clear_destination": true }));
    assert_eq!(composite(&s, f0).get(12, 5).unwrap().a, 255);
    let prev = run(&mut s, &ctx, "frame_preview", json!({ "frame": f1, "anchors": true, "regions": true, "grid": true }));
    assert_eq!(prev.images.len(), 1);
    assert!(prev.data["overlays"].as_array().unwrap().len() >= 2);
}

#[test]
fn layers_merge_reorder_lock() {
    let (mut s, ctx) = session(4, 4);
    run(&mut s, &ctx, "pixel_set", json!({ "x": 0, "y": 0, "color": "#ff0000" }));
    run(&mut s, &ctx, "layer_create", json!({ "name": "cima" }));
    run(&mut s, &ctx, "pixel_set", json!({ "layer": "cima", "x": 1, "y": 0, "color": "#00ff00" }));
    run(&mut s, &ctx, "layer_properties", json!({ "layer": "cima", "locked": true }));
    assert_eq!(run_err(&mut s, &ctx, "pixel_set", json!({ "layer": "cima", "x": 2, "y": 0, "color": "#000" })).code, ErrorCode::Locked);
    run(&mut s, &ctx, "layer_properties", json!({ "layer": "cima", "locked": false }));
    run(&mut s, &ctx, "layer_reorder", json!({ "layer": "cima", "index": 0 }));
    assert_eq!(s.doc.model.sprites[0].layers[0].name, "cima");
    run(&mut s, &ctx, "layer_reorder", json!({ "layer": "cima", "above": "Layer 1" }));
    run(&mut s, &ctx, "layer_merge", json!({ "layer": "cima" }));
    assert_eq!(s.doc.model.sprites[0].layers.len(), 1);
    let img = composite(&s, first_frame(&s));
    assert_eq!(img.get(1, 0), Some(Color::rgb(0, 255, 0)));
    assert_eq!(img.get(0, 0), Some(Color::rgb(255, 0, 0)));
    assert!(run_err(&mut s, &ctx, "layer_delete", json!({ "layer": "Layer 1" })).message.contains("pelo menos"));
}

#[test]
fn export_and_import_files() {
    let dir = temp_dir("io");
    let ctx = ExecContext {
        paths: PathPolicy { base: dir.clone(), allowed: vec![], allow_any: false },
        limits: Limits::default(),
        client: "teste".into(),
    };
    let mut s = create_project(
        &ProjectCreate { name: "io".into(), width: 8, height: 8, sprite_name: None, palette: vec![], palette_name: None, background: None },
        &ctx,
    )
    .unwrap();
    run(&mut s, &ctx, "draw_shape", json!({ "shape": "circle", "center": [4, 4], "radius": 3, "color": "#e0483e", "fill": true }));
    let f0 = first_frame(&s);
    run(&mut s, &ctx, "animation_create", json!({ "name": "idle", "frames": 3, "copy_from": f0 }));
    run(&mut s, &ctx, "export_png", json!({ "path": "frame.png", "scale": 4 }));
    let sheet = run(&mut s, &ctx, "export_spritesheet", json!({ "path": "sheet.png", "sheet": { "layout": "horizontal" } }));
    assert_eq!(sheet.data["size"], json!([24, 8]));
    assert!(dir.join("sheet.json").exists());
    run(&mut s, &ctx, "export_gif", json!({ "path": "idle.gif", "animation": "idle", "scale": 2 }));
    let godot = run(&mut s, &ctx, "export_godot_metadata", json!({ "path": "godot/hero.tres" }));
    assert_eq!(godot.data["texture_res_path"], "res://hero.png");
    let tres = std::fs::read_to_string(dir.join("godot/hero.tres")).unwrap();
    assert!(tres.contains("&\"idle\""));
    let frames = run(&mut s, &ctx, "export_frames", json!({ "directory": "frames" }));
    assert_eq!(frames.data["files"].as_array().unwrap().len(), 3);
    // Path policy blocks escapes.
    assert_eq!(run_err(&mut s, &ctx, "export_png", json!({ "path": "../fora.png" })).code, ErrorCode::PermissionDenied);
    // Import the sheet back, sliced into frames, as a new sprite with an animation.
    let imported = run(
        &mut s,
        &ctx,
        "import_image",
        json!({ "path": "sheet.png", "mode": "new_sprite", "name": "copia", "slice": { "cell_width": 8, "cell_height": 8 }, "animation": "idle" }),
    );
    assert_eq!(imported.data["frames"].as_array().unwrap().len(), 3);
    let copy = &s.doc.model.sprites[1];
    let a = compose_frame(&s.doc.model.sprites[0], s.doc.images(), s.doc.model.sprites[0].frames[1].id, &ComposeOptions::default());
    let b = compose_frame(copy, s.doc.images(), copy.frames[1].id, &ComposeOptions::default());
    assert_eq!(a, b, "import preserves pixels exactly");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn picture_to_pixel_art() {
    let dir = temp_dir("pixelate");
    let ctx = ExecContext::local(dir.clone());
    // A "photo": white background, a red disc with a blue band, 200x160.
    let mut photo = sprite_core::Image::filled(200, 160, Color::WHITE);
    for y in 0..160 {
        for x in 0..200 {
            let (dx, dy) = (x as f64 - 100.0, y as f64 - 80.0);
            if dx * dx + dy * dy <= 60.0 * 60.0 {
                photo.set(x, y, if (70..90).contains(&y) { Color::rgb(30, 60, 200) } else { Color::rgb(210, 40, 40) });
            }
        }
    }
    std::fs::write(dir.join("foto.png"), sprite_core::codec::encode_png(&photo).unwrap()).unwrap();

    // Preview: nothing is created, the image comes back.
    let args: ImagePixelate =
        serde_json::from_value(json!({ "path": "foto.png", "max_side": 24, "colors": 4, "preview": true, "with_source": true })).unwrap();
    let (created, out) = picture::image_pixelate(&args, &ctx).unwrap();
    assert!(created.is_none());
    assert_eq!(out.data["size"], json!([24, 24]), "cropped to the disc, so square");
    assert_eq!(out.data["background_removed"], "#ffffff");
    assert_eq!(out.images.len(), 2);
    assert!(out.data["color_count"].as_u64().unwrap() <= 4);

    // New project: one sprite with the result, its palette active.
    let args: ImagePixelate = serde_json::from_value(json!({ "path": "foto.png", "max_side": 24, "colors": 4, "outline": true })).unwrap();
    let (created, out) = picture::image_pixelate(&args, &ctx).unwrap();
    let mut s = created.expect("projeto novo");
    assert_eq!(s.doc.model.name, "foto");
    let sp = &s.doc.model.sprites[0];
    assert_eq!((sp.width, sp.height), (24, 24));
    assert_eq!(sp.layers[0].name, "pixel art");
    let img = composite(&s, first_frame(&s));
    assert_eq!(img.get(0, 0), Some(Color::TRANSPARENT), "background removed");
    assert_eq!(img.get(12, 12).map(|c| c.b > c.r), Some(true), "blue band in the middle");
    assert_eq!(img.get(12, 4).map(|c| c.r > c.b), Some(true), "red above it");
    let palette = s.doc.model.palette(None).unwrap();
    assert_eq!(serde_json::to_value(&palette.colors).unwrap(), out.data["colors"]);
    assert!(img.color_counts().keys().all(|c| c.is_transparent() || palette.colors.contains(c)));
    assert!(!s.dirty() || s.path.is_none());
    assert!(s.doc.history().0.is_empty(), "a new project starts with a clean history");

    // Import into an existing sprite as a layer: fits the sprite, can be undone.
    let (mut s2, _) = session(16, 12);
    let r = run(
        &mut s2,
        &ctx,
        "import_image",
        json!({ "path": "foto.png", "mode": "new_layer", "name": "foto", "pixelate": { "colors": 3, "dither": "ordered" } }),
    );
    assert_eq!(r.data["pixelate"]["size"], json!([16, 12]));
    assert_eq!(r.data["palette"], "foto (pixel art)");
    let layer = s2.doc.model.sprites[0].layers.iter().find(|l| l.name == "foto").unwrap().id;
    let img = sprite_animation::compose::compose_frame(
        &s2.doc.model.sprites[0],
        s2.doc.images(),
        first_frame(&s2),
        &ComposeOptions { layers: Some(vec![layer]), ..Default::default() },
    );
    assert_eq!(img.get(0, 0), Some(Color::TRANSPARENT));
    assert!(img.opaque_count() > 60);
    assert!(s2.doc.model.palettes.iter().any(|p| p.name == "foto (pixel art)"));
    assert_eq!(
        run_err(
            &mut s2,
            &ctx,
            "import_image",
            json!({ "path": "foto.png", "pixelate": {}, "slice": { "cell_width": 8, "cell_height": 8 } })
        )
        .code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(
        run_err(&mut s2, &ctx, "import_image", json!({ "path": "foto.png", "pixelate": { "colors": 1 } })).code,
        ErrorCode::InvalidArgument
    );
    s2.doc.undo().unwrap();
    assert!(!s2.doc.model.sprites[0].layers.iter().any(|l| l.name == "foto"));
    let _ = (&mut s, std::fs::remove_dir_all(&dir));
}

#[test]
fn explicit_transaction_groups_and_savepoints() {
    let (mut s, ctx) = session(4, 4);
    s.doc.begin("grupo").unwrap();
    run(&mut s, &ctx, "pixel_set", json!({ "x": 0, "y": 0, "color": "#ff0000" }));
    // Failing command inside the transaction leaves earlier work intact.
    assert!(run_err(&mut s, &ctx, "pixel_set", json!({ "x": 9, "y": 0, "color": "#ff0000" })).code == ErrorCode::OutOfBounds);
    run(&mut s, &ctx, "pixel_set", json!({ "x": 1, "y": 0, "color": "#00ff00" }));
    let info = s.doc.commit().unwrap();
    assert_eq!(info.changed_images.len(), 1);
    s.doc.undo().unwrap();
    assert!(composite(&s, first_frame(&s)).is_blank());
}

#[test]
fn sprite_resize_and_snapshots() {
    let (mut s, ctx) = session(8, 8);
    run(&mut s, &ctx, "pixel_set", json!({ "x": 0, "y": 7, "color": "#ffffff" }));
    run(&mut s, &ctx, "snapshot_create", json!({ "name": "antes" }));
    run(&mut s, &ctx, "sprite_update", json!({ "size": [16, 16], "anchor": "bottom_center" }));
    let img = composite(&s, first_frame(&s));
    assert_eq!((img.width(), img.height()), (16, 16));
    assert_eq!(img.get(4, 15), Some(Color::WHITE));
    run(&mut s, &ctx, "sprite_update", json!({ "size": [32, 32], "scale_content": true }));
    assert_eq!(composite(&s, first_frame(&s)).get(9, 31), Some(Color::WHITE));
    run(&mut s, &ctx, "snapshot_restore", json!({ "id": 1 }));
    assert_eq!(s.doc.model.sprites[0].width, 8);
    assert_eq!(composite(&s, first_frame(&s)).get(0, 7), Some(Color::WHITE));
}

#[test]
fn composite_cache_recomposes_only_changed_regions() {
    let (mut s, ctx) = session(16, 16);
    let sprite = s.doc.model.sprites[0].id;
    let f = first_frame(&s);
    run(
        &mut s,
        &ctx,
        "draw_shape",
        json!({ "shape": "rect", "rect": { "x": 2, "y": 2, "w": 8, "h": 8 }, "color": "#3976d2", "fill": true }),
    );
    let first = s.composite(sprite, f).unwrap();
    assert_eq!(first, composite(&s, f));
    run(&mut s, &ctx, "pixel_set", json!({ "x": 5, "y": 5, "color": "#ffffff" }));
    assert_eq!(s.composite(sprite, f).unwrap(), composite(&s, f), "partial update is exact");
    let (_, hits0, partial0, _) = s.cache_stats();
    assert!(hits0 >= 1 && partial0 >= 1, "a pixel change updates the cached frame in place");
    run(&mut s, &ctx, "layer_create", json!({ "name": "cima" }));
    run(&mut s, &ctx, "pixel_set", json!({ "layer": "cima", "x": 3, "y": 3, "color": "#ff000080" }));
    let cached = s.composite(sprite, f).unwrap();
    assert_eq!(cached, composite(&s, f), "cache stays exact after a partial update");
    run(&mut s, &ctx, "layer_properties", json!({ "layer": "cima", "visible": false }));
    assert_eq!(s.composite(sprite, f).unwrap(), composite(&s, f), "structural change invalidates");
    s.doc.undo().unwrap();
    s.invalidate_cache();
    assert_eq!(s.composite(sprite, f).unwrap(), composite(&s, f));
    let (_, hits, _, misses) = s.cache_stats();
    assert!(hits >= 1 && misses >= 1);
}
