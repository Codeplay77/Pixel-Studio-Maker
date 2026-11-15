//! Command-level tests of the text tools: JSON in, real pixels and files out.

use std::path::PathBuf;

use serde_json::{Value, json};
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_core::codec::decode_png;
use sprite_core::{Color, Image, Point, Rect};
use sprite_drawing::Canvas;
use sprite_drawing::sdc::ColorRef;
use sprite_drawing::text::{self, TextSpec, builtin_font};

use crate::{CommandError, ErrorCode, ExecContext, Outcome, ProjectCreate, Session, create_project, parse_command};

const RED: Color = Color::rgb(255, 0, 0);

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-texto-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn session(w: u32, h: u32) -> (Session, ExecContext) {
    let ctx = ExecContext::local(std::env::temp_dir());
    let s = create_project(
        &ProjectCreate {
            name: "texto".into(),
            width: w,
            height: h,
            sprite_name: Some("hud".into()),
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

fn composite(s: &Session, sprite: usize) -> Image {
    let sp = &s.doc.model.sprites[sprite];
    compose_frame(sp, s.doc.images(), sp.frames[0].id, &ComposeOptions::default())
}

/// The same text drawn directly with the drawing engine, for comparison.
fn reference(w: u32, h: u32, spec: &TextSpec) -> Image {
    let mut img = Image::new(w, h);
    let mut c = Canvas::new(&mut img, None);
    text::draw(&mut c, spec, builtin_font("pixel-5x7").unwrap(), &[]).unwrap();
    img
}

fn rect(v: &Value) -> Rect {
    serde_json::from_value(v.clone()).unwrap()
}

#[test]
fn draw_text_paints_measures_and_undoes() {
    let (mut s, ctx) = session(64, 32);
    let measured = run(&mut s, &ctx, "text_measure", json!({ "text": "Olá!", "at": [2, 3], "outline": "#000000" }));
    let out = run(&mut s, &ctx, "draw_text", json!({ "text": "Olá!", "at": [2, 3], "color": "#ff0000", "outline": "#000000" }));
    assert_eq!(out.data["box"], measured.data["box"]);
    assert_eq!(out.data["lines"], measured.data["lines"]);
    // On an empty layer the changed area is exactly the measured ink.
    assert_eq!(rect(&out.data["region"]), rect(&measured.data["ink"]));
    let b = rect(&measured.data["box"]);
    assert_eq!((b.x, b.y, b.h), (2, 3, 13));
    let mut spec = TextSpec::new("Olá!", Point::new(2, 3));
    spec.color = Some(ColorRef::Value("#ff0000".into()));
    spec.outline = Some(ColorRef::Value("#000000".into()));
    assert_eq!(composite(&s, 0), reference(64, 32, &spec));
    // One undo step.
    s.doc.undo().unwrap();
    assert!(composite(&s, 0).is_blank());
}

#[test]
fn draw_text_wraps_aligns_and_warns() {
    let (mut s, ctx) = session(64, 40);
    let out = run(
        &mut s,
        &ctx,
        "draw_text",
        json!({ "text": "AB CD €", "at": [0, 0], "color": "#ff0000", "max_width": 26, "align": "right", "font": "pixel-5x7" }),
    );
    let lines = out.data["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["text"], "AB CD");
    assert_eq!(lines[1]["text"], "€");
    assert_eq!(lines[1]["y"], 12);
    // Right-aligned inside 26 px: the replacement box (4 px) ends at column 25.
    assert_eq!(lines[1]["x"], 22);
    assert!(out.warnings.iter().any(|w| w.contains("'€'")), "{:?}", out.warnings);
    assert_eq!(out.data["missing"], json!(["€"]));
    let img = composite(&s, 0);
    assert_eq!(img.get(25, 14), Some(RED));
    assert_eq!(img.get(26, 14), Some(Color::TRANSPARENT));
    // Errors: built-in font without color, unknown font, empty text.
    assert!(run_err(&mut s, &ctx, "draw_text", json!({ "text": "x" })).message.contains("color"));
    assert_eq!(run_err(&mut s, &ctx, "draw_text", json!({ "text": "x", "color": 1, "font": "Nada" })).code, ErrorCode::NotFound);
    assert!(run_err(&mut s, &ctx, "draw_text", json!({ "text": "", "color": "#fff" })).message.contains("vazio"));
}

#[test]
fn text_op_in_a_batch_uses_sprite_fonts_and_warns() {
    let (mut s, ctx) = session(48, 24);
    run(&mut s, &ctx, "font_template", json!({ "name": "Minha Fonte", "base_font": "pixel-5x7", "color": "#00ff00" }));
    let out = run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "sprite": "hud", "ops": [
            { "op": "text", "text": "Ola", "at": [1, 1], "font": "Minha Fonte" },
            { "op": "text", "text": "Ω", "at": [30, 1], "font": "mini-3x5", "color": "#ffffff" }
        ]}),
    );
    assert!(out.warnings.iter().any(|w| w.contains("mini-3x5") && w.contains("'Ω'")), "{:?}", out.warnings);
    // The sprite font keeps the glyph colors (green) and matches the built-in pixel by pixel.
    let mut spec = TextSpec::new("Ola", Point::new(1, 1));
    spec.color = Some(ColorRef::Value("#00ff00".into()));
    let expected = reference(48, 24, &spec);
    let img = composite(&s, 0);
    assert_eq!(img.crop(Rect::new(0, 0, 29, 24)), expected.crop(Rect::new(0, 0, 29, 24)));
    // A sprite that is not a font.
    let err = run_err(&mut s, &ctx, "draw_text", json!({ "text": "x", "font": "hud" }));
    assert!(err.message.contains("font_define"), "{}", err.message);
}

#[test]
fn font_template_builds_a_grid_with_regions() {
    let (mut s, ctx) = session(16, 16);
    let out = run(&mut s, &ctx, "font_template", json!({ "name": "F", "base_font": "mini-3x5", "charset": "AB C", "columns": 2 }));
    assert_eq!(out.data["cell"], json!([6, 10]));
    assert_eq!(out.data["size"], json!([12, 20]));
    assert_eq!(out.data["regions"], 4);
    assert_eq!(out.data["prefilled_glyphs"], 3);
    assert!(!out.images.is_empty());
    let sp = &s.doc.model.sprites[1];
    assert_eq!(sp.name, "F");
    assert_eq!(sp.regions[3].name, "glifo C");
    assert_eq!(sp.regions[2].name, "glifo espaço");
    assert!(sp.layers[0].reference && sp.layers[0].name == "guias", "guias abaixo, como referência");
    let def = sp.extras.texto.font.clone().unwrap();
    assert_eq!((def.baseline, def.line_height, def.space_width), (7, 10, 1));
    // Guides are a reference layer: the composite only has the glyphs (white by default).
    let img = composite(&s, 1);
    assert_eq!(img.get(1, 2), Some(Color::WHITE), "topo do A da mini-3x5 na célula 0");
    assert_eq!(img.get(0, 0), Some(Color::TRANSPARENT));
    assert_eq!(img.get(6, 2), Some(Color::WHITE), "B na célula 1");
    // Too small a cell for the base font.
    let err = run_err(&mut s, &ctx, "font_template", json!({ "name": "G", "base_font": "pixel-5x7", "cell": [4, 8] }));
    assert!(err.message.contains("pequena demais"), "{}", err.message);
    assert_eq!(run_err(&mut s, &ctx, "font_template", json!({ "name": "F" })).code, ErrorCode::InvalidArgument);
}

#[test]
fn font_define_uses_the_real_width_and_the_baseline() {
    let (mut s, ctx) = session(24, 8);
    // Cell 8x8: 'i' is 1 px wide at x = 3, 'w' is 5 px wide; space has an empty cell.
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 3, "y": 1, "w": 1, "h": 5, "color": "#ff0000", "fill": true },
            { "op": "rect", "x": 9, "y": 2, "w": 5, "h": 4, "color": "#0000ff", "fill": true }
        ]}),
    );
    let bad = run_err(&mut s, &ctx, "font_define", json!({ "cell": [8, 8], "charset": "iw iw", "columns": 3 }));
    assert!(bad.message.contains("duas vezes"), "{}", bad.message);
    let bad = run_err(&mut s, &ctx, "font_define", json!({ "cell": [8, 8], "charset": "iw ab", "columns": 3 }));
    assert!(bad.message.contains("não cabe"), "{}", bad.message);
    let out = run(&mut s, &ctx, "font_define", json!({ "cell": [8, 8], "charset": "iw ", "baseline": 6, "space_width": 2 }));
    assert_eq!(out.data["widths"], json!({ "i": 1, "w": 5 }));
    assert_eq!(out.data["font"]["columns"], 3);
    assert!(!out.images.is_empty(), "amostra");
    // Partial update keeps the other fields.
    run(&mut s, &ctx, "font_define", json!({ "letter_spacing": 2 }));
    let info = run(&mut s, &ctx, "font_info", json!({}));
    assert_eq!(info.data["base"], 6);
    assert_eq!(info.data["letter_spacing"], 2);
    assert_eq!(info.data["kind"], "sprite");
    assert_eq!(info.data["charset"], " iw");
    // Drawing "iwi": i at 0, w at 1 + 2, i at 3 + 5 + 2.
    run(&mut s, &ctx, "sprite_create", json!({ "name": "tela", "width": 20, "height": 8 }));
    run(&mut s, &ctx, "draw_text", json!({ "sprite": "tela", "text": "iwi", "font": "hud" }));
    let img = composite(&s, 1);
    assert_eq!(img.get(0, 1), Some(RED));
    assert_eq!(img.get(1, 1), Some(Color::TRANSPARENT));
    assert_eq!(img.get(3, 2), Some(Color::rgb(0, 0, 255)));
    assert_eq!(img.get(7, 5), Some(Color::rgb(0, 0, 255)));
    assert_eq!(img.get(10, 1), Some(RED));
    assert_eq!(img.get(9, 1), Some(Color::TRANSPARENT));
    // Built-in fonts can be inspected too; clearing removes the definition.
    let info = run(&mut s, &ctx, "font_info", json!({ "font": "mini-3x5", "widths": false }));
    assert_eq!(info.data["missing_standard"], json!([]));
    assert!(info.data.get("widths").is_none());
    run(&mut s, &ctx, "font_define", json!({ "sprite": "hud", "clear": true }));
    assert!(s.doc.model.sprites[0].extras.texto.font.is_none());
}

