//! Tileset commands end to end at the command layer: JSON in, real pixels and files out.

use std::path::PathBuf;

use serde_json::{Value, json};
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_commands::{CommandError, ErrorCode, ExecContext, Outcome, ProjectCreate, Session, create_project, parse_command};
use sprite_core::codec::decode_png;
use sprite_core::{Color, Image, Rect};
use sprite_drawing::tiles::{PeeringBit, TerrainMode};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-tiles-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn session(w: u32, h: u32) -> (Session, ExecContext) {
    let ctx = ExecContext::local(std::env::temp_dir());
    let s = create_project(
        &ProjectCreate {
            name: "tiles".into(),
            width: w,
            height: h,
            sprite_name: Some("chao".into()),
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

fn composite_of(s: &Session, sprite: &str) -> Image {
    let sp = s.doc.model.sprites.iter().find(|x| x.name == sprite).expect("sprite");
    compose_frame(sp, s.doc.images(), sp.frames[0].id, &ComposeOptions::default())
}

fn tiles_of<'a>(s: &'a Session, sprite: &str) -> &'a sprite_drawing::tiles::TileData {
    s.doc.model.sprites.iter().find(|x| x.name == sprite).expect("sprite").extras.tiles.as_ref().expect("tiles")
}

fn hex(c: Color) -> String {
    c.to_hex()
}

/// Fills every `q`×`q` quarter of the sprite with its own color: rgb(qx*40+10, qy*40+10, 200).
fn paint_quarters(s: &mut Session, ctx: &ExecContext, w: u32, h: u32, q: u32) {
    let mut ops = Vec::new();
    for qy in 0..h / q {
        for qx in 0..w / q {
            ops.push(json!({ "op": "rect", "x": qx * q, "y": qy * q, "w": q, "h": q, "fill": true, "color": hex(quarter_color(qx, qy)) }));
        }
    }
    run(s, ctx, "draw_batch", json!({ "ops": ops }));
}

fn quarter_color(qx: u32, qy: u32) -> Color {
    Color::rgb((qx * 40 + 10) as u8, (qy * 40 + 10) as u8, 200)
}

/// Color at the center-ish of quarter `quadrant` (0 TL, 1 TR, 2 BL, 3 BR) of tile `[c, r]` (8x8 tiles).
fn quarter_at(img: &Image, tile: [u32; 2], quadrant: u32) -> Color {
    let x = tile[0] * 8 + (quadrant & 1) * 4 + 1;
    let y = tile[1] * 8 + (quadrant >> 1) * 4 + 2;
    img.get(x as i32, y as i32).unwrap()
}

#[test]
fn grid_is_set_validated_and_reported() {
    let (mut s, ctx) = session(32, 16);
    assert!(run_err(&mut s, &ctx, "tile_info", json!({})).message.contains("tile_grid_set"));
    assert_eq!(run_err(&mut s, &ctx, "tile_grid_set", json!({})).code, ErrorCode::InvalidArgument);
    assert_eq!(run_err(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 0 })).code, ErrorCode::InvalidArgument);
    assert_eq!(run_err(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 40 })).code, ErrorCode::OutOfBounds);
    assert_eq!(run_err(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8, "margin": [30, 0] })).code, ErrorCode::OutOfBounds);

    let out = run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8 }));
    assert_eq!(out.data["columns"], 4);
    assert_eq!(out.data["rows"], 2);
    assert_eq!(out.data["tile_height"], 8);
    assert!(out.warnings.is_empty());

    run(&mut s, &ctx, "draw_batch", json!({ "ops": [{ "op": "pixel", "x": 9, "y": 2, "color": "#ff0000" }] }));
    let info = run(&mut s, &ctx, "tile_info", json!({}));
    assert_eq!(info.data["non_empty"], 1);
    assert_eq!(info.data["empty"].as_array().unwrap().len(), 7);
    assert!(!info.data["empty"].as_array().unwrap().contains(&json!([1, 0])));

    // Margin and spacing follow Godot's atlas rule; leftovers are warned about.
    let out = run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8, "margin": [1, 1], "spacing": [2, 2] }));
    assert_eq!(out.data["columns"], 3);
    assert_eq!(out.data["rows"], 1);
    assert!(out.warnings.iter().any(|w| w.contains("sobram")), "{:?}", out.warnings);
    // Partial update keeps the tile size.
    let out = run(&mut s, &ctx, "tile_grid_set", json!({ "margin": [0, 0], "spacing": [0, 0] }));
    assert_eq!((out.data["tile_width"].as_u64(), out.data["columns"].as_u64()), (Some(8), Some(4)));
    let out = run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 16, "tile_height": 8 }));
    assert_eq!((out.data["columns"].as_u64(), out.data["rows"].as_u64()), (Some(2), Some(2)));

    // Undo brings the previous grid back; remove drops everything.
    s.doc.undo().unwrap();
    assert_eq!(tiles_of(&s, "chao").grid.tile_width, 8);
    run(&mut s, &ctx, "tile_grid_set", json!({ "remove": true }));
    assert!(s.doc.model.sprites[0].extras.tiles.is_none());
}

