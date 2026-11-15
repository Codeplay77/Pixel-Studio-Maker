//! RPG Maker commands: template sizes and regions, exact pixel positions in the exported sheets,
//! errors and persistence.

use std::path::PathBuf;

use serde_json::{Value, json};
use sprite_animation::RegionShape;
use sprite_animation::rpgmaker::{RpgMakerEngine, RpgMakerInfo, RpgMakerKind};
use sprite_core::codec::decode_png;
use sprite_core::{Color, Rect};
use sprite_project::rpgmaker::{TEXT_COLORS, text_color_point};

use crate::{CommandError, ErrorCode, ExecContext, Outcome, ProjectCreate, Session, create_project, parse_command};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("psm-rpgmaker-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn session(tag: &str, w: u32, h: u32) -> (Session, ExecContext, PathBuf) {
    let dir = temp_dir(tag);
    let ctx = ExecContext::local(&dir);
    let s = create_project(
        &ProjectCreate {
            name: "rpg".into(),
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
    (s, ctx, dir)
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

fn read_png(path: &std::path::Path) -> sprite_core::Image {
    decode_png(&std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

fn rect_shape(x: i32, y: i32, w: u32, h: u32) -> Option<RegionShape> {
    Some(RegionShape::Rect { rect: Rect::new(x, y, w, h) })
}

fn marker(row: usize, pattern: usize) -> Color {
    Color::rgb(20 + row as u8 * 50, 30 + pattern as u8 * 70, 200)
}

/// Animation `name` with 3 frames in each direction; every frame has a marker pixel at the pivot
/// (`marker(row, pattern)`, rows in RPG Maker order S, W, E, N) and a black pixel at (0, 0).
fn walk(s: &mut Session, ctx: &ExecContext, sprite: &str, name: &str, dirs: &[&str]) {
    run(s, ctx, "animation_create", json!({ "sprite": sprite, "name": name, "directions": dirs, "frames": 3 }));
    let origin = s.doc.model.sprites.iter().find(|sp| sp.name == sprite).unwrap().origin;
    for dir in dirs {
        let row = ["S", "W", "E", "N"].iter().position(|d| d == dir).unwrap();
        for p in 0..3 {
            let frame = json!({ "animation": name, "direction": dir, "index": p });
            run(
                s,
                ctx,
                "draw_batch",
                json!({ "sprite": sprite, "frame": frame, "ops": [
                    { "op": "pixel", "x": origin.x, "y": origin.y, "color": marker(row, p).to_hex() },
                    { "op": "pixel", "x": 0, "y": 0, "color": "#000000" }
                ]}),
            );
        }
    }
}

/// (kind, MV/MZ size, VX Ace size (0x0 = not available), regions).
type Expected = (RpgMakerKind, (u32, u32), (u32, u32), usize);

const EXPECTED: [Expected; 13] = [
    (RpgMakerKind::TilesetA1, (768, 576), (512, 384), 32),
    (RpgMakerKind::TilesetA2, (768, 576), (512, 384), 32),
    (RpgMakerKind::TilesetA3, (768, 384), (512, 256), 32),
    (RpgMakerKind::TilesetA4, (768, 720), (512, 480), 48),
    (RpgMakerKind::TilesetA5, (384, 768), (256, 512), 1),
    (RpgMakerKind::TilesetB, (768, 768), (512, 512), 2),
    (RpgMakerKind::TilesetC, (768, 768), (512, 512), 2),
    (RpgMakerKind::TilesetD, (768, 768), (512, 512), 2),
    (RpgMakerKind::TilesetE, (768, 768), (512, 512), 2),
    (RpgMakerKind::Character, (576, 384), (384, 256), 32),
    (RpgMakerKind::Face, (576, 288), (384, 192), 8),
    (RpgMakerKind::SvBattler, (576, 384), (0, 0), 18),
    (RpgMakerKind::Window, (192, 192), (128, 128), 8),
];

#[test]
fn templates_have_the_engine_sizes_and_named_regions() {
    let (mut s, ctx, _) = session("sizes", 16, 16);
    for (kind, mz, vx, regions) in EXPECTED {
        for (engine, size) in [(RpgMakerEngine::Mz, mz), (RpgMakerEngine::Mv, mz), (RpgMakerEngine::Vxace, vx)] {
            let args = json!({ "kind": kind, "engine": engine });
            if size == (0, 0) {
                let err = run_err(&mut s, &ctx, "rpgmaker_template", args);
                assert_eq!(err.code, ErrorCode::Unsupported, "{kind:?} {engine:?}");
                continue;
            }
            let out = run(&mut s, &ctx, "rpgmaker_template", args);
            assert_eq!(out.data["created"], true);
            assert_eq!(out.data["size"], json!([size.0, size.1]), "{kind:?} {engine:?}");
            assert_eq!(out.data["regions"].as_array().unwrap().len(), regions);
            let sp = s.doc.model.sprites.iter().find(|x| x.id as u64 == out.data["sprite"].as_u64().unwrap()).unwrap();
            assert_eq!((sp.width, sp.height), size, "{kind:?} {engine:?}");
            assert_eq!(sp.regions.len(), regions, "{kind:?} {engine:?}");
            assert!(sp.regions.iter().all(|r| r.part.is_some() && r.default.is_some()));
            let info = sp.extras.rpgmaker.as_ref().expect("extras.rpgmaker");
            assert_eq!((info.kind, info.engine, info.tile), (kind, engine, engine.tile()));
        }
    }
    // Names are unique per project: "Tileset A2", "Tileset A2 2", ...
    let names: Vec<&str> = s.doc.model.sprites.iter().map(|x| x.name.as_str()).collect();
    assert!(names.contains(&"Tileset A2") && names.contains(&"Tileset A2 2") && names.contains(&"Tileset A2 3"), "{names:?}");
    // Named blocks, as the RPG Maker lays them out.
    let a2 = s.doc.model.sprites.iter().find(|x| x.name == "Tileset A2").unwrap();
    let block = a2.regions.iter().find(|r| r.name == "A2 bloco 3,1").unwrap();
    assert_eq!(block.default, rect_shape(288, 144, 96, 144));
    let chars = s.doc.model.sprites.iter().find(|x| x.name == "Personagens").unwrap();
    let r = chars.regions.iter().find(|r| r.name == "personagem 5 cima").unwrap();
    assert_eq!(r.default, rect_shape(144, 336, 144, 48));
    assert_eq!(r.part.as_deref(), Some("N"));
    let battler = s.doc.model.sprites.iter().find(|x| x.name == "Battler lateral").unwrap();
    let dead = battler.regions.iter().find(|r| r.part.as_deref() == Some("dead")).unwrap();
    assert_eq!(dead.name, "morto");
    assert_eq!(dead.default, rect_shape(384, 320, 192, 64));
}

#[test]
fn template_on_an_existing_sprite_checks_the_size_and_can_be_undone() {
    let (mut s, ctx, _) = session("existing", 768, 576);
    let out = run(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "tileset_a1", "sprite": "hero" }));
    assert_eq!(out.data["created"], false);
    assert_eq!(s.doc.model.sprites.len(), 1);
    assert_eq!(s.doc.model.sprites[0].regions.len(), 32);
    assert!(s.doc.model.sprites[0].regions[0].part.as_deref().unwrap().contains("água animada"));
    // Applying again replaces the blocks instead of duplicating them.
    let again = run(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "tileset_a1", "sprite": "hero" }));
    assert_eq!(again.data["replaced_regions"], 32);
    assert_eq!(s.doc.model.sprites[0].regions.len(), 32);
    // Wrong size: clear error, nothing changes.
    let err = run_err(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "tileset_a3", "sprite": "hero" }));
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.message.contains("768x384"), "{}", err.message);
    assert_eq!(s.doc.model.sprites[0].extras.rpgmaker.as_ref().unwrap().kind, RpgMakerKind::TilesetA1);
    // VX Ace sizes differ.
    let err = run_err(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "tileset_a1", "engine": "vxace", "sprite": "hero" }));
    assert!(err.message.contains("512x384"), "{}", err.message);
    // One history step each.
    s.doc.undo().unwrap();
    s.doc.undo().unwrap();
    assert!(s.doc.model.sprites[0].extras.rpgmaker.is_none());
    assert!(s.doc.model.sprites[0].regions.is_empty());
}

