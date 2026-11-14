use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A straight (non-premultiplied) RGBA color. Fully transparent colors are always stored as
/// `#00000000`, so two transparent pixels compare equal whatever their RGB was.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("cor inválida \"{0}\": use #RGB, #RGBA, #RRGGBB, #RRGGBBAA ou \"transparent\"")]
pub struct ColorParseError(pub String);

impl Color {
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0 };
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        if a == 0 { Color::TRANSPARENT } else { Color { r, g, b, a } }
    }

    /// Canonical form: any color with alpha 0 becomes `TRANSPARENT`.
    pub const fn canonical(self) -> Self {
        Color::rgba(self.r, self.g, self.b, self.a)
    }

    pub fn parse(text: &str) -> Result<Self, ColorParseError> {
        let trimmed = text.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "transparent" || lower == "clear" || lower == "none" {
            return Ok(Color::TRANSPARENT);
        }
        let hex = trimmed.strip_prefix('#').ok_or_else(|| ColorParseError(text.to_string()))?;
        let digit = |c: u8| -> Result<u8, ColorParseError> {
            (c as char).to_digit(16).map(|d| d as u8).ok_or_else(|| ColorParseError(text.to_string()))
        };
        let bytes = hex.as_bytes();
        let pair = |i: usize| -> Result<u8, ColorParseError> { Ok(digit(bytes[i])? * 16 + digit(bytes[i + 1])?) };
        let single = |i: usize| -> Result<u8, ColorParseError> { Ok(digit(bytes[i])? * 17) };
        let color = match bytes.len() {
            3 => Color::rgb(single(0)?, single(1)?, single(2)?),
            4 => Color::rgba(single(0)?, single(1)?, single(2)?, single(3)?),
            6 => Color::rgb(pair(0)?, pair(2)?, pair(4)?),
            8 => Color::rgba(pair(0)?, pair(2)?, pair(4)?, pair(6)?),
            _ => return Err(ColorParseError(text.to_string())),
        };
        Ok(color)
    }

    /// `#rrggbb` for opaque colors, `#rrggbbaa` otherwise.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    pub const fn is_transparent(self) -> bool {
        self.a == 0
    }

    pub const fn is_opaque(self) -> bool {
        self.a == 255
    }

    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Multiplies alpha by `opacity / 255`, rounding to nearest.
    pub fn with_opacity(self, opacity: u8) -> Self {
        if opacity == 255 {
            return self;
        }
        let a = (self.a as u32 * opacity as u32 + 127) / 255;
        Color::rgba(self.r, self.g, self.b, a as u8)
    }

    /// Porter-Duff source-over of `self` onto `dst`, straight alpha, integer math (deterministic).
    pub fn over(self, dst: Color) -> Color {
        if self.a == 255 || dst.a == 0 {
            return self.canonical();
        }
        if self.a == 0 {
            return dst;
        }
        let sa = self.a as u32;
        let da = (dst.a as u32 * (255 - sa) + 127) / 255;
        let oa = sa + da;
        let mix = |s: u8, d: u8| ((s as u32 * sa + d as u32 * da + oa / 2) / oa) as u8;
        Color::rgba(mix(self.r, dst.r), mix(self.g, dst.g), mix(self.b, dst.b), oa.min(255) as u8)
    }

    /// Weighted squared distance, close to perceived difference and cheap. Alpha counts too.
    pub fn distance_sq(self, other: Color) -> u32 {
        let dr = self.r as i32 - other.r as i32;
        let dg = self.g as i32 - other.g as i32;
        let db = self.b as i32 - other.b as i32;
        let da = self.a as i32 - other.a as i32;
        (2 * dr * dr + 4 * dg * dg + 3 * db * db + 3 * da * da) as u32
    }

    /// Largest per-channel difference (0..=255), used as fill tolerance.
    pub fn max_channel_delta(self, other: Color) -> u8 {
        let d = |a: u8, b: u8| a.abs_diff(b);
        d(self.r, other.r).max(d(self.g, other.g)).max(d(self.b, other.b)).max(d(self.a, other.a))
    }

    /// Hue (0..360), saturation and value (0..1).
    pub fn to_hsv(self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let hue = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta).rem_euclid(6.0))
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0)
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };
        let saturation = if max == 0.0 { 0.0 } else { delta / max };
        (hue, saturation, max)
    }

    pub fn from_hsv(hue: f64, saturation: f64, value: f64, alpha: u8) -> Self {
        let h = hue.rem_euclid(360.0);
        let s = saturation.clamp(0.0, 1.0);
        let v = value.clamp(0.0, 1.0);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
        Color::rgba(to(r), to(g), to(b), alpha)
    }

    /// Pixel-art style shading: `amount` > 0 lightens, < 0 darkens. With `hue_shift`, light colors
    /// drift toward warm yellow and shadows toward cool blue/purple, as hand-picked ramps do.
    pub fn shade(self, amount: f64, hue_shift: bool) -> Self {
        if self.a == 0 || amount == 0.0 {
            return self;
        }
        let (h, s, v) = self.to_hsv();
        let amount = amount.clamp(-1.0, 1.0);
        let mut hue = h;
        if hue_shift && s > 0.05 {
            let target = if amount > 0.0 { 60.0 } else { 250.0 };
            let mut diff = (target - h).rem_euclid(360.0);
            if diff > 180.0 {
                diff -= 360.0;
            }
            hue = h + diff * amount.abs() * 0.35;
        }
        let (sat, val) = if amount > 0.0 {
            (s * (1.0 - 0.35 * amount), v + (1.0 - v) * amount)
        } else {
            let k = -amount;
            ((s + (1.0 - s) * 0.25 * k).min(1.0), v * (1.0 - k))
        };
        Color::from_hsv(hue, sat, val, self.a)
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl std::str::FromStr for Color {
    type Err = ColorParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Color::parse(s)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Color::parse(&text).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Color {
    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Cor em hexadecimal: #RGB, #RGBA, #RRGGBB ou #RRGGBBAA, ou \"transparent\".",
            "examples": ["#3976d2", "#00000000"]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_hex_forms() {
        assert_eq!(Color::parse("#fff").unwrap(), Color::WHITE);
        assert_eq!(Color::parse("#3976D2").unwrap(), Color::rgb(0x39, 0x76, 0xd2));
        assert_eq!(Color::parse("#11223380").unwrap(), Color::rgba(0x11, 0x22, 0x33, 0x80));
        assert_eq!(Color::parse("#1238").unwrap(), Color::rgba(0x11, 0x22, 0x33, 0x88));
        assert_eq!(Color::parse("transparent").unwrap(), Color::TRANSPARENT);
        assert!(Color::parse("3976d2").is_err());
        assert!(Color::parse("#12345").is_err());
        assert!(Color::parse("#gg0000").is_err());
    }

    #[test]
    fn transparent_is_canonical() {
        assert_eq!(Color::parse("#ff000000").unwrap(), Color::TRANSPARENT);
        assert_eq!(Color::rgba(9, 9, 9, 0), Color::TRANSPARENT);
    }

    #[test]
    fn hex_roundtrip() {
        for text in ["#000000", "#3976d2", "#12345678"] {
            assert_eq!(Color::parse(text).unwrap().to_hex(), text);
        }
    }

    #[test]
    fn source_over() {
        let red = Color::rgb(255, 0, 0);
        let half_blue = Color::rgba(0, 0, 255, 128);
        assert_eq!(red.over(Color::TRANSPARENT), red);
        assert_eq!(Color::TRANSPARENT.over(red), red);
        let mixed = half_blue.over(red);
        assert_eq!(mixed.a, 255);
        assert_eq!(mixed.b, 128);
        assert_eq!(mixed.r, 127);
        let two_halves = half_blue.over(Color::rgba(255, 0, 0, 128));
        assert_eq!(two_halves.a, 192);
    }

    #[test]
    fn shading_moves_value() {
        let base = Color::rgb(57, 118, 210);
        let dark = base.shade(-0.4, true);
        let light = base.shade(0.4, true);
        assert!(dark.to_hsv().2 < base.to_hsv().2);
        assert!(light.to_hsv().2 > base.to_hsv().2);
        assert_eq!(base.shade(0.0, true), base);
    }

    #[test]
    fn hsv_roundtrip() {
        for c in [Color::rgb(57, 118, 210), Color::rgb(200, 30, 90), Color::rgb(10, 10, 10)] {
            let (h, s, v) = c.to_hsv();
            assert_eq!(Color::from_hsv(h, s, v, 255), c);
        }
    }
}
