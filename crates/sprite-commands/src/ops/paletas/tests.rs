//! Tests of the palette tools: JSON in, real pixels and files out.

use std::path::PathBuf;

use serde_json::{Value, json};
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_core::codec::decode_png;
use sprite_core::{Color, Image};

use crate::*;

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-paletas-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn session(dir: &std::path::Path, w: u32, h: u32) -> (Session, ExecContext) {
    let ctx = ExecContext {
        paths: PathPolicy { base: dir.to_path_buf(), allowed: vec![], allow_any: false },
        limits: Limits::default(),
        client: "teste".into(),
    };
    let s = create_project(
        &ProjectCreate {
            name: "paletas".into(),
            width: w,
            height: h,
            sprite_name: Some("heroi".into()),
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

fn c(hex: &str) -> Color {
    Color::parse(hex).unwrap()
}

fn composite(s: &Session, si: usize, frame: u32) -> Image {
    compose_frame(&s.doc.model.sprites[si], s.doc.images(), frame, &ComposeOptions::default())
}

fn png_file(path: &std::path::Path) -> Image {
    decode_png(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn palette_files_import_and_export() {
    let dir = temp_dir("arquivos");
    let (mut s, ctx) = session(&dir, 8, 8);
    std::fs::write(dir.join("mar.gpl"), "GIMP Palette\nName: Mar\n# fundo\n 26  28  44\tnoite\n 57 118 210\n255 255 255\n").unwrap();
    std::fs::write(dir.join("lospec.hex"), "1a1c2c\n5d275d\n").unwrap();
    std::fs::write(dir.join("jasc.pal"), "JASC-PAL\r\n0100\r\n2\r\n0 0 0\r\n255 0 0\r\n").unwrap();
    std::fs::write(dir.join("pdn.txt"), "; paint.net\nFF00FF00\n80FF0000\n").unwrap();

    let out = run(&mut s, &ctx, "palette_import", json!({ "path": "mar.gpl" }));
    assert_eq!(out.data["palette"], "Mar", "name from the file");
    assert_eq!(s.doc.model.active_palette.as_deref(), Some("Mar"));
    assert_eq!(s.doc.model.palette(Some("Mar")).unwrap().colors, vec![c("#1a1c2c"), c("#3976d2"), c("#ffffff")]);
    // The same file again gets a free name; explicit names collide unless `replace`.
    assert_eq!(run(&mut s, &ctx, "palette_import", json!({ "path": "mar.gpl", "activate": false })).data["palette"], "Mar 2");
    assert_eq!(s.doc.model.active_palette.as_deref(), Some("Mar"));
    let err = run_err(&mut s, &ctx, "palette_import", json!({ "path": "lospec.hex", "name": "mar" }));
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    let out = run(&mut s, &ctx, "palette_import", json!({ "path": "lospec.hex", "name": "Mar 2", "replace": true }));
    assert_eq!(out.data["replaced"], true);
    assert_eq!(s.doc.model.palette(Some("Mar 2")).unwrap().colors, vec![c("#1a1c2c"), c("#5d275d")]);
    assert_eq!(run(&mut s, &ctx, "palette_import", json!({ "path": "jasc.pal" })).data["palette"], "jasc");
    let out = run(&mut s, &ctx, "palette_import", json!({ "path": "pdn.txt" }));
    assert_eq!(out.data["colors"], json!(["#00ff00", "#ff000080"]));

    // Errors say the line (also as details), and unknown extensions list the formats.
    std::fs::write(dir.join("ruim.gpl"), "GIMP Palette\n0 0 0\n1 2 300\n").unwrap();
    let err = run_err(&mut s, &ctx, "palette_import", json!({ "path": "ruim.gpl" }));
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.message.contains("ruim.gpl: linha 3"), "{}", err.message);
    assert_eq!(err.details.unwrap()["line"], 3);
    std::fs::write(dir.join("x.aco"), "x").unwrap();
    assert!(run_err(&mut s, &ctx, "palette_import", json!({ "path": "x.aco" })).message.contains(".gpl"));
    assert_eq!(run_err(&mut s, &ctx, "palette_import", json!({ "path": "nao-existe.hex" })).code, ErrorCode::Io);

    // Export to every format and read back.
    let mar = s.doc.model.palette(Some("Mar")).unwrap().colors.clone();
    for ext in ["gpl", "hex", "pal", "txt", "png", "gif"] {
        let out = run(&mut s, &ctx, "palette_export", json!({ "palette": "Mar", "path": format!("saida/mar.{ext}"), "swatch": 2 }));
        assert_eq!(out.data["count"], 3);
        assert_eq!(out.images.is_empty(), !["png", "gif"].contains(&ext), "{ext}: preview only for pictures");
        let back = run(
            &mut s,
            &ctx,
            "palette_import",
            json!({ "path": format!("saida/mar.{ext}"), "name": format!("volta {ext}"), "activate": false }),
        );
        assert_eq!(back.data["colors"], json!(mar), "{ext}");
    }
    let swatches = png_file(&dir.join("saida/mar.png"));
    assert_eq!((swatches.width(), swatches.height()), (6, 2));
    run(&mut s, &ctx, "palette_export", json!({ "palette": "Mar", "path": "grade.png", "swatch": 4, "columns": 2 }));
    let grid = png_file(&dir.join("grade.png"));
    assert_eq!((grid.width(), grid.height()), (8, 8));
    // A variant can be exported as a palette file too.
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Noite", "base": "Mar", "set": [{ "index": 2, "color": "#a0a0c0" }] }));
    run(&mut s, &ctx, "palette_export", json!({ "variant": "Noite", "path": "noite.hex" }));
    assert_eq!(std::fs::read_to_string(dir.join("noite.hex")).unwrap(), "1a1c2c\n3976d2\na0a0c0\n");
    // Path policy still applies.
    assert_eq!(run_err(&mut s, &ctx, "palette_export", json!({ "path": "../fora.gpl" })).code, ErrorCode::PermissionDenied);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A sprite drawn with the "Azul" palette: a blue body (two tones), white eye, a color outside
/// the palette, and a translucent layer on top.
fn painted(dir: &std::path::Path) -> (Session, ExecContext) {
    let (mut s, ctx) = session(dir, 8, 8);
    run(&mut s, &ctx, "palette_create", json!({ "name": "Azul", "colors": ["#1d4b8f", "#3976d2", "#ffffff"] }));
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 1, "y": 1, "w": 6, "h": 6, "color": 1, "fill": true },
            { "op": "rect", "x": 1, "y": 6, "w": 6, "h": 1, "color": 0, "fill": true },
            { "op": "pixel", "x": 3, "y": 3, "color": 2 },
            { "op": "pixel", "x": 6, "y": 1, "color": "#e0483e" }
        ]}),
    );
    run(&mut s, &ctx, "layer_create", json!({ "name": "brilho" }));
    run(&mut s, &ctx, "layer_properties", json!({ "layer": "brilho", "opacity": 128 }));
    run(&mut s, &ctx, "pixel_set", json!({ "layer": "brilho", "x": 0, "y": 0, "color": "#3976d2" }));
    (s, ctx)
}

