//! Text pixel matrices: the compact way agents read and write regions.
//!
//! A matrix is a list of rows; each character is looked up in a legend that maps it to a color,
//! to `null` (keep the existing pixel) or to `"clear"` (erase). Example:
//!
//! ```text
//! legend: { "k": "#1a1c2c", "b": "#3976d2", "w": "#ffffff" }
//! rows:   [ ".kk.", "kbbk", "kbwk", ".kk." ]
//! ```
//!
//! Characters missing from the legend are errors, except `.` and space, which mean "keep" unless
//! the legend says otherwise. Rows may also be run-length encoded (`"2.3k"` = `..kkk`).

use std::collections::BTreeMap;

use crate::color::Color;
use crate::geom::{Point, Rect};
use crate::image::Image;

/// What one matrix cell does to its pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Leave the existing pixel untouched.
    Keep,
    /// Make the pixel transparent.
    Clear,
    /// Paint this color.
    Paint(Color),
}

impl Cell {
    /// Parses a legend value: `null`, `"keep"`, `"clear"`/`"transparent"` or a hex color.
    pub fn from_legend_value(value: Option<&str>) -> Result<Cell, MatrixError> {
        match value {
            None => Ok(Cell::Keep),
            Some(v) => {
                let lower = v.trim().to_ascii_lowercase();
                match lower.as_str() {
                    "keep" | "" => Ok(Cell::Keep),
                    "clear" | "erase" | "transparent" => Ok(Cell::Clear),
                    _ => Color::parse(v)
                        .map(|c| if c.is_transparent() { Cell::Clear } else { Cell::Paint(c) })
                        .map_err(|e| MatrixError::BadLegend(e.to_string())),
                }
            }
        }
    }

