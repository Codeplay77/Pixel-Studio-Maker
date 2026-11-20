//! Programmatic checks and frame metrics. Checks produce warnings, never block edits, and are
//! no guarantee of aesthetic quality.

use std::collections::{BTreeMap, VecDeque};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image, Mask, Point, Rect};

use crate::compose::{ComposeOptions, compose_frame};
use crate::model::{ImageStore, Sprite};

/// Measurements of one composited frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrameMetrics {
    /// Bounding box of visible pixels.
    pub bounds: Option<Rect>,
    pub opaque_pixels: u32,
    /// Pixels with 0 < alpha < 255.
    pub translucent_pixels: u32,
    pub color_count: u32,
    /// Center of the visible bounding box.
    pub center: Option<[f64; 2]>,
    /// Lowest visible row (where the sprite stands).
    pub ground_y: Option<i32>,
    /// Transparent pixels fully enclosed by the shape.
    pub holes: u32,
}

pub fn frame_metrics(img: &Image) -> FrameMetrics {
    let bounds = img.opaque_bounds();
    let translucent = img.pixels().iter().filter(|c| c.a > 0 && c.a < 255).count() as u32;
    FrameMetrics {
        bounds,
        opaque_pixels: img.opaque_count(),
        translucent_pixels: translucent,
        color_count: img.color_counts().len() as u32,
        center: bounds.map(|b| [b.x as f64 + b.w as f64 / 2.0, b.y as f64 + b.h as f64 / 2.0]),
        ground_y: bounds.map(|b| b.bottom() - 1),
        holes: enclosed_transparent(img).count(),
    }
}

/// Transparent pixels not reachable from the image border through transparent pixels.
pub fn enclosed_transparent(img: &Image) -> Mask {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut outside = Mask::new(img.width(), img.height());
    let mut queue = VecDeque::new();
    for x in 0..w {
        for y in [0, h - 1] {
            if img.get_or_clear(x, y).is_transparent() && !outside.get(x, y) {
                outside.set(x, y, true);
                queue.push_back(Point::new(x, y));
            }
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            if img.get_or_clear(x, y).is_transparent() && !outside.get(x, y) {
                outside.set(x, y, true);
                queue.push_back(Point::new(x, y));
            }
        }
    }
    while let Some(p) = queue.pop_front() {
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (x, y) = (p.x + dx, p.y + dy);
            if img.in_bounds(x, y) && !outside.get(x, y) && img.get_or_clear(x, y).is_transparent() {
                outside.set(x, y, true);
                queue.push_back(Point::new(x, y));
            }
        }
    }
    let mut holes = Mask::new(img.width(), img.height());
    for y in 0..h {
        for x in 0..w {
            if img.get_or_clear(x, y).is_transparent() && !outside.get(x, y) {
                holes.set(x, y, true);
            }
        }
    }
    holes
}

