//! Tilesets: the tile grid of a sprite (tile size, margin, spacing), collision polygons and
//! terrain peering bits per tile (the Godot 4 terrain model), and a small preview map built from
//! the tiles. Everything here is plain data plus deterministic helpers; the autotile builders
//! live in [`crate::autotile`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image, PasteMode, Point, Rect};

/// Tile coordinates `[column, row]` in the grid.
pub type TileCoord = [u32; 2];

/// Largest preview map side, in tiles.
pub const MAX_MAP_SIDE: u32 = 64;
/// Most polygons per tile and points per polygon.
pub const MAX_POLYGONS: usize = 16;
pub const MAX_POLYGON_POINTS: usize = 256;

/// How a sprite is cut into tiles. Same rule as a Godot atlas: the first tile starts after the
/// margin, then every tile is `tile + spacing` further.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TileGrid {
    /// Tile width in pixels.
    pub tile_width: u32,
    /// Tile height in pixels.
    pub tile_height: u32,
    /// Pixels before the first tile `[x, y]` (left and top).
    #[serde(default)]
    pub margin: [u32; 2],
    /// Pixels between neighboring tiles `[x, y]`.
    #[serde(default)]
    pub spacing: [u32; 2],
}

impl TileGrid {
    pub fn new(tile_width: u32, tile_height: u32) -> Self {
        TileGrid { tile_width, tile_height, margin: [0, 0], spacing: [0, 0] }
    }

    /// Columns and rows that fit in an image of `width`×`height` (Godot's atlas grid rule).
    pub fn columns_rows(&self, width: u32, height: u32) -> (u32, u32) {
        let fit = |size: u32, margin: u32, tile: u32, spacing: u32| -> u32 {
            if tile == 0 || size < margin.saturating_add(tile) {
                return 0;
            }
            1 + (size - margin - tile) / (tile + spacing)
        };
        (fit(width, self.margin[0], self.tile_width, self.spacing[0]), fit(height, self.margin[1], self.tile_height, self.spacing[1]))
    }

    /// Pixel rectangle of a tile in the sprite.
    pub fn tile_rect(&self, tile: TileCoord) -> Rect {
        let x = self.margin[0] + tile[0] * (self.tile_width + self.spacing[0]);
        let y = self.margin[1] + tile[1] * (self.tile_height + self.spacing[1]);
        Rect::new(x as i32, y as i32, self.tile_width, self.tile_height)
    }

    /// Pixels left over on the right and bottom after the last full tile.
    pub fn leftover(&self, width: u32, height: u32) -> [u32; 2] {
        let (c, r) = self.columns_rows(width, height);
        let used = |n: u32, margin: u32, tile: u32, spacing: u32| if n == 0 { 0 } else { margin + n * tile + (n - 1) * spacing };
        [
            width.saturating_sub(used(c, self.margin[0], self.tile_width, self.spacing[0])),
            height.saturating_sub(used(r, self.margin[1], self.tile_height, self.spacing[1])),
        ]
    }

    /// Size of an image holding `columns`×`rows` tiles of this grid (margin only on the left/top).
    pub fn image_size(&self, columns: u32, rows: u32) -> (u32, u32) {
        let side = |n: u32, margin: u32, tile: u32, spacing: u32| margin + n * tile + n.saturating_sub(1) * spacing;
        (side(columns, self.margin[0], self.tile_width, self.spacing[0]), side(rows, self.margin[1], self.tile_height, self.spacing[1]))
    }
}

/// Collision polygons of one tile, in tile pixels (vertices on pixel corners, origin at the
/// tile's top-left, from 0 to the tile size).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TileCollision {
    pub tile: TileCoord,
    pub polygons: Vec<Vec<Point>>,
}

/// Which neighbors a terrain set compares (Godot `TileSet.TerrainMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerrainMode {
    /// Sides and corners (8 bits; the 47-tile "blob").
    #[default]
    MatchCornersAndSides,
    /// Corners only (4 bits; 16 tiles).
    MatchCorners,
    /// Sides only (4 bits; 16 tiles).
    MatchSides,
}

