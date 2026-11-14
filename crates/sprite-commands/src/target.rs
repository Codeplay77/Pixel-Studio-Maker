//! Resolving references (sprite, layer, frame, region) and getting writable cels.

use sprite_animation::{Cel, Sprite};
use sprite_core::{Color, FrameRef, Image, ItemRef, Mask};
use sprite_project::Document;

use crate::{CommandError, ErrorCode, Target};

pub(crate) struct Resolved {
    pub si: usize,
    pub layer: u32,
    pub frame: u32,
}

pub(crate) fn sprite_index(doc: &Document, r: Option<&ItemRef>) -> Result<usize, CommandError> {
    if doc.model.sprites.is_empty() {
        return Err(CommandError::not_found("o projeto não tem sprites; use sprite_create"));
    }
    doc.model
        .sprite_index(r)
        .ok_or_else(|| CommandError::not_found(format!("sprite {} não existe", r.map(|r| r.to_string()).unwrap_or_default())))
}

pub(crate) fn sprite(doc: &Document, si: usize) -> &Sprite {
    &doc.model.sprites[si]
}

/// Top-most layer that is not a reference layer.
pub(crate) fn default_layer(s: &Sprite) -> Option<u32> {
    s.layers.iter().rev().find(|l| !l.reference).or_else(|| s.layers.last()).map(|l| l.id)
}

pub(crate) fn layer_id(s: &Sprite, r: Option<&ItemRef>) -> Result<u32, CommandError> {
    match r {
        Some(r) => Ok(s.layer(r)?.id),
        None => default_layer(s).ok_or_else(|| CommandError::not_found("o sprite não tem layers")),
    }
}

pub(crate) fn frame_id(s: &Sprite, r: Option<&FrameRef>) -> Result<u32, CommandError> {
    match r {
        Some(r) => Ok(s.resolve_frame(r)?),
        None => s.frames.first().map(|f| f.id).ok_or_else(|| CommandError::not_found("o sprite não tem frames")),
    }
}

pub(crate) fn resolve(doc: &Document, t: &Target) -> Result<Resolved, CommandError> {
    let si = sprite_index(doc, t.sprite.as_ref())?;
    let s = sprite(doc, si);
    Ok(Resolved { si, layer: layer_id(s, t.layer.as_ref())?, frame: frame_id(s, t.frame.as_ref())? })
}

/// The image of (layer, frame), creating an empty cel when needed. Fails on locked layers.
pub(crate) fn writable_cel(doc: &mut Document, si: usize, layer: u32, frame: u32) -> Result<u32, CommandError> {
    let s = &doc.model.sprites[si];
    let l = s.layers.iter().find(|l| l.id == layer).ok_or_else(|| CommandError::not_found(format!("layer {layer} não existe")))?;
    if l.locked {
        return Err(CommandError::new(ErrorCode::Locked, format!("a layer \"{}\" está bloqueada", l.name)));
    }
    if let Some(c) = s.cel(layer, frame) {
        return Ok(c.image);
    }
    let (w, h) = (s.width, s.height);
    let image = doc.insert_image(Image::new(w, h));
    doc.model.sprites[si].cels.push(Cel { layer, frame, image, opacity: 255 });
    Ok(image)
}

/// Colors usable by index: the sprite's palette, else the project's active palette.
pub(crate) fn palette_colors(doc: &Document, si: usize) -> Vec<Color> {
    let s = sprite(doc, si);
    doc.model.palette(s.palette.as_deref()).map(|p| p.colors.clone()).unwrap_or_default()
}

pub(crate) fn region_mask(doc: &Document, si: usize, region: &ItemRef, frame: u32) -> Result<Mask, CommandError> {
    let s = sprite(doc, si);
    let r = s.region(region)?;
    let shape = r.shape(frame).ok_or_else(|| CommandError::invalid(format!("a região \"{}\" não tem forma no frame {frame}", r.name)))?;
    Ok(shape.to_mask(s.width, s.height))
}

pub(crate) fn check_point(s: &Sprite, x: i32, y: i32) -> Result<(), CommandError> {
    if x < 0 || y < 0 || x >= s.width as i32 || y >= s.height as i32 {
        return Err(CommandError::new(ErrorCode::OutOfBounds, format!("[{x}, {y}] fora do sprite {}x{}", s.width, s.height)));
    }
    Ok(())
}
