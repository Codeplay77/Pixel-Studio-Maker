//! Frame composition: layers × cels → one image.

use sprite_core::compose::{LayerView, compose_onto};
use sprite_core::{Color, Image};

use crate::model::{ImageStore, Sprite};

#[derive(Debug, Clone, Default)]
pub struct ComposeOptions {
    /// Include hidden layers.
    pub include_hidden: bool,
    /// Include reference layers.
    pub include_reference: bool,
    /// Only these layer ids (still bottom-to-top order).
    pub layers: Option<Vec<u32>>,
}

/// The image of one layer in one frame (blank if the cel does not exist).
pub fn layer_image(sprite: &Sprite, images: &ImageStore, layer: u32, frame: u32) -> Image {
    sprite.cel(layer, frame).and_then(|c| images.get(&c.image)).cloned().unwrap_or_else(|| Image::new(sprite.width, sprite.height))
}

/// Composites the layers of a frame, bottom to top.
pub fn compose_frame(sprite: &Sprite, images: &ImageStore, frame: u32, opts: &ComposeOptions) -> Image {
    let mut out = Image::new(sprite.width, sprite.height);
    for layer in &sprite.layers {
        if let Some(only) = &opts.layers {
            if !only.contains(&layer.id) {
                continue;
            }
        } else {
            if !layer.visible && !opts.include_hidden {
                continue;
            }
            if layer.reference && !opts.include_reference {
                continue;
            }
        }
        let Some(cel) = sprite.cel(layer.id, frame) else { continue };
        let Some(img) = images.get(&cel.image) else { continue };
        let opacity = ((layer.opacity as u32 * cel.opacity as u32 + 127) / 255) as u8;
        compose_onto(&mut out, LayerView { image: img, opacity, blend: layer.blend, offset: Default::default() });
    }
    out
}

/// Recomposes only `rect` of an already composited frame (composition cache: a small edit does
/// not recompose the whole frame). Same result as `compose_frame` inside the rectangle.
pub fn recompose_region(
    sprite: &Sprite,
    images: &ImageStore,
    frame: u32,
    opts: &ComposeOptions,
    target: &mut Image,
    rect: sprite_core::Rect,
) {
    let r = rect.intersect(&target.bounds());
    if r.is_empty() {
        return;
    }
    let mut part = Image::new(r.w, r.h);
    for layer in &sprite.layers {
        if let Some(only) = &opts.layers {
            if !only.contains(&layer.id) {
                continue;
            }
        } else if (!layer.visible && !opts.include_hidden) || (layer.reference && !opts.include_reference) {
            continue;
        }
        let Some(cel) = sprite.cel(layer.id, frame) else { continue };
        let Some(img) = images.get(&cel.image) else { continue };
        let opacity = ((layer.opacity as u32 * cel.opacity as u32 + 127) / 255) as u8;
        let piece = img.crop(r);
        compose_onto(&mut part, LayerView { image: &piece, opacity, blend: layer.blend, offset: Default::default() });
    }
    target.paste(&part, sprite_core::Point::new(r.x, r.y), sprite_core::PasteMode::Replace);
}

/// The current frame with tinted ghosts of neighbor frames under it (onion skin): previous
/// frames reddish, next frames bluish, fading with distance.
pub fn onion_skin(sprite: &Sprite, images: &ImageStore, sequence: &[u32], position: usize, before: usize, after: usize) -> Image {
    let opts = ComposeOptions::default();
    let mut out = Image::new(sprite.width, sprite.height);
    let mut ghost = |offset: isize, tint: Color, strength: u8| {
        let idx = position as isize + offset;
        if idx < 0 || idx as usize >= sequence.len() {
            return;
        }
        let img = compose_frame(sprite, images, sequence[idx as usize], &opts);
        let mut tinted = Image::new(sprite.width, sprite.height);
        for y in 0..img.height() as i32 {
            for x in 0..img.width() as i32 {
                let c = img.get_or_clear(x, y);
                if !c.is_transparent() {
                    let mix = |a: u8, b: u8| ((a as u32 + b as u32) / 2) as u8;
                    tinted.set(x, y, Color::rgba(mix(c.r, tint.r), mix(c.g, tint.g), mix(c.b, tint.b), strength));
                }
            }
        }
        compose_onto(&mut out, LayerView::new(&tinted));
    };
    for d in (1..=before).rev() {
        ghost(-(d as isize), Color::rgb(230, 70, 70), (110 / d) as u8);
    }
    for d in (1..=after).rev() {
        ghost(d as isize, Color::rgb(70, 130, 240), (110 / d) as u8);
    }
    if let Some(frame) = sequence.get(position) {
        let current = compose_frame(sprite, images, *frame, &opts);
        compose_onto(&mut out, LayerView::new(&current));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cel, Layer};
    use sprite_core::BlendMode;

    fn sprite_with_two_layers() -> (Sprite, ImageStore) {
        let mut n = 0;
        let mut ids = move || {
            n += 1;
            n
        };
        let mut s = Sprite::new(100, "s", 2, 1, &mut ids);
        s.layers.push(Layer {
            id: 50,
            name: "top".into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend: BlendMode::Normal,
            reference: false,
        });
        let frame = s.frames[0].id;
        let bottom = s.layers[0].id;
        let mut store = ImageStore::new();
        store.insert(1, Image::filled(2, 1, Color::rgb(255, 0, 0)));
        let mut top = Image::new(2, 1);
        top.set(1, 0, Color::rgb(0, 0, 255));
        store.insert(2, top);
        s.cels.push(Cel { layer: bottom, frame, image: 1, opacity: 255 });
        s.cels.push(Cel { layer: 50, frame, image: 2, opacity: 255 });
        (s, store)
    }

    #[test]
    fn composes_visible_layers() {
        let (mut s, store) = sprite_with_two_layers();
        let frame = s.frames[0].id;
        let img = compose_frame(&s, &store, frame, &ComposeOptions::default());
        assert_eq!(img.get(0, 0), Some(Color::rgb(255, 0, 0)));
        assert_eq!(img.get(1, 0), Some(Color::rgb(0, 0, 255)));
        s.layers[1].visible = false;
        let img = compose_frame(&s, &store, frame, &ComposeOptions::default());
        assert_eq!(img.get(1, 0), Some(Color::rgb(255, 0, 0)));
        let all = compose_frame(&s, &store, frame, &ComposeOptions { include_hidden: true, ..Default::default() });
        assert_eq!(all.get(1, 0), Some(Color::rgb(0, 0, 255)));
        let only_top = compose_frame(&s, &store, frame, &ComposeOptions { layers: Some(vec![50]), ..Default::default() });
        assert_eq!(only_top.get(0, 0), Some(Color::TRANSPARENT));
    }

    #[test]
    fn region_recompose_matches_full() {
        let (s, mut store) = sprite_with_two_layers();
        let frame = s.frames[0].id;
        let opts = ComposeOptions::default();
        let mut cached = compose_frame(&s, &store, frame, &opts);
        store.get_mut(&2).unwrap().set(0, 0, Color::rgba(0, 255, 0, 128));
        recompose_region(&s, &store, frame, &opts, &mut cached, sprite_core::Rect::new(0, 0, 1, 1));
        assert_eq!(cached, compose_frame(&s, &store, frame, &opts));
    }
}
