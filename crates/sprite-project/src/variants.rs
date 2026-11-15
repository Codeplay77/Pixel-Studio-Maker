//! Palette variants: alternative colors for a base palette (enemy recolors, team colors, night
//! versions...). Color `i` of the base becomes color `i` of the variant. Variants never touch the
//! pixels of the project: [`Recolor`] maps composed images for previews and exports.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::{Color, Image};

/// A recoloring of a base palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteVariant {
    pub name: String,
    /// Name of the base palette in the project.
    pub base: String,
    /// Same length as the base: color `i` of the base becomes `colors[i]`.
    pub colors: Vec<Color>,
}

/// Project data of the palette tools (`ProjectExtras.paletas`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PaletasExtras {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<PaletteVariant>,
}

impl PaletasExtras {
    pub fn is_empty(&self) -> bool {
        self.variants.is_empty()
    }

    pub fn variant(&self, name: &str) -> Option<&PaletteVariant> {
        self.variants.iter().find(|v| v.name.eq_ignore_ascii_case(name))
    }

    pub fn variant_index(&self, name: &str) -> Option<usize> {
        self.variants.iter().position(|v| v.name.eq_ignore_ascii_case(name))
    }
}

/// What a recoloring did to one image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecolorStats {
    /// Pixels whose color changed.
    pub changed: u64,
    /// Visible colors that are not in the base palette (left as they were), with pixel counts.
    pub unmatched: BTreeMap<Color, u64>,
}

impl RecolorStats {
    pub fn merge(&mut self, other: &RecolorStats) {
        self.changed += other.changed;
        for (c, n) in &other.unmatched {
            *self.unmatched.entry(*c).or_insert(0) += n;
        }
    }
}

/// Exact color swap from a base palette to a variant. A pixel equal to base color `i` becomes
/// variant color `i`. A pixel with the RGB of a base color but another alpha (a translucent
/// layer, for instance) gets the variant's RGB and keeps its alpha (scaled by the variant's
/// alpha relative to the base's). Anything else stays as it is.
#[derive(Debug, Clone, Default)]
pub struct Recolor {
    exact: BTreeMap<Color, Color>,
    rgb: BTreeMap<[u8; 3], (Color, Color)>,
}

impl Recolor {
    /// Pairs `base[i]` with `variant[i]` (up to the shorter list). Transparent base colors are
    /// skipped; for repeated base colors the first index wins.
    pub fn new(base: &[Color], variant: &[Color]) -> Self {
        let mut r = Recolor::default();
        for (b, v) in base.iter().zip(variant) {
            if b.is_transparent() {
                continue;
            }
            r.exact.entry(*b).or_insert(*v);
            r.rgb.entry([b.r, b.g, b.b]).or_insert((*b, *v));
        }
        r
    }

    pub fn map(&self, c: Color) -> Option<Color> {
        if c.is_transparent() {
            return None;
        }
        if let Some(v) = self.exact.get(&c) {
            return Some(*v);
        }
        let (b, v) = self.rgb.get(&[c.r, c.g, c.b])?;
        let a = (c.a as u32 * v.a as u32 + b.a as u32 / 2) / b.a.max(1) as u32;
        Some(Color::rgba(v.r, v.g, v.b, a.min(255) as u8))
    }

    /// The recolored copy of `img` and what changed.
    pub fn apply(&self, img: &Image) -> (Image, RecolorStats) {
        let mut out = img.clone();
        let mut stats = RecolorStats::default();
        for y in 0..img.height() as i32 {
            for x in 0..img.width() as i32 {
                let c = img.get_or_clear(x, y);
                match self.map(c) {
                    Some(n) => {
                        if n != c {
                            out.set(x, y, n);
                            stats.changed += 1;
                        }
                    }
                    None if !c.is_transparent() => *stats.unmatched.entry(c).or_insert(0) += 1,
                    None => {}
                }
            }
        }
        (out, stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(hex: &str) -> Color {
        Color::parse(hex).unwrap()
    }

    #[test]
    fn exact_swap_keeps_alpha_and_ignores_other_colors() {
        let base = [c("#3976d2"), c("#1d4b8f"), c("#ffffff")];
        let variant = [c("#d23939"), c("#8f1d1d"), c("#ffffff")];
        let r = Recolor::new(&base, &variant);
        let mut img = Image::new(4, 1);
        img.set(0, 0, c("#3976d2"));
        img.set(1, 0, c("#1d4b8f80"));
        img.set(2, 0, c("#123456"));
        let (out, stats) = r.apply(&img);
        assert_eq!(out.get(0, 0), Some(c("#d23939")));
        assert_eq!(out.get(1, 0), Some(c("#8f1d1d80")), "alpha preserved");
        assert_eq!(out.get(2, 0), Some(c("#123456")), "not in the base: unchanged");
        assert_eq!(out.get(3, 0), Some(Color::TRANSPARENT));
        assert_eq!(stats.changed, 2);
        assert_eq!(stats.unmatched.get(&c("#123456")), Some(&1));
        // Variant colors with alpha and duplicated base colors.
        let r = Recolor::new(&[c("#ff0000"), c("#ff0000")], &[c("#00ff0080"), c("#0000ff")]);
        assert_eq!(r.map(c("#ff0000")), Some(c("#00ff0080")), "first index wins");
        assert_eq!(r.map(c("#ff000080")), Some(Color::rgba(0, 255, 0, 64)));
    }

    #[test]
    fn extras_serialize_only_when_used() {
        let mut e = PaletasExtras::default();
        assert!(e.is_empty());
        e.variants.push(PaletteVariant { name: "Vermelho".into(), base: "P".into(), colors: vec![c("#ff0000")] });
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["variants"][0]["colors"][0], "#ff0000");
        assert_eq!(serde_json::from_value::<PaletasExtras>(json).unwrap(), e);
        assert_eq!(e.variant_index("vermelho"), Some(0));
    }
}
