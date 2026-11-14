//! Sprite sheet layout: where each frame goes. Pure geometry; rendering happens on export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SheetLayout {
    /// All frames in one row.
    Horizontal,
    /// All frames in one column.
    Vertical,
    /// A grid with `columns` (or `rows`) — square-ish by default.
    #[default]
    Grid,
    /// Each animation/direction track on its own row (what most engines expect).
    Rows,
    /// Trimmed frames packed tightly (texture atlas); cells have different sizes.
    Packed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellAlign {
    /// Keep every frame's pivot at the same spot inside its cell (default).
    #[default]
    Pivot,
    Center,
    TopLeft,
    BottomCenter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetOptions {
    #[serde(default)]
    pub layout: SheetLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    /// Pixels between cells.
    #[serde(default)]
    pub spacing: u32,
    /// Pixels around the whole sheet.
    #[serde(default)]
    pub margin: u32,
    /// Fixed cell size `[w, h]`; default fits the frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    #[serde(default)]
    pub align: CellAlign,
    /// Cut transparent borders before placing (always on for `packed`).
    #[serde(default)]
    pub trim: bool,
}

impl Default for SheetOptions {
    fn default() -> Self {
        SheetOptions {
            layout: SheetLayout::Grid,
            columns: None,
            rows: None,
            spacing: 0,
            margin: 0,
            cell: None,
            align: CellAlign::Pivot,
            trim: false,
        }
    }
}

/// One frame to place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetItem {
    pub frame: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub index: usize,
    pub duration_ms: u32,
    /// Row group (track number) for the `rows` layout.
    pub group: usize,
    /// Visible content of the frame (sprite coordinates); `None` if blank.
    pub content: Option<Rect>,
    /// Pivot in sprite coordinates.
    pub pivot: Point,
}

/// Where one frame lands in the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Placement {
    pub frame: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    pub index: usize,
    pub duration_ms: u32,
    /// The cell in the sheet.
    pub cell: Rect,
    /// Part of the sprite frame that is copied.
    pub source: Rect,
    /// Where `source`'s top-left goes in the sheet.
    pub dest: Point,
    /// Pivot position in sheet coordinates.
    pub pivot: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SheetPlan {
    pub width: u32,
    pub height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub placements: Vec<Placement>,
}