impl TerrainMode {
    /// Value of `terrain_set_N/mode` in a Godot TileSet.
    pub fn godot(self) -> u8 {
        match self {
            TerrainMode::MatchCornersAndSides => 0,
            TerrainMode::MatchCorners => 1,
            TerrainMode::MatchSides => 2,
        }
    }

    pub fn allows(self, bit: PeeringBit) -> bool {
        match self {
            TerrainMode::MatchCornersAndSides => true,
            TerrainMode::MatchCorners => bit.is_corner(),
            TerrainMode::MatchSides => !bit.is_corner(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TerrainMode::MatchCornersAndSides => "match_corners_and_sides",
            TerrainMode::MatchCorners => "match_corners",
            TerrainMode::MatchSides => "match_sides",
        }
    }
}

/// A side or corner of a square tile, named like Godot's `terrains_peering_bit/*` properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PeeringBit {
    RightSide,
    BottomRightCorner,
    BottomSide,
    BottomLeftCorner,
    LeftSide,
    TopLeftCorner,
    TopSide,
    TopRightCorner,
}

impl PeeringBit {
    /// Godot's order.
    pub const ALL: [PeeringBit; 8] = [
        PeeringBit::RightSide,
        PeeringBit::BottomRightCorner,
        PeeringBit::BottomSide,
        PeeringBit::BottomLeftCorner,
        PeeringBit::LeftSide,
        PeeringBit::TopLeftCorner,
        PeeringBit::TopSide,
        PeeringBit::TopRightCorner,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PeeringBit::RightSide => "right_side",
            PeeringBit::BottomRightCorner => "bottom_right_corner",
            PeeringBit::BottomSide => "bottom_side",
            PeeringBit::BottomLeftCorner => "bottom_left_corner",
            PeeringBit::LeftSide => "left_side",
            PeeringBit::TopLeftCorner => "top_left_corner",
            PeeringBit::TopSide => "top_side",
            PeeringBit::TopRightCorner => "top_right_corner",
        }
    }

    pub fn is_corner(self) -> bool {
        matches!(
            self,
            PeeringBit::BottomRightCorner | PeeringBit::BottomLeftCorner | PeeringBit::TopLeftCorner | PeeringBit::TopRightCorner
        )
    }

    /// Offset of the neighbor this bit looks at.
    pub fn offset(self) -> (i32, i32) {
        match self {
            PeeringBit::RightSide => (1, 0),
            PeeringBit::BottomRightCorner => (1, 1),
            PeeringBit::BottomSide => (0, 1),
            PeeringBit::BottomLeftCorner => (-1, 1),
            PeeringBit::LeftSide => (-1, 0),
            PeeringBit::TopLeftCorner => (-1, -1),
            PeeringBit::TopSide => (0, -1),
            PeeringBit::TopRightCorner => (1, -1),
        }
    }

    /// Bit of the 8-neighbor mask used by the blob layout: N=1, NE=2, E=4, SE=8, S=16, SW=32,
    /// W=64, NW=128.
    pub fn mask(self) -> u8 {
        match self {
            PeeringBit::TopSide => 1,
            PeeringBit::TopRightCorner => 2,
            PeeringBit::RightSide => 4,
            PeeringBit::BottomRightCorner => 8,
            PeeringBit::BottomSide => 16,
            PeeringBit::BottomLeftCorner => 32,
            PeeringBit::LeftSide => 64,
            PeeringBit::TopLeftCorner => 128,
        }
    }
}

/// Terrain index of each neighbor a tile expects (absent = no terrain).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PeeringBits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_side: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_right_corner: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_side: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom_left_corner: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_side: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_left_corner: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_side: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_right_corner: Option<u32>,
}