#[test]
fn window_text_colors_are_where_the_engine_reads_them() {
    let (mut s, ctx, _) = session("window", 16, 16);
    for engine in [RpgMakerEngine::Mz, RpgMakerEngine::Vxace] {
        let out = run(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "window", "engine": engine, "fill_text_colors": true }));
        assert_eq!(out.data["text_colors"].as_array().unwrap().len(), 32);
        let id = out.data["sprite"].as_u64().unwrap() as u32;
        let frame = s.doc.model.sprites.iter().find(|x| x.id == id).unwrap().frames[0].id;
        let img = s.composite(id, frame).unwrap();
        for (n, color) in TEXT_COLORS.iter().enumerate() {
            let p = text_color_point(engine, n);
            assert_eq!(img.get(p.x, p.y), Some(*color), "{engine:?} cor {n}");
        }
        assert!(img.get(0, 0).unwrap().is_transparent(), "o resto fica transparente");
        let sp = s.doc.model.sprites.iter().find(|x| x.id == id).unwrap();
        let arrow = sp.regions.iter().find(|r| r.name == "seta para baixo").unwrap();
        let expected = if engine == RpgMakerEngine::Vxace { rect_shape(88, 40, 16, 8) } else { rect_shape(132, 60, 24, 12) };
        assert_eq!(arrow.default, expected);
    }
    let warn = run(&mut s, &ctx, "rpgmaker_template", json!({ "kind": "face", "fill_text_colors": true, "cell": [10, 10] }));
    assert_eq!(warn.warnings.len(), 2, "{:?}", warn.warnings);
}

