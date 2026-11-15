//! Palette files: GIMP (`.gpl`), Lospec hex (`.hex`), JASC-PAL text and RIFF PAL binary (`.pal`),
//! Paint.NET (`.txt`) and palette pictures (`.png`/`.gif`). Reading validates every line and
//! reports the line number of the first problem; writing returns notes about what a format
//! cannot keep (alpha, color count).

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::codec::encode_png;
use sprite_core::{Color, Image};

use crate::import::decode_frames;

/// Most colors a palette may have.
pub const MAX_PALETTE_COLORS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PaletteFileError {
    #[error("linha {line}: {message}")]
    Line { line: usize, message: String },
    #[error("{0}")]
    Invalid(String),
}

impl PaletteFileError {
    fn line(line: usize, message: impl Into<String>) -> Self {
        PaletteFileError::Line { line, message: message.into() }
    }
}

/// Palette file formats, chosen by the file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaletteFormat {
    /// GIMP Palette (`.gpl`).
    Gpl,
    /// Lospec: one `RRGGBB` per line (`.hex`).
    Hex,
    /// JASC-PAL text (written) or RIFF PAL binary (also read) (`.pal`).
    Pal,
    /// Paint.NET: one `AARRGGBB` per line, `;` comments (`.txt`).
    Txt,
    /// Picture with the colors (`.png`).
    Png,
    /// Picture with the colors (`.gif`).
    Gif,
}

impl PaletteFormat {
    pub const EXTENSIONS: [&'static str; 6] = ["gpl", "hex", "pal", "txt", "png", "gif"];

    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "gpl" => PaletteFormat::Gpl,
            "hex" => PaletteFormat::Hex,
            "pal" => PaletteFormat::Pal,
            "txt" => PaletteFormat::Txt,
            "png" => PaletteFormat::Png,
            "gif" => PaletteFormat::Gif,
            _ => return None,
        })
    }

    pub fn from_path(path: &Path) -> Result<Self, PaletteFileError> {
        let ext = path.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        PaletteFormat::from_extension(&ext).ok_or_else(|| {
            PaletteFileError::Invalid(format!(
                "extensão \"{ext}\" não é um formato de paleta; use .{}",
                PaletteFormat::EXTENSIONS.join(", .")
            ))
        })
    }

    pub fn extension(self) -> &'static str {
        match self {
            PaletteFormat::Gpl => "gpl",
            PaletteFormat::Hex => "hex",
            PaletteFormat::Pal => "pal",
            PaletteFormat::Txt => "txt",
            PaletteFormat::Png => "png",
            PaletteFormat::Gif => "gif",
        }
    }

    pub fn is_image(self) -> bool {
        matches!(self, PaletteFormat::Png | PaletteFormat::Gif)
    }
}

/// A palette read from a file.
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteFile {
    /// Name stored in the file (GIMP `Name:`, Paint.NET `; Palette Name:`), if any.
    pub name: Option<String>,
    pub colors: Vec<Color>,
    /// Columns stored in the file (GIMP `Columns:`), if any.
    pub columns: Option<u32>,
    /// What the reader noticed but accepted.
    pub warnings: Vec<String>,
}

/// How palette pictures are drawn: one `swatch`×`swatch` square per color, `columns` per row
/// (all in one row by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwatchLayout {
    pub swatch: u32,
    pub columns: Option<u32>,
}

impl Default for SwatchLayout {
    fn default() -> Self {
        SwatchLayout { swatch: 8, columns: None }
    }
}

/// Reads a palette in `format` (RIFF PAL is recognized by its signature inside `.pal`).
pub fn read_palette(format: PaletteFormat, bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let file = match format {
        PaletteFormat::Gpl => read_gpl(bytes)?,
        PaletteFormat::Hex => read_hex(bytes)?,
        PaletteFormat::Pal if bytes.starts_with(b"RIFF") => read_riff_pal(bytes)?,
        PaletteFormat::Pal => read_jasc(bytes)?,
        PaletteFormat::Txt => read_paint_net(bytes)?,
        PaletteFormat::Png | PaletteFormat::Gif => read_picture(bytes)?,
    };
    if file.colors.is_empty() {
        return Err(PaletteFileError::Invalid("o arquivo não tem nenhuma cor".into()));
    }
    Ok(file)
}

