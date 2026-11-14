//! PNG encoding/decoding. Always 8-bit RGBA on the way out; any PNG color type on the way in.

use std::io::Cursor;

use crate::image::{Image, ImageError};

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("PNG inválido: {0}")]
    Decode(String),
    #[error("falha ao gerar PNG: {0}")]
    Encode(String),
    #[error(transparent)]
    Image(#[from] ImageError),
}

pub fn encode_png(image: &Image) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| CodecError::Encode(e.to_string()))?;
        writer.write_image_data(&image.to_rgba()).map_err(|e| CodecError::Encode(e.to_string()))?;
        writer.finish().map_err(|e| CodecError::Encode(e.to_string()))?;
    }
    Ok(out)
}

pub fn decode_png(bytes: &[u8]) -> Result<Image, CodecError> {
    let (w, h, rgba) = decode_png_rgba(bytes, crate::image::MAX_IMAGE_SIDE)?;
    Ok(Image::from_rgba(w, h, &rgba)?)
}

/// Most pixels a picture may have to be decoded whole (about 1 GB of RGBA). Bigger PNGs without
/// interlacing are still read, row by row (see [`decode_png_fit`]).
pub const MAX_PICTURE_PIXELS: u64 = 256 * 1024 * 1024;

fn png_reader(bytes: &[u8]) -> Result<png::Reader<Cursor<&[u8]>>, CodecError> {
    // The default 64 MiB budget is meant for untrusted web images; local pictures can be bigger.
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: usize::MAX });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let reader = decoder.read_info().map_err(|e| CodecError::Decode(e.to_string()))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 {
        return Err(CodecError::Image(crate::image::ImageError::InvalidSize(w, h)));
    }
    Ok(reader)
}

/// Appends `width` pixels of a decoded row as RGBA.
fn push_rgba(color: png::ColorType, data: &[u8], width: usize, out: &mut Vec<u8>) -> Result<(), CodecError> {
    match color {
        png::ColorType::Rgba => out.extend_from_slice(&data[..width * 4]),
        png::ColorType::Rgb => out.extend(data[..width * 3].as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255])),
        png::ColorType::GrayscaleAlpha => out.extend(data[..width * 2].as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]])),
        png::ColorType::Grayscale => out.extend(data[..width].iter().flat_map(|g| [*g, *g, *g, 255])),
        other => return Err(CodecError::Decode(format!("tipo de cor não suportado: {other:?}"))),
    }
    Ok(())
}

fn read_whole(mut reader: png::Reader<Cursor<&[u8]>>) -> Result<(u32, u32, Vec<u8>), CodecError> {
    let size = reader.output_buffer_size().ok_or_else(|| CodecError::Decode("imagem grande demais".into()))?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(|e| CodecError::Decode(e.to_string()))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in buf[..info.buffer_size()].chunks(info.line_size).take(h) {
        push_rgba(info.color_type, row, w, &mut rgba)?;
    }
    if rgba.len() != w * h * 4 {
        return Err(CodecError::Decode(format!("tamanho de dados inesperado ({} bytes)", rgba.len())));
    }
    Ok((info.width, info.height, rgba))
}

/// Decodes any PNG to tightly packed 8-bit RGBA, up to `max_side` pixels per side.
pub fn decode_png_rgba(bytes: &[u8], max_side: u32) -> Result<(u32, u32, Vec<u8>), CodecError> {
    let reader = png_reader(bytes)?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w > max_side || h > max_side || w as u64 * h as u64 > MAX_PICTURE_PIXELS {
        return Err(CodecError::Image(crate::image::ImageError::InvalidSize(w, h)));
    }
    read_whole(reader)
}

/// A picture read to fit a size: `factor` original pixels (per side) became one.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub factor: u32,
    pub original: (u32, u32),
}

/// Smallest integer factor that brings `width` x `height` within `max_side` per side.
pub fn reduction_factor(width: u32, height: u32, max_side: u32) -> u32 {
    let m = max_side.max(1);
    width.div_ceil(m).max(height.div_ceil(m)).max(1)
}

/// Box-filter reduction by an integer factor, fed one RGBA row at a time. Alpha-weighted, so
/// transparent pixels do not darken the edges.
pub struct Reducer {
    width: usize,
    factor: usize,
    acc: Vec<u64>,
    rows: usize,
    out: Vec<u8>,
}

impl Reducer {
    pub fn new(width: u32, factor: u32) -> Self {
        let (width, factor) = (width as usize, factor.max(1) as usize);
        Reducer { width, factor, acc: vec![0; width.div_ceil(factor) * 4], rows: 0, out: Vec::new() }
    }