#[test]
fn character_sheet_puts_directions_columns_and_baseline_in_place() {
    let (mut s, ctx, dir) = session("character", 16, 16);
    walk(&mut s, &ctx, "hero", "walk", &["N", "E", "S", "W"]);
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_character",
        json!({ "path": "img/characters/Heroi.png", "object": true, "characters": [{ "sprite": "hero", "animation": "walk" }] }),
    );
    assert_eq!(out.data["file"], "!Heroi.png");
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert_eq!(out.images.len(), 1, "prévia da folha");
    let sheet = read_png(&dir.join("img/characters/!Heroi.png"));
    assert_eq!((sheet.width(), sheet.height()), (576, 384));
    // Rows S, W, E, N; pivot (8, 15) on the bottom center (24, 47) of each 48x48 cell.
    for row in 0..4 {
        for p in 0..3 {
            let (cx, cy) = (p as i32 * 48, row as i32 * 48);
            assert_eq!(sheet.get(cx + 24, cy + 47), Some(marker(row, p)), "linha {row} coluna {p}");
            assert_eq!(sheet.get(cx + 16, cy + 32), Some(Color::BLACK), "linha {row} coluna {p}: canto do sprite");
        }
    }
    // The other character blocks stay empty.
    assert!(sheet.crop(Rect::new(144, 0, 432, 384)).is_blank());
    assert!(sheet.crop(Rect::new(0, 192, 144, 192)).is_blank());

    // Two characters: the second goes to the block at column 1.
    run(&mut s, &ctx, "sprite_create", json!({ "name": "npc", "width": 24, "height": 32 }));
    walk(&mut s, &ctx, "npc", "andar", &["S", "W", "E", "N"]);
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_character",
        json!({ "path": "Povo", "characters": [
            { "sprite": "hero", "animation": "walk" },
            { "sprite": "npc", "animation": "andar", "frames": [2, 1, 0] }
        ]}),
    );
    assert_eq!(out.data["file"], "Povo.png");
    let sheet = read_png(&dir.join("Povo.png"));
    assert_eq!(sheet.get(144 + 24, 47), Some(marker(0, 2)), "frames [2, 1, 0]: a coluna 0 recebe o frame 2");
    assert_eq!(sheet.get(144 + 2 * 48 + 24, 3 * 48 + 47), Some(marker(3, 0)));
    // npc is 24x32 with origin (12, 31): its (0, 0) lands 12 px left of and 31 px above the pivot.
    assert_eq!(sheet.get(144 + 24 - 12, 47 - 31), Some(Color::BLACK));
}