/// Writes `colors` in `format`. Returns the bytes and notes about what the format dropped.
pub fn write_palette(
    format: PaletteFormat,
    name: &str,
    colors: &[Color],
    layout: SwatchLayout,
) -> Result<(Vec<u8>, Vec<String>), PaletteFileError> {
    if colors.is_empty() {
        return Err(PaletteFileError::Invalid("a paleta não tem cores".into()));
    }
    if colors.len() > MAX_PALETTE_COLORS {
        return Err(PaletteFileError::Invalid(format!("a paleta tem {} cores; o máximo é {MAX_PALETTE_COLORS}", colors.len())));
    }
    let translucent = colors.iter().any(|c| !c.is_opaque());
    let mut notes = Vec::new();
    let bytes = match format {
        PaletteFormat::Gpl => {
            let mut out = format!("GIMP Palette\nName: {}\nColumns: {}\n", one_line(name), layout.columns.unwrap_or(16).min(256));
            if translucent {
                out.push_str("Channels: RGBA\n");
                notes.push(
                    "a paleta tem cores com transparência: gravada com \"Channels: RGBA\" (o GIMP pode ignorar a transparência)".into(),
                );
            }
            out.push_str("#\n");
            for c in colors {
                if translucent {
                    out.push_str(&format!("{:3} {:3} {:3} {:3}\t{}\n", c.r, c.g, c.b, c.a, c.to_hex()));
                } else {
                    out.push_str(&format!("{:3} {:3} {:3}\t{}\n", c.r, c.g, c.b, c.to_hex()));
                }
            }
            out.into_bytes()
        }
        PaletteFormat::Hex => {
            if translucent {
                notes.push("cores com transparência gravadas como RRGGBBAA (o formato Lospec só tem RRGGBB)".into());
            }
            colors.iter().map(|c| format!("{}\n", &c.to_hex()[1..])).collect::<String>().into_bytes()
        }
        PaletteFormat::Pal => {
            if translucent {
                notes.push("o formato JASC-PAL não guarda transparência: só o RGB foi gravado".into());
            }
            let mut out = format!("JASC-PAL\r\n0100\r\n{}\r\n", colors.len());
            for c in colors {
                out.push_str(&format!("{} {} {}\r\n", c.r, c.g, c.b));
            }
            out.into_bytes()
        }
        PaletteFormat::Txt => {
            if colors.len() > 96 {
                notes.push(format!("o Paint.NET lê só as 96 primeiras cores (a paleta tem {})", colors.len()));
            }
            let mut out = String::from(
                "; paint.net Palette File\r\n; Lines that start with a semicolon are comments\r\n; Colors are written as 8-digit hexadecimal numbers: aarrggbb\r\n",
            );
            out.push_str(&format!("; Palette Name: {}\r\n; Colors: {}\r\n", one_line(name), colors.len()));
            for c in colors {
                out.push_str(&format!("{:02X}{:02X}{:02X}{:02X}\r\n", c.a, c.r, c.g, c.b));
            }
            out.into_bytes()
        }
        PaletteFormat::Png | PaletteFormat::Gif => {
            let img = swatch_image(colors, layout)?;
            if format == PaletteFormat::Png {
                encode_png(&img).map_err(|e| PaletteFileError::Invalid(e.to_string()))?
            } else {
                if translucent {
                    notes.push("o GIF não guarda transparência parcial: cores com alfa abaixo de 128 viram transparentes".into());
                }
                crate::export::encode_gif(&[(img, 100)], 1, false).map_err(|e| PaletteFileError::Invalid(e.to_string()))?
            }
        }
    };
    Ok((bytes, notes))
}

/// The palette as a picture: one `swatch` px square per color, `columns` per row.
pub fn swatch_image(colors: &[Color], layout: SwatchLayout) -> Result<Image, PaletteFileError> {
    let n = colors.len().max(1) as u32;
    let swatch = layout.swatch.clamp(1, 256);
    let columns = layout.columns.unwrap_or(n).clamp(1, n);
    let rows = n.div_ceil(columns);
    let (w, h) = (columns * swatch, rows * swatch);
    Image::check_size(w, h).map_err(|_| {
        PaletteFileError::Invalid(format!("imagem da paleta de {w}x{h} grande demais (máximo 4096 px por lado): use menos px por amostra"))
    })?;
    let mut img = Image::new(w, h);
    for (i, c) in colors.iter().enumerate() {
        let (cx, cy) = ((i as u32 % columns) * swatch, (i as u32 / columns) * swatch);
        for y in cy..cy + swatch {
            for x in cx..cx + swatch {
                img.set(x as i32, y as i32, *c);
            }
        }
    }
    Ok(img)
}

fn one_line(text: &str) -> String {
    text.replace(['\r', '\n'], " ")
}

