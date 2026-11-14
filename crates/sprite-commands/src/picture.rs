//! Picture to pixel art (`image_pixelate`, and `import_image` with `pixelate`): reads the file
//! (PNG, JPG or GIF, any size), runs [`sprite_drawing::pixelate`] and packs the result.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde_json::{Value, json};
use sprite_core::{Image, Palette, Rect};
use sprite_drawing::pixelate::{Picture, PixelateOptions, Pixelated, pixelate, thumbnail};
use sprite_project::Document;
use sprite_project::export::scale_image;
use sprite_project::import::decode_picture;

use crate::args::ImagePixelate;
use crate::{CommandError, ErrorCode, ExecContext, Outcome, Session};

/// Pictures are worked on at most this big per side; bigger ones are reduced while being read.
pub const WORK_SIDE: u32 = 4096;

/// A decoded picture: straight RGBA bytes, possibly reduced from a bigger original.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// Original pixels per working pixel, per side (1 = not reduced).
    pub factor: u32,
    /// Size of the picture in the file.
    pub original: (u32, u32),
}

impl Decoded {
    pub fn picture(&self) -> Picture<'_> {
        Picture { width: self.width, height: self.height, rgba: &self.rgba }
    }
}

type CacheEntry = (PathBuf, Option<SystemTime>, u64, Arc<Decoded>);

/// The last picture read: previews ask again and again for the same file while options change.
static LAST: Mutex<Option<CacheEntry>> = Mutex::new(None);

fn io_error(path: &Path, e: std::io::Error) -> CommandError {
    CommandError::new(ErrorCode::Io, format!("não foi possível ler {}: {e}", path.display()))
}

/// Reads a PNG, JPG or GIF (first frame) of any size; pictures over [`WORK_SIDE`] per side are
/// reduced while being read.
pub fn read_picture(path: &Path) -> Result<Arc<Decoded>, CommandError> {
    let meta = std::fs::metadata(path).map_err(|e| io_error(path, e))?;
    let (modified, len) = (meta.modified().ok(), meta.len());
    if let Ok(cache) = LAST.lock()
        && let Some((p, m, l, d)) = cache.as_ref()
        && p == path
        && *m == modified
        && *l == len
    {
        return Ok(d.clone());
    }
    let bytes = std::fs::read(path).map_err(|e| io_error(path, e))?;
    let fit = decode_picture(&bytes, WORK_SIDE)?;
    let decoded = Arc::new(Decoded { width: fit.width, height: fit.height, rgba: fit.rgba, factor: fit.factor, original: fit.original });
    if let Ok(mut cache) = LAST.lock() {
        *cache = Some((path.to_path_buf(), modified, len, decoded.clone()));
    }
    Ok(decoded)
}

/// The options in working-picture pixels (`region` and `cell` are given in original pixels),
/// plus warnings about the reduction.
pub fn working_options(opts: &PixelateOptions, source: &Decoded) -> (PixelateOptions, Vec<String>) {
    let f = source.factor.max(1);
    let mut o = opts.clone();
    let mut warnings = Vec::new();
    if f == 1 {
        return (o, warnings);
    }
    warnings.push(format!(
        "imagem de {}x{} lida reduzida {f}x ({}x{}) para a conversão",
        source.original.0, source.original.1, source.width, source.height
    ));
    if let Some(r) = o.region {
        let (x, y) = (r.x.max(0) as u32, r.y.max(0) as u32);
        let (x0, y0, x1, y1) = (x / f, y / f, (x + r.w).div_ceil(f), (y + r.h).div_ceil(f));
        o.region = Some(Rect::new(x0 as i32, y0 as i32, x1.saturating_sub(x0).max(1), y1.saturating_sub(y0).max(1)));
    }
    if let Some(c) = o.cell {
        let reduced = ((c as f64 / f as f64).round() as u32).max(1);
        if c % f != 0 {
            warnings.push(format!("cell {c} não é múltiplo da redução {f}x: usando {reduced} na imagem reduzida"));
        }
        o.cell = Some(reduced);
    }
    (o, warnings)
}

/// Converts a picture file (options in original pixels).
pub fn convert(path: &Path, opts: &PixelateOptions) -> Result<(Pixelated, Arc<Decoded>, Vec<String>), CommandError> {
    let source = read_picture(path)?;
    let (o, warnings) = working_options(opts, &source);
    let result = pixelate(source.picture(), &o)?;
    Ok((result, source, warnings))
}

