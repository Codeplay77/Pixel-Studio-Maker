//! Pixel engine core: colors, RGBA images, palettes, masks, layer composition, image diffs,
//! text pixel matrices and PNG I/O.
//!
//! Everything here is deterministic integer math with no I/O besides byte buffers, and nothing
//! depends on Tauri, MCP or the UI. The desktop editor and the MCP server reach these operations
//! through `sprite-commands`.

pub mod codec;
pub mod color;
pub mod compose;
pub mod diff;
pub mod geom;
pub mod image;
pub mod mask;
pub mod matrix;
pub mod palette;
pub mod refs;

pub use color::{Color, ColorParseError};
pub use compose::{BlendMode, LayerView, compose};
pub use geom::{DirtyRect, Point, Rect};
pub use image::{Image, ImageError, PasteMode};
pub use mask::Mask;
pub use palette::Palette;
pub use refs::{FrameRef, ItemRef, SourceRef};