/// Text lines with their 1-based numbers (UTF-8, or Latin-1 when not UTF-8; BOM removed).
fn text_lines(bytes: &[u8]) -> Vec<(usize, String)> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t.to_string(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    };
    text.lines().enumerate().map(|(i, l)| (i + 1, l.trim().to_string())).collect()
}

fn push_color(colors: &mut Vec<Color>, color: Color, line: usize) -> Result<(), PaletteFileError> {
    if colors.len() >= MAX_PALETTE_COLORS {
        return Err(PaletteFileError::line(line, format!("mais de {MAX_PALETTE_COLORS} cores (o máximo de uma paleta)")));
    }
    colors.push(color);
    Ok(())
}

fn strip_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let head = line.get(..key.len())?;
    head.eq_ignore_ascii_case(key).then(|| line[key.len()..].trim())
}

/// `count` channel values (0..=255) at the start of a line; the rest (a color name) is ignored.
fn channels(line: &str, count: usize) -> Option<Vec<u8>> {
    let values: Vec<u8> = line.split_whitespace().take(count).map(|t| t.parse::<u8>().ok()).collect::<Option<_>>()?;
    (values.len() == count).then_some(values)
}

fn read_gpl(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let mut file = PaletteFile { name: None, colors: Vec::new(), columns: None, warnings: Vec::new() };
    let mut header = false;
    let mut count = 3;
    for (n, line) in text_lines(bytes) {
        if !header {
            if line.is_empty() {
                continue;
            }
            if !line.eq_ignore_ascii_case("GIMP Palette") {
                return Err(PaletteFileError::line(n, "um arquivo .gpl começa com \"GIMP Palette\""));
            }
            header = true;
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(v) = strip_key(&line, "Name:") {
            file.name = (!v.is_empty()).then(|| v.to_string());
            continue;
        }
        if let Some(v) = strip_key(&line, "Columns:") {
            file.columns =
                Some(v.parse().map_err(|_| PaletteFileError::line(n, format!("\"Columns:\" precisa de um número, veio \"{v}\"")))?);
            continue;
        }
        if let Some(v) = strip_key(&line, "Channels:") {
            count = match v.to_ascii_uppercase().as_str() {
                "RGB" => 3,
                "RGBA" => 4,
                _ => return Err(PaletteFileError::line(n, format!("\"Channels:\" deve ser RGB ou RGBA, veio \"{v}\""))),
            };
            continue;
        }
        let Some(c) = channels(&line, count) else {
            let what = if count == 4 { "R G B A" } else { "R G B" };
            return Err(PaletteFileError::line(n, format!("\"{line}\" não é uma cor (use {what}, cada um de 0 a 255)")));
        };
        let color = if count == 4 { Color::rgba(c[0], c[1], c[2], c[3]) } else { Color::rgb(c[0], c[1], c[2]) };
        push_color(&mut file.colors, color, n)?;
    }
    if !header {
        return Err(PaletteFileError::Invalid("arquivo .gpl vazio".into()));
    }
    Ok(file)
}

/// `RRGGBB` or `RRGGBBAA`, with or without `#`.
fn hex_color(text: &str) -> Option<Color> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !(digits.len() == 6 || digits.len() == 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Color::parse(&format!("#{digits}")).ok()
}

fn read_hex(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let mut colors = Vec::new();
    for (n, line) in text_lines(bytes) {
        if line.is_empty() || line.starts_with(';') || line.starts_with("//") {
            continue;
        }
        let color = hex_color(&line).ok_or_else(|| PaletteFileError::line(n, format!("\"{line}\" não é uma cor RRGGBB")))?;
        push_color(&mut colors, color, n)?;
    }
    Ok(PaletteFile { name: None, colors, columns: None, warnings: Vec::new() })
}

fn read_jasc(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let lines: Vec<(usize, String)> = text_lines(bytes).into_iter().filter(|(_, l)| !l.is_empty()).collect();
    let mut it = lines.into_iter();
    match it.next() {
        Some((_, l)) if l.eq_ignore_ascii_case("JASC-PAL") => {}
        Some((n, _)) => return Err(PaletteFileError::line(n, "um arquivo .pal de texto começa com \"JASC-PAL\"")),
        None => return Err(PaletteFileError::Invalid("arquivo .pal vazio".into())),
    }
    let (n, version) = it.next().ok_or_else(|| PaletteFileError::Invalid("falta a versão (0100) depois de JASC-PAL".into()))?;
    if !version.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PaletteFileError::line(n, format!("versão inválida \"{version}\" (esperado 0100)")));
    }
    let (n, count) = it.next().ok_or_else(|| PaletteFileError::Invalid("falta a quantidade de cores".into()))?;
    let count: usize = count.parse().map_err(|_| PaletteFileError::line(n, format!("quantidade de cores inválida \"{count}\"")))?;
    if count == 0 || count > MAX_PALETTE_COLORS {
        return Err(PaletteFileError::line(n, format!("quantidade de cores {count} fora de 1 a {MAX_PALETTE_COLORS}")));
    }
    let mut colors = Vec::new();
    for (n, line) in it {
        if colors.len() == count {
            return Err(PaletteFileError::line(n, format!("cor a mais: o arquivo declara {count} cores")));
        }
        let parts = line.split_whitespace().count();
        let c = channels(&line, parts.clamp(3, 4))
            .filter(|_| parts == 3 || parts == 4)
            .ok_or_else(|| PaletteFileError::line(n, format!("\"{line}\" não é uma cor (use R G B, cada um de 0 a 255)")))?;
        colors.push(if c.len() == 4 { Color::rgba(c[0], c[1], c[2], c[3]) } else { Color::rgb(c[0], c[1], c[2]) });
    }
    if colors.len() < count {
        return Err(PaletteFileError::Invalid(format!("o arquivo declara {count} cores, mas tem só {}", colors.len())));
    }
    Ok(PaletteFile { name: None, colors, columns: None, warnings: Vec::new() })
}