#[test]
fn collisions_shortcuts_polygons_preview_and_persistence() {
    let (mut s, ctx) = session(32, 32);
    assert!(run_err(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 0], "shape": "full" })).message.contains("tile_grid_set"));
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 16 }));
    let out = run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 0], "shape": "full" }));
    assert_eq!(out.data["tiles"][0]["polygons"], json!([[[0, 0], [16, 0], [16, 16], [0, 16]]]));
    run(&mut s, &ctx, "tile_collision_set", json!({ "tiles": [[1, 0], [1, 1]], "shape": "top_half" }));
    let out = run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [1, 0], "polygons": [[[0, 16], [16, 0], [16, 16]]], "add": true }));
    assert_eq!(out.data["tiles"][0]["polygons"].as_array().unwrap().len(), 2);
    assert_eq!(tiles_of(&s, "chao").collision([1, 1]).unwrap().polygons[0][2], sprite_core::Point::new(16, 8));

    // Validation.
    let e = run_err(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 1], "polygons": [[[0, 0], [17, 0], [0, 4]]] }));
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    assert!(e.message.contains("fora do tile"), "{}", e.message);
    assert_eq!(run_err(&mut s, &ctx, "tile_collision_set", json!({ "tile": [2, 0], "shape": "full" })).code, ErrorCode::OutOfBounds);
    assert_eq!(run_err(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 1] })).code, ErrorCode::InvalidArgument);
    assert_eq!(
        run_err(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 1], "clear": true, "shape": "full" })).code,
        ErrorCode::InvalidArgument
    );
    run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [1, 1], "clear": true }));
    assert!(tiles_of(&s, "chao").collision([1, 1]).is_none());

    // The preview draws the grid and the collision polygons.
    let out = run(&mut s, &ctx, "frame_preview", json!({ "scale": 4, "background": "none", "tiles": true }));
    let img = decode_png(&out.images[0].data).unwrap();
    assert_eq!((img.width(), img.height()), (128, 128));
    assert_eq!(img.get(64, 100).unwrap().g, 229, "grid line in cyan (left border of tile [1, 1])");
    assert_eq!(img.get(0, 0).unwrap().r, 255, "collision outline over the grid");
    let inside = img.get(20, 20).unwrap();
    assert_eq!((inside.r, inside.g, inside.b), (255, 64, 129), "collision fill");
    assert!(inside.a > 0 && inside.a < 255);
    assert_eq!(img.get(100, 100), Some(Color::TRANSPARENT), "tile [1,1] has no collision");
    assert!(out.data["overlays"].as_array().unwrap().iter().any(|o| o["tiles"]["collision_tiles"] == 2));

    // Saved and reopened, the data is the same.
    let dir = temp_dir("collisions");
    let path = dir.join("tiles.psmproj");
    sprite_project::format::save(&s.doc, &path).unwrap();
    let doc = sprite_project::format::load(&path).unwrap();
    assert_eq!(doc.model.sprites[0].extras.tiles, s.doc.model.sprites[0].extras.tiles);
    assert_eq!(doc.model.sprites[0].extras.tiles.as_ref().unwrap().collisions.len(), 2);
}