#[test]
fn single_sheets_mirroring_and_short_tracks() {
    let (mut s, ctx, dir) = session("single", 64, 64);
    walk(&mut s, &ctx, "hero", "walk", &["S", "E", "N"]);
    // 64x64 does not fit a 48x48 cell without `single`.
    let err = run_err(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "a.png", "characters": [{ "animation": "walk" }] }));
    assert_eq!(err.code, ErrorCode::InvalidArgument);
    assert!(err.message.contains("single"), "{}", err.message);
    let err = run_err(
        &mut s,
        &ctx,
        "export_rpgmaker_character",
        json!({ "path": "a.png", "single": true, "characters": [{ "animation": "walk" }, { "animation": "walk" }] }),
    );
    assert!(err.message.contains("exatamente 1"), "{}", err.message);

    // Single sheet: cell = sprite size, `$` prefix, W missing -> empty row with a warning.
    let one = json!([{ "animation": "walk" }]);
    let out = run(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "Heroi.png", "single": true, "characters": one }));
    assert_eq!(out.data["file"], "$Heroi.png");
    assert_eq!(out.data["cell"], json!([64, 64]));
    assert!(out.warnings.iter().any(|w| w.contains("direção W") && w.contains("mirror_missing")), "{:?}", out.warnings);
    let sheet = read_png(&dir.join("$Heroi.png"));
    assert_eq!((sheet.width(), sheet.height()), (192, 256));
    assert!(sheet.crop(Rect::new(0, 64, 192, 64)).is_blank(), "linha W vazia");
    assert_eq!(sheet.get(32, 63), Some(marker(0, 0)));

    // mirror_missing: W = E with each cell flipped. The black (0, 0) pixel of E sits at x = 0 of the
    // cell (pivot 32 -> 32), so in W it sits at x = 63.
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_character",
        json!({ "path": "Heroi.png", "single": true, "mirror_missing": true, "characters": one }),
    );
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert_eq!(out.data["rows"][1]["source"], "E espelhado");
    let sheet = read_png(&dir.join("$Heroi.png"));
    for p in 0..3 {
        let x = p as i32 * 64;
        assert_eq!(sheet.get(x, 2 * 64), Some(Color::BLACK), "E coluna {p}");
        assert_eq!(sheet.get(x + 63, 64), Some(Color::BLACK), "W coluna {p}");
        assert_eq!(sheet.get(x + 63 - 32, 64 + 63), Some(marker(2, p)), "W espelha o marcador de E");
    }

    // A track with 2 frames: the last one repeats in column 2, with a warning.
    run(&mut s, &ctx, "frame_delete", json!({ "frames": [{ "animation": "walk", "direction": "N", "index": 2 }] }));
    let out = run(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "b.png", "single": true, "characters": one }));
    assert!(out.warnings.iter().any(|w| w.contains("direção N") && w.contains("repetido")), "{:?}", out.warnings);
    let sheet = read_png(&dir.join("$b.png"));
    assert_eq!(sheet.get(2 * 64 + 32, 3 * 64 + 63), Some(marker(3, 1)));
    // Explicit indices beyond the track are an error.
    let explicit = json!([{ "animation": "walk", "frames": [0, 1, 2] }]);
    let err = run_err(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "b.png", "single": true, "characters": explicit }));
    assert!(err.message.contains("índice 2"), "{}", err.message);
    let two = json!([{ "animation": "walk", "frames": [0, 1] }]);
    let err = run_err(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "b.png", "single": true, "characters": two }));
    assert!(err.message.contains("3 índices"), "{}", err.message);
    // No usable direction at all.
    run(&mut s, &ctx, "animation_create", json!({ "name": "pulo", "directions": ["NE"], "frames": 1 }));
    let jump = json!([{ "animation": "pulo" }]);
    let err = run_err(&mut s, &ctx, "export_rpgmaker_character", json!({ "path": "c.png", "single": true, "characters": jump }));
    assert!(err.message.contains("S, W, E ou N"), "{}", err.message);
}