fn read_riff_pal(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let bad = |m: &str| PaletteFileError::Invalid(format!("RIFF PAL inválido: {m}"));
    if bytes.len() < 12 || &bytes[8..12] != b"PAL " {
        return Err(bad("falta a assinatura \"PAL \""));
    }
    let le32 = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = le32(at + 4);
        let body = at + 8;
        if &bytes[at..at + 4] == b"data" {
            if body + 4 > bytes.len() {
                return Err(bad("bloco data truncado"));
            }
            let count = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]) as usize;
            if count == 0 || count > MAX_PALETTE_COLORS {
                return Err(bad(&format!("quantidade de cores {count} fora de 1 a {MAX_PALETTE_COLORS}")));
            }
            let entries = body + 4;
            if entries + count * 4 > bytes.len() {
                return Err(bad(&format!("o arquivo declara {count} cores, mas termina antes")));
            }
            let colors = (0..count).map(|i| entries + i * 4).map(|p| Color::rgb(bytes[p], bytes[p + 1], bytes[p + 2])).collect();
            return Ok(PaletteFile { name: None, colors, columns: None, warnings: Vec::new() });
        }
        at = body + size + (size & 1);
    }
    Err(bad("falta o bloco \"data\""))
}

fn read_paint_net(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let mut file = PaletteFile { name: None, colors: Vec::new(), columns: None, warnings: Vec::new() };
    for (n, line) in text_lines(bytes) {
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix(';') {
            if let Some(v) = strip_key(comment.trim(), "Palette Name:") {
                file.name = (!v.is_empty()).then(|| v.to_string());
            }
            continue;
        }
        let digits = line.strip_prefix('#').unwrap_or(&line);
        let color = match (digits.len(), digits.bytes().all(|b| b.is_ascii_hexdigit())) {
            (8, true) => {
                let v = u32::from_str_radix(digits, 16).expect("8 hex digits");
                Color::rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, (v >> 24) as u8)
            }
            (6, true) => hex_color(digits).expect("6 hex digits"),
            _ => return Err(PaletteFileError::line(n, format!("\"{line}\" não é uma cor AARRGGBB"))),
        };
        push_color(&mut file.colors, color, n)?;
    }
    Ok(file)
}