fn fnt_field(line: &str, key: &str) -> i64 {
    line.split_whitespace().find_map(|kv| kv.strip_prefix(&format!("{key}="))).unwrap().parse().unwrap()
}

#[test]
fn export_bitmap_font_writes_fnt_and_atlas() {
    let dir = temp_dir("bmfont");
    let (mut s, ctx) = session(8, 8);
    let ctx = ExecContext { paths: crate::PathPolicy::permissive(&dir), ..ctx };
    run(&mut s, &ctx, "font_template", json!({ "name": "Pixel", "base_font": "pixel-5x7", "charset": " AÉ" }));
    let out = run(&mut s, &ctx, "export_bitmap_font", json!({ "font": "Pixel", "path": "fonte.fnt", "scale": 1 }));
    assert_eq!(out.data["chars"], 3);
    let fnt = std::fs::read_to_string(dir.join("fonte.fnt")).unwrap();
    let atlas = decode_png(&std::fs::read(dir.join("fonte.png")).unwrap()).unwrap();
    let lines: Vec<&str> = fnt.lines().collect();
    assert!(lines[0].starts_with("info face=\"Pixel\" size=12 bold=0 italic=0"));
    assert!(lines[1].starts_with("common lineHeight=12 base=9 "));
    assert_eq!(lines[2], "page id=0 file=\"fonte.png\"");
    assert_eq!(lines[3], "chars count=3");
    let e = lines.iter().find(|l| l.starts_with(&format!("char id={} ", 'É' as u32))).unwrap();
    let r = Rect::new(fnt_field(e, "x") as i32, fnt_field(e, "y") as i32, fnt_field(e, "width") as u32, fnt_field(e, "height") as u32);
    assert_eq!((r.w, r.h, fnt_field(e, "yoffset"), fnt_field(e, "xadvance")), (5, 9, 0, 6));
    // The atlas rectangle holds exactly the glyph drawn in the sprite (white).
    let sheet = composite(&s, 1);
    let cell = sheet.crop(Rect::new(12, 0, 6, 12));
    let ink = cell.opaque_bounds().unwrap();
    assert_eq!(atlas.crop(r), cell.crop(ink));
    // Built-in font, scaled, with a color.
    run(&mut s, &ctx, "export_bitmap_font", json!({ "font": "mini-3x5", "path": "mini.fnt", "scale": 2, "color": "#ff0000" }));
    let fnt = std::fs::read_to_string(dir.join("mini.fnt")).unwrap();
    let atlas = decode_png(&std::fs::read(dir.join("mini.png")).unwrap()).unwrap();
    let a = fnt.lines().find(|l| l.starts_with("char id=65 ")).unwrap();
    let r = Rect::new(fnt_field(a, "x") as i32, fnt_field(a, "y") as i32, fnt_field(a, "width") as u32, fnt_field(a, "height") as u32);
    assert_eq!((r.w, r.h, fnt_field(a, "yoffset")), (6, 10, 4));
    assert_eq!(atlas.get(r.x + 2, r.y), Some(RED), "topo do A ampliado");
    assert_eq!(atlas.get(r.x, r.y), Some(Color::TRANSPARENT));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nine_slice_set_render_and_preview_overlay() {
    let (mut s, ctx) = session(8, 8);
    // Frame: border color at the edges, a distinct center.
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 0, "y": 0, "w": 8, "h": 8, "color": "#ff0000", "fill": true },
            { "op": "rect", "x": 2, "y": 2, "w": 4, "h": 4, "color": "#0000ff", "fill": true },
            { "op": "pixel", "x": 0, "y": 0, "color": "#00ff00" }
        ]}),
    );
    assert!(run_err(&mut s, &ctx, "nine_slice_set", json!({ "left": 4, "right": 4 })).message.contains("left + right"));
    assert!(run_err(&mut s, &ctx, "nine_slice_render", json!({ "width": 16, "height": 16 })).message.contains("nine_slice_set"));
    let out = run(&mut s, &ctx, "nine_slice_set", json!({ "all": 2, "bottom": 3 }));
    assert_eq!(out.data["margins"], json!({ "left": 2, "top": 2, "right": 2, "bottom": 3 }));
    assert_eq!(out.data["center"], json!([4, 3]));
    let r = run(&mut s, &ctx, "nine_slice_render", json!({ "width": 20, "height": 12, "edges": "tile", "scale": 2, "background": "none" }));
    assert_eq!(r.data["size"], json!([20, 12]));
    let img = decode_png(&r.images[0].data).unwrap();
    assert_eq!((img.width(), img.height()), (40, 24));
    let small = img.scale_nearest(20, 12);
    assert_eq!(small.get(0, 0), Some(Color::rgb(0, 255, 0)), "canto intacto");
    assert_eq!(small.get(19, 11), Some(RED));
    assert_eq!(small.get(10, 6), Some(Color::rgb(0, 0, 255)), "centro");
    assert_eq!(small.get(10, 0), Some(RED), "borda");
    // Guides on the frame preview.
    let p = run(&mut s, &ctx, "frame_preview", json!({ "nine_slice": true, "scale": 4, "background": "none" }));
    assert!(p.data["overlays"].as_array().unwrap().iter().any(|o| o.get("nine_slice").is_some()));
    let img = decode_png(&p.images[0].data).unwrap();
    assert_eq!(img.get(8, 1), Some(Color::rgb(255, 64, 200)), "linha da margem esquerda (x = 2 × 4)");
    run(&mut s, &ctx, "nine_slice_set", json!({ "clear": true }));
    assert!(s.doc.model.sprites[0].extras.texto.nine_slice.is_none());
}

