//! Drawing algorithms: every structured instruction becomes real pixel changes here.
//!
//! All rasterizers are pixel-perfect and deterministic (no antialiasing, no smoothing). Drawing
//! goes through a [`Canvas`], which respects an optional clip mask and records what changed.
//! [`sdc`] defines the Sprite Drawing Commands, the JSON language agents batch operations in.

pub mod effects;
pub mod fill;
pub mod pixelate;
pub mod raster;
pub mod sdc;
pub mod transform;

use sprite_core::{Color, DirtyRect, Image, Mask, Point, Rect};

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DrawError {
    #[error("{0}")]
    InvalidArgument(String),
    #[error(transparent)]
    Matrix(#[from] sprite_core::matrix::MatrixError),
    #[error("cor: {0}")]
    Color(String),
    #[error("{0}")]
    Source(String),
    #[error("operação grande demais: {0}")]
    TooLarge(String),
}

/// What a drawing operation did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawStats {
    pub changed: u32,
    pub dirty: DirtyRect,
}

impl DrawStats {
    pub fn merge(&mut self, other: DrawStats) {
        self.changed += other.changed;
        self.dirty.merge(&other.dirty);
    }

    pub fn rect(&self) -> Option<Rect> {
        self.dirty.rect()
    }
}

/// A drawing surface over one image, with optional clipping. All writes go through [`Canvas::put`].
pub struct Canvas<'a> {
    image: &'a mut Image,
    clip: Option<&'a Mask>,
    stats: DrawStats,
}

impl<'a> Canvas<'a> {
    pub fn new(image: &'a mut Image, clip: Option<&'a Mask>) -> Self {
        Canvas { image, clip, stats: DrawStats::default() }
    }

    pub fn width(&self) -> u32 {
        self.image.width()
    }

    pub fn height(&self) -> u32 {
        self.image.height()
    }

    pub fn bounds(&self) -> Rect {
        self.image.bounds()
    }

    pub fn image(&self) -> &Image {
        self.image
    }

    pub fn get(&self, x: i32, y: i32) -> Color {
        self.image.get_or_clear(x, y)
    }

    pub fn allowed(&self, x: i32, y: i32) -> bool {
        self.image.in_bounds(x, y) && self.clip.is_none_or(|m| m.get(x, y))
    }

    /// Writes one pixel if it is inside the image and the clip. Returns whether it changed.
    pub fn put(&mut self, x: i32, y: i32, color: Color) -> bool {
        if !self.allowed(x, y) {
            return false;
        }
        if self.image.set(x, y, color) {
            self.stats.changed += 1;
            self.stats.dirty.add(Point::new(x, y));
            true
        } else {
            false
        }
    }

    pub fn put_all(&mut self, points: impl IntoIterator<Item = Point>, color: Color) {
        for p in points {
            self.put(p.x, p.y, color);
        }
    }

    /// The pixels an operation may touch: `rect` (or the whole image) intersected with the clip.
    pub fn area(&self, rect: Option<Rect>) -> Mask {
        let (w, h) = (self.width(), self.height());
        let base = match rect {
            Some(r) => Mask::from_rect(w, h, r),
            None => Mask::full(w, h),
        };
        match self.clip {
            Some(clip) => base.intersect(clip),
            None => base,
        }
    }

    pub fn stats(&self) -> DrawStats {
        self.stats
    }
}