#[test]
fn variants_recolor_previews_without_touching_pixels() {
    let dir = temp_dir("variantes");
    let (mut s, ctx) = painted(&dir);
    let frame = s.doc.model.sprites[0].frames[0].id;
    let before = composite(&s, 0, frame);
    let r0 = s.doc.revision();

    // Created from the active palette, then edited by index and by color.
    let out = run(
        &mut s,
        &ctx,
        "palette_variant_set",
        json!({ "name": "Vermelho", "set": [{ "index": 1, "color": "#d23939" }], "replace": [{ "from": "#1d4b8f", "to": "#8f1d1d" }] }),
    );
    assert_eq!(out.data["created"], true);
    assert_eq!(out.data["base"], "Azul");
    assert_eq!(out.data["changed_indices"], json!([0, 1]));
    assert_eq!(s.doc.revision(), r0 + 1, "a write command, undoable");
    let v = s.doc.model.extras.paletas.variant("vermelho").unwrap();
    assert_eq!(v.colors, vec![c("#8f1d1d"), c("#d23939"), c("#ffffff")]);

    // Preview: exact swap on the composite, alpha kept, other colors untouched.
    let out = run(&mut s, &ctx, "palette_variant_preview", json!({ "variant": "Vermelho", "scale": 1, "background": "none" }));
    let img = decode_png(&out.images[0].data).unwrap();
    assert_eq!(img.get(2, 2), Some(c("#d23939")));
    assert_eq!(img.get(2, 6), Some(c("#8f1d1d")));
    assert_eq!(img.get(3, 3), Some(c("#ffffff")));
    assert_eq!(img.get(6, 1), Some(c("#e0483e")), "not in the base palette");
    assert_eq!(before.get(0, 0).unwrap().a, 128);
    assert_eq!(img.get(0, 0), Some(Color::rgba(0xd2, 0x39, 0x39, 128)), "alpha preserved");
    assert_eq!(out.data["unmatched_colors"][0]["color"], "#e0483e");
    assert!(out.warnings.iter().any(|w| w.contains("não estão na paleta")));
    assert_eq!(composite(&s, 0, frame), before, "the project's pixels never change");
    // Side by side, with test colors that are not saved.
    let out = run(
        &mut s,
        &ctx,
        "palette_variant_preview",
        json!({ "colors": ["#000000", "#00ff00", "#ffffff"], "compare": true, "scale": 2, "background": "none" }),
    );
    let img = decode_png(&out.images[0].data).unwrap();
    assert_eq!((img.width(), img.height()), (34, 16));
    assert_eq!(img.get(4, 4), Some(c("#3976d2")), "original on the left");
    assert_eq!(img.get(18 + 4, 4), Some(c("#00ff00")), "test colors on the right");
    assert_eq!(s.doc.model.extras.paletas.variants.len(), 1);

    // Errors: wrong size, unknown color, unknown variant.
    assert!(run_err(&mut s, &ctx, "palette_variant_set", json!({ "name": "x", "colors": ["#000000"] })).message.contains("3 cores"));
    let err = run_err(&mut s, &ctx, "palette_variant_set", json!({ "name": "x", "replace": [{ "from": "#123456", "to": "#000000" }] }));
    assert!(err.message.contains("#123456"), "{}", err.message);
    assert_eq!(run_err(&mut s, &ctx, "palette_variant_preview", json!({ "variant": "nada" })).code, ErrorCode::NotFound);
    assert!(s.doc.model.extras.paletas.variant("x").is_none(), "failed commands leave nothing behind");

    // List, rename, delete, undo.
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Verde", "colors": ["#1d8f4b", "#39d276", "#ffffff"] }));
    let list = run(&mut s, &ctx, "palette_variant_list", json!({}));
    assert_eq!(list.data["count"], 2);
    assert_eq!(list.data["variants"][1]["changed"], 2);
    assert_eq!(list.data["variants"][1]["valid"], true);
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Verde", "rename": "Floresta" }));
    assert!(s.doc.model.extras.paletas.variant("Floresta").is_some());
    run(&mut s, &ctx, "palette_variant_delete", json!({ "name": "floresta" }));
    assert_eq!(s.doc.model.extras.paletas.variants.len(), 1);
    s.doc.undo().unwrap();
    assert!(s.doc.model.extras.paletas.variant("Floresta").is_some(), "delete is undoable");

    // The base palette grows: the list flags it, set adjusts it.
    run(&mut s, &ctx, "palette_update", json!({ "palette": "Azul", "add": ["#000000"] }));
    let list = run(&mut s, &ctx, "palette_variant_list", json!({ "base": "Azul" }));
    assert_eq!(list.data["variants"][0]["valid"], false);
    let out = run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Vermelho" }));
    assert_eq!(out.data["colors"].as_array().unwrap().len(), 4);
    assert!(!out.warnings.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn variants_export_one_file_each_and_persist() {
    let dir = temp_dir("exportar");
    let (mut s, ctx) = painted(&dir);
    let f0 = s.doc.model.sprites[0].frames[0].id;
    run(&mut s, &ctx, "animation_create", json!({ "name": "idle", "frames": 2, "copy_from": f0 }));
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Vermelho", "colors": ["#8f1d1d", "#d23939", "#ffffff"] }));
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Ouro", "colors": ["#8f6b1d", "#d2a939", "#fff4c0"] }));

    // Sprite sheets: same layout as export_spritesheet, recolored, with JSON next to each.
    let out = run(
        &mut s,
        &ctx,
        "export_palette_variants",
        json!({ "directory": "variantes", "sheet": { "layout": "horizontal" }, "include_original": true, "background": "#000000" }),
    );
    let files: Vec<String> = out.data["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap().to_string()).collect();
    assert_eq!(files.len(), 3);
    assert!(
        files[0].ends_with("heroi_Azul.png") && files[1].ends_with("heroi_Vermelho.png") && files[2].ends_with("heroi_Ouro.png"),
        "{files:?}"
    );
    assert!(dir.join("variantes/heroi_Vermelho.json").exists());
    run(
        &mut s,
        &ctx,
        "export_spritesheet",
        json!({ "path": "original.png", "sheet": { "layout": "horizontal" }, "background": "#000000" }),
    );
    let original = png_file(&dir.join("original.png"));
    assert_eq!(png_file(&dir.join("variantes/heroi_Azul.png")), original, "the original export is identical");
    let red = png_file(&dir.join("variantes/heroi_Vermelho.png"));
    assert_eq!((red.width(), red.height()), (original.width(), original.height()));
    assert_eq!(red.get(10, 2), Some(c("#d23939")), "second frame recolored");
    assert_eq!(red.get(8, 7), Some(c("#000000")), "the background stays");
    assert_eq!(out.images.len(), 1, "preview of the first file");
    let meta: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("variantes/heroi_Ouro.json")).unwrap()).unwrap();
    assert_eq!(meta["image"], "heroi_Ouro.png");

    // PNG of a frame, only the chosen variant, with a prefix and scale.
    let out = run(
        &mut s,
        &ctx,
        "export_palette_variants",
        json!({ "directory": "png", "format": "png", "variants": ["ouro"], "prefix": "slime", "scale": 2 }),
    );
    assert_eq!(out.data["files"][0]["changed_pixels"], 36);
    let gold = png_file(&dir.join("png/slime_Ouro.png"));
    assert_eq!((gold.width(), gold.height()), (16, 16));
    assert_eq!(gold.get(6, 6), Some(c("#fff4c0")));

    // GIF of an animation.
    run(&mut s, &ctx, "export_palette_variants", json!({ "directory": "gif", "format": "gif", "animation": "idle" }));
    let frames = sprite_project::import::decode_frames(&std::fs::read(dir.join("gif/heroi_Vermelho.gif")).unwrap()).unwrap();
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[1].image.get(2, 2), Some(c("#d23939")));
    assert_eq!(
        run_err(&mut s, &ctx, "export_palette_variants", json!({ "directory": "gif", "format": "gif" })).code,
        ErrorCode::InvalidArgument
    );

    // A variant whose base colors are not in the sprite warns.
    run(&mut s, &ctx, "palette_create", json!({ "name": "Outra", "colors": ["#010101"], "activate": false }));
    run(&mut s, &ctx, "palette_variant_set", json!({ "name": "Nada", "base": "Outra", "colors": ["#020202"] }));
    let out = run(&mut s, &ctx, "export_palette_variants", json!({ "directory": "nada", "format": "png", "variants": ["Nada"] }));
    assert!(out.warnings.iter().any(|w| w.contains("não mudou nenhum pixel")), "{:?}", out.warnings);

    // Variants are saved with the project.
    let path = dir.join("projeto.psmproj");
    sprite_project::format::save(&s.doc, &path).unwrap();
    let loaded = sprite_project::format::load(&path).unwrap();
    assert_eq!(loaded.model.extras.paletas, s.doc.model.extras.paletas);
    assert_eq!(loaded.model.extras.paletas.variants.len(), 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn aseprite_import_keeps_layers_frames_tags_and_palette() {
    let dir = temp_dir("aseprite");
    let (mut s, ctx) = session(&dir, 16, 16);
    std::fs::write(dir.join("heroi.aseprite"), sprite_project::aseprite::builder::sample()).unwrap();
    let out = run(&mut s, &ctx, "import_image", json!({ "path": "heroi.aseprite", "name": "ase" }));
    assert_eq!(out.data["format"], "aseprite");
    assert_eq!(out.data["linked_cels"], 1);
    assert!(out.warnings.iter().any(|w| w.contains("diferença")), "{:?}", out.warnings);
    let sp = &s.doc.model.sprites[1];
    assert_eq!((sp.name.as_str(), sp.width, sp.height), ("ase", 6, 4));
    let layers: Vec<(&str, bool)> = sp.layers.iter().map(|l| (l.name.as_str(), l.visible)).collect();
    assert_eq!(layers, [("Fundo", true), ("Detalhe", false), ("Personagem", true)]);
    assert_eq!(sp.frames.iter().map(|f| f.duration_ms).collect::<Vec<_>>(), [120, 80]);
    let (f0, f1) = (sp.frames[0].id, sp.frames[1].id);
    let fundo = sp.layers[0].id;
    assert_eq!(sp.cel(fundo, f0).unwrap().image, sp.cel(fundo, f1).unwrap().image, "linked cel shares the image");
    let top = sp.layers[2].id;
    assert_eq!(sp.cel(top, f1).unwrap().opacity, 200);
    assert_eq!(sp.animations.len(), 1);
    assert_eq!(sp.animations[0].name, "anda");
    assert_eq!(sp.animations[0].loop_mode, sprite_animation::LoopMode::PingPong);
    assert_eq!(sp.animations[0].tracks[0].frames, vec![f0, f1]);
    assert_eq!(sp.palette.as_deref(), Some("ase"));
    assert_eq!(s.doc.model.active_palette.as_deref(), Some("ase"));
    assert_eq!(s.doc.model.palette(Some("ase")).unwrap().colors[1], c("#ff0000"));
    let region = &sp.regions[0];
    assert_eq!(region.name, "mao");
    assert_eq!(region.shape(f0), Some(&sprite_animation::RegionShape::Rect { rect: sprite_core::Rect::new(2, 1, 2, 2) }));
    assert_eq!(region.shape(f1), Some(&sprite_animation::RegionShape::Rect { rect: sprite_core::Rect::new(3, 1, 2, 2) }));
    // Pixels: what the editor composes equals Aseprite's composite.
    let bg = c("#102030");
    let img0 = composite(&s, 1, f0);
    assert_eq!(img0.get(0, 0), Some(bg));
    assert_eq!(img0.get(2, 1), Some(c("#ff0000")));
    assert_eq!(img0.get(3, 1), Some(bg), "transparent index on a normal layer");
    assert_eq!(img0.get(3, 2), Some(c("#00ff00")));
    assert_eq!(img0.get(1, 1), Some(bg), "hidden by its group");
    let file = sprite_project::aseprite::parse(&sprite_project::aseprite::builder::sample()).unwrap();
    let (flat, _) = file.flat_layers();
    assert_eq!(composite(&s, 1, f1), file.composite(1, &flat));
    assert_eq!(composite(&s, 1, f1).get(3, 1), Some(c("#0000ff").with_opacity(200).over(bg)));
    // Undo removes the whole import in one step.
    s.doc.undo().unwrap();
    assert_eq!(s.doc.model.sprites.len(), 1);

    // As frames of an existing sprite: the composed images, with a warning.
    let out = run(&mut s, &ctx, "import_image", json!({ "path": "heroi.aseprite", "mode": "frames" }));
    assert!(out.warnings.iter().any(|w| w.contains("achatado")));
    let ids: Vec<u32> = out.data["frames"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(composite(&s, 0, ids[0]).get(2, 1), Some(c("#ff0000")));
    // A wrong file with the extension gets a clear error.
    std::fs::write(dir.join("falso.ase"), b"isto nao e aseprite").unwrap();
    let err = run_err(&mut s, &ctx, "import_image", json!({ "path": "falso.ase" }));
    assert!(err.message.contains("Aseprite"), "{}", err.message);
    // As a reference image.
    let out = run(&mut s, &ctx, "import_image", json!({ "path": "heroi.aseprite", "mode": "reference" }));
    assert_eq!(out.data["size"], json!([6, 4]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn normal_maps_match_frames_and_sheets() {
    let dir = temp_dir("normal");
    let (mut s, ctx) = session(&dir, 12, 12);
    run(
        &mut s,
        &ctx,
        "draw_shape",
        json!({ "shape": "rect", "rect": { "x": 1, "y": 1, "w": 10, "h": 10 }, "color": "#3976d2", "fill": true }),
    );
    let f0 = s.doc.model.sprites[0].frames[0].id;
    run(&mut s, &ctx, "animation_create", json!({ "name": "idle", "frames": 2, "copy_from": f0 }));

    let out = run(&mut s, &ctx, "export_normal_map", json!({ "path": "normal.png" }));
    assert_eq!(out.data["convention"], "opengl");
    assert_eq!(out.images.len(), 1, "preview");
    let n = png_file(&dir.join("normal.png"));
    assert_eq!(n.get(6, 6), Some(Color::rgb(128, 128, 255)), "flat inside");
    assert_eq!(n.get(0, 0), Some(Color::TRANSPARENT), "alpha copied");
    assert!(n.get(1, 6).unwrap().r < 128, "left edge slopes left");
    assert!(n.get(6, 1).unwrap().g > 128, "top edge faces up (OpenGL)");
    run(&mut s, &ctx, "export_normal_map", json!({ "path": "dx.png", "flip_y": true }));
    assert!(png_file(&dir.join("dx.png")).get(6, 1).unwrap().g < 128, "DirectX: green down");

    // Sheet layout: same size and same transparency as the exported sheet, pixel by pixel.
    let sheet = json!({ "layout": "grid", "columns": 2, "spacing": 1, "trim": true });
    run(&mut s, &ctx, "export_spritesheet", json!({ "path": "sheet.png", "sheet": sheet, "scale": 2 }));
    let out =
        run(&mut s, &ctx, "export_normal_map", json!({ "path": "sheet_n.png", "sheet": sheet, "scale": 2, "mode": "mix", "strength": 2 }));
    assert_eq!(out.data["layout"], "sheet");
    let (tex, nm) = (png_file(&dir.join("sheet.png")), png_file(&dir.join("sheet_n.png")));
    assert_eq!((tex.width(), tex.height()), (nm.width(), nm.height()));
    assert!(tex.pixels().iter().zip(nm.pixels()).all(|(a, b)| a.a == b.a), "same alpha everywhere");
    // Validation.
    assert_eq!(run_err(&mut s, &ctx, "export_normal_map", json!({ "path": "x.png", "bevel": 0 })).code, ErrorCode::InvalidArgument);
    assert_eq!(run_err(&mut s, &ctx, "export_normal_map", json!({ "path": "x.png", "strength": -1 })).code, ErrorCode::InvalidArgument);
    let _ = std::fs::remove_dir_all(&dir);
}
