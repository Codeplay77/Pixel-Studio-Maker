//! Exports: frames, sheets, GIFs, JSON metadata and Godot SpriteFrames. Everything scales with
//! nearest-neighbor only.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_animation::sheet::{Placement, SheetItem, SheetPlan};
use sprite_animation::{ImageStore, LoopMode, Sprite};
use sprite_core::palette::median_cut;
use sprite_core::{Color, Image, ItemRef, Palette, PasteMode, Point};

use crate::ProjectError;

pub fn scale_image(img: &Image, scale: u32) -> Image {
    if scale <= 1 { img.clone() } else { img.scale_nearest(img.width() * scale, img.height() * scale) }
}

/// Which frames go into a sheet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SheetSelection {
    /// Animations to include (all when empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<ItemRef>,
    /// Directions to include (all when empty).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    /// Also include frames that belong to no animation (always on when the sprite has none).
    #[serde(default)]
    pub include_unassigned: bool,
}

/// Lists the frames for a sheet in order: animations in project order, tracks in direction
/// order (non-directional first), frames in track order.
pub fn sheet_items(sprite: &Sprite, images: &ImageStore, sel: &SheetSelection) -> Result<Vec<SheetItem>, ProjectError> {
    let mut wanted = Vec::new();
    for r in &sel.animations {
        let a = sprite.animation(r).map_err(|e| ProjectError::Transaction(e.to_string()))?;
        wanted.push(a.id);
    }
    let dirs: Vec<String> = sel
        .directions
        .iter()
        .map(|d| sprite.direction_name(d).map_err(|e| ProjectError::Transaction(e.to_string())))
        .collect::<Result<_, _>>()?;
    let dir_order = |d: &Option<String>| match d {
        None => 0,
        Some(n) => 1 + sprite.directions.iter().position(|x| &x.name == n).unwrap_or(999),
    };
    let opts = ComposeOptions::default();
    let mut items = Vec::new();
    let mut group = 0;
    let make = |frame: u32, animation: Option<String>, direction: Option<String>, index: usize, group: usize| {
        let img = compose_frame(sprite, images, frame, &opts);
        SheetItem {
            frame,
            animation,
            direction,
            index,
            duration_ms: sprite.frame(frame).map(|f| f.duration_ms).unwrap_or(100),
            group,
            content: img.opaque_bounds(),
            pivot: sprite.pivot(frame),
        }
    };
    for anim in &sprite.animations {
        if !wanted.is_empty() && !wanted.contains(&anim.id) {
            continue;
        }
        let mut tracks: Vec<_> = anim.tracks.iter().collect();
        tracks.sort_by_key(|t| dir_order(&t.direction));
        for track in tracks {
            if !dirs.is_empty() && !track.direction.as_ref().is_some_and(|d| dirs.contains(d)) {
                continue;
            }
            for (i, f) in track.frames.iter().enumerate() {
                items.push(make(*f, Some(anim.name.clone()), track.direction.clone(), i, group));
            }
            group += 1;
        }
    }
    if sel.include_unassigned || sprite.animations.is_empty() {
        let used: std::collections::BTreeSet<u32> =
            sprite.animations.iter().flat_map(|a| a.tracks.iter().flat_map(|t| t.frames.iter().copied())).collect();
        let free: Vec<u32> = sprite.frames.iter().map(|f| f.id).filter(|f| !used.contains(f)).collect();
        for (i, f) in free.into_iter().enumerate() {
            items.push(make(f, None, None, i, group));
        }
    }
    if items.is_empty() {
        return Err(ProjectError::Transaction("nenhum frame selecionado para a sprite sheet".into()));
    }
    Ok(items)
}

/// Draws the sheet described by `plan`, scaled by `scale`.
pub fn render_sheet(sprite: &Sprite, images: &ImageStore, plan: &SheetPlan, scale: u32, background: Option<Color>) -> Image {
    let s = scale.max(1);
    let mut sheet = Image::filled(plan.width * s, plan.height * s, background.unwrap_or(Color::TRANSPARENT));
    let opts = ComposeOptions::default();
    let mut cache: BTreeMap<u32, Image> = BTreeMap::new();
    for p in &plan.placements {
        let frame = cache.entry(p.frame).or_insert_with(|| compose_frame(sprite, images, p.frame, &opts));
        let part = scale_image(&frame.crop(p.source), s);
        sheet.paste(&part, Point::new(p.dest.x * s as i32, p.dest.y * s as i32), PasteMode::Over);
    }
    sheet
}