/// Computes the sheet layout for `items` of a `sprite_w`×`sprite_h` sprite.
pub fn plan(items: &[SheetItem], sprite_w: u32, sprite_h: u32, opts: &SheetOptions) -> SheetPlan {
    let full = Rect::sized(sprite_w, sprite_h);
    let trim = opts.trim || opts.layout == SheetLayout::Packed;
    let sources: Vec<Rect> = items.iter().map(|it| if trim { it.content.unwrap_or(Rect::new(0, 0, 1, 1)) } else { full }).collect();

    if opts.layout == SheetLayout::Packed {
        return pack(items, &sources, opts);
    }

    // Cell size and where each source sits inside its cell.
    let (cell_w, cell_h, inner): (u32, u32, Vec<Point>) = match (opts.cell, opts.align) {
        _ if !trim && opts.cell.is_none() => (sprite_w, sprite_h, vec![Point::default(); items.len()]),
        (_, CellAlign::Pivot) => {
            let left = items.iter().zip(&sources).map(|(it, s)| it.pivot.x - s.x).max().unwrap_or(0).max(0);
            let up = items.iter().zip(&sources).map(|(it, s)| it.pivot.y - s.y).max().unwrap_or(0).max(0);
            let right = items.iter().zip(&sources).map(|(it, s)| s.right() - it.pivot.x).max().unwrap_or(1).max(1);
            let down = items.iter().zip(&sources).map(|(it, s)| s.bottom() - it.pivot.y).max().unwrap_or(1).max(1);
            let (fit_w, fit_h) = ((left + right) as u32, (up + down) as u32);
            let (cw, ch) = opts.cell.map(|[w, h]| (w, h)).unwrap_or((fit_w, fit_h));
            // Center the fitted box in a larger fixed cell.
            let (ox, oy) = ((cw as i32 - fit_w as i32) / 2, ch as i32 - fit_h as i32);
            let inner = items
                .iter()
                .zip(&sources)
                .map(|(it, s)| Point::new(ox + left - (it.pivot.x - s.x), oy + up - (it.pivot.y - s.y)))
                .collect();
            (cw, ch, inner)
        }
        (cell, align) => {
            let fit_w = sources.iter().map(|s| s.w).max().unwrap_or(1);
            let fit_h = sources.iter().map(|s| s.h).max().unwrap_or(1);
            let (cw, ch) = cell.map(|[w, h]| (w, h)).unwrap_or((fit_w, fit_h));
            let inner = sources
                .iter()
                .map(|s| match align {
                    CellAlign::TopLeft => Point::new(0, 0),
                    CellAlign::Center => Point::new((cw as i32 - s.w as i32) / 2, (ch as i32 - s.h as i32) / 2),
                    _ => Point::new((cw as i32 - s.w as i32) / 2, ch as i32 - s.h as i32),
                })
                .collect();
            (cw, ch, inner)
        }
    };

    // Grid coordinates (column, row) per item.
    let n = items.len() as u32;
    let coords: Vec<(u32, u32)> = match opts.layout {
        SheetLayout::Horizontal => (0..n).map(|i| (i, 0)).collect(),
        SheetLayout::Vertical => (0..n).map(|i| (0, i)).collect(),
        SheetLayout::Rows => {
            let mut groups: Vec<usize> = items.iter().map(|i| i.group).collect();
            groups.dedup();
            let mut col = 0;
            let mut row = 0;
            let mut last = None;
            items
                .iter()
                .map(|it| {
                    if last.is_some() && last != Some(it.group) {
                        row += 1;
                        col = 0;
                    }
                    last = Some(it.group);
                    let c = (col, row);
                    col += 1;
                    c
                })
                .collect()
        }
        _ => {
            let cols = match (opts.columns, opts.rows) {
                (Some(c), _) => c.max(1),
                (None, Some(r)) => n.div_ceil(r.max(1)).max(1),
                (None, None) => ((n as f64).sqrt().ceil() as u32).max(1),
            };
            (0..n).map(|i| (i % cols, i / cols)).collect()
        }
    };
    let cols = coords.iter().map(|c| c.0 + 1).max().unwrap_or(1);
    let rows = coords.iter().map(|c| c.1 + 1).max().unwrap_or(1);
    let (m, sp) = (opts.margin, opts.spacing);
    let width = 2 * m + cols * cell_w + cols.saturating_sub(1) * sp;
    let height = 2 * m + rows * cell_h + rows.saturating_sub(1) * sp;
    let placements = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let (c, r) = coords[i];
            let cell = Rect::new((m + c * (cell_w + sp)) as i32, (m + r * (cell_h + sp)) as i32, cell_w, cell_h);
            let source = sources[i];
            let dest = Point::new(cell.x + inner[i].x, cell.y + inner[i].y);
            Placement {
                frame: it.frame,
                animation: it.animation.clone(),
                direction: it.direction.clone(),
                index: it.index,
                duration_ms: it.duration_ms,
                cell,
                source,
                dest,
                pivot: Point::new(dest.x + it.pivot.x - source.x, dest.y + it.pivot.y - source.y),
            }
        })
        .collect();
    SheetPlan { width: width.max(1), height: height.max(1), cell_width: cell_w, cell_height: cell_h, placements }
}

