//! Drawing-tool commands: JSON in, real pixels and masks out.

use serde_json::{Value, json};
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_core::{Color, Image};

use crate::{CommandError, ExecContext, Outcome, ProjectCreate, Session, create_project, parse_command};

const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);

fn session(w: u32, h: u32) -> (Session, ExecContext) {
    let ctx = ExecContext::local(std::env::temp_dir());
    let s = create_project(
        &ProjectCreate {
            name: "ferramentas".into(),
            width: w,
            height: h,
            sprite_name: None,
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

fn image(s: &Session) -> Image {
    let sp = &s.doc.model.sprites[0];
    compose_frame(sp, s.doc.images(), sp.frames[0].id, &ComposeOptions::default())
}

/// Pixels of a row as chars: `.` transparent, `r` red, `b` blue, `?` anything else.
fn row(img: &Image, y: i32) -> String {
    (0..img.width() as i32)
        .map(|x| match img.get_or_clear(x, y) {
            c if c.is_transparent() => '.',
            c if c == RED => 'r',
            c if c == BLUE => 'b',
            _ => '?',
        })
        .collect()
}

fn stroke(s: &mut Session, ctx: &ExecContext, op: Value) -> Outcome {
    let mut op = op;
    op["op"] = json!("stroke");
    run(s, ctx, "draw_batch", json!({ "ops": [op] }))
}

#[test]
fn stroke_mirrors_on_both_axes() {
    let (mut s, ctx) = session(8, 8);
    // Default axes: the sprite center (3.5, 3.5), between two pixels.
    let out = stroke(&mut s, &ctx, json!({ "points": [[1, 1], [2, 1]], "color": "#ff0000", "symmetry": { "x": true, "y": true } }));
    assert_eq!(out.data["changed_pixels"], 8);
    let img = image(&s);
    assert_eq!(row(&img, 1), ".rr..rr.");
    assert_eq!(row(&img, 6), ".rr..rr.");
    assert_eq!(row(&img, 0), "........");
}

#[test]
fn stroke_mirror_axis_on_half_and_whole_pixels() {
    let (mut s, ctx) = session(9, 3);
    stroke(&mut s, &ctx, json!({ "points": [[0, 0]], "color": "#ff0000", "symmetry": { "x": 2.5 } }));
    stroke(&mut s, &ctx, json!({ "points": [[0, 2], [1, 2]], "color": "#0000ff", "symmetry": { "x": 4 } }));
    let img = image(&s);
    assert_eq!(row(&img, 0), "r....r...", "2.5: pixel 0 mirrors to 5");
    assert_eq!(row(&img, 2), "bb.....bb", "4: pixels 0,1 mirror to 8,7");
    let err = run_err(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [{ "op": "stroke", "points": [[0, 0]], "color": 0, "symmetry": { "y": 1.3 } }] }),
    );
    assert!(err.message.contains("eixo"), "{}", err.message);
}

#[test]
fn stroke_wraps_across_the_edges() {
    let (mut s, ctx) = session(6, 4);
    // Without wrap the pixels outside are lost.
    stroke(&mut s, &ctx, json!({ "points": [[4, 0], [7, 0]], "color": "#ff0000" }));
    assert_eq!(row(&image(&s), 0), "....rr");
    // With wrap they come back on the other side (x 6, 7 → 0, 1; y -1 → 3).
    stroke(&mut s, &ctx, json!({ "points": [[4, 1], [7, 1]], "color": "#0000ff", "wrap": true }));
    stroke(&mut s, &ctx, json!({ "points": [[2, -1]], "color": "#0000ff", "wrap": true, "thickness": 1 }));
    let img = image(&s);
    assert_eq!(row(&img, 1), "bb..bb");
    assert_eq!(row(&img, 3), "..b...");
}

#[test]
fn stroke_dither_with_and_without_secondary() {
    let (mut s, ctx) = session(4, 2);
    run(&mut s, &ctx, "draw_batch", json!({ "ops": [{ "op": "rect", "x": 0, "y": 0, "w": 4, "h": 2, "color": "#0000ff", "fill": true }] }));
    // Checker at 50%: the pattern is anchored on the canvas, so even (x+y) pixels get the color.
    stroke(&mut s, &ctx, json!({ "points": [[0, 0], [3, 0]], "color": "#ff0000", "mode": "dither", "pattern": "checker", "density": 0.5 }));
    assert_eq!(row(&image(&s), 0), "rbrb", "without secondary the others stay");
    stroke(
        &mut s,
        &ctx,
        json!({ "points": [[0, 1], [3, 1]], "color": "#ff0000", "secondary": "clear", "mode": "dither", "pattern": "checker", "density": 0.5 }),
    );
    assert_eq!(row(&image(&s), 1), ".r.r", "secondary paints the rest");
    // Density 1 paints everything; 0 paints nothing.
    stroke(&mut s, &ctx, json!({ "points": [[0, 1], [3, 1]], "color": "#0000ff", "mode": "dither", "pattern": "bayer4", "density": 1.0 }));
    assert_eq!(row(&image(&s), 1), "bbbb");
}

#[test]
fn stroke_shade_moves_each_pixel_once_along_the_ramp() {
    let (mut s, ctx) = session(5, 1);
    let ramp = ["#100000", "#200000", "#300000", "#400000"];
    run(&mut s, &ctx, "palette_create", json!({ "name": "rampa", "colors": ramp }));
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [{ "op": "rect", "x": 0, "y": 0, "w": 5, "h": 1, "color": 1, "fill": true }, { "op": "pixel", "x": 4, "y": 0, "color": "#00ff00" }] }),
    );
    // Back and forth over the same pixels: still one step each (ramp = active palette).
    stroke(&mut s, &ctx, json!({ "points": [[0, 0], [3, 0], [0, 0], [2, 0]], "mode": "shade" }));
    let img = image(&s);
    let hex = |x: i32| img.get_or_clear(x, 0).to_hex();
    assert_eq!([hex(0), hex(1), hex(2), hex(3)], ["#300000", "#300000", "#300000", "#300000"]);
    // A color outside the ramp stays.
    stroke(&mut s, &ctx, json!({ "points": [[4, 0]], "mode": "shade" }));
    assert_eq!(image(&s).get_or_clear(4, 0), Color::rgb(0, 255, 0));
    // Darken with an explicit ramp; the end of the ramp is a stop.
    stroke(&mut s, &ctx, json!({ "points": [[0, 0], [1, 0]], "mode": "shade", "step": -5, "ramp": ramp }));
    stroke(&mut s, &ctx, json!({ "points": [[2, 0], [3, 0]], "mode": "shade", "step": 1 }));
    let img = image(&s);
    let hex = |x: i32| img.get_or_clear(x, 0).to_hex();
    assert_eq!([hex(0), hex(1), hex(2), hex(3)], ["#100000", "#100000", "#400000", "#400000"]);
    // Without a ramp of 2+ colors shading is refused.
    let err = run_err(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [{ "op": "stroke", "points": [[0, 0]], "mode": "shade", "ramp": ["#ffffff"] }] }),
    );
    assert!(err.message.contains("rampa"), "{}", err.message);
}