impl PeeringBits {
    pub fn get(&self, bit: PeeringBit) -> Option<u32> {
        match bit {
            PeeringBit::RightSide => self.right_side,
            PeeringBit::BottomRightCorner => self.bottom_right_corner,
            PeeringBit::BottomSide => self.bottom_side,
            PeeringBit::BottomLeftCorner => self.bottom_left_corner,
            PeeringBit::LeftSide => self.left_side,
            PeeringBit::TopLeftCorner => self.top_left_corner,
            PeeringBit::TopSide => self.top_side,
            PeeringBit::TopRightCorner => self.top_right_corner,
        }
    }

    pub fn set(&mut self, bit: PeeringBit, value: Option<u32>) {
        let slot = match bit {
            PeeringBit::RightSide => &mut self.right_side,
            PeeringBit::BottomRightCorner => &mut self.bottom_right_corner,
            PeeringBit::BottomSide => &mut self.bottom_side,
            PeeringBit::BottomLeftCorner => &mut self.bottom_left_corner,
            PeeringBit::LeftSide => &mut self.left_side,
            PeeringBit::TopLeftCorner => &mut self.top_left_corner,
            PeeringBit::TopSide => &mut self.top_side,
            PeeringBit::TopRightCorner => &mut self.top_right_corner,
        };
        *slot = value;
    }

    /// Bits that are set, in Godot's order.
    pub fn iter(&self) -> impl Iterator<Item = (PeeringBit, u32)> + '_ {
        PeeringBit::ALL.into_iter().filter_map(|b| self.get(b).map(|t| (b, t)))
    }

    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Bits pointing at `terrain`, as an 8-neighbor mask (see [`PeeringBit::mask`]).
    pub fn mask_of(&self, terrain: u32) -> u8 {
        self.iter().filter(|(_, t)| *t == terrain).fold(0, |m, (b, _)| m | b.mask())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Terrain {
    pub name: String,
    pub color: Color,
}

/// A group of terrains that blend with each other (Godot `terrain_set_N`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TerrainSet {
    pub name: String,
    #[serde(default)]
    pub mode: TerrainMode,
    #[serde(default)]
    pub terrains: Vec<Terrain>,
}

/// Terrain data of one tile: the terrain of its center and what each neighbor should be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TileTerrain {
    pub tile: TileCoord,
    /// Index of the terrain set.
    pub set: u32,
    /// Terrain of the tile itself (index in the set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain: Option<u32>,
    #[serde(default, skip_serializing_if = "PeeringBits::is_empty")]
    pub peering: PeeringBits,
}

/// A small map painted with the tiles, to preview how they connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TileMap {
    /// Width in tiles.
    pub width: u32,
    /// Height in tiles.
    pub height: u32,
    /// Row by row, `width * height` cells: the tile shown, or null (empty).
    pub cells: Vec<Option<TileCoord>>,
}

impl TileMap {
    pub fn new(width: u32, height: u32) -> Self {
        TileMap { width, height, cells: vec![None; (width * height) as usize] }
    }

    pub fn get(&self, x: i32, y: i32) -> Option<TileCoord> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        self.cells[(y as u32 * self.width + x as u32) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, tile: Option<TileCoord>) {
        if x < self.width && y < self.height {
            self.cells[(y * self.width + x) as usize] = tile;
        }
    }

    /// Changes the size keeping the cells that still fit (top-left anchored).
    pub fn resized(&self, width: u32, height: u32) -> TileMap {
        let mut out = TileMap::new(width, height);
        for y in 0..height.min(self.height) {
            for x in 0..width.min(self.width) {
                out.set(x, y, self.get(x as i32, y as i32));
            }
        }
        out
    }
}

/// Everything a sprite knows about being a tileset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TileData {
    #[serde(flatten)]
    pub grid: TileGrid,
    /// Collision polygons, one entry per tile (row-major order).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collisions: Vec<TileCollision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrain_sets: Vec<TerrainSet>,
    /// Terrain and peering bits, one entry per tile (row-major order).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terrains: Vec<TileTerrain>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_map: Option<TileMap>,
}

fn row_major(t: &TileCoord) -> (u32, u32) {
    (t[1], t[0])
}