    pub fn resolve(self, existing: Color) -> Color {
        match self {
            Cell::Keep => existing,
            Cell::Clear => Color::TRANSPARENT,
            Cell::Paint(c) => c,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatrixError {
    #[error("o caractere '{ch}' (linha {row}) não está na legenda")]
    UnknownChar { ch: char, row: usize },
    #[error("legenda inválida: {0}")]
    BadLegend(String),
    #[error("chave de legenda \"{0}\" precisa ter exatamente um caractere")]
    BadKey(String),
    #[error("RLE inválido na linha {row}: {reason}")]
    BadRle { row: usize, reason: String },
    #[error("a matriz é grande demais ({0} células)")]
    TooLarge(usize),
}

/// A decoded rectangular grid of cells; rows may be ragged (short rows are padded with Keep).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<Cell>,
}

impl Grid {
    pub fn from_rows(rows: Vec<Vec<Cell>>) -> Grid {
        let width = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
        let height = rows.len() as u32;
        let mut cells = Vec::with_capacity(width as usize * height as usize);
        for row in rows {
            let pad = width as usize - row.len();
            cells.extend(row);
            cells.extend(std::iter::repeat_n(Cell::Keep, pad));
        }
        Grid { width, height, cells }
    }

    pub fn get(&self, x: u32, y: u32) -> Cell {
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Applies the grid to `image` with its top-left at `at`. Returns changed pixel positions.
    pub fn apply(&self, image: &mut Image, at: Point) -> Vec<Point> {
        let mut changed = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let cell = self.get(x, y);
                if cell == Cell::Keep {
                    continue;
                }
                let (px, py) = (at.x + x as i32, at.y + y as i32);
                if let Some(existing) = image.get(px, py)
                    && image.set(px, py, cell.resolve(existing))
                {
                    changed.push(Point::new(px, py));
                }
            }
        }
        changed
    }
}

/// Max cells a single matrix may hold (protects the runtime from huge payloads).
pub const MAX_MATRIX_CELLS: usize = 4 * 1024 * 1024;

/// Builds the char→cell table. `.` and space default to Keep.
pub fn parse_legend(legend: &BTreeMap<String, Option<String>>) -> Result<BTreeMap<char, Cell>, MatrixError> {
    let mut table = BTreeMap::new();
    table.insert('.', Cell::Keep);
    table.insert(' ', Cell::Keep);
    for (key, value) in legend {
        let mut chars = key.chars();
        let (Some(ch), None) = (chars.next(), chars.next()) else {
            return Err(MatrixError::BadKey(key.clone()));
        };
        table.insert(ch, Cell::from_legend_value(value.as_deref())?);
    }
    Ok(table)
}

/// Decodes text rows (optionally run-length encoded) into a grid.
pub fn decode_rows(rows: &[String], legend: &BTreeMap<char, Cell>, rle: bool) -> Result<Grid, MatrixError> {
    let mut out = Vec::with_capacity(rows.len());
    let mut total = 0usize;
    for (row_index, row) in rows.iter().enumerate() {
        let chars: Vec<char> = if rle { expand_rle(row, row_index)? } else { row.chars().collect() };
        total += chars.len();
        if total > MAX_MATRIX_CELLS {
            return Err(MatrixError::TooLarge(total));
        }
        let mut cells = Vec::with_capacity(chars.len());
        for ch in chars {
            let cell = legend.get(&ch).copied().ok_or(MatrixError::UnknownChar { ch, row: row_index })?;
            cells.push(cell);
        }
        out.push(cells);
    }
    Ok(Grid::from_rows(out))
}

fn expand_rle(row: &str, row_index: usize) -> Result<Vec<char>, MatrixError> {
    let mut out = Vec::new();
    let mut count = String::new();
    for ch in row.chars() {
        if ch.is_ascii_digit() {
            count.push(ch);
            continue;
        }
        let n: usize = if count.is_empty() {
            1
        } else {
            count.parse().map_err(|_| MatrixError::BadRle { row: row_index, reason: format!("número \"{count}\"") })?
        };
        if n == 0 || out.len() + n > MAX_MATRIX_CELLS {
            return Err(MatrixError::BadRle { row: row_index, reason: format!("contagem {n}") });
        }
        out.extend(std::iter::repeat_n(ch, n));
        count.clear();
    }
    if !count.is_empty() {
        return Err(MatrixError::BadRle { row: row_index, reason: "número sem caractere no fim".into() });
    }
    Ok(out)
}

/// Characters handed out to colors when encoding, in order. Digits are excluded so encoded rows
/// also work as RLE; `.` is reserved for transparent.
const ALPHABET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ#$%&*+-=?@^~<>()[]{}/|!:;,_'\"`";

/// A region encoded as text: legend values are hex colors, `"transparent"` for `.`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct TextMatrix {
    pub rect: Rect,
    pub legend: BTreeMap<String, String>,
    pub rows: Vec<String>,
}

/// Encodes a region of `image`. Colors get chars in order of appearance, unless `preferred`
/// (e.g. a palette) is given: then palette colors keep stable chars across calls.
/// Returns `None` when there are more distinct colors than available characters.
pub fn encode_region(image: &Image, rect: Rect, preferred: &[Color], rle: bool) -> Option<TextMatrix> {
    let alphabet: Vec<char> = ALPHABET.chars().collect();
    let mut table: Vec<(Color, char)> = Vec::new();
    for (i, c) in preferred.iter().enumerate().take(alphabet.len()) {
        if !c.is_transparent() && !table.iter().any(|(t, _)| t == c) {
            table.push((*c, alphabet[i]));
        }
    }
    let mut next = preferred.len().min(alphabet.len());
    let mut used: BTreeMap<String, String> = BTreeMap::new();
    let mut rows = Vec::with_capacity(rect.h as usize);
    for y in rect.y..rect.bottom() {
        let mut row = String::with_capacity(rect.w as usize);
        for x in rect.x..rect.right() {
            let c = image.get_or_clear(x, y);
            if c.is_transparent() {
                row.push('.');
                used.insert(".".into(), "transparent".into());
                continue;
            }
            let ch = match table.iter().find(|(t, _)| *t == c) {
                Some((_, ch)) => *ch,
                None => {
                    // Skip chars already taken by preferred colors.
                    while next < alphabet.len() && table.iter().any(|(_, ch)| *ch == alphabet[next]) {
                        next += 1;
                    }
                    let ch = *alphabet.get(next)?;
                    next += 1;
                    table.push((c, ch));
                    ch
                }
            };
            used.insert(ch.to_string(), c.to_hex());
            row.push(ch);
        }
        rows.push(if rle { rle_encode(&row) } else { row });
    }
    Some(TextMatrix { rect, legend: used, rows })
}

