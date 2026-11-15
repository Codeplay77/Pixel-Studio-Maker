//! Godot 4 integration: the animated scene (`.tscn`) generator and the export jobs stored in
//! the project (run on save and in batch by `psm export`).
//!
//! The scene places an `AnimatedSprite2D` (driven by the `SpriteFrames` written by
//! [`crate::export::godot_sprite_frames`]), one `Area2D` + `CollisionShape2D` per hitbox, one
//! `Marker2D` per anchor and an `AnimationPlayer` whose animations switch frames and move/enable
//! the boxes and markers frame by frame. Node positions are relative to the sprite pivot.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::extras::godot::Hitbox;
use sprite_animation::sheet::{Placement, SheetPlan};
use sprite_animation::{LoopMode, Sprite};
use sprite_core::{Color, Point, Rect};

// ---- export jobs ------------------------------------------------------------------------------

/// A saved export: a command of the command layer (`export_*`) with its arguments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ExportJob {
    /// Unique name of the job.
    pub name: String,
    /// Export command (`export_png`, `export_spritesheet`, `export_godot_scene`...).
    pub command: String,
    /// Arguments of the command. Relative paths start at the `.psmproj` folder.
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
    /// Run automatically after every successful `project_save`.
    #[serde(default)]
    pub on_save: bool,
}

/// Godot data of a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GodotProject {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub export_jobs: Vec<ExportJob>,
}

impl GodotProject {
    pub fn is_empty(&self) -> bool {
        self.export_jobs.is_empty()
    }
}

// ---- scene ------------------------------------------------------------------------------------

/// Options of [`godot_scene`].
#[derive(Debug, Clone)]
pub struct SceneOptions {
    /// `res://` path of the SpriteFrames resource (`.tres`).
    pub frames_res_path: String,
    /// Export scale (every position and size is multiplied by it).
    pub scale: u32,
    /// Start the first animation when the scene runs.
    pub autoplay: bool,
}

/// One AnimationPlayer animation (one SpriteFrames animation).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneAnimation {
    pub name: String,
    /// Frame ids in key order.
    pub frames: Vec<u32>,
    /// Key times in milliseconds (start of each frame).
    pub times_ms: Vec<u32>,
    pub length_ms: u32,
    /// Godot `loop_mode`: 0 none, 1 linear, 2 ping-pong.
    pub loop_mode: u8,
}

/// Summary of a generated scene (also used by tests).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SceneInfo {
    pub root: String,
    /// Node path of each hitbox (`Area2D`), by hitbox name.
    pub hitboxes: BTreeMap<String, String>,
    /// Node path of each anchor (`Marker2D`), by anchor name.
    pub markers: BTreeMap<String, String>,
    pub animations: Vec<SceneAnimation>,
    /// `AnimatedSprite2D.offset` (the pivot lands on the node origin).
    pub offset: [f64; 2],
    pub load_steps: usize,
}

fn godot_anim_name(anim: &str, dir: &Option<String>) -> String {
    match dir {
        Some(d) => format!("{anim}_{d}"),
        None => anim.to_string(),
    }
}

/// Valid, readable Godot node name (no `. : @ / " %`).
pub fn node_name(raw: &str) -> String {
    let cleaned: String =
        raw.trim().chars().map(|c| if matches!(c, '.' | ':' | '@' | '/' | '"' | '%' | '\\') || c.is_control() { '_' } else { c }).collect();
    if cleaned.is_empty() { "Node".into() } else { cleaned }
}

fn unique_name(base: String, used: &mut BTreeSet<String>) -> String {
    let mut name = base.clone();
    let mut n = 2;
    while used.contains(&name.to_lowercase()) {
        name = format!("{base}{n}");
        n += 1;
    }
    used.insert(name.to_lowercase());
    name
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Shortest decimal form (`3`, `-2.5`, `0.1`).
fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v}") }
}

fn vec2(x: f64, y: f64) -> String {
    format!("Vector2({}, {})", num(x), num(y))
}

fn color(c: Color, alpha: f64) -> String {
    let f = |v: u8| num((v as f64 / 255.0 * 1000.0).round() / 1000.0);
    format!("Color({}, {}, {}, {})", f(c.r), f(c.g), f(c.b), num(alpha))
}

/// Where a frame's pixel (0, 0) is inside its texture: the cell for grids, the full frame for
/// packed atlases (their `AtlasTexture` margins restore the original frame size).
fn frame_origin(p: &Placement, packed: bool) -> Point {
    if packed { Point::new(0, 0) } else { Point::new(p.dest.x - p.cell.x - p.source.x, p.dest.y - p.cell.y - p.source.y) }
}