impl TileData {
    pub fn new(grid: TileGrid) -> Self {
        TileData { grid, collisions: Vec::new(), terrain_sets: Vec::new(), terrains: Vec::new(), preview_map: None }
    }

    pub fn collision(&self, tile: TileCoord) -> Option<&TileCollision> {
        self.collisions.iter().find(|c| c.tile == tile)
    }

    /// Replaces the polygons of a tile (an empty list removes its collision).
    pub fn set_collision(&mut self, tile: TileCoord, polygons: Vec<Vec<Point>>) {
        self.collisions.retain(|c| c.tile != tile);
        if !polygons.is_empty() {
            self.collisions.push(TileCollision { tile, polygons });
            self.collisions.sort_by_key(|c| row_major(&c.tile));
        }
    }

    pub fn terrain(&self, tile: TileCoord) -> Option<&TileTerrain> {
        self.terrains.iter().find(|t| t.tile == tile)
    }

    /// Replaces the terrain data of a tile (`None` removes it).
    pub fn set_terrain(&mut self, tile: TileCoord, value: Option<TileTerrain>) {
        self.terrains.retain(|t| t.tile != tile);
        if let Some(v) = value {
            self.terrains.push(TileTerrain { tile, ..v });
            self.terrains.sort_by_key(|t| row_major(&t.tile));
        }
    }

    /// Drops data of tiles outside a `columns`×`rows` grid and polygons that no longer fit the
    /// tile size. Returns how many entries were removed.
    pub fn retain_valid(&mut self, columns: u32, rows: u32) -> usize {
        let inside = |t: &TileCoord| t[0] < columns && t[1] < rows;
        let (w, h) = (self.grid.tile_width, self.grid.tile_height);
        let before = self.collisions.len() + self.terrains.len();
        self.collisions.retain(|c| inside(&c.tile));
        for c in &mut self.collisions {
            c.polygons.retain(|p| validate_polygon(p, w, h).is_ok());
        }
        self.collisions.retain(|c| !c.polygons.is_empty());
        self.terrains.retain(|t| inside(&t.tile));
        let mut removed = before - (self.collisions.len() + self.terrains.len());
        if let Some(map) = &mut self.preview_map {
            for cell in &mut map.cells {
                if cell.is_some_and(|t| !inside(&t)) {
                    *cell = None;
                    removed += 1;
                }
            }
        }
        removed
    }

    /// Removes references to terrains that do not exist anymore (after a set lost terrains).
    pub fn prune_terrains(&mut self) -> usize {
        let sets = self.terrain_sets.clone();
        let mut removed = 0;
        for t in &mut self.terrains {
            let count = sets.get(t.set as usize).map(|s| s.terrains.len() as u32).unwrap_or(0);
            let mode = sets.get(t.set as usize).map(|s| s.mode).unwrap_or_default();
            if t.terrain.is_some_and(|x| x >= count) {
                t.terrain = None;
                removed += 1;
            }
            for bit in PeeringBit::ALL {
                if t.peering.get(bit).is_some_and(|x| x >= count) || (t.peering.get(bit).is_some() && !mode.allows(bit)) {
                    t.peering.set(bit, None);
                    removed += 1;
                }
            }
        }
        let before = self.terrains.len();
        self.terrains.retain(|t| (t.set as usize) < sets.len() && (t.terrain.is_some() || !t.peering.is_empty()));
        removed + before - self.terrains.len()
    }
}

/// Shortcut collision shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CollisionShape {
    /// The whole tile.
    Full,
    TopHalf,
    BottomHalf,
    LeftHalf,
    RightHalf,
}

/// The polygon of a shortcut shape for a `w`×`h` tile (halves split at `w/2`, `h/2`).
pub fn collision_shape(shape: CollisionShape, w: u32, h: u32) -> Vec<Point> {
    let (w, h) = (w as i32, h as i32);
    let (x0, y0, x1, y1) = match shape {
        CollisionShape::Full => (0, 0, w, h),
        CollisionShape::TopHalf => (0, 0, w, h / 2),
        CollisionShape::BottomHalf => (0, h / 2, w, h),
        CollisionShape::LeftHalf => (0, 0, w / 2, h),
        CollisionShape::RightHalf => (w / 2, 0, w, h),
    };
    vec![Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)]
}

