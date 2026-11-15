//! The sprite data model. Pixels live in an [`ImageStore`] keyed by image id; everything here is
//! light metadata, cheap to clone (the history keeps copies of it).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{BlendMode, Color, FrameRef, Image, ItemRef, Mask, Point, Rect};

use crate::extras::SpriteExtras;

/// Pixel storage shared by every sprite of a project. Cels point here by id; two cels pointing
/// at the same id are *linked* (editing one edits both).
pub type ImageStore = BTreeMap<u32, Image>;

fn yes() -> bool {
    true
}
fn opaque() -> u8 {
    255
}
fn default_duration() -> u32 {
    100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    /// 0 (invisible) to 255 (opaque).
    #[serde(default = "opaque")]
    pub opacity: u8,
    #[serde(default)]
    pub blend: BlendMode,
    /// Reference layers show in the editor but never in exports or composites for the agent
    /// unless asked.
    #[serde(default)]
    pub reference: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Frame {
    pub id: u32,
    /// How long the frame shows during playback, in milliseconds.
    #[serde(default = "default_duration")]
    pub duration_ms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// The pixels of one layer in one frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cel {
    pub layer: u32,
    pub frame: u32,
    pub image: u32,
    #[serde(default = "opaque")]
    pub opacity: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    /// 0,1,2,3,0,1,...
    #[default]
    Loop,
    /// Plays once and holds the last frame.
    Once,
    /// 0,1,2,3,2,1,0,1,...
    PingPong,
    /// 3,2,1,0,3,2,...
    Reverse,
}

/// The frames of an animation for one direction (or the only track, when non-directional).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Track {
    /// Direction name (e.g. "E", "NW", or a custom view). `None` = non-directional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub frames: Vec<u32>,
}

/// Notes about the motion cycle that guide the agent while drawing frames.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CycleInfo {
    /// Frames holding the key poses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key_frames: Vec<u32>,
    /// Frames where a foot (or other part) touches the ground.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact_frames: Vec<u32>,
    /// Ground line (y) the character should stand on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_y: Option<i32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Animation {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub loop_mode: LoopMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub cycle: CycleInfo,
}

impl Animation {
    pub fn track(&self, direction: Option<&str>) -> Option<&Track> {
        match direction {
            Some(d) => self.tracks.iter().find(|t| t.direction.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(d))),
            None => self.tracks.iter().find(|t| t.direction.is_none()).or_else(|| (self.tracks.len() == 1).then(|| &self.tracks[0])),
        }
    }

    pub fn track_mut(&mut self, direction: Option<&str>) -> Option<&mut Track> {
        let index = match direction {
            Some(d) => self.tracks.iter().position(|t| t.direction.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(d))),
            None => self.tracks.iter().position(|t| t.direction.is_none()).or_else(|| (self.tracks.len() == 1).then_some(0)),
        };
        index.map(move |i| &mut self.tracks[i])
    }

    /// Playback order for a track, expanded for the loop mode (one cycle).
    pub fn sequence(&self, track: &Track) -> Vec<u32> {
        let f = &track.frames;
        match self.loop_mode {
            LoopMode::Loop | LoopMode::Once => f.clone(),
            LoopMode::Reverse => f.iter().rev().copied().collect(),
            LoopMode::PingPong => {
                let mut s = f.clone();
                if f.len() > 2 {
                    s.extend(f[1..f.len() - 1].iter().rev());
                }
                s
            }
        }
    }
}

/// A viewing direction. The 8 standard ones are created with every sprite; custom views
/// ("3/4", "isometric-left", ...) can be added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Direction {
    /// Short unique name: N, NE, E, SE, S, SW, W, NW, or a custom one.
    pub name: String,
    pub label: String,
    /// Degrees, screen space, clockwise from east (E=0, S=90, W=180, N=270).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// Suggested mirror source (W mirrors E). Mirroring is optional; never assumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mirror_of: Option<String>,
    /// Direction used as visual reference while drawing this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default)]
    pub custom: bool,
}