fn scaled(p: &Placement, s: i32) -> serde_json::Value {
    json!({
        "frame": p.frame,
        "animation": p.animation,
        "direction": p.direction,
        "index": p.index,
        "duration": p.duration_ms,
        "x": p.cell.x * s, "y": p.cell.y * s, "w": p.cell.w as i32 * s, "h": p.cell.h as i32 * s,
        "source": { "x": p.source.x, "y": p.source.y, "w": p.source.w, "h": p.source.h },
        "dest": [p.dest.x * s, p.dest.y * s],
        "pivot": [p.pivot.x * s, p.pivot.y * s],
    })
}

/// JSON metadata describing a sheet (documented in `docs/exportacao.md`).
pub fn sheet_metadata(sprite: &Sprite, plan: &SheetPlan, image_file: &str, scale: u32) -> serde_json::Value {
    let s = scale.max(1) as i32;
    let frames: Vec<serde_json::Value> = plan.placements.iter().map(|p| scaled(p, s)).collect();
    let mut animations = Vec::new();
    for anim in &sprite.animations {
        for track in &anim.tracks {
            let indices: Vec<usize> = anim
                .sequence(track)
                .iter()
                .filter_map(|f| {
                    plan.placements
                        .iter()
                        .position(|p| p.frame == *f && p.animation.as_deref() == Some(anim.name.as_str()) && p.direction == track.direction)
                })
                .collect();
            if indices.is_empty() {
                continue;
            }
            animations.push(json!({
                "name": anim.name,
                "direction": track.direction,
                "loop": anim.loop_mode,
                "frames": indices,
                "durations": anim.sequence(track).iter().map(|f| sprite.frame(*f).map(|x| x.duration_ms).unwrap_or(100)).collect::<Vec<_>>(),
            }));
        }
    }
    json!({
        "app": "Pixel Studio Maker",
        "format": "psm-sheet",
        "version": 1,
        "image": image_file,
        "size": { "w": plan.width as i32 * s, "h": plan.height as i32 * s },
        "scale": s,
        "cell": { "w": plan.cell_width as i32 * s, "h": plan.cell_height as i32 * s },
        "sprite": { "name": sprite.name, "w": sprite.width, "h": sprite.height, "origin": [sprite.origin.x, sprite.origin.y] },
        "frames": frames,
        "animations": animations,
    })
}

/// Animated GIF with an exact palette (pixel art rarely has more than 255 colors; if it does,
/// colors are reduced with median cut). Alpha below 128 becomes transparent.
pub fn encode_gif(frames: &[(Image, u32)], scale: u32, looping: bool) -> Result<Vec<u8>, ProjectError> {
    let Some((first, _)) = frames.first() else {
        return Err(ProjectError::Image("GIF sem frames".into()));
    };
    let s = scale.max(1);
    let (w, h) = (first.width() * s, first.height() * s);
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return Err(ProjectError::Image("GIF grande demais".into()));
    }
    let solid = |c: Color| if c.a < 128 { None } else { Some(Color::rgb(c.r, c.g, c.b)) };
    let mut counts: BTreeMap<Color, u32> = BTreeMap::new();
    for (img, _) in frames {
        for c in img.pixels().iter().filter_map(|c| solid(*c)) {
            *counts.entry(c).or_insert(0) += 1;
        }
    }
    let colors: Vec<Color> = if counts.len() <= 255 { counts.keys().copied().collect() } else { median_cut(&counts, 255) };
    let palette = Palette::new("gif", colors.clone());
    let mut lookup: BTreeMap<Color, u8> = BTreeMap::new();
    let mut index_of = |c: Color| -> u8 {
        *lookup.entry(c).or_insert_with(|| palette.index_of(c).or_else(|| palette.nearest(c).map(|(i, _)| i)).unwrap_or(0) as u8 + 1)
    };
    let mut global = vec![0u8, 0, 0];
    for c in &colors {
        global.extend([c.r, c.g, c.b]);
    }
    let mut out = Vec::new();
    {
        let mut enc = gif::Encoder::new(&mut out, w as u16, h as u16, &global).map_err(|e| ProjectError::Image(e.to_string()))?;
        if looping {
            enc.set_repeat(gif::Repeat::Infinite).map_err(|e| ProjectError::Image(e.to_string()))?;
        }
        for (img, ms) in frames {
            let big = scale_image(img, s);
            let indices: Vec<u8> = big.pixels().iter().map(|c| solid(*c).map(&mut index_of).unwrap_or(0)).collect();
            let mut frame = gif::Frame::from_indexed_pixels(w as u16, h as u16, indices, Some(0));
            frame.delay = ((*ms + 5) / 10).clamp(2, 65_535) as u16;
            frame.dispose = gif::DisposalMethod::Background;
            enc.write_frame(&frame).map_err(|e| ProjectError::Image(e.to_string()))?;
        }
    }
    Ok(out)
}