/// Checks a collision polygon for a `w`×`h` tile: 3 to 256 vertices, all within the tile
/// (0..=w, 0..=h), with a non-zero area.
pub fn validate_polygon(points: &[Point], w: u32, h: u32) -> Result<(), String> {
    if points.len() < 3 {
        return Err(format!("um polígono precisa de pelo menos 3 pontos (veio com {})", points.len()));
    }
    if points.len() > MAX_POLYGON_POINTS {
        return Err(format!("um polígono tem no máximo {MAX_POLYGON_POINTS} pontos"));
    }
    if let Some(p) = points.iter().find(|p| p.x < 0 || p.y < 0 || p.x > w as i32 || p.y > h as i32) {
        return Err(format!("o ponto [{}, {}] fica fora do tile {w}x{h} (use de 0 a {w} em x e de 0 a {h} em y)", p.x, p.y));
    }
    if polygon_area2(points) == 0 {
        return Err("o polígono não tem área (pontos alinhados ou repetidos)".into());
    }
    Ok(())
}

/// Twice the signed area (shoelace).
pub fn polygon_area2(points: &[Point]) -> i64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.x as i64 * b.y as i64 - b.x as i64 * a.y as i64
        })
        .sum()
}

/// True when every pixel of the tile is transparent.
pub fn tile_is_empty(image: &Image, grid: &TileGrid, tile: TileCoord) -> bool {
    let r = grid.tile_rect(tile);
    r.points().all(|p| image.get_or_clear(p.x, p.y).is_transparent())
}

/// Every tile of the grid, row-major.
pub fn all_tiles(columns: u32, rows: u32) -> impl Iterator<Item = TileCoord> {
    (0..rows).flat_map(move |r| (0..columns).map(move |c| [c, r]))
}

/// Draws a preview map: each cell gets the pixels of its tile from `source`.
pub fn render_map(source: &Image, grid: &TileGrid, map: &TileMap) -> Image {
    let (tw, th) = (grid.tile_width, grid.tile_height);
    let mut out = Image::new((map.width * tw).max(1), (map.height * th).max(1));
    for y in 0..map.height {
        for x in 0..map.width {
            if let Some(t) = map.get(x as i32, y as i32) {
                let tile = source.crop(grid.tile_rect(t));
                out.paste(&tile, Point::new((x * tw) as i32, (y * th) as i32), PasteMode::Replace);
            }
        }
    }
    out
}