#[test]
fn stroke_pixel_perfect_and_eraser() {
    let (mut s, ctx) = session(4, 4);
    // A jagged freehand staircase: pixel_perfect drops the "L" corners.
    let pts = json!([[0, 0], [1, 0], [1, 1], [2, 1], [2, 2]]);
    stroke(&mut s, &ctx, json!({ "points": pts, "color": "#ff0000", "pixel_perfect": true }));
    let img = image(&s);
    assert_eq!(row(&img, 0), "r...");
    assert_eq!(row(&img, 1), ".r..");
    assert_eq!(row(&img, 2), "..r.");
    // The eraser is a stroke with "clear".
    stroke(&mut s, &ctx, json!({ "points": [[0, 0], [3, 3]], "color": "clear", "thickness": 2 }));
    assert!(image(&s).is_blank());
    // Paint mode needs a color.
    let err = run_err(&mut s, &ctx, "draw_batch", json!({ "ops": [{ "op": "stroke", "points": [[0, 0]] }] }));
    assert!(err.message.contains("color"), "{}", err.message);
}

/// Two blue blobs separated by a red wall, plus a slightly different blue pixel.
fn wand_scene(s: &mut Session, ctx: &ExecContext) {
    run(
        s,
        ctx,
        "pixel_write_region",
        json!({ "at": [0, 0], "legend": { "b": "#0000ff", "c": "#0000fa", "r": "#ff0000" }, "rows": [
            "bbrbb",
            "bbrbb",
            "rrrrc",
        ]}),
    );
}

fn rows(out: &Outcome) -> Vec<String> {
    serde_json::from_value(out.data["rows"].clone()).unwrap()
}