#[test]
fn terrains_sets_bits_and_validation() {
    let (mut s, ctx) = session(32, 32);
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 16 }));
    assert_eq!(
        run_err(&mut s, &ctx, "tile_terrain_set", json!({ "tiles": [{ "tile": [0, 0], "terrain": 0 }] })).code,
        ErrorCode::InvalidArgument
    );
    let out = run(
        &mut s,
        &ctx,
        "tile_terrain_set",
        json!({
            "sets": [{ "name": "chão", "mode": "match_sides", "terrains": [{ "name": "grama", "color": "#00ff00" }, { "name": "água" }] }],
            "tiles": [
                { "tile": [0, 0], "terrain": "grama", "bits": ["right_side", "bottom_side"], "bit_terrains": { "left_side": "água" } },
                { "tiles": [[1, 0], [1, 1]], "terrain": 1 }
            ]
        }),
    );
    assert_eq!(out.data["tiles_with_terrain"], 3);
    let d = tiles_of(&s, "chao");
    assert_eq!(d.terrain_sets[0].mode, TerrainMode::MatchSides);
    assert_eq!(d.terrain_sets[0].terrains[1].name, "água");
    let t = d.terrain([0, 0]).unwrap();
    assert_eq!(t.terrain, Some(0));
    assert_eq!(t.peering.get(PeeringBit::RightSide), Some(0));
    assert_eq!(t.peering.get(PeeringBit::BottomSide), Some(0));
    assert_eq!(t.peering.get(PeeringBit::LeftSide), Some(1));
    assert_eq!(t.peering.get(PeeringBit::TopSide), None);

    // Corner bits do not exist in match_sides; unknown terrains are reported.
    let e = run_err(&mut s, &ctx, "tile_terrain_set", json!({ "tiles": [{ "tile": [0, 1], "terrain": 0, "bits": ["top_left_corner"] }] }));
    assert!(e.message.contains("match_sides"), "{}", e.message);
    assert_eq!(
        run_err(&mut s, &ctx, "tile_terrain_set", json!({ "tiles": [{ "tile": [0, 1], "terrain": "lava" }] })).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        run_err(&mut s, &ctx, "tile_terrain_set", json!({ "tiles": [{ "tile": [0, 1], "bits": ["top_side"] }] })).code,
        ErrorCode::InvalidArgument
    );

    // tile_info speaks in names.
    let info = run(&mut s, &ctx, "tile_info", json!({}));
    let first = &info.data["terrains"][0];
    assert_eq!(first["terrain"], "grama");
    assert_eq!(first["bits"]["left_side"], "água");

    // Shrinking the set prunes references to the removed terrain.
    let out = run(
        &mut s,
        &ctx,
        "tile_terrain_set",
        json!({ "sets": [{ "name": "CHÃO", "mode": "match_sides", "terrains": [{ "name": "grama" }] }] }),
    );
    assert!(out.warnings.iter().any(|w| w.contains("removidas")), "{:?}", out.warnings);
    let d = tiles_of(&s, "chao");
    assert_eq!(d.terrain_sets.len(), 1, "replaced by name");
    assert_eq!(d.terrain([0, 0]).unwrap().peering.get(PeeringBit::LeftSide), None);
    assert!(d.terrain([1, 0]).is_none(), "its only terrain is gone");
    run(&mut s, &ctx, "tile_terrain_set", json!({ "tiles": [{ "tile": [0, 0], "clear": true }] }));
    assert!(tiles_of(&s, "chao").terrain([0, 0]).is_none());
    run(&mut s, &ctx, "tile_terrain_set", json!({ "remove_sets": ["chão"] }));
    assert!(tiles_of(&s, "chao").terrain_sets.is_empty());
}