#[test]
fn export_nine_slice_writes_a_godot_scene() {
    let dir = temp_dir("nine");
    let (mut s, ctx) = session(6, 6);
    let ctx = ExecContext { paths: crate::PathPolicy::permissive(&dir), ..ctx };
    run(&mut s, &ctx, "draw_batch", json!({ "ops": [{ "op": "rect", "x": 0, "y": 0, "w": 6, "h": 6, "color": "#334455", "fill": true }] }));
    assert!(run_err(&mut s, &ctx, "export_nine_slice", json!({ "path": "painel.tscn" })).message.contains("nine_slice_set"));
    run(&mut s, &ctx, "nine_slice_set", json!({ "left": 1, "top": 2, "right": 2, "bottom": 1 }));
    let out = run(
        &mut s,
        &ctx,
        "export_nine_slice",
        json!({ "path": "ui/painel.tscn", "scale": 2, "axis_stretch_horizontal": "tile", "axis_stretch_vertical": "tile_fit", "size": [40, 30] }),
    );
    assert!(!out.images.is_empty());
    let scene = std::fs::read_to_string(dir.join("ui/painel.tscn")).unwrap();
    for line in [
        "[ext_resource type=\"Texture2D\" path=\"res://painel.png\" id=\"1_tex\"]",
        "[node name=\"hud\" type=\"NinePatchRect\"]",
        "texture = ExtResource(\"1_tex\")",
        "patch_margin_left = 2",
        "patch_margin_top = 4",
        "patch_margin_right = 4",
        "patch_margin_bottom = 2",
        "axis_stretch_horizontal = 1",
        "axis_stretch_vertical = 2",
        "offset_right = 40.0",
    ] {
        assert!(scene.lines().any(|l| l == line), "falta {line} em\n{scene}");
    }
    let png = decode_png(&std::fs::read(dir.join("ui/painel.png")).unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (12, 12));
    let meta: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("ui/painel.json")).unwrap()).unwrap();
    assert_eq!(meta["margins"], json!({ "left": 2, "top": 4, "right": 4, "bottom": 2 }));
    assert_eq!(meta["godot_axis_stretch"], json!([1, 2]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn text_data_survives_save_and_open() {
    let dir = temp_dir("persist");
    let (mut s, ctx) = session(16, 16);
    run(&mut s, &ctx, "nine_slice_set", json!({ "all": 3 }));
    run(&mut s, &ctx, "font_define", json!({ "cell": [8, 8], "charset": "AB", "baseline": 6, "line_height": 9 }));
    let path = dir.join("p.psmproj");
    sprite_project::format::save(&s.doc, &path).unwrap();
    let back = sprite_project::format::load(&path).unwrap();
    assert_eq!(back.model.sprites[0].extras, s.doc.model.sprites[0].extras);
    let t = &back.model.sprites[0].extras.texto;
    assert_eq!(t.nine_slice.unwrap().left, 3);
    assert_eq!(t.font.as_ref().unwrap().line_height, 9);
    // Sprites without text data keep the field out of the file.
    let (plain, _) = session(4, 4);
    let json = serde_json::to_string(&plain.doc.model).unwrap();
    assert!(!json.contains("\"texto\":"));
    let _ = std::fs::remove_dir_all(&dir);
}