#[test]
fn magic_wand_contiguous_global_and_tolerance() {
    let (mut s, ctx) = session(5, 3);
    wand_scene(&mut s, &ctx);
    let out = run(&mut s, &ctx, "select_color", json!({ "x": 0, "y": 0 }));
    assert_eq!(out.data["bounds"], json!({ "x": 0, "y": 0, "w": 2, "h": 2 }));
    assert_eq!(out.data["count"], 4);
    assert_eq!(out.data["picked"], "#0000ff");
    let out = run(&mut s, &ctx, "select_color", json!({ "x": 0, "y": 0, "contiguous": false }));
    assert_eq!(out.data["count"], 8);
    assert_eq!(rows(&out), ["##.##", "##.##"]);
    // Tolerance 5 reaches the #0000fa pixel too (global), and the read is a pure query.
    let r0 = s.doc.revision();
    let out = run(&mut s, &ctx, "select_color", json!({ "x": 4, "y": 0, "contiguous": false, "tolerance": 5 }));
    assert_eq!(out.data["count"], 9);
    assert_eq!(s.doc.revision(), r0);
    // 8-connectivity crosses diagonals: the red wall is one piece either way.
    let out = run(&mut s, &ctx, "select_color", json!({ "x": 2, "y": 0, "connectivity": 8 }));
    assert_eq!(out.data["count"], 6);
    let err = run_err(&mut s, &ctx, "select_color", json!({ "x": 9, "y": 0 }));
    assert_eq!(err.code, crate::ErrorCode::OutOfBounds);
}

#[test]
fn lasso_with_a_concavity() {
    let (mut s, ctx) = session(8, 8);
    // A "U": the notch between the arms stays out.
    let out = run(&mut s, &ctx, "select_polygon", json!({ "points": [[0, 0], [2, 0], [2, 4], [5, 4], [5, 0], [7, 0], [7, 6], [0, 6]] }));
    let r = rows(&out);
    assert_eq!(out.data["bounds"], json!({ "x": 0, "y": 0, "w": 8, "h": 7 }));
    assert_eq!(r[1], "###..###", "notch is not selected");
    assert_eq!(r[5], "########");
    assert_eq!(out.data["count"].as_u64().unwrap(), r.iter().map(|l| l.matches('#').count() as u64).sum::<u64>());
}

#[test]
fn selections_combine() {
    let (mut s, ctx) = session(6, 6);
    let square = |x: i32, y: i32, side: i32| json!([[x, y], [x + side - 1, y], [x + side - 1, y + side - 1], [x, y + side - 1]]);
    let a = run(&mut s, &ctx, "select_polygon", json!({ "points": square(0, 0, 3) }));
    assert_eq!(a.data["count"], 9);
    let base = json!({ "bounds": a.data["bounds"], "rows": a.data["rows"] });
    let add = run(&mut s, &ctx, "select_polygon", json!({ "points": square(2, 2, 3), "base": base, "mode": "add" }));
    assert_eq!(add.data["count"], 17);
    let sub = run(&mut s, &ctx, "select_polygon", json!({ "points": square(2, 2, 3), "base": base, "mode": "subtract" }));
    assert_eq!(sub.data["count"], 8);
    assert_eq!(rows(&sub), ["###", "###", "##."]);
    let inter = run(&mut s, &ctx, "select_polygon", json!({ "points": square(2, 2, 3), "base": base, "mode": "intersect" }));
    assert_eq!(inter.data["count"], 1);
    assert_eq!(inter.data["bounds"], json!({ "x": 2, "y": 2, "w": 1, "h": 1 }));
    // The wand combines the same way.
    wand_scene(&mut s, &ctx);
    let wand = run(&mut s, &ctx, "select_color", json!({ "x": 3, "y": 0, "base": base, "mode": "add" }));
    assert_eq!(wand.data["count"], 13);
    let empty = run(&mut s, &ctx, "select_polygon", json!({ "points": square(0, 0, 3), "base": base, "mode": "subtract" }));
    assert_eq!(empty.data["count"], 0);
    assert_eq!(empty.data["bounds"], Value::Null);
}

/// A 4x2 scene: blue row on top, red row below; the mask picks a diagonal-ish shape.
fn masked_scene(s: &mut Session, ctx: &ExecContext) -> Value {
    run(s, ctx, "pixel_write_region", json!({ "at": [0, 0], "legend": { "b": "#0000ff", "r": "#ff0000" }, "rows": ["bbbb", "rrrr"] }));
    json!({ "bounds": { "x": 0, "y": 0, "w": 3, "h": 2 }, "rows": ["#.#", ".#."] })
}

#[test]
fn delete_only_the_selected_pixels() {
    let (mut s, ctx) = session(4, 2);
    let mask = masked_scene(&mut s, &ctx);
    let out = run(&mut s, &ctx, "pixel_clear", json!({ "mask": mask }));
    assert_eq!(out.data["changed_pixels"], 3);
    let img = image(&s);
    assert_eq!(row(&img, 0), ".b.b");
    assert_eq!(row(&img, 1), "r.rr");
    // Without a mask pixel_clear still clears everything.
    run(&mut s, &ctx, "pixel_clear", json!({}));
    assert!(image(&s).is_blank());
}

