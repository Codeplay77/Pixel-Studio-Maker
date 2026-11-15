//! Sprite model and animation engine: layers and cels per frame, frames with durations,
//! animations with one track per direction (8 standard directions plus custom views), anchors,
//! semantic regions and poses; frame composition, onion skin, sprite sheet layout and
//! programmatic validation.

pub mod analysis;
pub mod compose;
pub mod extras;
pub mod model;
pub mod rpgmaker;
pub mod sheet;

pub use extras::SpriteExtras;
pub use model::*;