#[test]
fn autotile_from_rpgmaker_a2_builds_a2_and_blob_47() {
    let (mut s, ctx) = session(16, 24);
    paint_quarters(&mut s, &ctx, 16, 24, 4);
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8 }));
    let out = run(&mut s, &ctx, "autotile_generate", json!({ "input": "rpgmaker_a2", "output": ["rpgmaker_a2", "blob_47"] }));
    assert_eq!(out.data["name"], "chao_autotile");
    assert_eq!(out.data["size"], json!([64, 72]));
    assert_eq!(out.data["parts"]["blob_47"]["origin"], json!([0, 3]));
    assert_eq!(out.images.len(), 1, "preview of the result");
    let src = composite_of(&s, "chao");
    let img = composite_of(&s, "chao_autotile");
    // The A2 part reproduces the source block exactly.
    assert_eq!(img.crop(Rect::new(0, 0, 16, 24)), src);

    // Blob tiles: index i at [i % 8, 3 + i / 8], increasing mask.
    let masks: Vec<u8> = out.data["parts"]["blob_47"]["masks"].as_array().unwrap().iter().map(|m| m.as_u64().unwrap() as u8).collect();
    assert_eq!(masks.len(), 47);
    let tile_of = |mask: u8| {
        let i = masks.iter().position(|m| *m == mask).unwrap() as u32;
        [i % 8, 3 + i / 8]
    };
    let q = quarter_color;
    // Frame quarters live in rows 2..5 of the 4x6 quarter grid; inner corners at (2..3, 0..1).
    let cases: [(u8, [Color; 4]); 6] = [
        (0, [q(0, 2), q(3, 2), q(0, 5), q(3, 5)]),               // isolated: outer corners
        (255, [q(2, 4), q(1, 4), q(2, 3), q(1, 3)]),             // surrounded: interior
        (1 | 4 | 16 | 64, [q(2, 0), q(3, 0), q(2, 1), q(3, 1)]), // four sides, no corners: inner corners
        (1, [q(0, 4), q(3, 4), q(0, 5), q(3, 5)]),               // only north: side edges on top, outer below
        (64, [q(2, 2), q(3, 2), q(2, 5), q(3, 5)]),              // only west: top/bottom edges on the left
        (1 | 64 | 128, [q(2, 4), q(3, 4), q(2, 5), q(3, 5)]),    // N + W + NW: interior, side edge, bottom edge, outer
    ];
    for (mask, expected) in cases {
        let t = tile_of(mask);
        for (quadrant, color) in expected.iter().enumerate() {
            assert_eq!(quarter_at(&img, t, quadrant as u32), *color, "mask {mask}, quadrant {quadrant}");
        }
    }

    // 47 tiles with terrain and the right peering bits, for Godot.
    let d = tiles_of(&s, "chao_autotile");
    assert_eq!((d.grid.tile_width, d.grid.tile_height), (8, 8));
    assert_eq!(d.terrain_sets[0].mode, TerrainMode::MatchCornersAndSides);
    assert_eq!(d.terrain_sets[0].terrains[0].name, "chao");
    assert_eq!(d.terrains.len(), 47);
    for (i, t) in d.terrains.iter().enumerate() {
        assert_eq!(t.tile, [i as u32 % 8, 3 + i as u32 / 8]);
        assert_eq!(t.terrain, Some(0));
        assert_eq!(t.peering.mask_of(0), masks[i], "tile {i}");
    }
    assert!(d.terrain([7, 8]).is_none());

    // The demo map already uses the terrain; painting by terrain picks the matching tiles.
    assert!(d.preview_map.is_some());
    run(
        &mut s,
        &ctx,
        "tile_preview_map_set",
        json!({ "sprite": "chao_autotile", "size": [5, 5], "clear": true, "terrain_rows": [".....", ".###.", ".###.", ".###.", "....#"] }),
    );
    let map = tiles_of(&s, "chao_autotile").preview_map.clone().unwrap();
    assert_eq!(map.get(2, 2), Some(tile_of(255)));
    assert_eq!(map.get(1, 1), Some(tile_of(4 | 8 | 16)), "top-left corner of the block: E, SE, S");
    assert_eq!(map.get(4, 4), Some(tile_of(0)), "the diagonal neighbor alone does not connect");
    assert_eq!(map.get(0, 0), None);

    // Same name again needs `replace`, which keeps the sprite id.
    let id = s.doc.model.sprites[1].id;
    assert!(run_err(&mut s, &ctx, "autotile_generate", json!({ "input": "rpgmaker_a2" })).message.contains("replace"));
    run(&mut s, &ctx, "autotile_generate", json!({ "input": "rpgmaker_a2", "replace": true, "terrain_name": "água" }));
    assert_eq!(s.doc.model.sprites.len(), 2);
    assert_eq!(s.doc.model.sprites[1].id, id);
    assert_eq!(s.doc.model.sprites[1].width, 64);
    assert_eq!(s.doc.model.sprites[1].height, 48, "blob only");
    assert_eq!(tiles_of(&s, "chao_autotile").terrain_sets[0].terrains[0].name, "água");
}