/// What the conversion reports back (the same for preview, new project and import). Rectangles
/// and blocks are in original picture pixels.
pub fn summary(r: &Pixelated, source: &Decoded) -> Value {
    let f = source.factor.max(1);
    let sr = r.source_rect;
    json!({
        "size": [r.image.width(), r.image.height()],
        "colors": r.palette,
        "color_count": r.palette.len(),
        "opaque_pixels": r.image.opaque_count(),
        "background_removed": r.background,
        "source_size": [source.original.0, source.original.1],
        "source_rect": Rect::new(sr.x * f as i32, sr.y * f as i32, sr.w * f, sr.h * f),
        "read_reduction": f,
        "block": [round2(r.block.0 * f as f64), round2(r.block.1 * f as f64)],
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn preview_scale(img: &Image, asked: Option<u32>) -> u32 {
    match asked {
        Some(s) => s.clamp(1, 16),
        None => (512 / img.width().max(img.height()).max(1)).clamp(1, 16),
    }
}

/// `image_pixelate`: converts the picture; unless it is a preview, also builds the new project
/// (one sprite, one layer, one frame, the result's palette active). The runtime registers it.
pub fn image_pixelate(a: &ImagePixelate, ctx: &ExecContext) -> Result<(Option<Session>, Outcome), CommandError> {
    let path = ctx.paths.resolve(&a.path)?;
    let (result, source, warnings) = convert(&path, &a.options)?;
    let mut out = Outcome::data(summary(&result, &source));
    out.warnings = warnings;
    let scale = preview_scale(&result.image, a.preview_scale);
    let label = format!("pixel art {}x{} (escala {scale}x)", result.image.width(), result.image.height());
    out.images.push(crate::render::png(label, &scale_image(&result.image, scale))?);
    if a.with_source {
        let thumb = thumbnail(source.picture(), 256)?;
        out.images.push(crate::render::png("original reduzida", &thumb)?);
    }
    if a.preview {
        out.data["preview"] = json!(true);
        return Ok((None, out));
    }
    let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "imagem".into());
    let name = a.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or(stem).trim().to_string();
    let mut doc = Document::new(&name);
    doc.begin("image_pixelate")?;
    let palette_name = format!("{name} (pixel art)");
    doc.model.palettes.insert(0, Palette::new(palette_name.clone(), result.palette.clone()));
    doc.model.active_palette = Some(palette_name.clone());
    let (w, h) = (result.image.width(), result.image.height());
    let si = crate::ops::structure::add_sprite(&mut doc, &name, w, h);
    let (layer, frame) = (doc.model.sprites[si].layers[0].id, doc.model.sprites[si].frames[0].id);
    if let Some(l) = doc.model.sprites[si].layers.iter_mut().find(|l| l.id == layer) {
        l.name = "pixel art".into();
    }
    let image = doc.insert_image(result.image);
    doc.model.sprites[si].cels.push(sprite_animation::Cel { layer, frame, image, opacity: 255 });
    doc.model.sprites[si].palette = Some(palette_name.clone());
    doc.commit()?;
    doc.reset_history();
    out.data["palette"] = json!(palette_name);
    out.data["sprite"] = json!({ "id": doc.model.sprites[si].id, "name": name, "width": w, "height": h, "layer": layer, "frame": frame });
    Ok((Some(Session::new(doc, None)), out))
}

/// Default size for a conversion placed into an existing sprite: fit the sprite.
pub fn fit_options(opts: &PixelateOptions, width: u32, height: u32) -> PixelateOptions {
    let mut o = opts.clone();
    if o.width.is_none() && o.height.is_none() && o.cell.is_none() {
        o.width = Some(width);
        o.height = Some(height);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_follow_the_reading_reduction() {
        let source = Decoded { width: 100, height: 50, rgba: vec![0; 100 * 50 * 4], factor: 4, original: (400, 200) };
        let opts = PixelateOptions { region: Some(Rect::new(40, 20, 100, 61)), cell: Some(8), ..Default::default() };
        let (o, warnings) = working_options(&opts, &source);
        assert_eq!(o.region, Some(Rect::new(10, 5, 25, 16)));
        assert_eq!(o.cell, Some(2));
        assert_eq!(warnings.len(), 1, "8 is a multiple of 4: only the reduction notice");
        let (o, warnings) = working_options(&PixelateOptions { cell: Some(6), ..Default::default() }, &source);
        assert_eq!(o.cell, Some(2));
        assert_eq!(warnings.len(), 2);
        let same = Decoded { factor: 1, original: (100, 50), ..source };
        assert_eq!(working_options(&opts, &same).0, opts);
    }
}
