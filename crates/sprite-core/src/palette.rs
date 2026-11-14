use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::color::Color;

/// An ordered list of colors. Indices are stable: drawing commands may refer to colors by index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<Color>,
}

impl Palette {
    pub fn new(name: impl Into<String>, colors: Vec<Color>) -> Self {
        Palette { name: name.into(), colors }
    }

    pub fn get(&self, index: usize) -> Option<Color> {
        self.colors.get(index).copied()
    }

    pub fn index_of(&self, color: Color) -> Option<usize> {
        self.colors.iter().position(|c| *c == color)
    }

    pub fn contains(&self, color: Color) -> bool {
        color.is_transparent() || self.colors.contains(&color)
    }

    /// Closest color by weighted RGBA distance; ties keep the lowest index (deterministic).
    pub fn nearest(&self, color: Color) -> Option<(usize, Color)> {
        self.colors.iter().enumerate().min_by_key(|(i, c)| (c.distance_sq(color), *i)).map(|(i, c)| (i, *c))
    }

    /// The editor's starting palette: 32 colors in ramps (neutrals, skin, warm, cool, nature),
    /// designed for this project.
    pub fn studio_default() -> Palette {
        let hex = [
            "#0d0b14", "#2b2836", "#4d4a5c", "#807c8f", "#b9b5c4", "#f2f0f5", // neutros
            "#4a2a22", "#7a4632", "#b0704a", "#e0a272", "#f6d2a8", // pele e madeira
            "#5c1426", "#a3243b", "#e0483e", "#f58a3d", "#fbc84a", "#fff1a1", // quentes
            "#163b2c", "#246b3c", "#43a047", "#8bd35a", // verdes
            "#0f2147", "#1d4b8f", "#3976d2", "#6cb2f0", "#b8e6ff", // azuis
            "#2c1b4d", "#5a35a0", "#9a6ee0", "#e08ad8", // roxos e rosa
            "#6b5a2e", "#a89254", // ocre
        ];
        Palette::new("Studio 32", hex.iter().map(|h| Color::parse(h).expect("cor fixa válida")).collect())
    }
}

/// Reduces a weighted color set to at most `max_colors` with median cut. Deterministic: same
/// input, same output, independent of hash order. Transparent colors are ignored.
pub fn median_cut(counts: &BTreeMap<Color, u32>, max_colors: usize) -> Vec<Color> {
    let entries: Vec<(Color, u32)> = counts.iter().filter(|(c, n)| !c.is_transparent() && **n > 0).map(|(c, n)| (*c, *n)).collect();
    if max_colors == 0 || entries.is_empty() {
        return Vec::new();
    }
    if entries.len() <= max_colors {
        return entries.into_iter().map(|(c, _)| c).collect();
    }
    let mut boxes: Vec<Vec<(Color, u32)>> = vec![entries];
    while boxes.len() < max_colors {
        // Split the box with the widest channel range (weighted by population to break ties).
        let pick = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .max_by_key(|(i, b)| (channel_range(b).0, b.iter().map(|e| e.1 as u64).sum::<u64>(), usize::MAX - i))
            .map(|(i, _)| i);
        let Some(index) = pick else { break };
        let mut target = boxes.swap_remove(index);
        let (_, channel) = channel_range(&target);
        target.sort_by_key(|(c, _)| (c.to_array()[channel], *c));
        let total: u64 = target.iter().map(|e| e.1 as u64).sum();
        let mut acc = 0u64;
        let mut split = 1;
        for (i, (_, n)) in target.iter().enumerate() {
            acc += *n as u64;
            if acc * 2 >= total {
                split = (i + 1).clamp(1, target.len() - 1);
                break;
            }
        }
        let right = target.split_off(split);
        boxes.push(target);
        boxes.push(right);
    }
    let mut result: Vec<Color> = boxes.iter().map(|b| weighted_average(b)).collect();
    result.sort_by_key(|c| {
        let (h, s, v) = c.to_hsv();
        ((v * 1000.0) as i64, (h * 10.0) as i64, (s * 1000.0) as i64)
    });
    result.dedup();
    result
}

fn channel_range(entries: &[(Color, u32)]) -> (u8, usize) {
    let mut best = (0u8, 0usize);
    for channel in 0..4 {
        let values = entries.iter().map(|(c, _)| c.to_array()[channel]);
        let (min, max) = values.fold((255u8, 0u8), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let range = max.saturating_sub(min);
        if range > best.0 {
            best = (range, channel);
        }
    }
    best
}

fn weighted_average(entries: &[(Color, u32)]) -> Color {
    let total: u64 = entries.iter().map(|e| e.1 as u64).sum::<u64>().max(1);
    let mut sum = [0u64; 4];
    for (c, n) in entries {
        for (i, v) in c.to_array().iter().enumerate() {
            sum[i] += *v as u64 * *n as u64;
        }
    }
    let avg = |i: usize| ((sum[i] + total / 2) / total) as u8;
    Color::rgba(avg(0), avg(1), avg(2), avg(3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_palette_is_unique() {
        let p = Palette::studio_default();
        assert_eq!(p.colors.len(), 32);
        let mut sorted = p.colors.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 32);
    }

    #[test]
    fn nearest_is_deterministic() {
        let p = Palette::new("t", vec![Color::BLACK, Color::WHITE, Color::rgb(255, 0, 0)]);
        assert_eq!(p.nearest(Color::rgb(200, 10, 10)), Some((2, Color::rgb(255, 0, 0))));
        assert_eq!(p.nearest(Color::rgb(20, 20, 20)), Some((0, Color::BLACK)));
        assert_eq!(p.index_of(Color::WHITE), Some(1));
    }

    #[test]
    fn median_cut_limits_and_repeats() {
        let mut counts = BTreeMap::new();
        for i in 0..64u8 {
            counts.insert(Color::rgb(i * 4, 255 - i * 4, i), 1 + i as u32);
        }
        let a = median_cut(&counts, 8);
        let b = median_cut(&counts, 8);
        assert_eq!(a, b);
        assert!(a.len() <= 8 && a.len() >= 6);
        let few = median_cut(&counts, 100);
        assert_eq!(few.len(), 64);
    }
}