/// Unique visible colors of a picture in reading order (rows, then frames).
fn read_picture(bytes: &[u8]) -> Result<PaletteFile, PaletteFileError> {
    let frames = decode_frames(bytes).map_err(|e| PaletteFileError::Invalid(e.to_string()))?;
    let mut colors: Vec<Color> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for f in &frames {
        for c in f.image.pixels() {
            if c.is_transparent() || !seen.insert(*c) {
                continue;
            }
            if colors.len() >= MAX_PALETTE_COLORS {
                return Err(PaletteFileError::Invalid(format!(
                    "a imagem tem mais de {MAX_PALETTE_COLORS} cores: reduza as cores antes (por exemplo com palette_quantize)"
                )));
            }
            colors.push(*c);
        }
    }
    Ok(PaletteFile { name: None, colors, columns: None, warnings: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(hex: &str) -> Color {
        Color::parse(hex).unwrap()
    }

    #[test]
    fn gpl_reads_comments_names_and_writes_back() {
        let text = "GIMP Palette\nName: Teste\nColumns: 4\n# um comentário\n\n  0   0   0\tPreto\n255 128   7 Laranja forte\n#\n";
        let p = read_palette(PaletteFormat::Gpl, text.as_bytes()).unwrap();
        assert_eq!(p.name.as_deref(), Some("Teste"));
        assert_eq!(p.columns, Some(4));
        assert_eq!(p.colors, vec![c("#000000"), c("#ff8007")]);
        let (bytes, notes) = write_palette(PaletteFormat::Gpl, "Teste", &p.colors, SwatchLayout::default()).unwrap();
        assert!(notes.is_empty());
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.starts_with("GIMP Palette\nName: Teste\n"), "{text}");
        assert_eq!(read_palette(PaletteFormat::Gpl, &bytes).unwrap().colors, p.colors);
        // RGBA (Aseprite's extension) round-trips the alpha.
        let alpha = vec![c("#ff000080"), c("#00ff00")];
        let (bytes, notes) = write_palette(PaletteFormat::Gpl, "a", &alpha, SwatchLayout::default()).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(read_palette(PaletteFormat::Gpl, &bytes).unwrap().colors, alpha);
    }

    #[test]
    fn gpl_errors_carry_the_line_number() {
        let err = read_palette(PaletteFormat::Gpl, b"GIMP Palette\nName: x\n0 0 0\n12 300 4 ruim\n").unwrap_err();
        assert_eq!(
            err,
            PaletteFileError::Line { line: 4, message: "\"12 300 4 ruim\" não é uma cor (use R G B, cada um de 0 a 255)".into() }
        );
        assert!(err.to_string().starts_with("linha 4:"));
        let err = read_palette(PaletteFormat::Gpl, b"\nPaleta\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 2, .. }), "{err}");
        let err = read_palette(PaletteFormat::Gpl, b"GIMP Palette\nColumns: muitas\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 2, .. }));
        // Too many colors: the 257th line is reported.
        let mut big = String::from("GIMP Palette\n");
        for i in 0..257 {
            big.push_str(&format!("{} 0 0\n", i % 256));
        }
        assert!(matches!(read_palette(PaletteFormat::Gpl, big.as_bytes()).unwrap_err(), PaletteFileError::Line { line: 258, .. }));
        assert!(read_palette(PaletteFormat::Gpl, b"GIMP Palette\n# nada\n").is_err(), "sem cores");
    }

    #[test]
    fn hex_lospec_roundtrip_and_errors() {
        let p = read_palette(PaletteFormat::Hex, b"\xEF\xBB\xBF1a1c2c\r\n#5D275D\r\n\r\nb13e53\r\n").unwrap();
        assert_eq!(p.colors, vec![c("#1a1c2c"), c("#5d275d"), c("#b13e53")]);
        let (bytes, _) = write_palette(PaletteFormat::Hex, "x", &p.colors, SwatchLayout::default()).unwrap();
        assert_eq!(String::from_utf8(bytes.clone()).unwrap(), "1a1c2c\n5d275d\nb13e53\n");
        assert_eq!(read_palette(PaletteFormat::Hex, &bytes).unwrap().colors, p.colors);
        let err = read_palette(PaletteFormat::Hex, b"1a1c2c\nzzzzzz\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 2, .. }), "{err}");
    }

    #[test]
    fn jasc_and_riff_pal() {
        let text = "JASC-PAL\r\n0100\r\n3\r\n0 0 0\r\n255 255 255\r\n57 118 210\r\n";
        let p = read_palette(PaletteFormat::Pal, text.as_bytes()).unwrap();
        assert_eq!(p.colors, vec![c("#000000"), c("#ffffff"), c("#3976d2")]);
        let (bytes, _) = write_palette(PaletteFormat::Pal, "x", &p.colors, SwatchLayout::default()).unwrap();
        assert_eq!(String::from_utf8(bytes.clone()).unwrap(), text);
        assert_eq!(read_palette(PaletteFormat::Pal, &bytes).unwrap().colors, p.colors);
        // Errors: bad color line, count mismatch, extra color.
        let err = read_palette(PaletteFormat::Pal, b"JASC-PAL\n0100\n2\n0 0 0\n1 2\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 5, .. }), "{err}");
        assert!(read_palette(PaletteFormat::Pal, b"JASC-PAL\n0100\n3\n0 0 0\n").unwrap_err().to_string().contains("só 1"));
        let err = read_palette(PaletteFormat::Pal, b"JASC-PAL\n0100\n1\n0 0 0\n9 9 9\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 5, .. }), "{err}");
        // RIFF PAL (binary), with an extra chunk before "data".
        let mut riff = b"RIFF\0\0\0\0PAL ".to_vec();
        riff.extend(b"note");
        riff.extend(3u32.to_le_bytes());
        riff.extend(b"abc\0");
        riff.extend(b"data");
        riff.extend(12u32.to_le_bytes());
        riff.extend(0x0300u16.to_le_bytes());
        riff.extend(2u16.to_le_bytes());
        riff.extend([255, 0, 0, 0, 10, 20, 30, 0]);
        let size = (riff.len() - 8) as u32;
        riff[4..8].copy_from_slice(&size.to_le_bytes());
        let p = read_palette(PaletteFormat::Pal, &riff).unwrap();
        assert_eq!(p.colors, vec![c("#ff0000"), c("#0a141e")]);
        riff.truncate(riff.len() - 3);
        assert!(read_palette(PaletteFormat::Pal, &riff).unwrap_err().to_string().contains("termina antes"));
    }

    #[test]
    fn paint_net_txt() {
        let text = "; paint.net Palette File\n; Palette Name: Mar\nFF000000\n80FF0000\n;fim\nFF3976D2\n";
        let p = read_palette(PaletteFormat::Txt, text.as_bytes()).unwrap();
        assert_eq!(p.name.as_deref(), Some("Mar"));
        assert_eq!(p.colors, vec![c("#000000"), c("#ff000080"), c("#3976d2")]);
        let (bytes, _) = write_palette(PaletteFormat::Txt, "Mar", &p.colors, SwatchLayout::default()).unwrap();
        let written = String::from_utf8(bytes.clone()).unwrap();
        assert!(written.contains("80FF0000\r\n") && written.contains("; Palette Name: Mar"), "{written}");
        let back = read_palette(PaletteFormat::Txt, &bytes).unwrap();
        assert_eq!((back.colors, back.name), (p.colors, Some("Mar".into())));
        let err = read_palette(PaletteFormat::Txt, b"; x\nFF000000\nFF00\n").unwrap_err();
        assert!(matches!(err, PaletteFileError::Line { line: 3, .. }), "{err}");
    }

    #[test]
    fn palette_pictures() {
        let colors = vec![c("#ff0000"), c("#00ff00"), c("#0000ff"), c("#ffffff"), c("#000000")];
        let (png, _) = write_palette(PaletteFormat::Png, "x", &colors, SwatchLayout { swatch: 4, columns: Some(2) }).unwrap();
        let img = sprite_core::codec::decode_png(&png).unwrap();
        assert_eq!((img.width(), img.height()), (8, 12));
        assert_eq!(img.get(5, 1), Some(c("#00ff00")));
        assert_eq!(img.get(7, 11), Some(Color::TRANSPARENT), "empty cell after the last color");
        // Reading keeps reading order and skips transparency.
        assert_eq!(read_palette(PaletteFormat::Png, &png).unwrap().colors, colors);
        let (gif, _) = write_palette(PaletteFormat::Gif, "x", &colors, SwatchLayout { swatch: 1, columns: None }).unwrap();
        assert_eq!(read_palette(PaletteFormat::Gif, &gif).unwrap().colors, colors);
        // More than 256 colors is refused.
        let mut many = Image::new(20, 20);
        for i in 0..400 {
            many.set(i % 20, i / 20, Color::rgb(i as u8, (i / 256) as u8, 7));
        }
        let png = encode_png(&many).unwrap();
        assert!(read_palette(PaletteFormat::Png, &png).unwrap_err().to_string().contains("mais de 256"));
        assert!(swatch_image(&[Color::BLACK; 20], SwatchLayout { swatch: 256, columns: None }).is_err(), "too wide");
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(PaletteFormat::from_path(Path::new("a/b.GPL")).unwrap(), PaletteFormat::Gpl);
        assert_eq!(PaletteFormat::from_path(Path::new("x.hex")).unwrap(), PaletteFormat::Hex);
        assert!(PaletteFormat::from_path(Path::new("x.aco")).unwrap_err().to_string().contains(".gpl"));
        assert!(write_palette(PaletteFormat::Hex, "x", &[], SwatchLayout::default()).is_err());
    }
}
