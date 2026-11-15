//! Reader for Aseprite files (`.aseprite`/`.ase`), following the file format Aseprite publishes
//! (`docs/ase-file-specs.md` in its repository). Only the file is read: nothing of the Aseprite
//! program is used.
//!
//! [`parse`] keeps the structure of the file (layers with groups, cels with their positions,
//! linked cels, palette, tags, slices). [`AseFile::flat_layers`] flattens groups,
//! [`AseFile::cel_image`] turns a cel into a sprite-sized RGBA image and [`AseFile::composite`]
//! composes a whole frame.

use std::path::Path;

use sprite_animation::LoopMode;
use sprite_core::compose::{LayerView, compose_onto};
use sprite_core::{BlendMode, Color, Image, Point, Rect};

use crate::ProjectError;
use crate::import::ImportedFrame;

const HEADER_MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
/// Most frames read from one file.
pub const MAX_FRAMES: usize = 4096;
/// Biggest cel side accepted (cels may be bigger than the canvas; they are clipped).
const MAX_CEL_SIDE: u32 = 8192;

/// Whether the bytes start like an Aseprite file (magic number at offset 4).
pub fn is_aseprite(bytes: &[u8]) -> bool {
    bytes.len() >= 128 && bytes[4] == 0xE0 && bytes[5] == 0xA5
}

pub fn has_aseprite_extension(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("aseprite") || e.eq_ignore_ascii_case("ase"))
}