/// Fraction of changed pixels between `a` and `b` (over `visible`) after shifting `a` by the
/// offset within ±`max` px that matches best. Returns the ratio and that offset.
pub fn best_aligned_change(a: &Image, b: &Image, max: i32, visible: u32) -> (f64, (i32, i32)) {
    let mut best: (u32, (i32, i32)) = (u32::MAX, (0, 0));
    for dy in -max..=max {
        for dx in -max..=max {
            let mut changed = 0u32;
            for y in 0..b.height() as i32 {
                for x in 0..b.width() as i32 {
                    if a.get_or_clear(x - dx, y - dy) != b.get_or_clear(x, y) {
                        changed += 1;
                    }
                }
            }
            if changed < best.0 || (changed == best.0 && dx.abs() + dy.abs() < best.1.0.abs() + best.1.1.abs()) {
                best = (changed, (dx, dy));
            }
        }
    }
    (best.0 as f64 / visible.max(1) as f64, best.1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
}

/// One finding of [`validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub severity: Severity,
    /// Stable code: cel_size, out_of_bounds, off_palette, duplicate_frame, empty_frame, holes,
    /// translucent, jump, big_change, ground_drift, pivot_drift.
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ValidateOptions {
    /// Colors allowed; `None` skips the palette check.
    pub palette: Option<Vec<Color>>,
    /// Center jump (px) between consecutive frames considered abrupt.
    pub max_jump: f64,
    /// Fraction of changed pixels (vs. visible area) considered abnormal between neighbors.
    pub max_change: f64,
    /// Allowed variation of the ground line / pivot inside a track (px).
    pub ground_tolerance: i32,
}

impl ValidateOptions {
    pub fn for_sprite(sprite: &Sprite) -> Self {
        ValidateOptions {
            palette: None,
            max_jump: (sprite.width.max(sprite.height) as f64 / 8.0).max(2.0),
            max_change: 0.6,
            ground_tolerance: 1,
        }
    }
}

fn issue(severity: Severity, code: &str, message: String) -> Issue {
    Issue { severity, code: code.into(), message, frame: None, animation: None, direction: None }
}

/// Runs every check over a sprite. Order of findings is deterministic.
pub fn validate(sprite: &Sprite, images: &ImageStore, opts: &ValidateOptions) -> Vec<Issue> {
    let mut out = Vec::new();
    let (w, h) = (sprite.width, sprite.height);
    let bounds = Rect::sized(w, h);

    for cel in &sprite.cels {
        if let Some(img) = images.get(&cel.image) {
            if img.width() != w || img.height() != h {
                let mut i = issue(Severity::Warning, "cel_size", format!("cel {}x{} difere do sprite {w}x{h}", img.width(), img.height()));
                i.frame = Some(cel.frame);
                out.push(i);
            }
        } else {
            let mut i = issue(Severity::Warning, "cel_size", format!("cel sem imagem (id {})", cel.image));
            i.frame = Some(cel.frame);
            out.push(i);
        }
    }
    for a in &sprite.anchors {
        for (frame, p) in a.positions.iter().map(|(f, p)| (Some(*f), *p)).chain(a.default.map(|p| (None, p))) {
            if !bounds.contains(p) {
                let mut i =
                    issue(Severity::Warning, "out_of_bounds", format!("anchor \"{}\" em [{}, {}] fora do canvas", a.name, p.x, p.y));
                i.frame = frame;
                out.push(i);
            }
        }
    }
    for r in &sprite.regions {
        for (frame, shape) in r.shapes.iter().map(|(f, s)| (Some(*f), s)).chain(r.default.as_ref().map(|s| (None, s))) {
            let raw = shape.raw_bounds();
            if raw.is_empty() || raw.intersect(&bounds) != raw {
                let mut i = issue(Severity::Warning, "out_of_bounds", format!("região \"{}\" sai do canvas", r.name));
                i.frame = frame;
                out.push(i);
            }
        }
    }

    let composites: BTreeMap<u32, Image> =
        sprite.frames.iter().map(|f| (f.id, compose_frame(sprite, images, f.id, &ComposeOptions::default()))).collect();

    if let Some(palette) = &opts.palette {
        let mut off: BTreeMap<Color, u32> = BTreeMap::new();
        let mut frames_with = Vec::new();
        for (fid, img) in &composites {
            let mut hit = false;
            for (c, n) in img.color_counts() {
                if !palette.contains(&c) {
                    *off.entry(c).or_insert(0) += n;
                    hit = true;
                }
            }
            if hit {
                frames_with.push(*fid);
            }
        }
        if !off.is_empty() {
            let list: Vec<String> = off.iter().take(12).map(|(c, n)| format!("{c} ({n}px)")).collect();
            out.push(issue(
                Severity::Warning,
                "off_palette",
                format!("{} cores fora da paleta em {} frames: {}", off.len(), frames_with.len(), list.join(", ")),
            ));
        }
    }

    for (fid, img) in &composites {
        let holes = enclosed_transparent(img);
        if holes.count() > 0 {
            let b = holes.bounds().unwrap_or_default();
            let mut i = issue(
                Severity::Info,
                "holes",
                format!("{} pixels transparentes cercados pela forma (região {},{} {}x{})", holes.count(), b.x, b.y, b.w, b.h),
            );
            i.frame = Some(*fid);
            out.push(i);
        }
        let translucent = img.pixels().iter().filter(|c| c.a > 0 && c.a < 255).count();
        if translucent > 0 {
            let mut i = issue(Severity::Info, "translucent", format!("{translucent} pixels semitransparentes"));
            i.frame = Some(*fid);
            out.push(i);
        }
    }

    for anim in &sprite.animations {
        for track in &anim.tracks {
            let tag = |mut i: Issue, frame: u32| {
                i.frame = Some(frame);
                i.animation = Some(anim.name.clone());
                i.direction = track.direction.clone();
                i
            };
            let mut metrics = Vec::new();
            for &f in &track.frames {
                let Some(img) = composites.get(&f) else { continue };
                let m = frame_metrics(img);
                if m.opaque_pixels == 0 {
                    out.push(tag(issue(Severity::Warning, "empty_frame", "frame vazio dentro da animação".into()), f));
                }
                metrics.push((f, m));
            }
            let mut adjacent: Vec<(u32, u32)> = track.frames.windows(2).map(|p| (p[0], p[1])).collect();
            if track.frames.len() > 2 && matches!(anim.loop_mode, crate::model::LoopMode::Loop | crate::model::LoopMode::Reverse) {
                adjacent.push((*track.frames.last().unwrap(), track.frames[0]));
            }
            for (a, b) in adjacent.iter().copied() {
                let (Some(ia), Some(ib)) = (composites.get(&a), composites.get(&b)) else { continue };
                if a != b && ia == ib {
                    out.push(tag(issue(Severity::Warning, "duplicate_frame", format!("frame {b} é idêntico ao anterior ({a})")), b));
                    continue;
                }
                let visible = ia.opaque_count().max(ib.opaque_count()).max(1);
                // Small whole-body shifts (bobbing) are normal motion: measure the change after
                // the best alignment within ±2 px.
                let (ratio, shift) = best_aligned_change(ia, ib, 2, visible);
                if ratio > opts.max_change {
                    let note = if shift != (0, 0) {
                        format!(" (mesmo descontando o deslocamento de {}, {} px)", shift.0, shift.1)
                    } else {
                        String::new()
                    };
                    out.push(tag(
                        issue(
                            Severity::Warning,
                            "big_change",
                            format!("{:.0}% dos pixels visíveis mudam de {a} para {b}{note}", ratio * 100.0),
                        ),
                        b,
                    ));
                }
            }
            for (fa, fb) in adjacent {
                let ma = metrics.iter().find(|(f, _)| *f == fa).map(|(_, m)| m);
                let mb = metrics.iter().find(|(f, _)| *f == fb).map(|(_, m)| m);
                if let (Some(ca), Some(cb)) = (ma.and_then(|m| m.center), mb.and_then(|m| m.center)) {
                    let jump = ((ca[0] - cb[0]).powi(2) + (ca[1] - cb[1]).powi(2)).sqrt();
                    if jump > opts.max_jump {
                        out.push(tag(issue(Severity::Warning, "jump", format!("o centro se desloca {jump:.1}px de {fa} para {fb}")), fb));
                    }
                }
            }
            let grounds: Vec<(u32, i32)> = metrics.iter().filter_map(|(f, m)| m.ground_y.map(|g| (*f, g))).collect();
            if let Some(ground) = anim.cycle.ground_y {
                for (f, g) in &grounds {
                    if (g - ground).abs() > opts.ground_tolerance {
                        out.push(tag(issue(Severity::Warning, "ground_drift", format!("base em y={g}, chão definido em y={ground}")), *f));
                    }
                }
            }
            let pivots: Vec<(u32, Point)> = track.frames.iter().map(|f| (*f, sprite.pivot(*f))).collect();
            if let Some((_, first)) = pivots.first() {
                for (f, p) in &pivots[1..] {
                    if (p.x - first.x).abs() > opts.ground_tolerance || (p.y - first.y).abs() > opts.ground_tolerance {
                        out.push(tag(
                            issue(
                                Severity::Info,
                                "pivot_drift",
                                format!("pivô em [{}, {}], primeiro frame em [{}, {}]", p.x, p.y, first.x, first.y),
                            ),
                            *f,
                        ));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Animation, Cel, CycleInfo, Frame, Track};

    fn ring() -> Image {
        let mut img = Image::new(8, 8);
        for p in Rect::new(2, 2, 4, 4).points() {
            img.set(p.x, p.y, Color::BLACK);
        }
        img.set(3, 3, Color::TRANSPARENT);
        img
    }

    #[test]
    fn metrics_and_holes() {
        let m = frame_metrics(&ring());
        assert_eq!(m.bounds, Some(Rect::new(2, 2, 4, 4)));
        assert_eq!(m.holes, 1);
        assert_eq!(m.ground_y, Some(5));
        assert_eq!(m.opaque_pixels, 15);
    }

    #[test]
    fn validation_finds_problems() {
        let mut n = 0;
        let mut ids = move || {
            n += 1;
            n
        };
        let mut s = Sprite::new(100, "s", 8, 8, &mut ids);
        let layer = s.layers[0].id;
        let f0 = s.frames[0].id;
        s.frames.push(Frame { id: 40, duration_ms: 100, tags: vec![] });
        s.frames.push(Frame { id: 41, duration_ms: 100, tags: vec![] });
        let mut store = ImageStore::new();
        store.insert(1, ring());
        store.insert(2, ring());
        let mut moved = Image::new(8, 8);
        moved.set(7, 0, Color::rgb(255, 0, 0));
        store.insert(3, moved);
        s.cels.push(Cel { layer, frame: f0, image: 1, opacity: 255 });
        s.cels.push(Cel { layer, frame: 40, image: 2, opacity: 255 });
        s.cels.push(Cel { layer, frame: 41, image: 3, opacity: 255 });
        s.animations.push(Animation {
            id: 50,
            name: "idle".into(),
            loop_mode: Default::default(),
            tags: vec![],
            tracks: vec![Track { direction: None, frames: vec![f0, 40, 41] }],
            cycle: CycleInfo::default(),
        });
        let mut opts = ValidateOptions::for_sprite(&s);
        opts.palette = Some(vec![Color::BLACK]);
        let issues = validate(&s, &store, &opts);
        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"duplicate_frame"), "{codes:?}");
        assert!(codes.contains(&"off_palette"), "{codes:?}");
        assert!(codes.contains(&"holes"), "{codes:?}");
        assert!(codes.contains(&"jump"), "{codes:?}");
        assert!(codes.contains(&"big_change"), "{codes:?}");
    }
    #[test]
    fn ground_and_loop_closure_are_checked_independently() {
        let mut n = 0;
        let mut ids = || {
            n += 1;
            n
        };
        let mut sprite = Sprite::new(100, "s", 16, 16, &mut ids);
        let layer = sprite.layers[0].id;
        let first = sprite.frames[0].id;
        let mut images = ImageStore::new();
        for (i, x) in [0, 3, 6].iter().enumerate() {
            let frame = if i == 0 { first } else { 40 + i as u32 };
            if i > 0 {
                sprite.frames.push(Frame { id: frame, duration_ms: 100, tags: vec![] });
            }
            let mut image = Image::new(16, 16);
            image.set(*x, 5, Color::BLACK);
            images.insert(i as u32 + 1, image);
            sprite.cels.push(Cel { layer, frame, image: i as u32 + 1, opacity: 255 });
        }
        let frames: Vec<_> = sprite.frames.iter().map(|f| f.id).collect();
        sprite.animations.push(Animation {
            id: 50,
            name: "walk".into(),
            loop_mode: crate::model::LoopMode::Loop,
            tags: vec![],
            tracks: vec![Track { direction: None, frames }],
            cycle: CycleInfo { ground_y: Some(10), ..Default::default() },
        });
        let mut opts = ValidateOptions::for_sprite(&sprite);
        opts.max_jump = 4.0;
        let issues = validate(&sprite, &images, &opts);
        assert!(issues.iter().any(|i| i.code == "ground_drift"));
        assert!(issues.iter().any(|i| i.code == "jump" && i.frame == Some(first)));
        sprite.animations[0].loop_mode = crate::model::LoopMode::Once;
        assert!(!validate(&sprite, &images, &opts).iter().any(|i| i.code == "jump"));
    }
}