struct Ctx {
    s: f64,
    packed: bool,
    /// Texture offset of the AnimatedSprite2D (unscaled texture pixels).
    offset: Point,
}

impl Ctx {
    /// Node-space position of a frame pixel coordinate (fractions allowed).
    fn node(&self, p: &Placement, x: f64, y: f64) -> (f64, f64) {
        let o = frame_origin(p, self.packed);
        ((o.x as f64 + x + self.offset.x as f64) * self.s, (o.y as f64 + y + self.offset.y as f64) * self.s)
    }

    fn box_center(&self, p: &Placement, r: Rect) -> (f64, f64) {
        self.node(p, r.x as f64 + r.w as f64 / 2.0, r.y as f64 + r.h as f64 / 2.0)
    }

    fn box_size(&self, r: Rect) -> (f64, f64) {
        (r.w as f64 * self.s, r.h as f64 * self.s)
    }
}

struct Track {
    path: String,
    times: Vec<String>,
    values: Vec<String>,
}

fn track_text(i: usize, t: &Track) -> String {
    let transitions = vec!["1"; t.times.len()].join(", ");
    format!(
        "tracks/{i}/type = \"value\"\ntracks/{i}/imported = false\ntracks/{i}/enabled = true\ntracks/{i}/path = NodePath({})\ntracks/{i}/interp = 1\ntracks/{i}/loop_wrap = true\ntracks/{i}/keys = {{\n\"times\": PackedFloat32Array({}),\n\"transitions\": PackedFloat32Array({transitions}),\n\"update\": 1,\n\"values\": [{}]\n}}\n",
        quote(&t.path),
        t.times.join(", "),
        t.values.join(", ")
    )
}

