//! Reading images from outside: PNG (one frame), GIF (animated), Aseprite (frames composed; the
//! layered import is in `aseprite`) and sprite sheet slicing.

use std::io::Cursor;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_core::codec::{Fitted, MAX_PICTURE_PIXELS, decode_png, decode_png_fit, reduce_rgba, reduction_factor};
use sprite_core::{Color, Image, PasteMode, Point, Rect};

use crate::ProjectError;

/// One decoded frame and how long it shows.
pub struct ImportedFrame {
    pub image: Image,
    pub duration_ms: u32,
}

/// Decodes PNG, GIF or Aseprite bytes (detected by signature). Aseprite files come composed,
/// one image per frame.
pub fn decode_frames(bytes: &[u8]) -> Result<Vec<ImportedFrame>, ProjectError> {
    if bytes.starts_with(b"\x89PNG") {
        return Ok(vec![ImportedFrame { image: decode_png(bytes)?, duration_ms: 100 }]);
    }
    if bytes.starts_with(b"GIF8") {
        return decode_gif(bytes);
    }
    if crate::aseprite::is_aseprite(bytes) {
        return Ok(crate::aseprite::parse(bytes)?.composites());
    }
    Err(ProjectError::Image("formato não suportado: use PNG, GIF ou Aseprite (.aseprite/.ase)".into()))
}

/// Reads a picture to be converted (PNG, JPG or the first frame of a GIF or Aseprite file) of
/// any size, reduced while reading to at most `max_side` pixels per side.
pub fn decode_picture(bytes: &[u8], max_side: u32) -> Result<Fitted, ProjectError> {
    if bytes.starts_with(b"\x89PNG") {
        return Ok(decode_png_fit(bytes, max_side)?);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return decode_jpeg(bytes, max_side);
    }
    if bytes.starts_with(b"GIF8") || crate::aseprite::is_aseprite(bytes) {
        let first = decode_frames(bytes)?.into_iter().next().ok_or_else(|| ProjectError::Image("arquivo sem frames".into()))?;
        let (w, h) = (first.image.width(), first.image.height());
        let factor = reduction_factor(w, h, max_side);
        let (rw, rh, rgba) = reduce_rgba(w, h, &first.image.to_rgba(), factor);
        return Ok(Fitted { width: rw, height: rh, rgba, factor, original: (w, h) });
    }
    Err(ProjectError::Image("formato não suportado: use PNG, JPG, GIF ou Aseprite".into()))
}

fn decode_jpeg(bytes: &[u8], max_side: u32) -> Result<Fitted, ProjectError> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let err = |e: zune_jpeg::errors::DecodeErrors| ProjectError::Image(format!("JPG inválido: {e:?}"));
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA).set_max_width(1 << 16).set_max_height(1 << 16);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder.decode_headers().map_err(err)?;
    let info = decoder.info().ok_or_else(|| ProjectError::Image("JPG sem cabeçalho".into()))?;
    let (w, h) = (info.width as u32, info.height as u32);
    if w == 0 || h == 0 || w as u64 * h as u64 > MAX_PICTURE_PIXELS {
        return Err(ProjectError::Image(format!("JPG de {w}x{h} grande demais")));
    }
    let raw = decoder.decode().map_err(err)?;
    let n = w as usize * h as usize;
    let rgba: Vec<u8> = match raw.len() / n.max(1) {
        4 => raw,
        3 => raw.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        1 => raw.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        _ => return Err(ProjectError::Image("JPG com formato de cor não suportado".into())),
    };
    let factor = reduction_factor(w, h, max_side);
    let (rw, rh, rgba) = reduce_rgba(w, h, &rgba, factor);
    Ok(Fitted { width: rw, height: rh, rgba, factor, original: (w, h) })
}

fn decode_gif(bytes: &[u8]) -> Result<Vec<ImportedFrame>, ProjectError> {
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = opts.read_info(Cursor::new(bytes)).map_err(|e| ProjectError::Image(format!("GIF: {e}")))?;
    let (w, h) = (decoder.width() as u32, decoder.height() as u32);
    Image::check_size(w, h).map_err(|e| ProjectError::Image(e.to_string()))?;
    let mut canvas = Image::new(w, h);
    let mut out = Vec::new();
    while let Some(frame) = decoder.read_next_frame().map_err(|e| ProjectError::Image(format!("GIF: {e}")))? {
        if out.len() >= 1024 {
            return Err(ProjectError::Image("GIF com mais de 1024 frames".into()));
        }
        let (fw, fh) = (frame.width as u32, frame.height as u32);
        let at = Point::new(frame.left as i32, frame.top as i32);
        let previous = canvas.clone();
        let patch = Image::from_rgba(fw.max(1), fh.max(1), &frame.buffer).map_err(|e| ProjectError::Image(e.to_string()))?;
        canvas.paste(&patch, at, PasteMode::SkipTransparent);
        out.push(ImportedFrame { image: canvas.clone(), duration_ms: (frame.delay as u32 * 10).max(20) });
        match frame.dispose {
            gif::DisposalMethod::Background => {
                for p in Rect::new(at.x, at.y, fw, fh).points() {
                    canvas.set(p.x, p.y, Color::TRANSPARENT);
                }
            }
            gif::DisposalMethod::Previous => canvas = previous,
            _ => {}
        }
    }
    if out.is_empty() {
        return Err(ProjectError::Image("GIF sem frames".into()));
    }
    Ok(out)
}

