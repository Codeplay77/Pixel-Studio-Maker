//! Autotiles assembled from quarter tiles. A source block (RPG Maker A2 or a 3x3 frame with inner
//! corners) is split into half-tile "quarters"; each quarter of an output tile is chosen from its
//! neighbors:
//!
//! - no vertical and no horizontal neighbor → outer corner;
//! - only the vertical one → side edge (left/right border);
//! - only the horizontal one → top/bottom edge;
//! - both but not the diagonal → inner corner;
//! - all three → interior.
//!
//! Outputs: the RPG Maker A2 block (2×3 tiles) and the 47-tile "blob" set (every valid
//! combination of the 8 neighbors, corners only counting when both adjacent sides are there).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Image, PasteMode, Point, Rect};

use crate::tiles::PeeringBit;

/// Layout of the source block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutotileInput {
    /// 2×3 tiles: (0,0) icon, (1,0) inner corners, rows 1–2 the frame in 4×4 quarters.
    RpgmakerA2,
    /// 4×3 tiles: columns 0–2 a 3×3 frame (corners, edges, center), (3,0) inner corners.
    #[serde(rename = "frame_3x3")]
    Frame3x3,
}

impl AutotileInput {
    /// Size of the source block in tiles.
    pub fn size(self) -> (u32, u32) {
        match self {
            AutotileInput::RpgmakerA2 => (2, 3),
            AutotileInput::Frame3x3 => (4, 3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutotileOutput {
    /// 2×3 RPG Maker A2 block.
    RpgmakerA2,
    /// 47 tiles in rows of 8, by increasing neighbor mask.
    #[serde(rename = "blob_47")]
    Blob47,
}

/// Quadrants of a tile: top-left, top-right, bottom-left, bottom-right.
pub const QUADRANTS: [&str; 4] = ["top_left", "top_right", "bottom_left", "bottom_right"];

/// What a quarter shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuarterKind {
    OuterCorner,
    /// Border on the left or right side (the vertical neighbor exists, the horizontal does not).
    SideEdge,
    /// Border on the top or bottom (the horizontal neighbor exists, the vertical does not).
    TopBottomEdge,
    InnerCorner,
    Interior,
}

impl QuarterKind {
    pub const ALL: [QuarterKind; 5] =
        [QuarterKind::OuterCorner, QuarterKind::SideEdge, QuarterKind::TopBottomEdge, QuarterKind::InnerCorner, QuarterKind::Interior];

    fn index(self) -> usize {
        self as usize
    }

    /// The rule of the module doc.
    pub fn choose(vertical: bool, horizontal: bool, diagonal: bool) -> QuarterKind {
        match (vertical, horizontal) {
            (false, false) => QuarterKind::OuterCorner,
            (true, false) => QuarterKind::SideEdge,
            (false, true) => QuarterKind::TopBottomEdge,
            (true, true) if !diagonal => QuarterKind::InnerCorner,
            (true, true) => QuarterKind::Interior,
        }
    }
}

/// Mask bits of the neighbors a quadrant looks at: (vertical, horizontal, diagonal).
pub fn quadrant_neighbors(quadrant: usize) -> (u8, u8, u8) {
    use PeeringBit as B;
    match quadrant {
        0 => (B::TopSide.mask(), B::LeftSide.mask(), B::TopLeftCorner.mask()),
        1 => (B::TopSide.mask(), B::RightSide.mask(), B::TopRightCorner.mask()),
        2 => (B::BottomSide.mask(), B::LeftSide.mask(), B::BottomLeftCorner.mask()),
        _ => (B::BottomSide.mask(), B::RightSide.mask(), B::BottomRightCorner.mask()),
    }
}

/// Removes corner bits whose two adjacent sides are not both set.
pub fn reduce_mask(mask: u8) -> u8 {
    let mut m = mask;
    for q in 0..4 {
        let (v, h, d) = quadrant_neighbors(q);
        if mask & v == 0 || mask & h == 0 {
            m &= !d;
        }
    }
    m
}

/// The 47 reduced masks, increasing.
pub fn blob47_masks() -> Vec<u8> {
    let mut v: Vec<u8> = (0..=255u8).map(reduce_mask).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// The quarter pieces of an autotile, read from a source block.
#[derive(Debug, Clone)]
pub struct QuarterSet {
    pub tile_width: u32,
    pub tile_height: u32,
    /// `[quadrant][kind]`, each `tile/2` in size.
    pieces: Vec<Vec<Image>>,
    /// The source's own icon tile (RPG Maker A2 only).
    pub icon: Option<Image>,
}

fn quarter(block: &Image, tile: (u32, u32), quadrant: usize, tw: u32, th: u32) -> Image {
    let (hw, hh) = (tw / 2, th / 2);
    let x = tile.0 * tw + (quadrant as u32 & 1) * hw;
    let y = tile.1 * th + (quadrant as u32 >> 1) * hh;
    block.crop(Rect::new(x as i32, y as i32, hw, hh))
}

/// Quarter `(qx, qy)` of a block measured in quarters.
fn quarter_at(block: &Image, qx: u32, qy: u32, tw: u32, th: u32) -> Image {
    let (hw, hh) = (tw / 2, th / 2);
    block.crop(Rect::new((qx * hw) as i32, (qy * hh) as i32, hw, hh))
}

/// Position of a piece in the A2 frame (4×4 quarters): borders at 0 and 3, the inside at 1–2,
/// always with the parity of the quadrant (even = left/top, odd = right/bottom).
fn a2_frame_position(quadrant: usize, kind: QuarterKind) -> Option<(u32, u32)> {
    let (px, py) = (quadrant as u32 & 1, quadrant as u32 >> 1);
    let border = |p: u32| p * 3;
    let inside = |p: u32| if p == 0 { 2 } else { 1 };
    match kind {
        QuarterKind::OuterCorner => Some((border(px), border(py))),
        QuarterKind::SideEdge => Some((border(px), inside(py))),
        QuarterKind::TopBottomEdge => Some((inside(px), border(py))),
        QuarterKind::Interior => Some((inside(px), inside(py))),
        QuarterKind::InnerCorner => None,
    }
}

impl QuarterSet {
    /// Reads the pieces from a contiguous block of `input.size()` tiles (no margin or spacing).
    pub fn from_block(input: AutotileInput, block: &Image, tw: u32, th: u32) -> Result<QuarterSet, String> {
        if tw < 2 || th < 2 || !tw.is_multiple_of(2) || !th.is_multiple_of(2) {
            return Err(format!(
                "o tile {tw}x{th} não divide em quartos: o autotile precisa de largura e altura pares (ex.: 16x16, 32x32, 48x48)"
            ));
        }
        let (bw, bh) = input.size();
        if block.width() < bw * tw || block.height() < bh * th {
            return Err(format!("a área de entrada precisa de {bw}x{bh} tiles"));
        }
        let mut pieces = Vec::with_capacity(4);
        for q in 0..4 {
            let row: Vec<Image> = QuarterKind::ALL
                .iter()
                .map(|kind| match input {
                    AutotileInput::RpgmakerA2 => match a2_frame_position(q, *kind) {
                        Some((fx, fy)) => quarter_at(block, fx, fy + 2, tw, th),
                        None => quarter(block, (1, 0), q, tw, th),
                    },
                    AutotileInput::Frame3x3 => {
                        let (px, py) = (q as u32 & 1, q as u32 >> 1);
                        let tile = match kind {
                            QuarterKind::OuterCorner => (px * 2, py * 2),
                            QuarterKind::SideEdge => (px * 2, 1),
                            QuarterKind::TopBottomEdge => (1, py * 2),
                            QuarterKind::Interior => (1, 1),
                            QuarterKind::InnerCorner => (3, 0),
                        };
                        quarter(block, tile, q, tw, th)
                    }
                })
                .collect();
            pieces.push(row);
        }
        let icon = match input {
            AutotileInput::RpgmakerA2 => Some(block.crop(Rect::new(0, 0, tw, th))),
            AutotileInput::Frame3x3 => None,
        };
        Ok(QuarterSet { tile_width: tw, tile_height: th, pieces, icon })
    }

    pub fn piece(&self, quadrant: usize, kind: QuarterKind) -> &Image {
        &self.pieces[quadrant][kind.index()]
    }

    /// The kind each quadrant uses for an 8-neighbor mask (see [`PeeringBit::mask`]).
    pub fn kinds(mask: u8) -> [QuarterKind; 4] {
        let mut out = [QuarterKind::OuterCorner; 4];
        for (q, slot) in out.iter_mut().enumerate() {
            let (v, h, d) = quadrant_neighbors(q);
            *slot = QuarterKind::choose(mask & v != 0, mask & h != 0, mask & d != 0);
        }
        out
    }

    /// One tile for an 8-neighbor mask.
    pub fn assemble(&self, mask: u8) -> Image {
        let (tw, th) = (self.tile_width, self.tile_height);
        let mut out = Image::new(tw, th);
        for (q, kind) in QuarterSet::kinds(mask).into_iter().enumerate() {
            let at = Point::new(((q as u32 & 1) * tw / 2) as i32, ((q as u32 >> 1) * th / 2) as i32);
            out.paste(self.piece(q, kind), at, PasteMode::Replace);
        }
        out
    }

    /// The RPG Maker A2 block (2×3 tiles). The icon is the source's icon, or the isolated tile.
    pub fn rpgmaker_a2(&self) -> Image {
        let (tw, th) = (self.tile_width, self.tile_height);
        let (hw, hh) = (tw / 2, th / 2);
        let mut out = Image::new(tw * 2, th * 3);
        let icon = self.icon.clone().unwrap_or_else(|| self.assemble(0));
        out.paste(&icon, Point::new(0, 0), PasteMode::Replace);
        for q in 0..4 {
            let at = Point::new((tw + (q as u32 & 1) * hw) as i32, ((q as u32 >> 1) * hh) as i32);
            out.paste(self.piece(q, QuarterKind::InnerCorner), at, PasteMode::Replace);
            for kind in QuarterKind::ALL {
                if let Some((fx, fy)) = a2_frame_position(q, kind) {
                    out.paste(self.piece(q, kind), Point::new((fx * hw) as i32, (th + fy * hh) as i32), PasteMode::Replace);
                }
            }
        }
        out
    }

    /// The 47 blob tiles in rows of `columns`, by increasing mask. Returns the image and, per
    /// tile, its `[column, row]` and mask.
    pub fn blob47(&self, columns: u32) -> (Image, Vec<([u32; 2], u8)>) {
        let (tw, th) = (self.tile_width, self.tile_height);
        let masks = blob47_masks();
        let columns = columns.max(1);
        let rows = (masks.len() as u32).div_ceil(columns);
        let mut out = Image::new(tw * columns, th * rows);
        let mut list = Vec::with_capacity(masks.len());
        for (i, m) in masks.iter().enumerate() {
            let (c, r) = (i as u32 % columns, i as u32 / columns);
            out.paste(&self.assemble(*m), Point::new((c * tw) as i32, (r * th) as i32), PasteMode::Replace);
            list.push(([c, r], *m));
        }
        (out, list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Color;

    #[test]
    fn there_are_47_blob_masks() {
        let m = blob47_masks();
        assert_eq!(m.len(), 47);
        assert_eq!(m[0], 0);
        assert_eq!(m[46], 255);
        assert!(m.contains(&(1 | 4 | 2)), "N + E + NE");
        assert!(!m.contains(&(1 | 2)), "NE without E is reduced away");
        assert_eq!(reduce_mask(2 | 8 | 32 | 128), 0);
    }

    #[test]
    fn quarter_rule() {
        assert_eq!(QuarterKind::choose(false, false, true), QuarterKind::OuterCorner);
        assert_eq!(QuarterKind::choose(true, false, false), QuarterKind::SideEdge);
        assert_eq!(QuarterKind::choose(false, true, false), QuarterKind::TopBottomEdge);
        assert_eq!(QuarterKind::choose(true, true, false), QuarterKind::InnerCorner);
        assert_eq!(QuarterKind::choose(true, true, true), QuarterKind::Interior);
        // Only north: the top quadrants are side edges, the bottom ones outer corners.
        assert_eq!(
            QuarterSet::kinds(1),
            [QuarterKind::SideEdge, QuarterKind::SideEdge, QuarterKind::OuterCorner, QuarterKind::OuterCorner]
        );
    }

    /// A block where every quarter has its own color: (qx, qy) → rgb(qx*40, qy*40, 7).
    fn coded_block(tiles_w: u32, tiles_h: u32, tw: u32, th: u32) -> Image {
        let mut img = Image::new(tiles_w * tw, tiles_h * th);
        for y in 0..img.height() {
            for x in 0..img.width() {
                let (qx, qy) = (x / (tw / 2), y / (th / 2));
                img.set(x as i32, y as i32, Color::rgb((qx * 40) as u8, (qy * 40) as u8, 7));
            }
        }
        img
    }

    #[test]
    fn a2_round_trip_is_identity() {
        let block = coded_block(2, 3, 8, 8);
        let set = QuarterSet::from_block(AutotileInput::RpgmakerA2, &block, 8, 8).unwrap();
        assert_eq!(set.rpgmaker_a2(), block);
    }

    #[test]
    fn odd_tiles_are_rejected() {
        let block = coded_block(2, 3, 8, 8);
        assert!(QuarterSet::from_block(AutotileInput::RpgmakerA2, &block, 7, 8).unwrap_err().contains("pares"));
    }
}