/// The Godot 4 scene (`.tscn`) for `sprite` laid out by `plan` (the same plan used for the sheet
/// and the SpriteFrames). Returns the text and a summary.
pub fn godot_scene(sprite: &Sprite, plan: &SheetPlan, opts: &SceneOptions) -> (String, SceneInfo) {
    let s = opts.scale.max(1) as f64;
    let packed = plan.cell_width == 0;
    let by_track: BTreeMap<(u32, Option<String>, Option<String>), &Placement> =
        plan.placements.iter().map(|p| ((p.frame, p.animation.clone(), p.direction.clone()), p)).collect();
    // The pivot of the first frame lands on the node origin; every other frame keeps its place
    // in the sheet cell relative to it (with `align: pivot`, every pivot lands there).
    let offset = plan
        .placements
        .first()
        .map(|p| {
            let o = frame_origin(p, packed);
            let pv = sprite.pivot(p.frame);
            Point::new(-(o.x + pv.x), -(o.y + pv.y))
        })
        .unwrap_or_default();
    let cx = Ctx { s, packed, offset };

    // Animations: one per track present in the plan, then "default" for frames outside animations.
    let mut anims: Vec<(SceneAnimation, Vec<&Placement>)> = Vec::new();
    for anim in &sprite.animations {
        for track in &anim.tracks {
            let seq = anim.sequence(track);
            let keyed: Vec<u32> = if anim.loop_mode == LoopMode::PingPong { track.frames.clone() } else { seq };
            let places: Vec<&Placement> =
                keyed.iter().filter_map(|f| by_track.get(&(*f, Some(anim.name.clone()), track.direction.clone())).copied()).collect();
            if places.is_empty() || places.len() != keyed.len() {
                continue;
            }
            let loop_mode = match anim.loop_mode {
                LoopMode::Once => 0,
                LoopMode::PingPong => 2,
                LoopMode::Loop | LoopMode::Reverse => 1,
            };
            anims.push((timed(godot_anim_name(&anim.name, &track.direction), &places, sprite, loop_mode), places));
        }
    }
    let free: Vec<&Placement> = plan.placements.iter().filter(|p| p.animation.is_none()).collect();
    if !free.is_empty() {
        anims.push((timed("default".into(), &free, sprite, 1), free));
    }

    // Node names.
    let mut used: BTreeSet<String> = ["sprite", "animationplayer"].into_iter().map(String::from).collect();
    let root = node_name(&sprite.name);
    let boxes: Vec<(&Hitbox, String)> =
        sprite.extras.godot.hitboxes.iter().map(|h| (h, unique_name(node_name(&h.name), &mut used))).collect();
    let markers: Vec<(&sprite_animation::Anchor, String)> =
        sprite.anchors.iter().map(|a| (a, unique_name(node_name(&a.name), &mut used))).collect();

    // Resources.
    let mut subs = String::new();
    let mut shape_ids = Vec::new();
    let first = anims.first().and_then(|(_, p)| p.first().copied()).or(plan.placements.first());
    for (i, (hb, _)) in boxes.iter().enumerate() {
        let id = format!("RectangleShape2D_{}", i + 1);
        let start = first.and_then(|p| hb.at(p.frame)).or_else(|| hb.frames.values().next().copied()).unwrap_or(Rect::new(0, 0, 1, 1));
        let (w, h) = cx.box_size(start);
        subs.push_str(&format!(
            "[sub_resource type=\"RectangleShape2D\" id=\"{id}\"]\nresource_local_to_scene = true\nsize = {}\n\n",
            vec2(w, h)
        ));
        shape_ids.push(id);
    }
    let mut anim_ids = Vec::new();
    for (k, (an, places)) in anims.iter().enumerate() {
        let id = format!("Animation_{}", k + 1);
        let times: Vec<String> = an.times_ms.iter().map(|t| num(*t as f64 / 1000.0)).collect();
        let mut tracks = vec![
            Track { path: "Sprite:animation".into(), times: vec!["0".into()], values: vec![format!("&{}", quote(&an.name))] },
            Track { path: "Sprite:frame".into(), times: times.clone(), values: (0..places.len()).map(|i| i.to_string()).collect() },
        ];
        for (hb, name) in &boxes {
            let mut pos = Track { path: format!("{name}/Shape:position"), times: vec![], values: vec![] };
            let mut size = Track { path: format!("{name}/Shape:shape:size"), times: vec![], values: vec![] };
            let mut off = Track { path: format!("{name}/Shape:disabled"), times: vec![], values: vec![] };
            for (i, p) in places.iter().enumerate() {
                match hb.at(p.frame) {
                    Some(r) => {
                        let (x, y) = cx.box_center(p, r);
                        let (w, h) = cx.box_size(r);
                        pos.times.push(times[i].clone());
                        pos.values.push(vec2(x, y));
                        size.times.push(times[i].clone());
                        size.values.push(vec2(w, h));
                        off.times.push(times[i].clone());
                        off.values.push("false".into());
                    }
                    None => {
                        off.times.push(times[i].clone());
                        off.values.push("true".into());
                    }
                }
            }
            if !pos.times.is_empty() {
                tracks.push(pos);
                tracks.push(size);
            }
            tracks.push(off);
        }
        for (an_ref, name) in &markers {
            let mut pos = Track { path: format!("{name}:position"), times: vec![], values: vec![] };
            for (i, p) in places.iter().enumerate() {
                if let Some(pt) = an_ref.at(p.frame) {
                    let (x, y) = cx.node(p, pt.x as f64, pt.y as f64);
                    pos.times.push(times[i].clone());
                    pos.values.push(vec2(x, y));
                }
            }
            if !pos.times.is_empty() {
                tracks.push(pos);
            }
        }
        subs.push_str(&format!(
            "[sub_resource type=\"Animation\" id=\"{id}\"]\nresource_name = {}\nlength = {}\nloop_mode = {}\nstep = 0.001\n",
            quote(&an.name),
            num(an.length_ms as f64 / 1000.0),
            an.loop_mode
        ));
        for (i, t) in tracks.iter().enumerate() {
            subs.push_str(&track_text(i, t));
        }
        subs.push('\n');
        anim_ids.push(id);
    }
    let library = !anims.is_empty();
    if library {
        let entries: Vec<String> =
            anims.iter().zip(&anim_ids).map(|((an, _), id)| format!("{}: SubResource(\"{id}\")", quote(&an.name))).collect();
        subs.push_str(&format!(
            "[sub_resource type=\"AnimationLibrary\" id=\"AnimationLibrary_1\"]\n_data = {{\n{}\n}}\n\n",
            entries.join(",\n")
        ));
    }
    let load_steps = 1 + shape_ids.len() + anim_ids.len() + usize::from(library) + 1;

    // Nodes.
    let mut out = String::new();
    out.push_str(&format!("[gd_scene load_steps={load_steps} format=3]\n\n"));
    out.push_str(&format!("[ext_resource type=\"SpriteFrames\" path={} id=\"1_frames\"]\n\n", quote(&opts.frames_res_path)));
    out.push_str(&subs);
    out.push_str(&format!("[node name={} type=\"Node2D\"]\ntexture_filter = 1\n\n", quote(&root)));
    out.push_str("[node name=\"Sprite\" type=\"AnimatedSprite2D\" parent=\".\"]\nsprite_frames = ExtResource(\"1_frames\")\n");
    if let Some((an, _)) = anims.first() {
        out.push_str(&format!("animation = &{}\n", quote(&an.name)));
    }
    let off = [offset.x as f64 * s, offset.y as f64 * s];
    out.push_str(&format!("centered = false\noffset = {}\n\n", vec2(off[0], off[1])));
    let mut info_boxes = BTreeMap::new();
    for ((hb, name), shape) in boxes.iter().zip(&shape_ids) {
        out.push_str(&format!(
            "[node name={} type=\"Area2D\" parent=\".\" groups=[{}]]\nmetadata/psm_kind = {}\n\n",
            quote(name),
            quote(hb.kind.as_str()),
            quote(hb.kind.as_str())
        ));
        let start = first.and_then(|p| hb.at(p.frame).map(|r| cx.box_center(p, r)));
        out.push_str(&format!("[node name=\"Shape\" type=\"CollisionShape2D\" parent={}]\n", quote(name)));
        if let Some((x, y)) = start {
            out.push_str(&format!("position = {}\n", vec2(x, y)));
        }
        out.push_str(&format!("shape = SubResource(\"{shape}\")\n"));
        if start.is_none() {
            out.push_str("disabled = true\n");
        }
        out.push_str(&format!("debug_color = {}\n\n", color(hb.display_color(), 0.42)));
        info_boxes.insert(hb.name.clone(), name.clone());
    }
    let mut info_markers = BTreeMap::new();
    for (an, name) in &markers {
        out.push_str(&format!("[node name={} type=\"Marker2D\" parent=\".\"]\n", quote(name)));
        if let Some((x, y)) = first.and_then(|p| an.at(p.frame).map(|pt| cx.node(p, pt.x as f64, pt.y as f64))) {
            out.push_str(&format!("position = {}\n", vec2(x, y)));
        }
        out.push('\n');
        info_markers.insert(an.name.clone(), name.clone());
    }
    out.push_str("[node name=\"AnimationPlayer\" type=\"AnimationPlayer\" parent=\".\"]\n");
    if library {
        out.push_str("libraries = {\n\"\": SubResource(\"AnimationLibrary_1\")\n}\n");
        if opts.autoplay {
            out.push_str(&format!("autoplay = {}\n", quote(&anims[0].0.name)));
        }
    }
    let info = SceneInfo {
        root,
        hitboxes: info_boxes,
        markers: info_markers,
        animations: anims.into_iter().map(|(a, _)| a).collect(),
        offset: off,
        load_steps,
    };
    (out, info)
}

