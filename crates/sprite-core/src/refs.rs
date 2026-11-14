//! References to project items as agents and the UI write them: by numeric id or by name.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A layer, sprite, palette, animation, region or anchor: numeric id or name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ItemRef {
    Id(u32),
    Name(String),
}

impl ItemRef {
    pub fn matches(&self, id: u32, name: &str) -> bool {
        match self {
            ItemRef::Id(i) => *i == id,
            ItemRef::Name(n) => n == name || n.parse::<u32>().is_ok_and(|i| i == id),
        }
    }
}

impl std::fmt::Display for ItemRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemRef::Id(i) => write!(f, "{i}"),
            ItemRef::Name(n) => write!(f, "\"{n}\""),
        }
    }
}

impl From<u32> for ItemRef {
    fn from(v: u32) -> Self {
        ItemRef::Id(v)
    }
}

impl From<&str> for ItemRef {
    fn from(v: &str) -> Self {
        ItemRef::Name(v.to_string())
    }
}

/// A frame: its numeric id, or its position inside an animation track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FrameRef {
    /// Frame id (stable; returned by every frame operation).
    Id(u32),
    /// `index`-th frame (0-based) of an animation, in one direction (omit for non-directional).
    InAnimation {
        animation: ItemRef,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<String>,
        index: usize,
    },
}

impl From<u32> for FrameRef {
    fn from(v: u32) -> Self {
        FrameRef::Id(v)
    }
}

impl std::fmt::Display for FrameRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameRef::Id(i) => write!(f, "{i}"),
            FrameRef::InAnimation { animation, direction: Some(d), index } => write!(f, "{animation}/{d}#{index}"),
            FrameRef::InAnimation { animation, direction: None, index } => write!(f, "{animation}#{index}"),
        }
    }
}

/// Where to read pixels from: a layer (or the composite of all visible layers) of a frame.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceRef {
    /// Layer to read. Omit to use the same layer as the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<ItemRef>,
    /// Frame to read. Omit to use the same frame as the operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<FrameRef>,
    /// Read the composite of all visible layers instead of one layer.
    #[serde(default)]
    pub composite: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_parse_numbers_and_names() {
        let a: ItemRef = serde_json::from_str("3").unwrap();
        let b: ItemRef = serde_json::from_str("\"body\"").unwrap();
        assert!(a.matches(3, "x"));
        assert!(b.matches(9, "body"));
        assert!(ItemRef::Name("9".into()).matches(9, "other"));
        let f: FrameRef = serde_json::from_str(r#"{"animation":"walk","direction":"E","index":2}"#).unwrap();
        assert_eq!(f.to_string(), "\"walk\"/E#2");
        let g: FrameRef = serde_json::from_str("7").unwrap();
        assert_eq!(g, FrameRef::Id(7));
    }
}