/// Frames of one animation track in playback order (loop mode applied), composited.
pub fn track_frames(
    sprite: &Sprite,
    images: &ImageStore,
    animation: &ItemRef,
    direction: Option<&str>,
) -> Result<Vec<(u32, Image, u32)>, ProjectError> {
    let anim = sprite.animation(animation).map_err(|e| ProjectError::Transaction(e.to_string()))?;
    let dir = direction.map(|d| sprite.direction_name(d)).transpose().map_err(|e| ProjectError::Transaction(e.to_string()))?;
    let track = anim.track(dir.as_deref()).ok_or_else(|| {
        ProjectError::Transaction(format!("a animação \"{}\" não tem a direção {}", anim.name, dir.as_deref().unwrap_or("(sem direção)")))
    })?;
    let opts = ComposeOptions::default();
    Ok(anim
        .sequence(track)
        .into_iter()
        .map(|f| (f, compose_frame(sprite, images, f, &opts), sprite.frame(f).map(|x| x.duration_ms).unwrap_or(100)))
        .collect())
}

fn godot_name(anim: &str, dir: &Option<String>) -> String {
    match dir {
        Some(d) => format!("{anim}_{d}"),
        None => anim.to_string(),
    }
}

/// Godot 4 `SpriteFrames` resource (`.tres`) pointing at the exported sheet texture. One
/// animation per track (`walk_E`, `idle`, ...). Per-frame durations become relative durations
/// over the track's shortest frame; ping-pong and reverse are expanded.
pub fn godot_sprite_frames(sprite: &Sprite, plan: &SheetPlan, texture_res_path: &str, scale: u32) -> String {
    let s = scale.max(1) as i32;
    let packed = plan.cell_width == 0;
    let mut subs = String::new();
    let mut sub_of: BTreeMap<(u32, Option<String>, Option<String>), usize> = BTreeMap::new();
    for (i, p) in plan.placements.iter().enumerate() {
        let (rx, ry, rw, rh) = if packed {
            (p.dest.x * s, p.dest.y * s, p.source.w as i32 * s, p.source.h as i32 * s)
        } else {
            (p.cell.x * s, p.cell.y * s, p.cell.w as i32 * s, p.cell.h as i32 * s)
        };
        subs.push_str(&format!("[sub_resource type=\"AtlasTexture\" id=\"AtlasTexture_{i}\"]\natlas = ExtResource(\"1_sheet\")\nregion = Rect2({rx}, {ry}, {rw}, {rh})\n"));
        if packed {
            let m = (p.source.x * s, p.source.y * s, (sprite.width - p.source.w) as i32 * s, (sprite.height - p.source.h) as i32 * s);
            subs.push_str(&format!("margin = Rect2({}, {}, {}, {})\n", m.0, m.1, m.2, m.3));
        }
        subs.push('\n');
        sub_of.entry((p.frame, p.animation.clone(), p.direction.clone())).or_insert(i);
    }
    let mut anims = Vec::new();
    let mut add = |name: String, frames: Vec<(usize, u32)>, looped: bool| {
        if frames.is_empty() {
            return;
        }
        let base = frames.iter().map(|f| f.1).min().unwrap_or(100).max(1);
        let list: Vec<String> = frames
            .iter()
            .map(|(sub, ms)| {
                format!("{{\n\"duration\": {:.3},\n\"texture\": SubResource(\"AtlasTexture_{sub}\")\n}}", *ms as f64 / base as f64)
            })
            .collect();
        anims.push(format!(
            "{{\n\"frames\": [{}],\n\"loop\": {looped},\n\"name\": &\"{name}\",\n\"speed\": {:.3}\n}}",
            list.join(", "),
            1000.0 / base as f64
        ));
    };
    for anim in &sprite.animations {
        for track in &anim.tracks {
            let frames: Vec<(usize, u32)> = anim
                .sequence(track)
                .iter()
                .filter_map(|f| {
                    sub_of
                        .get(&(*f, Some(anim.name.clone()), track.direction.clone()))
                        .map(|sub| (*sub, sprite.frame(*f).map(|x| x.duration_ms).unwrap_or(100)))
                })
                .collect();
            add(godot_name(&anim.name, &track.direction), frames, anim.loop_mode != LoopMode::Once);
        }
    }
    let free: Vec<(usize, u32)> =
        plan.placements.iter().enumerate().filter(|(_, p)| p.animation.is_none()).map(|(i, p)| (i, p.duration_ms)).collect();
    add("default".into(), free, true);
    let header_offset = plan.placements.first().map(|p| {
        let (cw, ch) = if packed { (sprite.width as i32, sprite.height as i32) } else { (p.cell.w as i32, p.cell.h as i32) };
        let (px, py) =
            if packed { (sprite.pivot(p.frame).x, sprite.pivot(p.frame).y) } else { (p.pivot.x - p.cell.x, p.pivot.y - p.cell.y) };
        ((cw / 2 - px) * s, (ch / 2 - py) * s)
    });
    let mut out = String::new();
    out.push_str(&format!("[gd_resource type=\"SpriteFrames\" load_steps={} format=3]\n\n", plan.placements.len() + 2));
    if let Some((ox, oy)) = header_offset {
        out.push_str(&format!("; Gerado pelo Pixel Studio Maker. Para o pivô ficar na origem do AnimatedSprite2D\n; (centered = true), use offset = Vector2({ox}, {oy}).\n\n"));
    }
    out.push_str(&format!("[ext_resource type=\"Texture2D\" path=\"{texture_res_path}\" id=\"1_sheet\"]\n\n"));
    out.push_str(&subs);
    out.push_str(&format!("[resource]\nanimations = [{}]\n", anims.join(", ")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_animation::sheet::{SheetLayout, SheetOptions, plan};
    use sprite_animation::{Animation, Cel, CycleInfo, Frame, Track};

    fn walking() -> (Sprite, ImageStore) {
        let mut n = 100;
        let mut ids = move || {
            n += 1;
            n
        };
        let mut s = Sprite::new(1, "hero", 4, 4, &mut ids);
        let layer = s.layers[0].id;
        s.frames = vec![Frame { id: 10, duration_ms: 100, tags: vec![] }, Frame { id: 11, duration_ms: 200, tags: vec![] }];
        let mut store = ImageStore::new();
        for (i, f) in [10u32, 11].iter().enumerate() {
            let mut img = Image::new(4, 4);
            img.set(i as i32, 3, Color::BLACK);
            store.insert(*f + 1000, img);
            s.cels.push(Cel { layer, frame: *f, image: *f + 1000, opacity: 255 });
        }
        s.animations.push(Animation {
            id: 20,
            name: "walk".into(),
            loop_mode: LoopMode::Loop,
            tags: vec![],
            tracks: vec![Track { direction: Some("E".into()), frames: vec![10, 11] }],
            cycle: CycleInfo::default(),
        });
        (s, store)
    }

    #[test]
    fn sheet_render_and_metadata() {
        let (s, store) = walking();
        let items = sheet_items(&s, &store, &SheetSelection::default()).unwrap();
        assert_eq!(items.len(), 2);
        let p = plan(&items, 4, 4, &SheetOptions { layout: SheetLayout::Horizontal, ..Default::default() });
        let img = render_sheet(&s, &store, &p, 2, None);
        assert_eq!((img.width(), img.height()), (16, 8));
        assert_eq!(img.get(0, 7), Some(Color::BLACK));
        assert_eq!(img.get(10, 7), Some(Color::BLACK), "second frame at x=8+1*2");
        let meta = sheet_metadata(&s, &p, "hero.png", 2);
        assert_eq!(meta["frames"][1]["x"], 8);
        assert_eq!(meta["animations"][0]["durations"], json!([100, 200]));
    }

    #[test]
    fn godot_resource_lists_animation() {
        let (s, store) = walking();
        let items = sheet_items(&s, &store, &SheetSelection::default()).unwrap();
        let p = plan(&items, 4, 4, &SheetOptions { layout: SheetLayout::Horizontal, ..Default::default() });
        let tres = godot_sprite_frames(&s, &p, "res://hero.png", 1);
        assert!(tres.starts_with("[gd_resource type=\"SpriteFrames\" load_steps=4 format=3]"));
        assert!(tres.contains("&\"walk_E\""));
        assert!(tres.contains("region = Rect2(4, 0, 4, 4)"));
        assert!(tres.contains("\"duration\": 2.000"));
        assert!(tres.contains("\"speed\": 10.000"));
    }

    #[test]
    fn gif_keeps_exact_colors() {
        let mut a = Image::new(2, 2);
        a.set(0, 0, Color::rgb(57, 118, 210));
        let bytes = encode_gif(&[(a.clone(), 100)], 3, true).unwrap();
        let frames = crate::import::decode_frames(&bytes).unwrap();
        assert_eq!(frames[0].image.get(0, 0), Some(Color::rgb(57, 118, 210)));
        assert_eq!(frames[0].image.get(2, 2), Some(Color::rgb(57, 118, 210)));
        assert_eq!(frames[0].image.get(3, 3), Some(Color::TRANSPARENT));
    }
}