/// How to cut a sprite sheet into frames.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SliceOptions {
    /// Cell width and height.
    pub cell_width: u32,
    pub cell_height: u32,
    #[serde(default)]
    pub margin: u32,
    #[serde(default)]
    pub spacing: u32,
    /// Take at most this many cells (row by row).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Skip cells that are fully transparent.
    #[serde(default = "yes")]
    pub skip_empty: bool,
}

fn yes() -> bool {
    true
}

pub fn slice_sheet(sheet: &Image, opts: &SliceOptions) -> Result<Vec<Image>, ProjectError> {
    if opts.cell_width == 0 || opts.cell_height == 0 {
        return Err(ProjectError::Image("célula precisa ter tamanho maior que zero".into()));
    }
    let mut out = Vec::new();
    let mut y = opts.margin;
    while y + opts.cell_height <= sheet.height() {
        let mut x = opts.margin;
        while x + opts.cell_width <= sheet.width() {
            let cell = sheet.crop(Rect::new(x as i32, y as i32, opts.cell_width, opts.cell_height));
            if !(opts.skip_empty && cell.is_blank()) {
                out.push(cell);
            }
            if opts.count.is_some_and(|c| out.len() as u32 >= c) {
                return Ok(out);
            }
            x += opts.cell_width + opts.spacing;
        }
        y += opts.cell_height + opts.spacing;
    }
    if out.is_empty() {
        return Err(ProjectError::Image("nenhuma célula encontrada com esse tamanho".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::codec::encode_png;

    #[test]
    fn png_and_slicing() {
        let mut sheet = Image::new(10, 4);
        sheet.set(0, 0, Color::BLACK);
        sheet.set(4, 1, Color::WHITE);
        let frames = decode_frames(&encode_png(&sheet).unwrap()).unwrap();
        assert_eq!(frames.len(), 1);
        let cells = slice_sheet(
            &frames[0].image,
            &SliceOptions { cell_width: 4, cell_height: 4, margin: 0, spacing: 0, count: None, skip_empty: true },
        )
        .unwrap();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[1].get(0, 1), Some(Color::WHITE));
        assert!(decode_frames(b"BM....").is_err());
    }

    #[test]
    fn pictures_in_any_format_fit_the_working_size() {
        // JPG: 24x16, red on the left, blue on the right (lossy, so compare loosely).
        let jpg = decode_picture(include_bytes!("../testdata/duas-cores.jpg"), 4096).unwrap();
        assert_eq!((jpg.width, jpg.height, jpg.factor), (24, 16, 1));
        let px = |x: usize, y: usize| &jpg.rgba[(y * 24 + x) * 4..(y * 24 + x) * 4 + 4];
        assert!(px(3, 8)[0] > 150 && px(3, 8)[2] < 90, "{:?}", px(3, 8));
        assert!(px(20, 8)[2] > 150 && px(20, 8)[0] < 90, "{:?}", px(20, 8));
        assert_eq!(px(3, 8)[3], 255);
        // Reduced while reading.
        let small = decode_picture(include_bytes!("../testdata/duas-cores.jpg"), 8).unwrap();
        assert_eq!((small.width, small.height, small.factor, small.original), (8, 6, 3, (24, 16)));
        // PNG and GIF go through the same door.
        let mut img = Image::new(10, 4);
        img.set(1, 1, Color::rgb(255, 0, 0));
        let png = decode_picture(&sprite_core::codec::encode_png(&img).unwrap(), 4096).unwrap();
        assert_eq!(png.rgba, img.to_rgba());
        let gif = crate::export::encode_gif(&[(img.clone(), 100)], 1, false).unwrap();
        assert_eq!(decode_picture(&gif, 5).unwrap().factor, 2);
        assert!(decode_picture(b"BM....", 64).is_err());
    }

    #[test]
    fn gif_roundtrip_through_export() {
        let mut a = Image::new(3, 3);
        a.set(0, 0, Color::rgb(255, 0, 0));
        let mut b = Image::new(3, 3);
        b.set(2, 2, Color::rgb(0, 0, 255));
        let bytes = crate::export::encode_gif(&[(a.clone(), 100), (b.clone(), 250)], 1, true).unwrap();
        let frames = decode_frames(&bytes).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].image, a);
        assert_eq!(frames[1].image, b);
        assert_eq!(frames[1].duration_ms, 250);
    }
}