/// Run-length encodes one row; runs of 1 or 2 stay literal (shorter than "2x").
pub fn rle_encode(row: &str) -> String {
    let chars: Vec<char> = row.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        while j < chars.len() && chars[j] == chars[i] {
            j += 1;
        }
        let n = j - i;
        if n >= 3 {
            out.push_str(&n.to_string());
            out.push(chars[i]);
        } else {
            out.extend(std::iter::repeat_n(chars[i], n));
        }
        i = j;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legend(pairs: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.map(|s| s.to_string()))).collect()
    }

    #[test]
    fn decode_and_apply() {
        let table = parse_legend(&legend(&[("k", Some("#000000")), ("w", Some("#ffffff")), ("x", Some("clear"))])).unwrap();
        let rows = vec![".kk.".to_string(), "kwwk".to_string(), "x".to_string()];
        let grid = decode_rows(&rows, &table, false).unwrap();
        assert_eq!((grid.width, grid.height), (4, 3));
        let mut img = Image::filled(6, 6, Color::rgb(9, 9, 9));
        let changed = grid.apply(&mut img, Point::new(1, 1));
        assert_eq!(img.get(1, 1), Some(Color::rgb(9, 9, 9)), "'.' keeps");
        assert_eq!(img.get(2, 1), Some(Color::BLACK));
        assert_eq!(img.get(2, 2), Some(Color::WHITE));
        assert_eq!(img.get(1, 3), Some(Color::TRANSPARENT), "explicit clear");
        assert_eq!(changed.len(), 7);
    }

    #[test]
    fn unknown_char_is_an_error() {
        let table = parse_legend(&legend(&[("k", Some("#000"))])).unwrap();
        let err = decode_rows(&["kz".to_string()], &table, false).unwrap_err();
        assert_eq!(err, MatrixError::UnknownChar { ch: 'z', row: 0 });
        assert!(parse_legend(&legend(&[("kk", Some("#000"))])).is_err());
    }

    #[test]
    fn rle_roundtrip() {
        let row = "....kkkkkbw..";
        let enc = rle_encode(row);
        assert_eq!(enc, "4.5kbw..");
        assert_eq!(expand_rle(&enc, 0).unwrap().iter().collect::<String>(), row);
        assert!(expand_rle("3", 0).is_err());
        assert!(expand_rle("0k", 0).is_err());
    }

    #[test]
    fn encode_then_decode_is_identity() {
        let mut img = Image::new(5, 3);
        img.set(0, 0, Color::rgb(10, 20, 30));
        img.set(4, 2, Color::rgb(200, 100, 0));
        img.set(2, 1, Color::rgba(1, 2, 3, 128));
        for rle in [false, true] {
            let m = encode_region(&img, img.bounds(), &[], rle).unwrap();
            let legend: BTreeMap<String, Option<String>> = m.legend.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect();
            let table = parse_legend(&legend).unwrap();
            let grid = decode_rows(&m.rows, &table, rle).unwrap();
            let mut back = Image::filled(5, 3, Color::WHITE);
            grid.apply(&mut back, Point::new(0, 0));
            assert_eq!(back, img);
        }
    }

    #[test]
    fn preferred_palette_keeps_chars_stable() {
        let pal = [Color::BLACK, Color::WHITE];
        let mut img = Image::new(2, 1);
        img.set(1, 0, Color::WHITE);
        let m = encode_region(&img, img.bounds(), &pal, false).unwrap();
        assert_eq!(m.rows, vec![".b".to_string()]);
    }
}