fn timed(name: String, places: &[&Placement], sprite: &Sprite, loop_mode: u8) -> SceneAnimation {
    let mut t = 0;
    let mut times = Vec::new();
    for p in places {
        times.push(t);
        t += sprite.frame(p.frame).map(|f| f.duration_ms).unwrap_or(p.duration_ms);
    }
    SceneAnimation { name, frames: places.iter().map(|p| p.frame).collect(), times_ms: times, length_ms: t, loop_mode }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_animation::extras::godot::HitboxKind;
    use sprite_animation::sheet::{SheetLayout, SheetOptions, plan};
    use sprite_animation::{Anchor, AnchorKind, Animation, CycleInfo, Frame, ImageStore, Track as AnimTrack};

    use crate::export::{SheetSelection, sheet_items};

    fn hero() -> (Sprite, ImageStore) {
        let mut n = 100;
        let mut ids = move || {
            n += 1;
            n
        };
        let mut s = Sprite::new(1, "hero", 16, 16, &mut ids);
        s.origin = Point::new(8, 15);
        s.frames = vec![
            Frame { id: 10, duration_ms: 100, tags: vec![] },
            Frame { id: 11, duration_ms: 150, tags: vec![] },
            Frame { id: 12, duration_ms: 50, tags: vec![] },
        ];
        s.animations.push(Animation {
            id: 20,
            name: "attack".into(),
            loop_mode: LoopMode::Once,
            tags: vec![],
            tracks: vec![AnimTrack { direction: Some("E".into()), frames: vec![10, 11, 12] }],
            cycle: CycleInfo::default(),
        });
        s.extras.godot.hitboxes.push(Hitbox {
            id: 30,
            name: "golpe".into(),
            kind: HitboxKind::Hitbox,
            color: None,
            frames: BTreeMap::from([(11, Rect::new(10, 4, 4, 2))]),
        });
        s.extras.godot.hitboxes.push(Hitbox {
            id: 31,
            name: "corpo".into(),
            kind: HitboxKind::Hurtbox,
            color: Some(Color::rgb(0, 255, 0)),
            frames: BTreeMap::from([(10, Rect::new(4, 2, 8, 14)), (11, Rect::new(4, 2, 8, 14)), (12, Rect::new(5, 3, 7, 13))]),
        });
        s.anchors.push(Anchor {
            id: 40,
            name: "mão".into(),
            kind: AnchorKind::Attachment,
            default: Some(Point::new(12, 8)),
            positions: BTreeMap::from([(11, Point::new(14, 6))]),
            color: None,
        });
        (s, ImageStore::new())
    }

    fn scene(s: &Sprite, store: &ImageStore, opts: SheetOptions, scale: u32) -> (String, SceneInfo) {
        let items = sheet_items(s, store, &SheetSelection::default()).unwrap();
        let p = plan(&items, s.width, s.height, &opts);
        godot_scene(s, &p, &SceneOptions { frames_res_path: "res://hero.tres".into(), scale, autoplay: true })
    }

    /// Every `SubResource("id")`/`ExtResource("id")` points at a declared resource, and
    /// `load_steps` counts them all plus one.
    fn check_references(text: &str) {
        let declared: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with("[sub_resource") || l.starts_with("[ext_resource"))
            .map(|l| l.split("id=\"").nth(1).unwrap().split('"').next().unwrap())
            .collect();
        let unique: BTreeSet<&&str> = declared.iter().collect();
        assert_eq!(unique.len(), declared.len(), "ids únicos");
        for kw in ["SubResource(\"", "ExtResource(\""] {
            for part in text.split(kw).skip(1) {
                let id = part.split('"').next().unwrap();
                assert!(declared.contains(&id), "referência a {id} não declarada");
            }
        }
        let steps: usize = text.lines().next().unwrap().split("load_steps=").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
        assert_eq!(steps, declared.len() + 1);
    }

    #[test]
    fn scene_structure_and_keys() {
        let (s, store) = hero();
        let (text, info) = scene(&s, &store, SheetOptions { layout: SheetLayout::Rows, ..Default::default() }, 1);
        check_references(&text);
        assert!(text.starts_with("[gd_scene load_steps=6 format=3]"), "{text}");
        assert!(text.contains("[ext_resource type=\"SpriteFrames\" path=\"res://hero.tres\" id=\"1_frames\"]"));
        assert!(text.contains("[node name=\"hero\" type=\"Node2D\"]"));
        assert!(text.contains("centered = false\noffset = Vector2(-8, -15)"));
        assert!(text.contains("[node name=\"golpe\" type=\"Area2D\" parent=\".\" groups=[\"hitbox\"]]"));
        assert!(text.contains("[node name=\"Shape\" type=\"CollisionShape2D\" parent=\"golpe\"]"));
        assert!(text.contains("[node name=\"mão\" type=\"Marker2D\" parent=\".\"]\nposition = Vector2(4, -7)"));
        assert!(text.contains("libraries = {\n\"\": SubResource(\"AnimationLibrary_1\")\n}"));
        assert!(text.contains("autoplay = \"attack_E\""));
        assert!(text.contains("resource_name = \"attack_E\"\nlength = 0.3\nloop_mode = 0"));
        // Frames switch at 0, 100 and 250 ms.
        assert!(text.contains("tracks/1/path = NodePath(\"Sprite:frame\")"));
        assert!(text.contains("\"times\": PackedFloat32Array(0, 0.1, 0.25),\n\"transitions\": PackedFloat32Array(1, 1, 1),\n\"update\": 1,\n\"values\": [0, 1, 2]"));
        assert!(text.contains("\"values\": [&\"attack_E\"]"));
        // "golpe" only exists in the middle frame: center (12, 5) minus pivot (8, 15).
        assert!(text.contains("NodePath(\"golpe/Shape:position\")"));
        assert!(text.contains(
            "\"times\": PackedFloat32Array(0.1),\n\"transitions\": PackedFloat32Array(1),\n\"update\": 1,\n\"values\": [Vector2(4, -10)]"
        ));
        assert!(text.contains("\"values\": [Vector2(4, 2)]"), "size of golpe");
        assert!(text.contains("NodePath(\"golpe/Shape:disabled\")"));
        assert!(text.contains("\"values\": [true, false, true]"));
        assert!(text.contains("NodePath(\"golpe/Shape:shape:size\")"));
        // "corpo": 7x13 at (5, 3) in the last frame → center (8.5, 9.5) - pivot.
        assert!(text.contains("Vector2(0.5, -5.5)"));
        // Anchor: default, then its own position in frame 11.
        assert!(text.contains("NodePath(\"mão:position\")"));
        assert!(text.contains("\"values\": [Vector2(4, -7), Vector2(6, -9), Vector2(4, -7)]"));
        assert_eq!(info.animations[0].times_ms, vec![0, 100, 250]);
        assert_eq!(info.animations[0].length_ms, 300);
        assert_eq!(info.hitboxes["golpe"], "golpe");
        // The first frame has no "golpe": it starts disabled.
        let golpe_shape = text.split("parent=\"golpe\"]").nth(1).unwrap().split("\n\n").next().unwrap();
        assert!(golpe_shape.contains("disabled = true"), "{golpe_shape}");
    }

    #[test]
    fn scale_loop_modes_and_unique_names() {
        let (mut s, store) = hero();
        s.animations[0].loop_mode = LoopMode::PingPong;
        s.anchors[0].name = "Sprite".into();
        s.extras.godot.hitboxes[1].name = "corpo/a.b".into();
        let (text, info) = scene(&s, &store, SheetOptions { layout: SheetLayout::Horizontal, ..Default::default() }, 2);
        check_references(&text);
        assert!(text.contains("loop_mode = 2"));
        assert!(text.contains("offset = Vector2(-16, -30)"));
        assert_eq!(info.markers["Sprite"], "Sprite2", "the reserved name gets a suffix");
        assert_eq!(info.hitboxes["corpo/a.b"], "corpo_a_b");
        assert!(text.contains("NodePath(\"corpo_a_b/Shape:position\")"));
        // Ping-pong keys the forward frames only (Godot plays them back).
        assert_eq!(info.animations[0].frames, vec![10, 11, 12]);
        s.animations[0].loop_mode = LoopMode::Reverse;
        let (_, info) = scene(&s, &store, SheetOptions { layout: SheetLayout::Horizontal, ..Default::default() }, 1);
        assert_eq!(info.animations[0].frames, vec![12, 11, 10]);
        assert_eq!(info.animations[0].loop_mode, 1);
        assert_eq!(info.animations[0].times_ms, vec![0, 50, 200]);
    }

    #[test]
    fn trimmed_sheets_keep_positions_relative_to_the_pivot() {
        let (s, store) = hero();
        // With content, trimming moves frames inside their cells; node positions must not move.
        let mut store = store;
        let layer = s.layers[0].id;
        let mut s = s;
        for (i, f) in [10u32, 11, 12].iter().enumerate() {
            let mut img = sprite_core::Image::new(16, 16);
            img.set(4 + i as i32, 2, Color::BLACK);
            img.set(11, 15, Color::BLACK);
            store.insert(*f + 1000, img);
            s.cels.push(sprite_animation::Cel { layer, frame: *f, image: *f + 1000, opacity: 255 });
        }
        let (plain, _) = scene(&s, &store, SheetOptions { layout: SheetLayout::Rows, ..Default::default() }, 1);
        let (trimmed, _) = scene(&s, &store, SheetOptions { layout: SheetLayout::Rows, trim: true, ..Default::default() }, 1);
        let (packed, _) = scene(&s, &store, SheetOptions { layout: SheetLayout::Packed, ..Default::default() }, 1);
        let tracks = |t: &str| {
            t.split("[sub_resource type=\"Animation\"")
                .nth(1)
                .unwrap()
                .split("[sub_resource type=\"AnimationLibrary\"")
                .next()
                .unwrap()
                .to_string()
        };
        assert_eq!(tracks(&plain), tracks(&trimmed));
        assert_eq!(tracks(&plain), tracks(&packed));
        check_references(&packed);
    }

    #[test]
    fn sprites_without_animations_get_a_default_animation() {
        let (mut s, store) = hero();
        s.animations.clear();
        let (text, info) = scene(&s, &store, SheetOptions::default(), 1);
        check_references(&text);
        assert_eq!(info.animations.len(), 1);
        assert_eq!(info.animations[0].name, "default");
        assert_eq!(info.animations[0].loop_mode, 1);
        assert!(text.contains("animation = &\"default\""));
    }
}