/// Picks the tile of `set`/`terrain` whose peering bits best match which neighbors are terrain
/// (`neighbors(bit)`). Ties prefer fewer claimed neighbors that are not there, then the first
/// tile in row-major order.
pub fn pick_terrain_tile(data: &TileData, set: u32, terrain: u32, neighbors: impl Fn(PeeringBit) -> bool) -> Option<TileCoord> {
    let mode = data.terrain_sets.get(set as usize)?.mode;
    let mut best: Option<((u32, i32), TileCoord)> = None;
    for t in data.terrains.iter().filter(|t| t.set == set && t.terrain == Some(terrain)) {
        let mut matches = 0u32;
        let mut false_claims = 0i32;
        for bit in PeeringBit::ALL.into_iter().filter(|b| mode.allows(*b)) {
            let claims = t.peering.get(bit) == Some(terrain);
            let present = neighbors(bit);
            if claims == present {
                matches += 1;
            } else if claims {
                false_claims += 1;
            }
        }
        let score = (matches, -false_claims);
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, t.tile));
        }
    }
    best.map(|(_, t)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_counts_like_godot() {
        let g = TileGrid { tile_width: 16, tile_height: 16, margin: [1, 2], spacing: [2, 1] };
        // 1 + 16 + 2 + 16 = 35 → 2 columns; 2 + 16 + 1 + 16 = 35 → 2 rows.
        assert_eq!(g.columns_rows(35, 35), (2, 2));
        assert_eq!(g.columns_rows(52, 34), (2, 1));
        assert_eq!(g.columns_rows(53, 52), (3, 3));
        assert_eq!(g.columns_rows(16, 16), (0, 0));
        assert_eq!(g.tile_rect([1, 1]), Rect::new(19, 19, 16, 16));
        assert_eq!(g.image_size(2, 2), (35, 35));
        assert_eq!(g.leftover(40, 35), [5, 0]);
        assert_eq!(TileGrid::new(8, 8).columns_rows(32, 16), (4, 2));
    }

    #[test]
    fn polygons_are_validated() {
        assert!(validate_polygon(&collision_shape(CollisionShape::Full, 16, 16), 16, 16).is_ok());
        assert!(validate_polygon(&[Point::new(0, 0), Point::new(17, 0), Point::new(0, 4)], 16, 16).is_err());
        assert!(validate_polygon(&[Point::new(0, 0), Point::new(4, 4), Point::new(8, 8)], 16, 16).is_err());
        assert!(validate_polygon(&[Point::new(0, 0), Point::new(4, 4)], 16, 16).is_err());
        assert_eq!(collision_shape(CollisionShape::BottomHalf, 16, 16)[0], Point::new(0, 8));
    }

    #[test]
    fn data_keeps_row_major_order_and_prunes() {
        let mut d = TileData::new(TileGrid::new(8, 8));
        d.set_collision([1, 1], vec![collision_shape(CollisionShape::Full, 8, 8)]);
        d.set_collision([3, 0], vec![collision_shape(CollisionShape::TopHalf, 8, 8)]);
        assert_eq!(d.collisions[0].tile, [3, 0]);
        d.set_collision([3, 0], vec![]);
        assert_eq!(d.collisions.len(), 1);
        assert_eq!(d.retain_valid(1, 1), 1);
        assert!(d.collisions.is_empty());
    }

    #[test]
    fn map_renders_tiles() {
        let mut src = Image::new(16, 8);
        for p in Rect::new(8, 0, 8, 8).points() {
            src.set(p.x, p.y, Color::rgb(255, 0, 0));
        }
        let mut map = TileMap::new(3, 1);
        map.set(1, 0, Some([1, 0]));
        map.set(2, 0, Some([0, 0]));
        let img = render_map(&src, &TileGrid::new(8, 8), &map);
        assert_eq!((img.width(), img.height()), (24, 8));
        assert_eq!(img.get(0, 0), Some(Color::TRANSPARENT));
        assert_eq!(img.get(8, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(img.get(23, 7), Some(Color::TRANSPARENT));
        assert_eq!(map.resized(2, 2).get(1, 0), Some([1, 0]));
    }

    #[test]
    fn serde_round_trip() {
        let mut d = TileData::new(TileGrid { tile_width: 16, tile_height: 8, margin: [1, 0], spacing: [0, 2] });
        d.terrain_sets.push(TerrainSet {
            name: "chão".into(),
            mode: TerrainMode::MatchSides,
            terrains: vec![Terrain { name: "grama".into(), color: Color::rgb(0, 200, 0) }],
        });
        let mut bits = PeeringBits::default();
        bits.set(PeeringBit::TopSide, Some(0));
        d.set_terrain([0, 0], Some(TileTerrain { tile: [0, 0], set: 0, terrain: Some(0), peering: bits }));
        let text = serde_json::to_string(&d).unwrap();
        assert!(text.contains("\"tile_width\":16"), "{text}");
        assert!(text.contains("\"top_side\":0"), "{text}");
        assert!(text.contains("\"mode\":\"match_sides\""), "{text}");
        let back: TileData = serde_json::from_str(&text).unwrap();
        assert_eq!(back, d);
        assert_eq!(bits.mask_of(0), 1);
    }
}
