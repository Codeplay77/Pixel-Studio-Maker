//! Godot integration data stored in each sprite: hitboxes, hurtboxes and collision boxes with a
//! rectangle per frame. A frame without a rectangle means the box is off in that frame.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Point, Rect};

/// What a box is for (becomes the Godot group of its `Area2D`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HitboxKind {
    /// Area that deals damage (attack).
    #[default]
    Hitbox,
    /// Area that receives damage (body).
    Hurtbox,
    /// Physical collision / body shape.
    Collision,
    /// Anything else (trigger, interaction, sensor...).
    Custom,
}

impl HitboxKind {
    pub fn as_str(self) -> &'static str {
        match self {
            HitboxKind::Hitbox => "hitbox",
            HitboxKind::Hurtbox => "hurtbox",
            HitboxKind::Collision => "collision",
            HitboxKind::Custom => "custom",
        }
    }

    /// Default overlay color of each kind.
    pub fn default_color(self) -> Color {
        match self {
            HitboxKind::Hitbox => Color::rgb(255, 59, 48),
            HitboxKind::Hurtbox => Color::rgb(52, 199, 89),
            HitboxKind::Collision => Color::rgb(10, 132, 255),
            HitboxKind::Custom => Color::rgb(255, 204, 0),
        }
    }
}

/// A named box with one rectangle per frame (sprite pixel coordinates).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hitbox {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub kind: HitboxKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Frame id → rectangle in that frame. No entry: the box is off in that frame.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frames: BTreeMap<u32, Rect>,
}

impl Hitbox {
    pub fn at(&self, frame: u32) -> Option<Rect> {
        self.frames.get(&frame).copied()
    }

    pub fn display_color(&self) -> Color {
        self.color.unwrap_or(self.kind.default_color())
    }
}

/// Godot data of a sprite.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GodotSprite {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hitboxes: Vec<Hitbox>,
}

impl GodotSprite {
    pub fn is_empty(&self) -> bool {
        self.hitboxes.is_empty()
    }

    /// Copies every box's rectangle of frame `from` to frame `to` (mirrored horizontally inside
    /// a canvas `width` px wide when asked). Used when frames are duplicated.
    pub fn copy_frame(&mut self, from: u32, to: u32, mirror: Option<u32>) {
        for hb in &mut self.hitboxes {
            match hb.frames.get(&from).copied() {
                Some(r) => {
                    let r = match mirror {
                        Some(w) => Rect::new(w as i32 - r.x - r.w as i32, r.y, r.w, r.h),
                        None => r,
                    };
                    hb.frames.insert(to, r);
                }
                None => {
                    hb.frames.remove(&to);
                }
            }
        }
    }

    /// Follows a canvas resize: moves every rectangle by `offset` (canvas only) or scales it
    /// from `from` to `to` (content scaled with nearest-neighbor).
    pub fn resize_canvas(&mut self, from: (u32, u32), to: (u32, u32), offset: Point, scale_content: bool) {
        let sx = |v: i32| (v as i64 * to.0 as i64 / from.0.max(1) as i64) as i32;
        let sy = |v: i32| (v as i64 * to.1 as i64 / from.1.max(1) as i64) as i32;
        for hb in &mut self.hitboxes {
            for r in hb.frames.values_mut() {
                *r = if scale_content {
                    let (x0, y0, x1, y1) = (sx(r.x), sy(r.y), sx(r.right()), sy(r.bottom()));
                    Rect::new(x0, y0, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32)
                } else {
                    r.translate(offset.x, offset.y)
                };
            }
        }
    }

    /// Forgets the rectangles of deleted frames.
    pub fn remove_frames(&mut self, frames: &[u32]) {
        for hb in &mut self.hitboxes {
            hb.frames.retain(|f, _| !frames.contains(f));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_and_mirror_rectangles() {
        let mut g = GodotSprite {
            hitboxes: vec![Hitbox {
                id: 1,
                name: "golpe".into(),
                kind: HitboxKind::Hitbox,
                color: None,
                frames: BTreeMap::from([(10, Rect::new(2, 3, 4, 5))]),
            }],
        };
        g.copy_frame(10, 11, None);
        g.copy_frame(10, 12, Some(16));
        assert_eq!(g.hitboxes[0].at(11), Some(Rect::new(2, 3, 4, 5)));
        assert_eq!(g.hitboxes[0].at(12), Some(Rect::new(10, 3, 4, 5)), "16 - 2 - 4 = 10");
        g.copy_frame(99, 11, None);
        assert_eq!(g.hitboxes[0].at(11), None, "copying an empty frame turns the box off");
        let mut scaled = g.clone();
        scaled.resize_canvas((16, 16), (32, 32), Point::default(), true);
        assert_eq!(scaled.hitboxes[0].at(10), Some(Rect::new(4, 6, 8, 10)));
        let mut moved = g.clone();
        moved.resize_canvas((16, 16), (20, 20), Point::new(2, 4), false);
        assert_eq!(moved.hitboxes[0].at(10), Some(Rect::new(4, 7, 4, 5)));
        g.remove_frames(&[12]);
        assert_eq!(g.hitboxes[0].frames.len(), 1);
        let json = serde_json::to_value(&g).unwrap();
        assert_eq!(json["hitboxes"][0]["frames"]["10"]["w"], 4);
        assert_eq!(json["hitboxes"][0]["kind"], "hitbox");
        assert!(GodotSprite::default().is_empty());
    }
}