#[test]
fn battler_places_each_motion_where_sprite_actor_reads_it() {
    let (mut s, ctx, dir) = session("battler", 32, 32);
    // walk: a non-directional track with 3 frames; dead: 2 frames (the last repeats).
    run(&mut s, &ctx, "animation_create", json!({ "name": "andar", "frames": 3 }));
    run(&mut s, &ctx, "animation_create", json!({ "name": "caido", "frames": 2 }));
    run(&mut s, &ctx, "animation_create", json!({ "name": "festa", "directions": ["E", "W"], "frames": 3 }));
    for (anim, direction, count, row) in [("andar", None, 3, 0usize), ("caido", None, 2, 1), ("festa", Some("W"), 3, 2)] {
        for p in 0..count {
            let mut frame = json!({ "animation": anim, "index": p });
            if let Some(d) = direction {
                frame["direction"] = json!(d);
            }
            run(&mut s, &ctx, "pixel_set", json!({ "frame": frame, "x": 16, "y": 31, "color": marker(row, p).to_hex() }));
        }
    }
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_battler",
        json!({ "path": "img/sv_actors/Heroi.png", "motions": {
            "walk": "andar",
            "morto": "caido",
            "vitória": { "animation": "festa", "direction": "W" }
        }}),
    );
    let sheet = read_png(&dir.join("img/sv_actors/Heroi.png"));
    assert_eq!((sheet.width(), sheet.height()), (576, 384));
    // Pivot (16, 31) on the bottom center (32, 63) of each 64x64 cell.
    for p in 0..3usize {
        let x = p as i32 * 64;
        assert_eq!(sheet.get(x + 32, 63), Some(marker(0, p)), "walk {p}");
        // dead = 17: column floor(17/6)*3 + p = 6 + p, row 17 % 6 = 5.
        assert_eq!(sheet.get(6 * 64 + x + 32, 5 * 64 + 63), Some(marker(1, p.min(1))), "dead {p}");
        // victory = 13: column 6 + p, row 1.
        assert_eq!(sheet.get(6 * 64 + x + 32, 64 + 63), Some(marker(2, p)), "victory {p}");
    }
    assert!(sheet.crop(Rect::new(0, 64, 192, 64)).is_blank(), "wait (1) ficou vazio");
    assert!(out.warnings.iter().any(|w| w.contains("chant") && w.contains("sleep")), "{:?}", out.warnings);
    assert!(out.warnings.iter().any(|w| w.contains("movimento dead") && w.contains("repetido")), "{:?}", out.warnings);
    assert_eq!(out.data["empty"].as_array().unwrap().len(), 15);

    let err =
        run_err(&mut s, &ctx, "export_rpgmaker_battler", json!({ "path": "x.png", "engine": "vxace", "motions": { "walk": "andar" } }));
    assert_eq!(err.code, ErrorCode::Unsupported);
    let err = run_err(&mut s, &ctx, "export_rpgmaker_battler", json!({ "path": "x.png", "motions": { "voar": "andar" } }));
    assert!(err.message.contains("voar"), "{}", err.message);
    let err =
        run_err(&mut s, &ctx, "export_rpgmaker_battler", json!({ "path": "x.png", "motions": { "walk": "andar", "andar": "caido" } }));
    assert!(err.message.contains("mesmo movimento"), "{}", err.message);
    // Bigger than the cell: error, unless the cell grows (9x6 cells of any size).
    let err = run_err(&mut s, &ctx, "export_rpgmaker_battler", json!({ "path": "x.png", "scale": 3, "motions": { "walk": "andar" } }));
    assert!(err.message.contains("cell"), "{}", err.message);
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_battler",
        json!({ "path": "x.png", "scale": 3, "cell": [96, 96], "motions": { "walk": "andar" } }),
    );
    assert_eq!(out.data["size"], json!([864, 576]));
    let sheet = read_png(&dir.join("x.png"));
    // Pivot pixel scaled 3x: x 48..51 of the frame, bottom row 95 -> cell x 48, y 95.
    assert_eq!(sheet.get(96 + 48, 95), Some(marker(0, 1)));
    assert_eq!(sheet.get(96 + 50, 93), Some(marker(0, 1)));
}