/// Shelf packing, tallest first (ties keep input order), into a width near the square root of
/// the total area. Deterministic.
fn pack(items: &[SheetItem], sources: &[Rect], opts: &SheetOptions) -> SheetPlan {
    let (m, sp) = (opts.margin, opts.spacing.max(1));
    let area: u64 = sources.iter().map(|s| (s.w + sp) as u64 * (s.h + sp) as u64).sum();
    let widest = sources.iter().map(|s| s.w).max().unwrap_or(1);
    let target_w = ((area as f64).sqrt().ceil() as u32).max(widest).next_power_of_two();
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(sources[i].h), i));
    let mut placed: Vec<Option<Point>> = vec![None; items.len()];
    let (mut x, mut y, mut shelf_h, mut used_w) = (0u32, 0u32, 0u32, 0u32);
    for &i in &order {
        let s = sources[i];
        if x > 0 && x + s.w > target_w {
            y += shelf_h + sp;
            x = 0;
            shelf_h = 0;
        }
        placed[i] = Some(Point::new((m + x) as i32, (m + y) as i32));
        x += s.w + sp;
        used_w = used_w.max(x - sp);
        shelf_h = shelf_h.max(s.h);
    }
    let width = 2 * m + used_w;
    let height = 2 * m + y + shelf_h;
    let placements = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let dest = placed[i].unwrap_or_default();
            let s = sources[i];
            Placement {
                frame: it.frame,
                animation: it.animation.clone(),
                direction: it.direction.clone(),
                index: it.index,
                duration_ms: it.duration_ms,
                cell: Rect::new(dest.x, dest.y, s.w, s.h),
                source: s,
                dest,
                pivot: Point::new(dest.x + it.pivot.x - s.x, dest.y + it.pivot.y - s.y),
            }
        })
        .collect();
    SheetPlan { width: width.max(1), height: height.max(1), cell_width: 0, cell_height: 0, placements }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize, groups: &[usize]) -> Vec<SheetItem> {
        (0..n)
            .map(|i| SheetItem {
                frame: i as u32,
                animation: Some("walk".into()),
                direction: None,
                index: i,
                duration_ms: 100,
                group: groups.get(i).copied().unwrap_or(0),
                content: Some(Rect::new(4 + i as i32, 2, 8, 12)),
                pivot: Point::new(8, 15),
            })
            .collect()
    }

    #[test]
    fn horizontal_and_grid() {
        let it = items(5, &[]);
        let h = plan(&it, 16, 16, &SheetOptions { layout: SheetLayout::Horizontal, ..Default::default() });
        assert_eq!((h.width, h.height), (80, 16));
        assert_eq!(h.placements[3].cell, Rect::new(48, 0, 16, 16));
        let g =
            plan(&it, 16, 16, &SheetOptions { layout: SheetLayout::Grid, columns: Some(2), spacing: 1, margin: 2, ..Default::default() });
        assert_eq!((g.width, g.height), (2 * 2 + 2 * 16 + 1, 2 * 2 + 3 * 16 + 2));
        assert_eq!(g.placements[3].cell, Rect::new(19, 19, 16, 16));
    }

    #[test]
    fn rows_layout_breaks_on_groups() {
        let it = items(5, &[0, 0, 0, 1, 1]);
        let r = plan(&it, 16, 16, &SheetOptions { layout: SheetLayout::Rows, ..Default::default() });
        assert_eq!((r.width, r.height), (48, 32));
        assert_eq!(r.placements[3].cell.y, 16);
        assert_eq!(r.placements[3].cell.x, 0);
    }

    #[test]
    fn trimmed_pivot_alignment_keeps_pivot_fixed() {
        let it = items(4, &[]);
        let p =
            plan(&it, 16, 16, &SheetOptions { layout: SheetLayout::Horizontal, trim: true, align: CellAlign::Pivot, ..Default::default() });
        let offsets: Vec<(i32, i32)> = p.placements.iter().map(|pl| (pl.pivot.x - pl.cell.x, pl.pivot.y - pl.cell.y)).collect();
        assert!(offsets.windows(2).all(|w| w[0] == w[1]), "pivot fixed inside cells: {offsets:?}");
        for pl in &p.placements {
            assert!(pl.dest.x >= pl.cell.x && pl.dest.x + pl.source.w as i32 <= pl.cell.right());
        }
    }

    #[test]
    fn packed_has_no_overlaps() {
        let it = items(9, &[]);
        let p = plan(&it, 16, 16, &SheetOptions { layout: SheetLayout::Packed, ..Default::default() });
        for (i, a) in p.placements.iter().enumerate() {
            assert!(a.cell.right() <= p.width as i32 && a.cell.bottom() <= p.height as i32);
            for b in &p.placements[i + 1..] {
                assert!(a.cell.intersect(&b.cell).is_empty(), "overlap");
            }
        }
    }
}
