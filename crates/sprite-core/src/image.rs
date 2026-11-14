use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::geom::{DirtyRect, Point, Rect};

/// Hard ceiling for any single image side. Projects use their own, lower, configurable limits.
pub const MAX_IMAGE_SIDE: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("tamanho inválido {0}x{1}: cada lado precisa ficar entre 1 e {MAX_IMAGE_SIDE}")]
    InvalidSize(u32, u32),
    #[error("os dados têm {actual} bytes, mas {width}x{height} RGBA precisa de {expected}")]
    WrongLength { width: u32, height: u32, expected: usize, actual: usize },
}

/// How pixels of a source image are written onto a destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PasteMode {
    /// Every source pixel replaces the destination, transparent ones included.
    #[default]
    Replace,
    /// Alpha compositing (source-over).
    Over,
    /// Opaque or translucent source pixels replace the destination; transparent ones are skipped.
    SkipTransparent,
}

/// A raster of RGBA pixels, row-major, origin at the top-left.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<Color>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

impl Image {
    pub fn check_size(width: u32, height: u32) -> Result<(), ImageError> {
        if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
            Err(ImageError::InvalidSize(width, height))
        } else {
            Ok(())
        }
    }

    /// A fully transparent image. Panics on sizes outside `1..=MAX_IMAGE_SIDE`; validate first
    /// with [`Image::check_size`] when the size comes from outside.
    pub fn new(width: u32, height: u32) -> Self {
        Image::filled(width, height, Color::TRANSPARENT)
    }

    pub fn filled(width: u32, height: u32, color: Color) -> Self {
        Image::check_size(width, height).expect("tamanho de imagem inválido");
        Image { width, height, pixels: vec![color.canonical(); width as usize * height as usize] }
    }

    pub fn from_rgba(width: u32, height: u32, bytes: &[u8]) -> Result<Self, ImageError> {
        Image::check_size(width, height)?;
        let expected = width as usize * height as usize * 4;
        if bytes.len() != expected {
            return Err(ImageError::WrongLength { width, height, expected, actual: bytes.len() });
        }
        let pixels = bytes.as_chunks::<4>().0.iter().map(|p| Color::rgba(p[0], p[1], p[2], p[3])).collect();
        Ok(Image { width, height, pixels })
    }

    pub fn to_rgba(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|c| c.to_array()).collect()
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn bounds(&self) -> Rect {
        Rect::sized(self.width, self.height)
    }

    pub fn pixels(&self) -> &[Color] {
        &self.pixels
    }

    pub const fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        self.in_bounds(x, y).then(|| y as usize * self.width as usize + x as usize)
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Out-of-bounds reads count as transparent.
    pub fn get_or_clear(&self, x: i32, y: i32) -> Color {
        self.get(x, y).unwrap_or(Color::TRANSPARENT)
    }

    /// Writes one pixel. Returns `true` when the stored value actually changed.
    pub fn set(&mut self, x: i32, y: i32, color: Color) -> bool {
        match self.index(x, y) {
            Some(i) => {
                let color = color.canonical();
                if self.pixels[i] == color {
                    false
                } else {
                    self.pixels[i] = color;
                    true
                }
            }
            None => false,
        }
    }

    pub fn fill(&mut self, color: Color) {
        let color = color.canonical();
        self.pixels.iter_mut().for_each(|p| *p = color);
    }

    pub fn is_blank(&self) -> bool {
        self.pixels.iter().all(|p| p.is_transparent())
    }

    /// Copies a region; parts outside the image come back transparent.
    pub fn crop(&self, rect: Rect) -> Image {
        let mut out = Image::new(rect.w.max(1), rect.h.max(1));
        for y in 0..rect.h as i32 {
            for x in 0..rect.w as i32 {
                if let Some(c) = self.get(rect.x + x, rect.y + y) {
                    out.pixels[y as usize * out.width as usize + x as usize] = c;
                }
            }
        }
        out
    }

    /// Writes `src` with its top-left at `at`. Returns the region that changed.
    pub fn paste(&mut self, src: &Image, at: Point, mode: PasteMode) -> DirtyRect {
        let mut dirty = DirtyRect::default();
        for sy in 0..src.height as i32 {
            for sx in 0..src.width as i32 {
                let (dx, dy) = (at.x + sx, at.y + sy);
                if !self.in_bounds(dx, dy) {
                    continue;
                }
                let s = src.pixels[sy as usize * src.width as usize + sx as usize];
                let new = match mode {
                    PasteMode::Replace => s,
                    PasteMode::Over => s.over(self.get_or_clear(dx, dy)),
                    PasteMode::SkipTransparent => {
                        if s.is_transparent() {
                            continue;
                        }
                        s
                    }
                };
                if self.set(dx, dy, new) {
                    dirty.add(Point::new(dx, dy));
                }
            }
        }
        dirty
    }

    /// Bounding box of every pixel with alpha > 0.
    pub fn opaque_bounds(&self) -> Option<Rect> {
        let mut dirty = DirtyRect::default();
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                if !self.pixels[y as usize * self.width as usize + x as usize].is_transparent() {
                    dirty.add(Point::new(x, y));
                }
            }
        }
        dirty.rect()
    }

    pub fn flip_horizontal(&self) -> Image {
        self.remap(self.width, self.height, |x, y| (self.width as i32 - 1 - x, y))
    }

    pub fn flip_vertical(&self) -> Image {
        self.remap(self.width, self.height, |x, y| (x, self.height as i32 - 1 - y))
    }

    /// Rotates by `quarter_turns` × 90° clockwise.
    pub fn rotate_quarters(&self, quarter_turns: i32) -> Image {
        let (w, h) = (self.width as i32, self.height as i32);
        match quarter_turns.rem_euclid(4) {
            0 => self.clone(),
            1 => self.remap(self.height, self.width, |x, y| (y, h - 1 - x)),
            2 => self.remap(self.width, self.height, |x, y| (w - 1 - x, h - 1 - y)),
            _ => self.remap(self.height, self.width, |x, y| (w - 1 - y, x)),
        }
    }

    /// Nearest-neighbor resample to `new_w`×`new_h`. No smoothing is ever applied.
    pub fn scale_nearest(&self, new_w: u32, new_h: u32) -> Image {
        let (sw, sh) = (self.width as u64, self.height as u64);
        self.remap(new_w, new_h, |x, y| ((x as u64 * sw / new_w as u64) as i32, (y as u64 * sh / new_h as u64) as i32))
    }

    /// New canvas size; the old content is placed with its top-left at `offset`.
    pub fn resize_canvas(&self, new_w: u32, new_h: u32, offset: Point) -> Image {
        let mut out = Image::new(new_w, new_h);
        out.paste(self, offset, PasteMode::Replace);
        out
    }

    /// Builds an image where each destination pixel reads `src(map(x, y))`.
    fn remap(&self, w: u32, h: u32, map: impl Fn(i32, i32) -> (i32, i32)) -> Image {
        let mut out = Image::new(w, h);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (sx, sy) = map(x, y);
                out.pixels[y as usize * w as usize + x as usize] = self.get_or_clear(sx, sy);
            }
        }
        out
    }

    /// How many pixels use each color, transparent excluded.
    pub fn color_counts(&self) -> BTreeMap<Color, u32> {
        let mut counts = BTreeMap::new();
        for p in self.pixels.iter().filter(|p| !p.is_transparent()) {
            *counts.entry(*p).or_insert(0) += 1;
        }
        counts
    }

    pub fn opaque_count(&self) -> u32 {
        self.pixels.iter().filter(|p| !p.is_transparent()).count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Image {
        // 3x2:  R G B
        //       . . W
        let mut img = Image::new(3, 2);
        img.set(0, 0, Color::rgb(255, 0, 0));
        img.set(1, 0, Color::rgb(0, 255, 0));
        img.set(2, 0, Color::rgb(0, 0, 255));
        img.set(2, 1, Color::WHITE);
        img
    }

    #[test]
    fn get_set_bounds() {
        let mut img = Image::new(2, 2);
        assert!(img.set(1, 1, Color::BLACK));
        assert!(!img.set(1, 1, Color::BLACK), "same value is not a change");
        assert!(!img.set(2, 0, Color::BLACK), "out of bounds is ignored");
        assert_eq!(img.get(1, 1), Some(Color::BLACK));
        assert_eq!(img.get(-1, 0), None);
        assert_eq!(img.opaque_count(), 1);
    }

    #[test]
    fn rgba_roundtrip_and_validation() {
        let img = sample();
        let back = Image::from_rgba(3, 2, &img.to_rgba()).unwrap();
        assert_eq!(back, img);
        assert!(Image::from_rgba(3, 2, &[0; 5]).is_err());
        assert!(Image::check_size(0, 4).is_err());
    }

    #[test]
    fn transforms() {
        let img = sample();
        let flipped = img.flip_horizontal();
        assert_eq!(flipped.get(0, 0), Some(Color::rgb(0, 0, 255)));
        assert_eq!(flipped.get(0, 1), Some(Color::WHITE));
        let rotated = img.rotate_quarters(1);
        assert_eq!((rotated.width(), rotated.height()), (2, 3));
        // Clockwise: the old bottom-left goes to the top-left.
        assert_eq!(rotated.get(1, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(rotated.get(0, 2), Some(Color::WHITE));
        assert_eq!(img.rotate_quarters(4), img);
        assert_eq!(img.rotate_quarters(1).rotate_quarters(-1), img);
        assert_eq!(img.flip_vertical().flip_vertical(), img);
    }

    #[test]
    fn nearest_scale_has_no_new_colors() {
        let img = sample();
        let big = img.scale_nearest(9, 6);
        assert_eq!(big.get(4, 1), Some(Color::rgb(0, 255, 0)));
        assert_eq!(big.color_counts().len(), img.color_counts().len());
        assert_eq!(big.scale_nearest(3, 2), img);
    }

    #[test]
    fn crop_and_paste() {
        let img = sample();
        let part = img.crop(Rect::new(1, 0, 3, 2));
        assert_eq!(part.get(0, 0), Some(Color::rgb(0, 255, 0)));
        assert_eq!(part.get(2, 0), Some(Color::TRANSPARENT), "outside source is transparent");
        let mut canvas = Image::filled(4, 4, Color::BLACK);
        let dirty = canvas.paste(&img, Point::new(1, 1), PasteMode::SkipTransparent);
        assert_eq!(canvas.get(1, 2), Some(Color::BLACK), "transparent pixel skipped");
        assert_eq!(dirty.rect(), Some(Rect::new(1, 1, 3, 2)));
        assert_eq!(img.opaque_bounds(), Some(Rect::new(0, 0, 3, 2)));
        assert_eq!(Image::new(3, 3).opaque_bounds(), None);
    }
}