#[test]
fn faces_are_centered_in_a_four_by_two_grid() {
    let (mut s, ctx, dir) = session("faces", 48, 48);
    run(&mut s, &ctx, "frame_create", json!({ "count": 1 }));
    let frames: Vec<u32> = s.doc.model.sprites[0].frames.iter().map(|f| f.id).collect();
    run(&mut s, &ctx, "pixel_set", json!({ "frame": frames[0], "x": 0, "y": 0, "color": "#ff0000" }));
    run(&mut s, &ctx, "pixel_set", json!({ "frame": frames[1], "x": 47, "y": 47, "color": "#00ff00" }));
    let out = run(
        &mut s,
        &ctx,
        "export_rpgmaker_faces",
        json!({ "path": "img/faces/Heroi.png", "scale": 2, "faces": [{ "frame": frames[0] }, { "frame": frames[1], "index": 5 }] }),
    );
    assert_eq!(out.data["size"], json!([576, 288]));
    let sheet = read_png(&dir.join("img/faces/Heroi.png"));
    // A 96x96 face centered in 144x144: 24 px of margin.
    assert_eq!(sheet.get(24, 24), Some(Color::rgb(255, 0, 0)));
    assert_eq!(sheet.get(25, 25), Some(Color::rgb(255, 0, 0)));
    assert!(sheet.get(23, 23).unwrap().is_transparent());
    // Face 5: column 1, row 1.
    assert_eq!(sheet.get(144 + 24 + 95, 144 + 24 + 95), Some(Color::rgb(0, 255, 0)));
    let vx = run(&mut s, &ctx, "export_rpgmaker_faces", json!({ "path": "vx.png", "engine": "vxace", "faces": [{ "frame": frames[0] }] }));
    assert_eq!(vx.data["size"], json!([384, 192]));
    assert_eq!(read_png(&dir.join("vx.png")).get(24, 24), Some(Color::rgb(255, 0, 0)));
    let err = run_err(&mut s, &ctx, "export_rpgmaker_faces", json!({ "path": "x.png", "scale": 4, "faces": [{ "frame": frames[0] }] }));
    assert!(err.message.contains("não cabe"), "{}", err.message);
    let err = run_err(
        &mut s,
        &ctx,
        "export_rpgmaker_faces",
        json!({ "path": "x.png", "faces": [{ "frame": frames[0], "index": 2 }, { "frame": frames[1], "index": 2 }] }),
    );
    assert!(err.message.contains("posição 2"), "{}", err.message);
    let err = run_err(&mut s, &ctx, "export_rpgmaker_faces", json!({ "path": "x.png", "faces": [] }));
    assert_eq!(err.code, ErrorCode::InvalidArgument);
}

#[test]
fn rpgmaker_info_survives_save_and_open() {
    let (mut s, ctx, dir) = session("persist", 16, 16);
    run(
        &mut s,
        &ctx,
        "rpgmaker_template",
        json!({ "kind": "character", "engine": "vxace", "single": true, "cell": [32, 48], "name": "Heroi" }),
    );
    let path = dir.join("rpg.psmproj");
    sprite_project::format::save(&s.doc, &path).unwrap();
    let doc = sprite_project::format::load(&path).unwrap();
    let sp = doc.model.sprites.iter().find(|x| x.name == "Heroi").unwrap();
    assert_eq!(
        sp.extras.rpgmaker,
        Some(RpgMakerInfo { kind: RpgMakerKind::Character, engine: RpgMakerEngine::Vxace, tile: 32, cell: Some([32, 48]), single: true })
    );
    assert_eq!((sp.width, sp.height), (96, 192));
    assert_eq!(sp.regions.len(), 4);
    assert_eq!(sp.regions[3].name, "cima");
    // Sprites without a template keep no RPG Maker data.
    assert!(doc.model.sprites[0].extras.rpgmaker.is_none());
}