pub fn standard_directions() -> Vec<Direction> {
    let d = |name: &str, label: &str, angle: f64, mirror: Option<&str>| Direction {
        name: name.into(),
        label: label.into(),
        angle: Some(angle),
        mirror_of: mirror.map(Into::into),
        reference: None,
        custom: false,
    };
    vec![
        d("N", "Norte", 270.0, None),
        d("NE", "Nordeste", 315.0, Some("NW")),
        d("E", "Leste", 0.0, Some("W")),
        d("SE", "Sudeste", 45.0, Some("SW")),
        d("S", "Sul", 90.0, None),
        d("SW", "Sudoeste", 135.0, Some("SE")),
        d("W", "Oeste", 180.0, Some("E")),
        d("NW", "Noroeste", 225.0, Some("NE")),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnchorKind {
    /// A generic reference point.
    #[default]
    Point,
    /// Sprite pivot/origin used to align frames on export.
    Pivot,
    /// Where the sprite touches the ground.
    Ground,
    /// A body joint (neck, shoulder, elbow, hip, knee...).
    Joint,
    /// Where something is held or attached (hand, weapon grip...).
    Attachment,
}

/// A named point, optionally with a different position per frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Anchor {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub kind: AnchorKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Point>,
    /// Frame id → position.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub positions: BTreeMap<u32, Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl Anchor {
    pub fn at(&self, frame: u32) -> Option<Point> {
        self.positions.get(&frame).copied().or(self.default)
    }
}

/// Area of a semantic region in one frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RegionShape {
    Rect {
        rect: Rect,
    },
    /// `rows` like `"..##.."` placed at `at`: any char other than `.`/space is inside.
    Mask {
        at: Point,
        rows: Vec<String>,
    },
    Polygon {
        points: Vec<Point>,
    },
}

impl RegionShape {
    pub fn to_mask(&self, width: u32, height: u32) -> Mask {
        match self {
            RegionShape::Rect { rect } => Mask::from_rect(width, height, *rect),
            RegionShape::Mask { at, rows } => Mask::from_rows(width, height, *at, rows),
            RegionShape::Polygon { points } => {
                let mut m = Mask::new(width, height);
                for p in sprite_drawing::raster::polygon_fill(points) {
                    m.set(p.x, p.y, true);
                }
                m
            }
        }
    }

    pub fn bounds(&self, width: u32, height: u32) -> Option<Rect> {
        self.to_mask(width, height).bounds()
    }

    /// Bounds of the shape as written, without clipping to any canvas.
    pub fn raw_bounds(&self) -> Rect {
        match self {
            RegionShape::Rect { rect } => *rect,
            RegionShape::Mask { at, rows } => {
                Rect::new(at.x, at.y, rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u32, rows.len() as u32)
            }
            RegionShape::Polygon { points } => {
                let mut d = sprite_core::DirtyRect::default();
                points.iter().for_each(|p| d.add(*p));
                d.rect().unwrap_or_default()
            }
        }
    }
}