#[test]
fn autotile_from_frame_3x3_to_rpgmaker_a2() {
    let (mut s, ctx) = session(40, 32);
    // 4x3 tiles of 8x8 at margin [4, 4] with spacing 0... no: plain grid, input at [1, 1].
    paint_quarters(&mut s, &ctx, 40, 32, 4);
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8 }));
    let out =
        run(&mut s, &ctx, "autotile_generate", json!({ "input": "frame_3x3", "source": [1, 1], "output": ["rpgmaker_a2"], "name": "a2" }));
    assert_eq!(out.data["size"], json!([16, 24]));
    let img = composite_of(&s, "a2");
    // Source quarters: tile (c, r) of the input block starts at quarter (2 + 2c, 2 + 2r).
    let q = |c: u32, r: u32, quadrant: u32| quarter_color(2 + 2 * c + (quadrant & 1), 2 + 2 * r + (quadrant >> 1));
    let a2 = |qx: u32, qy: u32| img.get((qx * 4 + 1) as i32, (qy * 4 + 1) as i32).unwrap();
    // Icon = the isolated tile: four outer corners.
    assert_eq!(a2(0, 0), q(0, 0, 0));
    assert_eq!(a2(1, 0), q(2, 0, 1));
    assert_eq!(a2(0, 1), q(0, 2, 2));
    assert_eq!(a2(1, 1), q(2, 2, 3));
    // Inner corners come from tile (3, 0).
    assert_eq!(a2(2, 0), q(3, 0, 0));
    assert_eq!(a2(3, 1), q(3, 0, 3));
    // The frame, in 4x4 quarters starting at quarter row 2.
    assert_eq!(a2(0, 2), q(0, 0, 0), "outer top-left");
    assert_eq!(a2(3, 2), q(2, 0, 1), "outer top-right");
    assert_eq!(a2(1, 2), q(1, 0, 1), "top edge, right half");
    assert_eq!(a2(2, 2), q(1, 0, 0), "top edge, left half");
    assert_eq!(a2(0, 3), q(0, 1, 2), "left edge, bottom half");
    assert_eq!(a2(1, 3), q(1, 1, 3), "interior");
    assert_eq!(a2(3, 5), q(2, 2, 3), "outer bottom-right");
    let d = tiles_of(&s, "a2");
    assert_eq!(d.terrain_sets.len(), 1);
    assert!(d.terrains.is_empty(), "A2 alone has no Godot terrain bits");

    // Errors: input outside the grid, empty input, odd tiles.
    assert_eq!(run_err(&mut s, &ctx, "autotile_generate", json!({ "input": "frame_3x3", "source": [2, 1] })).code, ErrorCode::OutOfBounds);
    run(&mut s, &ctx, "pixel_clear", json!({}));
    assert!(run_err(&mut s, &ctx, "autotile_generate", json!({ "input": "frame_3x3" })).message.contains("vazia"));
    let (mut s, ctx) = session(14, 21);
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [{ "op": "rect", "x": 0, "y": 0, "w": 14, "h": 21, "fill": true, "color": "#336699" }] }),
    );
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 7 }));
    let e = run_err(&mut s, &ctx, "autotile_generate", json!({ "input": "rpgmaker_a2" }));
    assert_eq!(e.code, ErrorCode::InvalidArgument);
    assert!(e.message.contains("pares"), "{}", e.message);
}

