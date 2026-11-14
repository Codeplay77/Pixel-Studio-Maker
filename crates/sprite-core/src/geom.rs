use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Integer pixel coordinate. Origin (0,0) is the top-left pixel; y grows downward.
/// Serialized as `[x, y]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Point { x, y }
    }

    pub const fn offset(self, dx: i32, dy: i32) -> Self {
        Point { x: self.x + dx, y: self.y + dy }
    }
}

impl From<(i32, i32)> for Point {
    fn from((x, y): (i32, i32)) -> Self {
        Point { x, y }
    }
}

impl Serialize for Point {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Pair([i32; 2]),
            Object { x: i32, y: i32 },
        }
        Ok(match Repr::deserialize(deserializer)? {
            Repr::Pair([x, y]) => Point { x, y },
            Repr::Object { x, y } => Point { x, y },
        })
    }
}

impl JsonSchema for Point {
    fn schema_name() -> Cow<'static, str> {
        "Point".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "description": "Coordenada [x, y] em pixels; (0,0) é o canto superior esquerdo.",
            "items": { "type": "integer" },
            "minItems": 2,
            "maxItems": 2
        })
    }
}

/// Axis-aligned rectangle in pixels: `x`,`y` is the top-left pixel, `w`×`h` its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Rect { x, y, w, h }
    }

    pub const fn sized(w: u32, h: u32) -> Self {
        Rect { x: 0, y: 0, w, h }
    }

    /// Smallest rectangle containing both points (inclusive).
    pub fn from_corners(a: Point, b: Point) -> Self {
        let x0 = a.x.min(b.x);
        let y0 = a.y.min(b.y);
        let x1 = a.x.max(b.x);
        let y1 = a.y.max(b.y);
        Rect::new(x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32)
    }

    pub const fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Exclusive right edge.
    pub const fn right(&self) -> i32 {
        self.x + self.w as i32
    }

    /// Exclusive bottom edge.
    pub const fn bottom(&self) -> i32 {
        self.y + self.h as i32
    }

    pub const fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }

    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }

    pub fn intersect(&self, other: &Rect) -> Rect {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 <= x0 || y1 <= y0 { Rect::new(x0, y0, 0, 0) } else { Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32) }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32)
    }

    pub fn translate(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Iterates every pixel row by row.
    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        let (x0, y0, x1, y1) = (self.x, self.y, self.right(), self.bottom());
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| Point::new(x, y)))
    }
}

/// Grows a rectangle as pixels are touched; used to report the changed region of an edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyRect {
    bounds: Option<(i32, i32, i32, i32)>,
}

impl DirtyRect {
    pub fn add(&mut self, p: Point) {
        self.bounds = Some(match self.bounds {
            None => (p.x, p.y, p.x, p.y),
            Some((x0, y0, x1, y1)) => (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        });
    }

    pub fn add_rect(&mut self, r: Rect) {
        if r.is_empty() {
            return;
        }
        self.add(Point::new(r.x, r.y));
        self.add(Point::new(r.right() - 1, r.bottom() - 1));
    }

    pub fn merge(&mut self, other: &DirtyRect) {
        if let Some(r) = other.rect() {
            self.add_rect(r);
        }
    }

    pub fn rect(&self) -> Option<Rect> {
        self.bounds.map(|(x0, y0, x1, y1)| Rect::from_corners(Point::new(x0, y0), Point::new(x1, y1)))
    }

    pub fn is_empty(&self) -> bool {
        self.bounds.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_math() {
        let a = Rect::new(0, 0, 4, 4);
        let b = Rect::new(2, 2, 4, 4);
        assert_eq!(a.intersect(&b), Rect::new(2, 2, 2, 2));
        assert_eq!(a.union(&b), Rect::new(0, 0, 6, 6));
        assert!(a.intersect(&Rect::new(10, 10, 1, 1)).is_empty());
        assert_eq!(Rect::from_corners(Point::new(3, 5), Point::new(1, 2)), Rect::new(1, 2, 3, 4));
        assert_eq!(a.points().count(), 16);
    }

    #[test]
    fn point_serde_accepts_pair_and_object() {
        let p: Point = serde_json::from_str("[3, -2]").unwrap();
        assert_eq!(p, Point::new(3, -2));
        let q: Point = serde_json::from_str(r#"{"x": 1, "y": 2}"#).unwrap();
        assert_eq!(q, Point::new(1, 2));
        assert_eq!(serde_json::to_string(&p).unwrap(), "[3,-2]");
    }

    #[test]
    fn dirty_rect_grows() {
        let mut d = DirtyRect::default();
        assert!(d.rect().is_none());
        d.add(Point::new(5, 5));
        d.add(Point::new(2, 7));
        assert_eq!(d.rect(), Some(Rect::new(2, 5, 4, 3)));
    }
}