#[test]
fn copy_and_paste_only_the_selected_pixels() {
    let (mut s, ctx) = session(4, 4);
    let mask = masked_scene(&mut s, &ctx);
    let copied = run(&mut s, &ctx, "selection_read", json!({ "mask": mask }));
    assert_eq!(copied.data["count"], 3);
    assert_eq!(copied.data["rect"], json!({ "x": 0, "y": 0, "w": 3, "h": 2 }));
    assert!(copied.data["legend"].get(".").is_none(), "`.` keeps the destination");
    // Cut: clearing the source does not affect what was read.
    run(&mut s, &ctx, "pixel_clear", json!({ "mask": mask }));
    // Paste two rows down over a red background: unselected pixels keep the destination.
    run(&mut s, &ctx, "draw_batch", json!({ "ops": [{ "op": "rect", "x": 0, "y": 2, "w": 4, "h": 2, "color": "#ff0000", "fill": true }] }));
    run(&mut s, &ctx, "pixel_write_region", json!({ "at": [0, 2], "rows": copied.data["rows"], "legend": copied.data["legend"] }));
    let img = image(&s);
    assert_eq!(row(&img, 2), "brbr");
    assert_eq!(row(&img, 3), "rrrr");
    assert_eq!(row(&img, 0), ".b.b", "the cut left holes");
}

#[test]
fn selection_read_falls_back_to_numeric_with_many_colors() {
    let (mut s, ctx) = session(16, 8);
    let pixels: Vec<Value> = (0..128).map(|i| json!([i % 16, i / 16, format!("#{:02x}0000", i + 1)])).collect();
    run(&mut s, &ctx, "pixel_set_batch", json!({ "pixels": pixels }));
    let read = run(&mut s, &ctx, "selection_read", json!({ "rect": { "x": 0, "y": 0, "w": 16, "h": 8 } }));
    assert_eq!(read.data["count"], 128);
    assert_eq!(read.data["palette"].as_array().unwrap().len(), 128);
    let before = image(&s);
    run(&mut s, &ctx, "pixel_clear", json!({}));
    run(&mut s, &ctx, "pixel_write_region", json!({ "at": [0, 0], "pixels": read.data["pixels"], "palette": read.data["palette"] }));
    assert_eq!(image(&s), before);
}

#[test]
fn move_and_flip_only_the_selected_pixels() {
    let (mut s, ctx) = session(4, 4);
    let mask = masked_scene(&mut s, &ctx);
    let out = run(&mut s, &ctx, "draw_transform", json!({ "transform": { "kind": "move", "dx": 0, "dy": 2 }, "mask": mask }));
    let img = image(&s);
    assert_eq!(row(&img, 0), ".b.b");
    assert_eq!(row(&img, 1), "r.rr");
    assert_eq!(row(&img, 2), "b.b.");
    assert_eq!(row(&img, 3), ".r..");
    // The answer carries the moved selection.
    assert_eq!(out.data["selection"]["bounds"], json!({ "x": 0, "y": 2, "w": 3, "h": 2 }));
    assert_eq!(out.data["selection"]["rows"], json!(["#.#", ".#."]));
    // Flip with a mask mirrors inside the mask's box and returns the mirrored mask.
    let moved = json!({ "bounds": out.data["selection"]["bounds"], "rows": out.data["selection"]["rows"] });
    let mask2 = json!({ "bounds": { "x": 0, "y": 2, "w": 2, "h": 2 }, "rows": ["##", "#."] });
    run(&mut s, &ctx, "draw_transform", json!({ "transform": { "kind": "move", "dx": 0, "dy": -2 }, "mask": moved }));
    run(
        &mut s,
        &ctx,
        "pixel_write_region",
        json!({ "at": [0, 2], "legend": { "b": "#0000ff", "r": "#ff0000" }, "rows": ["br..", "r..."] }),
    );
    let flipped = run(&mut s, &ctx, "draw_transform", json!({ "transform": { "kind": "flip_h" }, "mask": mask2 }));
    let img = image(&s);
    assert_eq!(row(&img, 2), "rb..");
    assert_eq!(row(&img, 3), ".r..");
    assert_eq!(flipped.data["selection"]["rows"], json!(["##", ".#"]));
    // Untouched outside the mask.
    assert_eq!(row(&img, 0), "bbbb");
}