#[test]
fn godot_tileset_resource_and_png() {
    let (mut s, ctx) = session(48, 16);
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 0, "y": 0, "w": 16, "h": 16, "fill": true, "color": "#00ff00" },
            { "op": "pixel", "x": 40, "y": 8, "color": "#0000ff" }
        ]}),
    );
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 16 }));
    run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [0, 0], "shape": "full" }));
    run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [2, 0], "polygons": [[[0, 16], [16, 0], [16, 16]]] }));
    run(&mut s, &ctx, "tile_collision_set", json!({ "tile": [1, 0], "shape": "bottom_half" }));
    run(
        &mut s,
        &ctx,
        "tile_terrain_set",
        json!({
            "sets": [{ "name": "chão", "terrains": [{ "name": "grama", "color": "#00ff00" }] }],
            "tiles": [{ "tile": [0, 0], "terrain": "grama", "bits": ["right_side", "bottom_right_corner", "bottom_side"] }]
        }),
    );
    let dir = temp_dir("godot");
    let tres = dir.join("chao.tres");
    let out = run(&mut s, &ctx, "export_godot_tileset", json!({ "path": tres.to_str().unwrap(), "res_path": "res://arte/chao.png" }));
    assert_eq!(out.data["tiles"], 2);
    assert!(out.warnings.iter().any(|w| w.contains("[1, 0]")), "empty tile with collision is reported: {:?}", out.warnings);
    assert_eq!(out.images.len(), 1);
    let png = decode_png(&std::fs::read(dir.join("chao.png")).unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (48, 16));
    assert_eq!(png.get(40, 8), Some(Color::rgb(0, 0, 255)));

    let text = std::fs::read_to_string(&tres).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let expected = [
        "[gd_resource type=\"TileSet\" load_steps=3 format=3]",
        "",
        "[ext_resource type=\"Texture2D\" path=\"res://arte/chao.png\" id=\"1_texture\"]",
        "",
        "[sub_resource type=\"TileSetAtlasSource\" id=\"TileSetAtlasSource_1\"]",
        "texture = ExtResource(\"1_texture\")",
        "margins = Vector2i(0, 0)",
        "separation = Vector2i(0, 0)",
        "texture_region_size = Vector2i(16, 16)",
        "0:0/0 = 0",
        "0:0/0/terrain_set = 0",
        "0:0/0/terrain = 0",
        "0:0/0/terrains_peering_bit/right_side = 0",
        "0:0/0/terrains_peering_bit/bottom_right_corner = 0",
        "0:0/0/terrains_peering_bit/bottom_side = 0",
        "0:0/0/physics_layer_0/polygon_0/points = PackedVector2Array(-8, -8, 8, -8, 8, 8, -8, 8)",
        "2:0/0 = 0",
        "2:0/0/physics_layer_0/polygon_0/points = PackedVector2Array(-8, 8, 8, -8, 8, 8)",
        "",
        "[resource]",
        "tile_size = Vector2i(16, 16)",
        "physics_layer_0/collision_layer = 1",
        "terrain_set_0/mode = 0",
        "terrain_set_0/terrain_0/name = \"grama\"",
        "terrain_set_0/terrain_0/color = Color(0, 1, 0, 1)",
        "sources/0 = SubResource(\"TileSetAtlasSource_1\")",
    ];
    assert_eq!(lines, expected, "\n{text}");
    assert!(!text.contains("1:0/0"), "empty tiles are left out");

    // Scaled export: sizes and collision points follow.
    let out = run(&mut s, &ctx, "export_godot_tileset", json!({ "path": dir.join("x2.tres").to_str().unwrap(), "scale": 2 }));
    assert_eq!(out.data["texture_res_path"], "res://x2.png");
    let text = std::fs::read_to_string(dir.join("x2.tres")).unwrap();
    assert!(text.contains("texture_region_size = Vector2i(32, 32)"));
    assert!(text.contains("0:0/0/physics_layer_0/polygon_0/points = PackedVector2Array(-16, -16, 16, -16, 16, 16, -16, 16)"));
    assert_eq!(decode_png(&std::fs::read(dir.join("x2.png")).unwrap()).unwrap().width(), 96);
    let e = run_err(&mut s, &ctx, "export_godot_tileset", json!({ "path": dir.join("big.tres").to_str().unwrap(), "scale": 100 }));
    assert_eq!(e.code, ErrorCode::LimitExceeded);
}