    pub fn push_row(&mut self, row: &[u8]) {
        for (x, p) in row.as_chunks::<4>().0.iter().take(self.width).enumerate() {
            let a = p[3] as u64;
            let o = (x / self.factor) * 4;
            self.acc[o] += p[0] as u64 * a;
            self.acc[o + 1] += p[1] as u64 * a;
            self.acc[o + 2] += p[2] as u64 * a;
            self.acc[o + 3] += a;
        }
        self.rows += 1;
        if self.rows == self.factor {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.rows == 0 {
            return;
        }
        let (width, factor, rows) = (self.width, self.factor, self.rows as u64);
        for (ox, a) in self.acc.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let n = factor.min(width - ox * factor) as u64 * rows;
            let sa = a[3];
            if sa == 0 {
                self.out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let c = |v: u64| ((v + sa / 2) / sa) as u8;
                self.out.extend_from_slice(&[c(a[0]), c(a[1]), c(a[2]), ((sa + n / 2) / n) as u8]);
            }
            *a = [0; 4];
        }
        self.rows = 0;
    }

    /// The reduced RGBA rows.
    pub fn finish(mut self) -> Vec<u8> {
        self.flush();
        self.out
    }
}

/// Reduces a whole RGBA buffer by an integer factor (a copy when the factor is 1).
pub fn reduce_rgba(width: u32, height: u32, rgba: &[u8], factor: u32) -> (u32, u32, Vec<u8>) {
    if factor <= 1 {
        return (width, height, rgba.to_vec());
    }
    let mut r = Reducer::new(width, factor);
    for row in rgba.chunks(width as usize * 4).take(height as usize) {
        r.push_row(row);
    }
    (width.div_ceil(factor), height.div_ceil(factor), r.finish())
}

/// Decodes a PNG of any size into RGBA no bigger than `max_side` per side: bigger pictures are
/// reduced (box filter, integer factor) while being read, row by row, so memory stays bounded.
pub fn decode_png_fit(bytes: &[u8], max_side: u32) -> Result<Fitted, CodecError> {
    let mut reader = png_reader(bytes)?;
    let (w, h) = (reader.info().width, reader.info().height);
    let factor = reduction_factor(w, h, max_side);
    if factor == 1 || reader.info().interlaced {
        if w as u64 * h as u64 > MAX_PICTURE_PIXELS {
            return Err(CodecError::Decode(format!("PNG entrelaçado de {w}x{h} grande demais para ler inteiro; salve sem entrelaçamento")));
        }
        let (w, h, rgba) = read_whole(reader)?;
        let (rw, rh, rgba) = reduce_rgba(w, h, &rgba, factor);
        return Ok(Fitted { width: rw, height: rh, rgba, factor, original: (w, h) });
    }
    let (color, _) = reader.output_color_type();
    let mut reducer = Reducer::new(w, factor);
    let mut line = Vec::with_capacity(w as usize * 4);
    for _ in 0..h {
        let row = reader
            .next_row()
            .map_err(|e| CodecError::Decode(e.to_string()))?
            .ok_or_else(|| CodecError::Decode("PNG terminou antes da hora".into()))?;
        line.clear();
        push_rgba(color, row.data(), w as usize, &mut line)?;
        reducer.push_row(&line);
    }
    Ok(Fitted { width: w.div_ceil(factor), height: h.div_ceil(factor), rgba: reducer.finish(), factor, original: (w, h) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    #[test]
    fn png_roundtrip_is_lossless() {
        let mut img = Image::new(7, 5);
        img.set(0, 0, Color::rgb(57, 118, 210));
        img.set(6, 4, Color::rgba(200, 10, 10, 77));
        img.set(3, 2, Color::WHITE);
        let bytes = encode_png(&img).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        assert_eq!(decode_png(&bytes).unwrap(), img);
    }

    #[test]
    fn big_pictures_are_reduced_while_reading() {
        // 10x6: 2x2 blocks of solid colors, and transparent blocks.
        let mut img = Image::new(10, 6);
        for y in 0..6 {
            for x in 0..10 {
                let c = match (x / 2 + y / 2) % 3 {
                    0 => Color::rgb(200, 30, 30),
                    1 => Color::rgb(30, 30, 200),
                    _ => continue,
                };
                img.set(x, y, c);
            }
        }
        let bytes = encode_png(&img).unwrap();
        // Fits already: exact.
        let same = decode_png_fit(&bytes, 10).unwrap();
        assert_eq!((same.width, same.height, same.factor), (10, 6, 1));
        assert_eq!(same.rgba, img.to_rgba());
        // Factor 2, row by row, equals reducing the whole buffer.
        let fit = decode_png_fit(&bytes, 5).unwrap();
        assert_eq!((fit.width, fit.height, fit.factor, fit.original), (5, 3, 2, (10, 6)));
        assert_eq!(fit.rgba, reduce_rgba(10, 6, &img.to_rgba(), 2).2);
        let small = Image::from_rgba(5, 3, &fit.rgba).unwrap();
        assert_eq!(small.get(0, 0), Some(Color::rgb(200, 30, 30)));
        assert_eq!(small.get(1, 0), Some(Color::rgb(30, 30, 200)));
        assert_eq!(small.get(2, 0), Some(Color::TRANSPARENT));
        // Uneven factor: the last block is partial.
        let odd = decode_png_fit(&bytes, 4).unwrap();
        assert_eq!((odd.width, odd.height, odd.factor), (4, 2, 3));
        assert_eq!(reduction_factor(40_000, 300, 4096), 10);
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_png(b"not a png").is_err());
    }
}