/// A named part of the sprite (head, torso, left arm, weapon...). Defined by the agent or the
/// user, never guessed automatically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Region {
    pub id: u32,
    pub name: String,
    /// Body part or category label, e.g. "head", "arm_left", "weapon".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    /// Layer holding this part, if it has its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<RegionShape>,
    /// Frame id → shape in that frame.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shapes: BTreeMap<u32, RegionShape>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl Region {
    pub fn shape(&self, frame: u32) -> Option<&RegionShape> {
        self.shapes.get(&frame).or(self.default.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PoseKind {
    Initial,
    Final,
    #[default]
    Key,
    Breakdown,
    Contact,
    Passing,
    Custom,
}

/// A recorded pose: where the anchors/joints are, which frame shows it, ground alignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pose {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub kind: PoseKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<u32>,
    /// Anchor name → position in this pose.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub anchors: BTreeMap<String, Point>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_y: Option<i32>,
    /// Free description of limbs, silhouette and intent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

/// One sprite: a canvas size plus layers × frames of pixels and everything describing them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Sprite {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Pivot used to align frames on export (default: bottom center).
    pub origin: Point,
    /// Bottom to top.
    pub layers: Vec<Layer>,
    /// Timeline order of every frame of the sprite.
    pub frames: Vec<Frame>,
    pub cels: Vec<Cel>,
    #[serde(default)]
    pub animations: Vec<Animation>,
    #[serde(default = "standard_directions")]
    pub directions: Vec<Direction>,
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub regions: Vec<Region>,
    #[serde(default)]
    pub poses: Vec<Pose>,
    /// Name of this sprite's palette in the project (falls back to the project's active one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Dados das ferramentas de jogo (tiles, hitboxes, modelos do RPG Maker, fontes, 9-slice).
    #[serde(default, skip_serializing_if = "SpriteExtras::is_empty")]
    pub extras: SpriteExtras,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("{kind} {reference} não existe")]
    NotFound { kind: &'static str, reference: String },
    #[error("{0}")]
    Invalid(String),
}

pub fn not_found(kind: &'static str, reference: impl std::fmt::Display) -> ModelError {
    ModelError::NotFound { kind, reference: reference.to_string() }
}

impl Sprite {
    /// A new sprite with one layer and one empty frame. Ids come from `next_id`.
    pub fn new(id: u32, name: impl Into<String>, width: u32, height: u32, next_id: &mut impl FnMut() -> u32) -> Self {
        let layer = Layer {
            id: next_id(),
            name: "Layer 1".into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend: BlendMode::Normal,
            reference: false,
        };
        let frame = Frame { id: next_id(), duration_ms: 100, tags: Vec::new() };
        Sprite {
            id,
            name: name.into(),
            width,
            height,
            origin: Point::new(width as i32 / 2, height as i32 - 1),
            layers: vec![layer],
            frames: vec![frame],
            cels: Vec::new(),
            animations: Vec::new(),
            directions: standard_directions(),
            anchors: Vec::new(),
            regions: Vec::new(),
            poses: Vec::new(),
            palette: None,
            extras: SpriteExtras::default(),
        }
    }

    pub fn layer(&self, r: &ItemRef) -> Result<&Layer, ModelError> {
        self.layers.iter().find(|l| r.matches(l.id, &l.name)).ok_or_else(|| not_found("layer", r))
    }

    pub fn layer_index(&self, id: u32) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn frame_index(&self, id: u32) -> Option<usize> {
        self.frames.iter().position(|f| f.id == id)
    }

    pub fn frame(&self, id: u32) -> Option<&Frame> {
        self.frames.iter().find(|f| f.id == id)
    }

    pub fn frame_mut(&mut self, id: u32) -> Option<&mut Frame> {
        self.frames.iter_mut().find(|f| f.id == id)
    }

    pub fn animation(&self, r: &ItemRef) -> Result<&Animation, ModelError> {
        self.animations.iter().find(|a| r.matches(a.id, &a.name)).ok_or_else(|| not_found("animação", r))
    }

    pub fn animation_index(&self, r: &ItemRef) -> Result<usize, ModelError> {
        self.animations.iter().position(|a| r.matches(a.id, &a.name)).ok_or_else(|| not_found("animação", r))
    }

    pub fn direction(&self, name: &str) -> Option<&Direction> {
        self.directions.iter().find(|d| d.name.eq_ignore_ascii_case(name) || d.label.eq_ignore_ascii_case(name))
    }

    /// Canonical direction name for user input ("e", "Leste" → "E").
    pub fn direction_name(&self, name: &str) -> Result<String, ModelError> {
        self.direction(name).map(|d| d.name.clone()).ok_or_else(|| not_found("direção", name))
    }

    /// Resolves a frame reference to a frame id.
    pub fn resolve_frame(&self, r: &FrameRef) -> Result<u32, ModelError> {
        match r {
            FrameRef::Id(id) => self.frame(*id).map(|f| f.id).ok_or_else(|| not_found("frame", id)),
            FrameRef::InAnimation { animation, direction, index } => {
                let anim = self.animation(animation)?;
                let dir = direction.as_deref().map(|d| self.direction_name(d)).transpose()?;
                let track = anim.track(dir.as_deref()).ok_or_else(|| {
                    not_found("trilha", format!("{} na direção {}", anim.name, dir.as_deref().unwrap_or("(sem direção)")))
                })?;
                track
                    .frames
                    .get(*index)
                    .copied()
                    .ok_or_else(|| ModelError::Invalid(format!("a trilha tem {} frames; índice {index} não existe", track.frames.len())))
            }
        }
    }

    pub fn cel(&self, layer: u32, frame: u32) -> Option<&Cel> {
        self.cels.iter().find(|c| c.layer == layer && c.frame == frame)
    }

    pub fn cel_mut(&mut self, layer: u32, frame: u32) -> Option<&mut Cel> {
        self.cels.iter_mut().find(|c| c.layer == layer && c.frame == frame)
    }

    /// Cels sharing the image of (layer, frame), including itself.
    pub fn linked_cels(&self, image: u32) -> Vec<&Cel> {
        self.cels.iter().filter(|c| c.image == image).collect()
    }

    pub fn anchor(&self, r: &ItemRef) -> Result<&Anchor, ModelError> {
        self.anchors.iter().find(|a| r.matches(a.id, &a.name)).ok_or_else(|| not_found("anchor", r))
    }

    pub fn region(&self, r: &ItemRef) -> Result<&Region, ModelError> {
        self.regions.iter().find(|a| r.matches(a.id, &a.name)).ok_or_else(|| not_found("região", r))
    }

    /// Which animations/directions use a frame.
    pub fn frame_usage(&self, frame: u32) -> Vec<(String, Option<String>, usize)> {
        let mut out = Vec::new();
        for a in &self.animations {
            for t in &a.tracks {
                for (i, f) in t.frames.iter().enumerate() {
                    if *f == frame {
                        out.push((a.name.clone(), t.direction.clone(), i));
                    }
                }
            }
        }
        out
    }

    /// Pivot for a frame: a `pivot` anchor's position if any, else the sprite origin.
    pub fn pivot(&self, frame: u32) -> Point {
        self.anchors.iter().find(|a| a.kind == AnchorKind::Pivot).and_then(|a| a.at(frame)).unwrap_or(self.origin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter() -> impl FnMut() -> u32 {
        let mut n = 10;
        move || {
            n += 1;
            n
        }
    }

    #[test]
    fn new_sprite_has_defaults() {
        let mut ids = counter();
        let s = Sprite::new(1, "hero", 32, 32, &mut ids);
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.frames.len(), 1);
        assert_eq!(s.directions.len(), 8);
        assert_eq!(s.origin, Point::new(16, 31));
        assert_eq!(s.direction_name("leste").unwrap(), "E");
        assert!(s.direction_name("up").is_err());
    }

    #[test]
    fn resolve_frame_in_animation() {
        let mut ids = counter();
        let mut s = Sprite::new(1, "hero", 8, 8, &mut ids);
        s.frames.push(Frame { id: 50, duration_ms: 80, tags: vec![] });
        s.animations.push(Animation {
            id: 60,
            name: "walk".into(),
            loop_mode: LoopMode::PingPong,
            tags: vec![],
            tracks: vec![Track { direction: Some("E".into()), frames: vec![12, 50] }],
            cycle: CycleInfo::default(),
        });
        let r = FrameRef::InAnimation { animation: ItemRef::Name("walk".into()), direction: Some("e".into()), index: 1 };
        assert_eq!(s.resolve_frame(&r).unwrap(), 50);
        assert!(s.resolve_frame(&FrameRef::Id(999)).is_err());
        let bad = FrameRef::InAnimation { animation: ItemRef::Name("walk".into()), direction: Some("E".into()), index: 5 };
        assert!(s.resolve_frame(&bad).is_err());
        assert_eq!(s.frame_usage(50), vec![("walk".to_string(), Some("E".to_string()), 1)]);
    }

    #[test]
    fn loop_sequences() {
        let mut a =
            Animation { id: 1, name: "a".into(), loop_mode: LoopMode::PingPong, tags: vec![], tracks: vec![], cycle: CycleInfo::default() };
        let t = Track { direction: None, frames: vec![1, 2, 3, 4] };
        assert_eq!(a.sequence(&t), vec![1, 2, 3, 4, 3, 2]);
        a.loop_mode = LoopMode::Reverse;
        assert_eq!(a.sequence(&t), vec![4, 3, 2, 1]);
    }

    #[test]
    fn region_shapes_to_masks() {
        let r = RegionShape::Mask { at: Point::new(1, 1), rows: vec!["##".into(), ".#".into()] };
        assert_eq!(r.to_mask(8, 8).count(), 3);
        let p = RegionShape::Polygon { points: vec![Point::new(0, 0), Point::new(3, 0), Point::new(3, 3), Point::new(0, 3)] };
        assert_eq!(p.to_mask(8, 8).count(), 16);
        assert_eq!(RegionShape::Rect { rect: Rect::new(0, 0, 2, 2) }.bounds(8, 8), Some(Rect::new(0, 0, 2, 2)));
    }
}