#[test]
fn preview_map_is_painted_and_rendered() {
    let (mut s, ctx) = session(16, 8);
    run(
        &mut s,
        &ctx,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 0, "y": 0, "w": 8, "h": 8, "fill": true, "color": "#ff0000" },
            { "op": "rect", "x": 8, "y": 0, "w": 8, "h": 8, "fill": true, "color": "#0000ff" }
        ]}),
    );
    run(&mut s, &ctx, "tile_grid_set", json!({ "tile_width": 8 }));
    assert!(run_err(&mut s, &ctx, "tile_preview_render", json!({})).message.contains("tile_preview_map_set"));
    assert_eq!(run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "paint": [] })).code, ErrorCode::InvalidArgument);
    let out = run(
        &mut s,
        &ctx,
        "tile_preview_map_set",
        json!({ "size": [3, 2], "paint": [{ "at": [0, 0], "tile": [1, 0] }, { "at": [2, 1], "tile": [0, 0] }] }),
    );
    assert_eq!(out.data["filled"], 2);
    assert_eq!(out.images.len(), 1);
    let r = run(&mut s, &ctx, "tile_preview_render", json!({ "scale": 1, "background": "none" }));
    let img = decode_png(&r.images[0].data).unwrap();
    assert_eq!((img.width(), img.height()), (24, 16));
    assert_eq!(img.get(0, 0), Some(Color::rgb(0, 0, 255)));
    assert_eq!(img.get(7, 7), Some(Color::rgb(0, 0, 255)));
    assert_eq!(img.get(8, 0), Some(Color::TRANSPARENT));
    assert_eq!(img.get(16, 8), Some(Color::rgb(255, 0, 0)));
    assert_eq!(img.get(23, 15), Some(Color::rgb(255, 0, 0)));
    let r = run(&mut s, &ctx, "tile_preview_render", json!({ "scale": 2, "grid": true }));
    assert_eq!((r.images[0].width, r.images[0].height), (48, 32));

    // Whole map at once, fill, resize, validation.
    run(&mut s, &ctx, "tile_preview_map_set", json!({ "cells": [[0, 0], null, [0, 0], null, [1, 0], null] }));
    assert_eq!(tiles_of(&s, "chao").preview_map.as_ref().unwrap().get(1, 1), Some([1, 0]));
    assert_eq!(run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "cells": [[0, 0]] })).code, ErrorCode::InvalidArgument);
    assert_eq!(run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "fill": [2, 0] })).code, ErrorCode::OutOfBounds);
    assert_eq!(
        run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "paint": [{ "at": [3, 0], "tile": null }] })).code,
        ErrorCode::OutOfBounds
    );
    assert_eq!(run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "size": [65, 1] })).code, ErrorCode::LimitExceeded);
    run(&mut s, &ctx, "tile_preview_map_set", json!({ "size": [2, 2], "fill": [0, 0] }));
    let map = tiles_of(&s, "chao").preview_map.clone().unwrap();
    assert_eq!((map.width, map.height), (2, 2));
    assert!(map.cells.iter().all(|c| *c == Some([0, 0])));
    assert!(run_err(&mut s, &ctx, "tile_preview_map_set", json!({ "terrain_rows": ["#"] })).message.contains("terrenos"));
    run(&mut s, &ctx, "tile_preview_map_set", json!({ "delete": true }));
    assert!(tiles_of(&s, "chao").preview_map.is_none());
}