fn err(message: impl std::fmt::Display) -> ProjectError {
    ProjectError::Image(format!("arquivo do Aseprite inválido: {message}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    /// 32 bpp: RGBA.
    Rgba,
    /// 16 bpp: gray value + alpha.
    Grayscale,
    /// 8 bpp: palette index.
    Indexed,
}

impl ColorDepth {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            ColorDepth::Rgba => 4,
            ColorDepth::Grayscale => 2,
            ColorDepth::Indexed => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseLayer {
    pub name: String,
    pub flags: u16,
    /// 0 image, 1 group, 2 tilemap.
    pub kind: u16,
    pub child_level: u16,
    pub blend_mode: u16,
    pub opacity: u8,
}

impl AseLayer {
    pub fn visible(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn background(&self) -> bool {
        self.flags & 8 != 0
    }
    pub fn reference(&self) -> bool {
        self.flags & 64 != 0
    }
    pub fn is_group(&self) -> bool {
        self.kind == 1
    }
    pub fn is_tilemap(&self) -> bool {
        self.kind == 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CelContent {
    /// Pixels as stored (in the depth of the file), row-major, `width`×`height`.
    Pixels { width: u32, height: u32, data: Vec<u8> },
    /// Same content as the cel of this layer in another frame (index).
    Linked(usize),
    /// Tilemap cels are not imported.
    Tilemap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseCel {
    /// Index of the layer in the file.
    pub layer: usize,
    pub x: i32,
    pub y: i32,
    pub opacity: u8,
    pub z_index: i16,
    pub content: CelContent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseFrame {
    pub duration_ms: u32,
    pub cels: Vec<AseCel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseTag {
    pub name: String,
    pub from: usize,
    pub to: usize,
    /// 0 forward, 1 reverse, 2 ping-pong, 3 ping-pong reverse.
    pub direction: u8,
    /// Times to play (0 = forever).
    pub repeat: u16,
}

impl AseTag {
    /// Loop mode for this tag and whether its frames must be listed backwards.
    pub fn loop_mode(&self) -> (LoopMode, bool) {
        match (self.direction, self.repeat) {
            (0, 1) => (LoopMode::Once, false),
            (1, _) => (LoopMode::Reverse, false),
            (2, _) => (LoopMode::PingPong, false),
            (3, _) => (LoopMode::PingPong, true),
            _ => (LoopMode::Loop, false),
        }
    }

    /// Frame indices of the tag, in the order they should be listed.
    pub fn frames(&self) -> Vec<usize> {
        let (_, reversed) = self.loop_mode();
        let list: Vec<usize> = (self.from..=self.to).collect();
        if reversed { list.into_iter().rev().collect() } else { list }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseSliceKey {
    /// First frame this key applies to (until the next key).
    pub frame: usize,
    pub rect: Rect,
    pub center: Option<Rect>,
    pub pivot: Option<Point>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AseSlice {
    pub name: String,
    pub keys: Vec<AseSliceKey>,
}

impl AseSlice {
    /// The key active in `frame`.
    pub fn key_at(&self, frame: usize) -> Option<&AseSliceKey> {
        self.keys.iter().filter(|k| k.frame <= frame).max_by_key(|k| k.frame).or_else(|| self.keys.first())
    }
}

/// An Aseprite file as read.
#[derive(Debug, Clone, PartialEq)]
pub struct AseFile {
    pub width: u32,
    pub height: u32,
    pub depth: ColorDepth,
    /// Palette index drawn as transparent (indexed files, except on the background layer).
    pub transparent_index: u8,
    /// Every layer in file order (bottom to top, each group before its children).
    pub layers: Vec<AseLayer>,
    pub frames: Vec<AseFrame>,
    pub palette: Vec<Color>,
    pub tags: Vec<AseTag>,
    pub slices: Vec<AseSlice>,
    /// What was read but could not be kept.
    pub warnings: Vec<String>,
}

/// A layer of the flattened stack (groups removed, their visibility and opacity applied).
#[derive(Debug, Clone, PartialEq)]
pub struct FlatLayer {
    /// Index of the layer in the file (what cels refer to).
    pub index: usize,
    pub name: String,
    pub visible: bool,
    pub opacity: u8,
    pub blend: BlendMode,
    pub reference: bool,
}

fn blend_mode(code: u16) -> Option<BlendMode> {
    Some(match code {
        0 => BlendMode::Normal,
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        16 => BlendMode::Add,
        17 => BlendMode::Subtract,
        _ => return None,
    })
}

fn blend_name(code: u16) -> String {
    const NAMES: [&str; 19] = [
        "normal",
        "multiplicar",
        "tela",
        "sobrepor",
        "escurecer",
        "clarear",
        "subexposição de cor",
        "superexposição de cor",
        "luz forte",
        "luz suave",
        "diferença",
        "exclusão",
        "matiz",
        "saturação",
        "cor",
        "luminosidade",
        "adição",
        "subtração",
        "divisão",
    ];
    NAMES.get(code as usize).map(|s| s.to_string()).unwrap_or_else(|| format!("#{code}"))
}

fn mul(a: u8, b: u8) -> u8 {
    ((a as u32 * b as u32 + 127) / 255) as u8
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], ProjectError> {
        let end = self.pos.checked_add(n).filter(|e| *e <= self.data.len()).ok_or_else(|| err("dados terminam antes da hora"))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), ProjectError> {
        self.take(n).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, ProjectError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ProjectError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn i16(&mut self) -> Result<i16, ProjectError> {
        Ok(self.u16()? as i16)
    }
    fn u32(&mut self) -> Result<u32, ProjectError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn i32(&mut self) -> Result<i32, ProjectError> {
        Ok(self.u32()? as i32)
    }
    fn string(&mut self) -> Result<String, ProjectError> {
        let n = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).to_string())
    }
    fn rest(&mut self) -> &'a [u8] {
        let s = &self.data[self.pos..];
        self.pos = self.data.len();
        s
    }
}

/// Reads an Aseprite file.
pub fn parse(bytes: &[u8]) -> Result<AseFile, ProjectError> {
    if bytes.len() < 128 {
        return Err(err("menor que o cabeçalho de 128 bytes"));
    }
    let mut h = Reader::new(&bytes[..128]);
    let _file_size = h.u32()?;
    if h.u16()? != HEADER_MAGIC {
        return Err(err("falta a assinatura 0xA5E0 no cabeçalho"));
    }
    let frame_count = h.u16()? as usize;
    let width = h.u16()? as u32;
    let height = h.u16()? as u32;
    let depth = match h.u16()? {
        32 => ColorDepth::Rgba,
        16 => ColorDepth::Grayscale,
        8 => ColorDepth::Indexed,
        d => return Err(err(format!("profundidade de cor {d} desconhecida (esperado 32, 16 ou 8)"))),
    };
    let flags = h.u32()?;
    let speed = h.u16()? as u32;
    h.skip(8)?;
    let transparent_index = h.u8()?;
    Image::check_size(width, height).map_err(|_| err(format!("tamanho {width}x{height} fora do limite (1 a 4096 px por lado)")))?;
    if frame_count == 0 {
        return Err(err("nenhum frame"));
    }
    if frame_count > MAX_FRAMES {
        return Err(err(format!("{frame_count} frames; o máximo é {MAX_FRAMES}")));
    }
    let opacity_valid = flags & 1 != 0;
    let mut file = AseFile {
        width,
        height,
        depth,
        transparent_index,
        layers: Vec::new(),
        frames: Vec::new(),
        palette: Vec::new(),
        tags: Vec::new(),
        slices: Vec::new(),
        warnings: Vec::new(),
    };
    let mut old_palette: Option<Vec<Color>> = None;
    let mut palette_frame: Option<usize> = None;
    let mut palette_changes = false;
    let (mut tilemap_cels, mut z_index) = (false, false);
    let mut pos = 128;
    for fi in 0..frame_count {
        let mut fr = Reader { data: bytes, pos };
        let size = fr.u32().map_err(|_| err(format!("frame {fi} truncado")))? as usize;
        if fr.u16()? != FRAME_MAGIC {
            return Err(err(format!("frame {fi}: falta a assinatura 0xF1FA")));
        }
        let old_chunks = fr.u16()? as u32;
        let duration = fr.u16()? as u32;
        fr.skip(2)?;
        let new_chunks = fr.u32()?;
        let chunks = if new_chunks == 0 { old_chunks } else { new_chunks };
        let end = pos.checked_add(size).filter(|e| *e <= bytes.len() && size >= 16).ok_or_else(|| err(format!("frame {fi} truncado")))?;
        let duration_ms = if duration > 0 {
            duration
        } else if speed > 0 {
            speed
        } else {
            100
        };
        let mut frame = AseFrame { duration_ms, cels: Vec::new() };
        let mut at = pos + 16;
        for _ in 0..chunks {
            if at + 6 > end {
                return Err(err(format!("frame {fi}: chunk truncado")));
            }
            let csize = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;
            let ctype = u16::from_le_bytes([bytes[at + 4], bytes[at + 5]]);
            if csize < 6 || at + csize > end {
                return Err(err(format!("frame {fi}: chunk 0x{ctype:04X} truncado")));
            }
            let body = &bytes[at + 6..at + csize];
            let within = |e: ProjectError| match e {
                ProjectError::Image(m) => ProjectError::Image(format!("{m} (frame {fi}, chunk 0x{ctype:04X})")),
                other => other,
            };
            match ctype {
                0x2004 => file.layers.push(parse_layer(body, opacity_valid).map_err(within)?),
                0x2005 => {
                    if let Some(cel) = parse_cel(body, depth).map_err(within)? {
                        tilemap_cels |= cel.content == CelContent::Tilemap;
                        z_index |= cel.z_index != 0;
                        frame.cels.push(cel);
                    }
                }
                0x2019 => {
                    if palette_frame.is_none_or(|f| f == fi) {
                        parse_palette(body, &mut file.palette).map_err(within)?;
                        palette_frame = Some(fi);
                    } else {
                        palette_changes = true;
                    }
                }
                0x0004 | 0x0011 => {
                    if old_palette.is_none() {
                        let mut p = Vec::new();
                        parse_old_palette(body, ctype == 0x0011, &mut p).map_err(within)?;
                        old_palette = Some(p);
                    }
                }
                0x2018 => parse_tags(body, &mut file.tags).map_err(within)?,
                0x2022 => file.slices.push(parse_slice(body).map_err(within)?),
                _ => {}
            }
            at += csize;
        }
        file.frames.push(frame);
        pos = end;
    }
    if palette_frame.is_none()
        && let Some(p) = old_palette
    {
        file.palette = p;
    }
    if palette_changes {
        file.warnings.push("o arquivo troca a paleta ao longo dos frames: só a primeira paleta foi usada".into());
    }
    if tilemap_cels {
        file.warnings.push("cels de tilemap foram ignorados (tilemaps não são importados)".into());
    }
    if z_index {
        file.warnings.push("o arquivo usa z-index nos cels: a ordem das layers foi mantida".into());
    }
    for (fi, f) in file.frames.iter().enumerate() {
        for c in &f.cels {
            if c.layer >= file.layers.len() {
                return Err(err(format!("frame {fi}: um cel aponta para a layer {}, que não existe", c.layer)));
            }
            if let CelContent::Linked(src) = c.content
                && src >= frame_count
            {
                return Err(err(format!("frame {fi}: um cel vinculado aponta para o frame {src}, que não existe")));
            }
        }
    }
    let n = file.frames.len();
    file.tags.retain_mut(|t| {
        if t.from >= n || t.from > t.to {
            file.warnings.push(format!("a tag \"{}\" ({}..{}) não cabe nos {n} frames e foi ignorada", t.name, t.from, t.to));
            return false;
        }
        t.to = t.to.min(n - 1);
        true
    });
    Ok(file)
}

fn parse_layer(body: &[u8], opacity_valid: bool) -> Result<AseLayer, ProjectError> {
    let mut r = Reader::new(body);
    let flags = r.u16()?;
    let kind = r.u16()?;
    let child_level = r.u16()?;
    r.skip(4)?;
    let blend_mode = r.u16()?;
    let opacity = r.u8()?;
    r.skip(3)?;
    let name = r.string()?;
    Ok(AseLayer { name, flags, kind, child_level, blend_mode, opacity: if opacity_valid { opacity } else { 255 } })
}

fn parse_cel(body: &[u8], depth: ColorDepth) -> Result<Option<AseCel>, ProjectError> {
    let mut r = Reader::new(body);
    let layer = r.u16()? as usize;
    let x = r.i16()? as i32;
    let y = r.i16()? as i32;
    let opacity = r.u8()?;
    let kind = r.u16()?;
    let z_index = r.i16()?;
    r.skip(5)?;
    let content = match kind {
        0 | 2 => {
            let (w, h) = (r.u16()? as u32, r.u16()? as u32);
            if w == 0 || h == 0 {
                return Ok(None);
            }
            if w > MAX_CEL_SIDE || h > MAX_CEL_SIDE {
                return Err(err(format!("cel de {w}x{h} grande demais")));
            }
            let expected = w as usize * h as usize * depth.bytes_per_pixel();
            let data = if kind == 0 {
                r.take(expected).map_err(|_| err("pixels do cel terminam antes da hora"))?.to_vec()
            } else {
                let out = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(r.rest(), expected)
                    .map_err(|e| err(format!("pixels compactados inválidos ({:?})", e.status)))?;
                if out.len() != expected {
                    return Err(err(format!("pixels compactados incompletos ({} de {expected} bytes)", out.len())));
                }
                out
            };
            CelContent::Pixels { width: w, height: h, data }
        }
        1 => CelContent::Linked(r.u16()? as usize),
        3 => CelContent::Tilemap,
        k => return Err(err(format!("tipo de cel {k} desconhecido"))),
    };
    Ok(Some(AseCel { layer, x, y, opacity, z_index, content }))
}

fn parse_palette(body: &[u8], palette: &mut Vec<Color>) -> Result<(), ProjectError> {
    let mut r = Reader::new(body);
    let size = r.u32()? as usize;
    let first = r.u32()? as usize;
    let last = r.u32()? as usize;
    r.skip(8)?;
    if size > 65_536 || first > last || last >= size {
        return Err(err(format!("paleta com índices inválidos ({first}..{last} de {size})")));
    }
    palette.resize(size, Color::BLACK);
    for entry in palette.iter_mut().take(last + 1).skip(first) {
        let flags = r.u16()?;
        let (cr, cg, cb, ca) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
        *entry = Color::rgba(cr, cg, cb, ca);
        if flags & 1 != 0 {
            r.string()?;
        }
    }
    Ok(())
}

fn parse_old_palette(body: &[u8], six_bits: bool, palette: &mut Vec<Color>) -> Result<(), ProjectError> {
    let mut r = Reader::new(body);
    let packets = r.u16()?;
    let mut index = 0usize;
    for _ in 0..packets {
        index += r.u8()? as usize;
        let count = match r.u8()? {
            0 => 256,
            n => n as usize,
        };
        for _ in 0..count {
            let mut ch = [r.u8()?, r.u8()?, r.u8()?];
            if six_bits {
                ch = ch.map(|v| (v << 2) | (v >> 4));
            }
            if index >= 256 {
                return Err(err("paleta antiga com mais de 256 cores"));
            }
            if palette.len() <= index {
                palette.resize(index + 1, Color::BLACK);
            }
            palette[index] = Color::rgb(ch[0], ch[1], ch[2]);
            index += 1;
        }
    }
    Ok(())
}

fn parse_tags(body: &[u8], tags: &mut Vec<AseTag>) -> Result<(), ProjectError> {
    let mut r = Reader::new(body);
    let count = r.u16()?;
    r.skip(8)?;
    for _ in 0..count {
        let from = r.u16()? as usize;
        let to = r.u16()? as usize;
        let direction = r.u8()?;
        let repeat = r.u16()?;
        r.skip(6 + 3 + 1)?;
        let name = r.string()?;
        tags.push(AseTag { name, from, to, direction, repeat });
    }
    Ok(())
}

fn parse_slice(body: &[u8]) -> Result<AseSlice, ProjectError> {
    let mut r = Reader::new(body);
    let count = r.u32()?;
    let flags = r.u32()?;
    r.skip(4)?;
    let name = r.string()?;
    if count > 65_536 {
        return Err(err(format!("slice \"{name}\" com {count} chaves")));
    }
    let mut keys = Vec::new();
    for _ in 0..count {
        let frame = r.u32()? as usize;
        let rect = Rect::new(r.i32()?, r.i32()?, r.u32()?, r.u32()?);
        let center = if flags & 1 != 0 { Some(Rect::new(r.i32()?, r.i32()?, r.u32()?, r.u32()?)) } else { None };
        let pivot = if flags & 2 != 0 { Some(Point::new(r.i32()?, r.i32()?)) } else { None };
        keys.push(AseSliceKey { frame, rect, center, pivot });
    }
    Ok(AseSlice { name, keys })
}

impl AseFile {
    /// The layers that hold pixels, bottom to top: groups are flattened (visibility and opacity
    /// pass to their children), tilemaps are skipped. Also returns what could not be kept.
    pub fn flat_layers(&self) -> (Vec<FlatLayer>, Vec<String>) {
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        // Open groups: (child level, visible, opacity, reference).
        let mut groups: Vec<(u16, bool, u8, bool)> = Vec::new();
        for (index, l) in self.layers.iter().enumerate() {
            while groups.last().is_some_and(|g| g.0 >= l.child_level) {
                groups.pop();
            }
            let (pv, po, pr) = groups.last().map(|g| (g.1, g.2, g.3)).unwrap_or((true, 255, false));
            let (visible, opacity, reference) = (pv && l.visible(), mul(po, l.opacity), pr || l.reference());
            if l.is_group() {
                if l.blend_mode != 0 {
                    warnings.push(format!(
                        "o grupo \"{}\" usa o modo de mistura \"{}\", que não foi aplicado (grupos são achatados)",
                        l.name,
                        blend_name(l.blend_mode)
                    ));
                }
                groups.push((l.child_level, visible, opacity, reference));
                continue;
            }
            if l.is_tilemap() {
                warnings.push(format!("a layer de tilemap \"{}\" foi ignorada (tilemaps não são importados)", l.name));
                continue;
            }
            let blend = blend_mode(l.blend_mode).unwrap_or_else(|| {
                warnings.push(format!(
                    "a layer \"{}\" usa o modo de mistura \"{}\", que não existe aqui: ficou normal",
                    l.name,
                    blend_name(l.blend_mode)
                ));
                BlendMode::Normal
            });
            out.push(FlatLayer { index, name: l.name.clone(), visible, opacity, blend, reference });
        }
        (out, warnings)
    }

    /// The cel of `layer` (file index) in `frame`, if any.
    pub fn cel(&self, frame: usize, layer: usize) -> Option<&AseCel> {
        self.frames.get(frame)?.cels.iter().find(|c| c.layer == layer)
    }

    /// Follows linked cels to the one holding the pixels: (its frame, the cel).
    pub fn source_cel(&self, frame: usize, layer: usize) -> Option<(usize, &AseCel)> {
        let mut f = frame;
        for _ in 0..=self.frames.len() {
            let cel = self.cel(f, layer)?;
            match cel.content {
                CelContent::Linked(src) => f = src,
                _ => return Some((f, cel)),
            }
        }
        None
    }

    fn pixel(&self, data: &[u8], i: usize, background: bool) -> Color {
        match self.depth {
            ColorDepth::Rgba => Color::rgba(data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]),
            ColorDepth::Grayscale => Color::rgba(data[i * 2], data[i * 2], data[i * 2], data[i * 2 + 1]),
            ColorDepth::Indexed => {
                let index = data[i];
                if index == self.transparent_index && !background {
                    Color::TRANSPARENT
                } else {
                    self.palette.get(index as usize).copied().unwrap_or(Color::TRANSPARENT)
                }
            }
        }
    }

    /// The pixels of a cel (not linked) on a transparent canvas the size of the sprite.
    pub fn cel_image(&self, cel: &AseCel) -> Option<Image> {
        let CelContent::Pixels { width, height, data } = &cel.content else { return None };
        let background = self.layers.get(cel.layer).is_some_and(|l| l.background());
        let mut img = Image::new(self.width, self.height);
        let (w, h) = (*width as i32, *height as i32);
        let xs = (-cel.x).max(0)..(self.width as i32 - cel.x).min(w);
        let ys = (-cel.y).max(0)..(self.height as i32 - cel.y).min(h);
        for y in ys {
            for x in xs.clone() {
                let c = self.pixel(data, (y * w + x) as usize, background);
                if !c.is_transparent() {
                    img.set(cel.x + x, cel.y + y, c);
                }
            }
        }
        Some(img)
    }

    /// One frame composed like the editor does: visible, non-reference layers, bottom to top,
    /// with layer × cel opacity and the layer's blend mode.
    pub fn composite(&self, frame: usize, layers: &[FlatLayer]) -> Image {
        let mut out = Image::new(self.width, self.height);
        for l in layers.iter().filter(|l| l.visible && !l.reference) {
            let Some(cel) = self.cel(frame, l.index) else { continue };
            let Some(img) = self.source_cel(frame, l.index).and_then(|(_, src)| self.cel_image(src)) else { continue };
            compose_onto(
                &mut out,
                LayerView { image: &img, opacity: mul(l.opacity, cel.opacity), blend: l.blend, offset: Point::default() },
            );
        }
        out
    }

    /// Every frame composed, with its duration.
    pub fn composites(&self) -> Vec<ImportedFrame> {
        let (layers, _) = self.flat_layers();
        (0..self.frames.len())
            .map(|f| ImportedFrame { image: self.composite(f, &layers), duration_ms: self.frames[f].duration_ms })
            .collect()
    }
}

/// Writes small Aseprite files in memory, chunk by chunk (used by tests in several crates).
#[doc(hidden)]
pub mod builder {
    /// A frame: duration and chunks (type, data).
    type FrameChunks = (u16, Vec<(u16, Vec<u8>)>);

    /// Chunks per frame; `build` adds the headers.
    pub struct Builder {
        width: u16,
        height: u16,
        depth: u16,
        transparent: u8,
        frames: Vec<FrameChunks>,
    }

    fn string(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as u16).to_le_bytes());
        out.extend(s.as_bytes());
    }

    impl Builder {
        pub fn new(width: u16, height: u16, depth: u16, transparent: u8) -> Self {
            Builder { width, height, depth, transparent, frames: Vec::new() }
        }

        /// Starts a new frame; the following chunks go into it.
        pub fn frame(&mut self, duration_ms: u16) -> &mut Self {
            self.frames.push((duration_ms, Vec::new()));
            self
        }

        pub fn chunk(&mut self, kind: u16, data: Vec<u8>) -> &mut Self {
            self.frames.last_mut().expect("frame() first").1.push((kind, data));
            self
        }

        pub fn layer(&mut self, name: &str, flags: u16, kind: u16, child_level: u16, blend: u16, opacity: u8) -> &mut Self {
            let mut d = Vec::new();
            for v in [flags, kind, child_level, 0, 0, blend] {
                d.extend(v.to_le_bytes());
            }
            d.extend([opacity, 0, 0, 0]);
            string(&mut d, name);
            self.chunk(0x2004, d)
        }

        fn cel_header(layer: u16, x: i16, y: i16, opacity: u8, kind: u16) -> Vec<u8> {
            let mut d = Vec::new();
            d.extend(layer.to_le_bytes());
            d.extend(x.to_le_bytes());
            d.extend(y.to_le_bytes());
            d.push(opacity);
            d.extend(kind.to_le_bytes());
            d.extend([0u8; 7]);
            d
        }

        #[allow(clippy::too_many_arguments)]
        pub fn cel(&mut self, layer: u16, x: i16, y: i16, opacity: u8, w: u16, h: u16, pixels: &[u8], compressed: bool) -> &mut Self {
            let mut d = Self::cel_header(layer, x, y, opacity, if compressed { 2 } else { 0 });
            d.extend(w.to_le_bytes());
            d.extend(h.to_le_bytes());
            if compressed {
                d.extend(miniz_oxide::deflate::compress_to_vec_zlib(pixels, 6));
            } else {
                d.extend(pixels);
            }
            self.chunk(0x2005, d)
        }

        pub fn linked_cel(&mut self, layer: u16, x: i16, y: i16, opacity: u8, frame: u16) -> &mut Self {
            let mut d = Self::cel_header(layer, x, y, opacity, 1);
            d.extend(frame.to_le_bytes());
            self.chunk(0x2005, d)
        }

        pub fn palette(&mut self, colors: &[[u8; 4]]) -> &mut Self {
            let mut d = Vec::new();
            d.extend((colors.len() as u32).to_le_bytes());
            d.extend(0u32.to_le_bytes());
            d.extend((colors.len() as u32 - 1).to_le_bytes());
            d.extend([0u8; 8]);
            for c in colors {
                d.extend(0u16.to_le_bytes());
                d.extend(c);
            }
            self.chunk(0x2019, d)
        }

        /// Tags: (from, to, direction, repeat, name).
        pub fn tags(&mut self, tags: &[(u16, u16, u8, u16, &str)]) -> &mut Self {
            let mut d = Vec::new();
            d.extend((tags.len() as u16).to_le_bytes());
            d.extend([0u8; 8]);
            for (from, to, dir, repeat, name) in tags {
                d.extend(from.to_le_bytes());
                d.extend(to.to_le_bytes());
                d.push(*dir);
                d.extend(repeat.to_le_bytes());
                d.extend([0u8; 10]);
                string(&mut d, name);
            }
            self.chunk(0x2018, d)
        }

        /// A slice with keys (frame, x, y, w, h).
        pub fn slice(&mut self, name: &str, keys: &[(u32, i32, i32, u32, u32)]) -> &mut Self {
            let mut d = Vec::new();
            d.extend((keys.len() as u32).to_le_bytes());
            d.extend([0u8; 8]);
            string(&mut d, name);
            for (f, x, y, w, h) in keys {
                d.extend(f.to_le_bytes());
                d.extend(x.to_le_bytes());
                d.extend(y.to_le_bytes());
                d.extend(w.to_le_bytes());
                d.extend(h.to_le_bytes());
            }
            self.chunk(0x2022, d)
        }

        pub fn build(&self) -> Vec<u8> {
            let mut body = Vec::new();
            for (duration, chunks) in &self.frames {
                let size: usize = 16 + chunks.iter().map(|(_, d)| d.len() + 6).sum::<usize>();
                body.extend((size as u32).to_le_bytes());
                body.extend(0xF1FAu16.to_le_bytes());
                body.extend((chunks.len().min(0xFFFF) as u16).to_le_bytes());
                body.extend(duration.to_le_bytes());
                body.extend([0u8; 2]);
                body.extend((chunks.len() as u32).to_le_bytes());
                for (kind, d) in chunks {
                    body.extend(((d.len() + 6) as u32).to_le_bytes());
                    body.extend(kind.to_le_bytes());
                    body.extend(d);
                }
            }
            let mut out = Vec::new();
            out.extend(((128 + body.len()) as u32).to_le_bytes());
            out.extend(0xA5E0u16.to_le_bytes());
            out.extend((self.frames.len() as u16).to_le_bytes());
            out.extend(self.width.to_le_bytes());
            out.extend(self.height.to_le_bytes());
            out.extend(self.depth.to_le_bytes());
            out.extend(1u32.to_le_bytes()); // layer opacity is valid
            out.extend(100u16.to_le_bytes());
            out.extend([0u8; 8]);
            out.push(self.transparent);
            out.extend([0u8; 3]);
            out.extend(0u16.to_le_bytes());
            out.resize(128, 0);
            out.extend(body);
            out
        }
    }

    /// The sample used by the tests: 6x4, 8 bpp (transparent index 0), 2 frames (120 and 80 ms).
    /// Layers, bottom to top: "Fundo" (background, filled with index 0 → `#102030`), group
    /// "Grupo" (hidden) with "Detalhe" (difference blend), "Personagem". Frame 0: "Personagem"
    /// is a zlib cel at (2,1) with red/transparent/transparent/green; frame 1: raw cel at (3,1)
    /// with blue, blue, transparent, red at cel opacity 200; "Fundo" is linked to frame 0.
    /// Ping-pong tag "anda" over both frames; slice "mao" moves from (2,1) to (3,1).
    pub fn sample() -> Vec<u8> {
        let mut b = Builder::new(6, 4, 8, 0);
        b.frame(120)
            .palette(&[[0x10, 0x20, 0x30, 255], [255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]])
            .layer("Fundo", 1 | 8, 0, 0, 0, 255)
            .layer("Grupo", 0, 1, 0, 0, 255)
            .layer("Detalhe", 1, 0, 1, 10, 255)
            .layer("Personagem", 1, 0, 0, 0, 255)
            .cel(0, 0, 0, 255, 6, 4, &[0; 24], false)
            .cel(2, 1, 1, 255, 1, 1, &[4], false)
            .cel(3, 2, 1, 255, 2, 2, &[1, 0, 0, 2], true)
            .tags(&[(0, 1, 2, 0, "anda")])
            .slice("mao", &[(0, 2, 1, 2, 2), (1, 3, 1, 2, 2)]);
        b.frame(80).linked_cel(0, 0, 0, 255, 0).cel(3, 3, 1, 200, 2, 2, &[3, 3, 0, 1], false);
        b.build()
    }
}

#[cfg(test)]
mod tests {
    use super::builder::{Builder, sample};
    use super::*;

    fn c(hex: &str) -> Color {
        Color::parse(hex).unwrap()
    }

    #[test]
    fn reads_the_sample_structure() {
        let bytes = sample();
        assert!(is_aseprite(&bytes));
        let f = parse(&bytes).unwrap();
        assert_eq!((f.width, f.height, f.depth, f.transparent_index), (6, 4, ColorDepth::Indexed, 0));
        assert_eq!(f.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Fundo", "Grupo", "Detalhe", "Personagem"]);
        assert_eq!(f.frames.iter().map(|x| x.duration_ms).collect::<Vec<_>>(), [120, 80]);
        assert_eq!(f.palette.len(), 5);
        assert_eq!(f.tags, vec![AseTag { name: "anda".into(), from: 0, to: 1, direction: 2, repeat: 0 }]);
        assert_eq!(f.tags[0].loop_mode(), (LoopMode::PingPong, false));
        assert_eq!(f.slices[0].key_at(1).unwrap().rect, Rect::new(3, 1, 2, 2));
        assert_eq!(f.slices[0].key_at(0).unwrap().rect, Rect::new(2, 1, 2, 2));
        assert!(matches!(f.cel(1, 0).unwrap().content, CelContent::Linked(0)));
        assert_eq!(f.source_cel(1, 0).unwrap().0, 0);
        assert!(matches!(&f.cel(0, 3).unwrap().content, CelContent::Pixels { width: 2, height: 2, data } if data == &vec![1, 0, 0, 2]));

        let (flat, warnings) = f.flat_layers();
        assert_eq!(
            flat.iter().map(|l| (l.name.as_str(), l.visible, l.index)).collect::<Vec<_>>(),
            [("Fundo", true, 0), ("Detalhe", false, 2), ("Personagem", true, 3)]
        );
        assert!(warnings.iter().any(|w| w.contains("diferença")), "{warnings:?}");
    }

    #[test]
    fn indexed_pixels_transparency_and_background() {
        let f = parse(&sample()).unwrap();
        let (flat, _) = f.flat_layers();
        let bg = c("#102030");
        // Background layer: index 0 is drawn with its palette color.
        let fundo = f.cel_image(f.cel(0, 0).unwrap()).unwrap();
        assert!(fundo.pixels().iter().all(|p| *p == bg));
        // Other layers: index 0 is transparent.
        let top = f.cel_image(f.cel(0, 3).unwrap()).unwrap();
        assert_eq!(top.get(2, 1), Some(c("#ff0000")));
        assert_eq!(top.get(3, 1), Some(Color::TRANSPARENT));
        assert_eq!(top.get(3, 2), Some(c("#00ff00")));
        assert_eq!(top.get(0, 0), Some(Color::TRANSPARENT));
        // Composite: hidden group child stays out, linked background shows in frame 1.
        let f0 = f.composite(0, &flat);
        assert_eq!(f0.get(1, 1), Some(bg), "Detalhe is hidden by its group");
        assert_eq!(f0.get(2, 1), Some(c("#ff0000")));
        assert_eq!(f0.get(3, 1), Some(bg));
        let f1 = f.composite(1, &flat);
        assert_eq!(f1.get(3, 1), Some(c("#0000ff").with_opacity(200).over(bg)));
        assert_eq!(f1.get(2, 1), Some(bg));
        // The generic decoders see the composed frames (import as layer/frames/reference).
        let frames = crate::import::decode_frames(&sample()).unwrap();
        assert_eq!((frames.len(), frames[1].duration_ms), (2, 80));
        assert_eq!(frames[1].image, f1);
        assert_eq!(crate::import::decode_picture(&sample(), 3).unwrap().factor, 2);
    }

    #[test]
    fn rgba_grayscale_and_old_palettes() {
        // 32 bpp, a cel partly outside the canvas.
        let mut b = Builder::new(2, 2, 32, 0);
        b.frame(0).layer("a", 1, 0, 0, 0, 128).cel(0, 1, 1, 255, 2, 1, &[1, 2, 3, 255, 9, 9, 9, 255], true);
        let f = parse(&b.build()).unwrap();
        assert_eq!(f.frames[0].duration_ms, 100, "0 ms uses the header speed");
        let img = f.cel_image(f.cel(0, 0).unwrap()).unwrap();
        assert_eq!(img.get(1, 1), Some(Color::rgb(1, 2, 3)));
        assert_eq!(img.opaque_count(), 1);
        assert_eq!(f.flat_layers().0[0].opacity, 128);
        // 16 bpp: gray + alpha.
        let mut b = Builder::new(1, 1, 16, 0);
        b.frame(50).layer("g", 1, 0, 0, 0, 255).cel(0, 0, 0, 255, 1, 1, &[200, 100], false);
        let f = parse(&b.build()).unwrap();
        assert_eq!(f.cel_image(f.cel(0, 0).unwrap()).unwrap().get(0, 0), Some(Color::rgba(200, 200, 200, 100)));
        // Old palette chunk (0x0004) when there is no new one; 6-bit variant (0x0011).
        let mut b = Builder::new(1, 1, 8, 0);
        b.frame(50).chunk(0x0004, vec![1, 0, 1, 2, 10, 20, 30, 40, 50, 60]);
        assert_eq!(parse(&b.build()).unwrap().palette, vec![Color::rgb(0, 0, 0), Color::rgb(10, 20, 30), Color::rgb(40, 50, 60)]);
        let mut b = Builder::new(1, 1, 8, 0);
        b.frame(50).chunk(0x0011, vec![1, 0, 0, 1, 63, 0, 32]);
        assert_eq!(parse(&b.build()).unwrap().palette, vec![Color::rgb(255, 0, 130)]);
    }

    #[test]
    fn rejects_broken_files() {
        assert!(parse(b"short").is_err());
        let mut bytes = sample();
        bytes[4] = 0;
        assert!(parse(&bytes).unwrap_err().to_string().contains("0xA5E0"));
        let bytes = sample();
        assert!(parse(&bytes[..bytes.len() - 5]).unwrap_err().to_string().contains("truncado"));
        let mut b = Builder::new(2, 2, 8, 0);
        b.frame(10).layer("a", 1, 0, 0, 0, 255).cel(0, 0, 0, 255, 2, 2, &[1, 1, 1, 1], false);
        let mut bytes = b.build();
        let n = bytes.len();
        bytes[n - 1] = 0;
        bytes.extend([0, 0]); // still parses: sizes come from the chunk headers
        assert!(parse(&bytes).is_ok());
        // A cel pointing to a layer that does not exist.
        let mut b = Builder::new(2, 2, 8, 0);
        b.frame(10).cel(3, 0, 0, 255, 1, 1, &[1], false);
        assert!(parse(&b.build()).unwrap_err().to_string().contains("layer 3"));
        // Corrupt zlib data.
        let mut b = Builder::new(2, 2, 8, 0);
        let mut cel = vec![0u8; 16];
        cel[7] = 2;
        cel.extend([2, 0, 2, 0, 1, 2, 3, 4]);
        b.frame(10).layer("a", 1, 0, 0, 0, 255).chunk(0x2005, cel);
        assert!(parse(&b.build()).unwrap_err().to_string().contains("compactados"));
    }
}
