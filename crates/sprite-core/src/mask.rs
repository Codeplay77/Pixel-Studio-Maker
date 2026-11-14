use crate::color::Color;
use crate::geom::{DirtyRect, Point, Rect};
use crate::image::Image;

/// A boolean pixel mask the size of a canvas: selections, semantic regions and clipping.
#[derive(Clone, PartialEq, Eq)]
pub struct Mask {
    width: u32,
    height: u32,
    bits: Vec<bool>,
}

impl std::fmt::Debug for Mask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mask({}x{}, {} on)", self.width, self.height, self.count())
    }
}

impl Mask {
    pub fn new(width: u32, height: u32) -> Self {
        Mask { width, height, bits: vec![false; width as usize * height as usize] }
    }

    pub fn full(width: u32, height: u32) -> Self {
        Mask { width, height, bits: vec![true; width as usize * height as usize] }
    }

    pub fn from_rect(width: u32, height: u32, rect: Rect) -> Self {
        let mut mask = Mask::new(width, height);
        for p in rect.intersect(&Rect::sized(width, height)).points() {
            mask.set(p.x, p.y, true);
        }
        mask
    }

    /// Pixels with alpha above zero.
    pub fn from_opaque(image: &Image) -> Self {
        let mut mask = Mask::new(image.width(), image.height());
        for (i, p) in image.pixels().iter().enumerate() {
            mask.bits[i] = !p.is_transparent();
        }
        mask
    }

    /// Pixels whose color is within `tolerance` (max channel delta) of `color`.
    pub fn from_color(image: &Image, color: Color, tolerance: u8) -> Self {
        let mut mask = Mask::new(image.width(), image.height());
        for (i, p) in image.pixels().iter().enumerate() {
            mask.bits[i] = p.max_channel_delta(color) <= tolerance;
        }
        mask
    }

    /// Text rows where any char other than `.`/space marks the pixel, placed at `origin`.
    pub fn from_rows(width: u32, height: u32, origin: Point, rows: &[String]) -> Self {
        let mut mask = Mask::new(width, height);
        for (dy, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if ch != '.' && ch != ' ' {
                    mask.set(origin.x + dx as i32, origin.y + dy as i32, true);
                }
            }
        }
        mask
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn get(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height && self.bits[y as usize * self.width as usize + x as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, on: bool) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            self.bits[y as usize * self.width as usize + x as usize] = on;
        }
    }

    pub fn count(&self) -> u32 {
        self.bits.iter().filter(|b| **b).count() as u32
    }

    pub fn is_empty(&self) -> bool {
        !self.bits.iter().any(|b| *b)
    }

    pub fn bounds(&self) -> Option<Rect> {
        let mut dirty = DirtyRect::default();
        for p in self.points() {
            dirty.add(p);
        }
        dirty.rect()
    }

    pub fn points(&self) -> impl Iterator<Item = Point> + '_ {
        let w = self.width as usize;
        self.bits.iter().enumerate().filter(|(_, b)| **b).map(move |(i, _)| Point::new((i % w) as i32, (i / w) as i32))
    }

    fn combine(&self, other: &Mask, f: impl Fn(bool, bool) -> bool) -> Mask {
        let mut out = Mask::new(self.width, self.height);
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                out.set(x, y, f(self.get(x, y), other.get(x, y)));
            }
        }
        out
    }

    pub fn union(&self, other: &Mask) -> Mask {
        self.combine(other, |a, b| a || b)
    }

    pub fn intersect(&self, other: &Mask) -> Mask {
        self.combine(other, |a, b| a && b)
    }

    pub fn subtract(&self, other: &Mask) -> Mask {
        self.combine(other, |a, b| a && !b)
    }

    pub fn invert(&self) -> Mask {
        Mask { width: self.width, height: self.height, bits: self.bits.iter().map(|b| !b).collect() }
    }

    /// Shifts the mask content; bits pushed outside are lost.
    pub fn translate(&self, dx: i32, dy: i32) -> Mask {
        let mut out = Mask::new(self.width, self.height);
        for p in self.points() {
            out.set(p.x + dx, p.y + dy, true);
        }
        out
    }

    /// Text rows covering `bounds()`, `#` on, `.` off. Empty mask gives no rows.
    pub fn to_rows(&self) -> (Option<Rect>, Vec<String>) {
        let Some(b) = self.bounds() else { return (None, Vec::new()) };
        let rows = (b.y..b.bottom()).map(|y| (b.x..b.right()).map(|x| if self.get(x, y) { '#' } else { '.' }).collect()).collect();
        (Some(b), rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_ops() {
        let a = Mask::from_rect(4, 4, Rect::new(0, 0, 2, 2));
        let b = Mask::from_rect(4, 4, Rect::new(1, 1, 2, 2));
        assert_eq!(a.union(&b).count(), 7);
        assert_eq!(a.intersect(&b).count(), 1);
        assert_eq!(a.subtract(&b).count(), 3);
        assert_eq!(a.invert().count(), 12);
        assert_eq!(a.translate(3, 3).count(), 1);
    }

    #[test]
    fn rows_roundtrip() {
        let rows = vec!["#.#".to_string(), ".#.".to_string()];
        let mask = Mask::from_rows(8, 8, Point::new(2, 3), &rows);
        assert_eq!(mask.count(), 3);
        let (bounds, back) = mask.to_rows();
        assert_eq!(bounds, Some(Rect::new(2, 3, 3, 2)));
        assert_eq!(back, rows);
    }

    #[test]
    fn from_image() {
        let mut img = Image::new(3, 1);
        img.set(0, 0, Color::rgb(10, 10, 10));
        img.set(2, 0, Color::rgb(12, 10, 10));
        assert_eq!(Mask::from_opaque(&img).count(), 2);
        assert_eq!(Mask::from_color(&img, Color::rgb(10, 10, 10), 0).count(), 1);
        assert_eq!(Mask::from_color(&img, Color::rgb(10, 10, 10), 2).count(), 2);
    }
}
