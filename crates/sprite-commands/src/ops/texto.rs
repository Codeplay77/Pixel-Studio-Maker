//! Text tools: drawing and measuring text, bitmap fonts (built-in or drawn in a sprite), BMFont
//! export and 9-slice (preview and Godot export).

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame};
use sprite_animation::{Layer, Region, RegionShape, Sprite};
use sprite_core::codec::encode_png;
use sprite_core::{Color, Image, ItemRef, Point, Rect};
use sprite_drawing::nine_slice::{self, NineSlice};
use sprite_drawing::sdc::{ColorRef, DrawOp};
use sprite_drawing::text::{
    self, BUILTIN_FONTS, BitmapFont, SpriteFontDef, TextColors, TextSpec, builtin_font, builtin_for, font_from_sheet, standard_charset,
};
use sprite_project::Document;
use sprite_project::export::scale_image;
use sprite_project::format::write_atomic;

use super::draw::{Cel, ChangeSet, apply_op};
use super::ensure_unique;
use super::structure::{add_sprite, set_cel_image};
use crate::args::*;
use crate::render::{self, auto_scale, check_scale, preview};
use crate::target::{frame_id, palette_colors, region_mask, resolve, sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

// ---- fonts ----------------------------------------------------------------------------------

/// A font ready to use: a built-in one, or one built from a sprite.
pub(crate) enum FontHandle {
    Builtin(&'static BitmapFont),
    Sprite { font: Arc<BitmapFont>, si: usize, empty: Vec<char> },
}

impl std::ops::Deref for FontHandle {
    type Target = BitmapFont;
    fn deref(&self) -> &BitmapFont {
        match self {
            FontHandle::Builtin(f) => f,
            FontHandle::Sprite { font, .. } => font,
        }
    }
}

impl FontHandle {
    /// What a `text` operation carries (built-in fonts need nothing).
    fn op_data(&self) -> Option<Arc<BitmapFont>> {
        match self {
            FontHandle::Builtin(_) => None,
            FontHandle::Sprite { font, .. } => Some(font.clone()),
        }
    }
}

fn unknown_font(r: &ItemRef) -> CommandError {
    CommandError::not_found(format!(
        "fonte {r} não existe: use {} ou o nome/id de um sprite definido com font_define",
        BUILTIN_FONTS.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
    ))
}

/// The frame that holds a sprite font's glyphs.
fn font_frame(sp: &Sprite, def: &SpriteFontDef) -> Result<u32, CommandError> {
    match def.frame {
        Some(f) => sp.frame(f).map(|x| x.id).ok_or_else(|| {
            CommandError::not_found(format!("o frame {f} da fonte \"{}\" não existe mais; redefina com font_define", sp.name))
        }),
        None => frame_id(sp, None),
    }
}

/// Builds the font of sprite `r` from its pixels.
fn sprite_font(doc: &Document, r: &ItemRef) -> Result<FontHandle, CommandError> {
    let si = doc.model.sprite_index(Some(r)).ok_or_else(|| unknown_font(r))?;
    let sp = sprite(doc, si);
    let def = sp.extras.texto.font.as_ref().ok_or_else(|| {
        CommandError::invalid(format!("o sprite \"{}\" não é uma fonte: defina com font_define (ou crie um com font_template)", sp.name))
    })?;
    let img = compose_frame(sp, doc.images(), font_frame(sp, def)?, &ComposeOptions::default());
    let (font, empty) = font_from_sheet(&sp.name, &img, def)?;
    Ok(FontHandle::Sprite { font: Arc::new(font), si, empty })
}

/// Resolves a font reference: built-in names first, then sprites.
pub(crate) fn load_font(doc: &Document, r: &ItemRef) -> Result<FontHandle, CommandError> {
    match builtin_for(r) {
        Some(f) => Ok(FontHandle::Builtin(f)),
        None => sprite_font(doc, r),
    }
}

/// Loads the sprite font of a `text` operation before it is drawn (built-in fonts and fonts
/// already loaded pass through untouched).
pub(crate) fn prepare_op<'o>(doc: &Document, op: &'o DrawOp) -> Result<Cow<'o, DrawOp>, CommandError> {
    match op {
        DrawOp::Text { spec, font_data: None } if builtin_for(&spec.font).is_none() => {
            let font = sprite_font(doc, &spec.font)?;
            Ok(Cow::Owned(DrawOp::Text { spec: spec.clone(), font_data: font.op_data() }))
        }
        _ => Ok(Cow::Borrowed(op)),
    }
}

fn missing_warning(font: &str, missing: &[char]) -> String {
    let list: Vec<String> = missing.iter().map(|c| format!("'{c}' (U+{:04X})", *c as u32)).collect();
    format!("a fonte {font} não tem {}: desenhados como retângulo", list.join(", "))
}

/// Warnings for the `text` operations of a batch (characters the font lacks).
pub(crate) fn batch_warnings(doc: &Document, ops: &[BatchOp]) -> Vec<String> {
    let mut out = Vec::new();
    for bop in ops {
        if let DrawOp::Text { spec, .. } = &bop.op
            && let Ok(font) = load_font(doc, &spec.font)
            && let Ok(lay) = text::layout(spec, &font)
            && !lay.missing.is_empty()
        {
            out.push(missing_warning(&font.name, &lay.missing));
        }
    }
    out
}

fn layout_json(lay: &text::TextLayout) -> serde_json::Value {
    json!({
        "box": lay.rect, "width": lay.rect.w, "height": lay.rect.h,
        "lines": lay.lines, "line_count": lay.lines.len(),
        "missing": lay.missing.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
    })
}

// ---- text -----------------------------------------------------------------------------------

pub(crate) fn draw_text(s: &mut Session, a: DrawText) -> Result<Outcome, CommandError> {
    let r = resolve(&s.doc, &a.target)?;
    let palette = palette_colors(&s.doc, r.si);
    let clip = a.clip.as_ref().map(|reg| region_mask(&s.doc, r.si, reg, r.frame)).transpose()?;
    let font = load_font(&s.doc, &a.spec.font)?;
    let lay = text::layout(&a.spec, &font)?;
    let op = DrawOp::Text { spec: a.spec.clone(), font_data: font.op_data() };
    let mut changes = ChangeSet::new(sprite(&s.doc, r.si).id);
    apply_op(&mut s.doc, Cel { si: r.si, layer: r.layer, frame: r.frame }, &op, clip.as_ref(), &palette, &mut changes)?;
    let mut extra = layout_json(&lay);
    extra["layer"] = json!(r.layer);
    extra["frame"] = json!(r.frame);
    extra["font"] = json!(font.name);
    let mut out = changes.outcome(extra);
    if !lay.missing.is_empty() {
        out.warnings.push(missing_warning(&font.name, &lay.missing));
    }
    let sp = sprite(&s.doc, r.si);
    if !lay.rect.is_empty() && lay.rect.intersect(&Rect::sized(sp.width, sp.height)) != lay.rect {
        out.warnings
            .push(format!("parte do texto ({}x{} em [{}, {}]) ficou fora do sprite", lay.rect.w, lay.rect.h, lay.rect.x, lay.rect.y));
    }
    Ok(out)
}

pub(crate) fn text_measure(s: &mut Session, a: TextMeasure) -> Result<Outcome, CommandError> {
    let font = load_font(&s.doc, &a.spec.font)?;
    let lay = text::layout(&a.spec, &font)?;
    // Geometry only: any opaque color gives the same pixels.
    let probe = TextColors {
        text: Some(Color::WHITE),
        outline: a.spec.outline.as_ref().map(|_| Color::WHITE),
        shadow: a.spec.shadow.as_ref().map(|_| Color::WHITE),
    };
    let ink = text::render(&lay, &font, &a.spec, &probe).and_then(|p| p.ink());
    let mut data = layout_json(&lay);
    data["ink"] = json!(ink);
    data["scale"] = json!(lay.scale);
    data["font"] = json!({
        "name": font.name, "line_height": font.line_height, "cell_height": font.cell_height, "base": font.base,
        "letter_spacing": font.letter_spacing, "space_width": font.space_width,
    });
    Ok(Outcome::data(data))
}

/// Renders `sample` with `font` on a checkerboard, for the agent to look at.
fn sample_image(
    font: &BitmapFont,
    sample: Option<&str>,
    scale: Option<u32>,
    ctx: &ExecContext,
) -> Result<crate::RenderedImage, CommandError> {
    let text: String = match sample {
        Some(t) if !t.trim().is_empty() => t.to_string(),
        _ => {
            // Every glyph, 16 per line.
            let chars: Vec<char> = font.glyphs.keys().copied().collect();
            chars.chunks(16).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("\n")
        }
    };
    let mut spec = TextSpec::new(text, Point::new(1, 1));
    spec.font = ItemRef::Name(font.name.clone());
    if font.builtin {
        spec.color = Some(ColorRef::Value("#1a1c2c".into()));
    }
    let lay = text::layout(&spec, font)?;
    let (w, h) = (lay.rect.w.max(1) + 2, lay.rect.h.max(1) + 2);
    let mut img = Image::new(w.min(4096), h.min(4096));
    let mut canvas = sprite_drawing::Canvas::new(&mut img, None);
    if !lay.rect.is_empty() {
        text::draw(&mut canvas, &spec, font, &[])?;
    }
    let max = ctx.limits.max_preview_side;
    let scale = check_scale(img.width(), img.height(), scale.unwrap_or_else(|| auto_scale(img.width(), img.height(), max).min(6)), max)?;
    render::png(format!("amostra da fonte {}", font.name), &preview(&img, scale, Background::Checker, false))
}

fn char_list(chars: &[char]) -> Vec<String> {
    chars.iter().map(|c| c.to_string()).collect()
}

fn structure_change(s: &Session, si: usize) -> Change {
    Change { sprite: s.doc.model.sprites[si].id, layer: None, frame: None, rect: None, kind: "structure".into() }
}

// ---- fonts in sprites -----------------------------------------------------------------------

pub(crate) fn font_define(s: &mut Session, a: FontDefine, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    if a.clear {
        let had = s.doc.model.sprites[si].extras.texto.font.take().is_some();
        let sp = sprite(&s.doc, si);
        return Ok(Outcome {
            data: json!({ "sprite": sp.id, "name": sp.name, "cleared": had }),
            changes: vec![structure_change(s, si)],
            ..Default::default()
        });
    }
    let sp = sprite(&s.doc, si);
    let cur = sp.extras.texto.font.clone();
    let cell =
        a.cell.or(cur.as_ref().map(|d| d.cell)).ok_or_else(|| CommandError::invalid("informe `cell`: [largura, altura] de cada célula"))?;
    let charset = a
        .charset
        .clone()
        .or(cur.as_ref().map(|d| d.charset.clone()))
        .ok_or_else(|| CommandError::invalid("informe `charset`: os caracteres na ordem das células"))?;
    if cell[0] == 0 || cell[1] == 0 {
        return Err(CommandError::invalid("a célula precisa de pelo menos 1x1 px"));
    }
    let frame = match &a.frame {
        Some(f) => Some(sp.resolve_frame(f)?),
        None => cur.as_ref().and_then(|d| d.frame),
    };
    let def = SpriteFontDef {
        cell,
        charset,
        columns: a.columns.or(cur.as_ref().map(|d| d.columns)).unwrap_or((sp.width / cell[0]).max(1)),
        baseline: a.baseline.or(cur.as_ref().map(|d| d.baseline)).unwrap_or(cell[1]),
        letter_spacing: a.letter_spacing.or(cur.as_ref().map(|d| d.letter_spacing)).unwrap_or(1),
        space_width: a.space_width.or(cur.as_ref().map(|d| d.space_width)).unwrap_or(cell[0].div_ceil(2).max(1)),
        line_height: a.line_height.or(cur.as_ref().map(|d| d.line_height)).unwrap_or(cell[1]),
        frame,
    };
    def.validate(sp.width, sp.height)?;
    let sprite_id = sp.id;
    s.doc.model.sprites[si].extras.texto.font = Some(def.clone());
    let FontHandle::Sprite { font, empty, .. } = sprite_font(&s.doc, &ItemRef::Id(sprite_id))? else { unreachable!("sprite font") };
    let mut out = Outcome {
        data: json!({
            "sprite": sprite_id, "name": font.name, "font": def, "glyphs": font.glyphs.len(),
            "empty_cells": char_list(&empty),
            "widths": font.glyphs.iter().map(|(c, g)| (c.to_string(), g.advance)).collect::<BTreeMap<_, _>>(),
        }),
        changes: vec![structure_change(s, si)],
        ..Default::default()
    };
    if !empty.is_empty() {
        out.warnings.push(format!("{} células vazias ficaram fora da fonte: {}", empty.len(), empty.iter().collect::<String>()));
    }
    if font.glyphs.is_empty() {
        out.warnings.push("nenhuma célula tem pixels: desenhe os glyphs (ou use font_template com base_font)".into());
    } else {
        out.images.push(sample_image(&font, a.sample.as_deref(), None, ctx)?);
    }
    Ok(out)
}

pub(crate) fn font_info(s: &mut Session, a: FontInfo, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let r = match &a.font {
        Some(f) => f.clone(),
        None => ItemRef::Id(sprite(&s.doc, sprite_index(&s.doc, a.target.sprite.as_ref())?).id),
    };
    let font = load_font(&s.doc, &r)?;
    let missing: Vec<char> = standard_charset().chars().filter(|c| !font.has(*c)).collect();
    let mut data = json!({
        "name": font.name,
        "kind": if font.builtin { "builtin" } else { "sprite" },
        "builtin_fonts": BUILTIN_FONTS,
        "cell_height": font.cell_height, "line_height": font.line_height, "base": font.base,
        "letter_spacing": font.letter_spacing, "space_width": font.space_width,
        "glyph_count": font.glyphs.len(),
        "charset": font.charset(),
        "missing_standard": char_list(&missing),
    });
    if a.widths {
        data["widths"] = json!(font.glyphs.iter().map(|(c, g)| (c.to_string(), g.advance)).collect::<BTreeMap<_, _>>());
    }
    if let FontHandle::Sprite { si, empty, .. } = &font {
        let sp = sprite(&s.doc, *si);
        data["sprite"] = json!(sp.id);
        data["definition"] = json!(sp.extras.texto.font);
        data["empty_cells"] = json!(char_list(empty));
    }
    let mut out = Outcome::data(data);
    if !font.glyphs.is_empty() {
        out.images.push(sample_image(&font, a.sample.as_deref(), a.scale, ctx)?);
    }
    Ok(out)
}

fn glyph_region_name(c: char) -> String {
    match c {
        ' ' => "glifo espaço".into(),
        c => format!("glifo {c}"),
    }
}

pub(crate) fn font_template(s: &mut Session, a: FontTemplate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let base =
        match &a.base_font {
            Some(n) => Some(builtin_font(n).ok_or_else(|| {
                CommandError::not_found(format!("fonte embutida \"{n}\" não existe (use {})", BUILTIN_FONTS.join(" ou ")))
            })?),
            None => None,
        };
    let metrics = base.unwrap_or_else(|| builtin_font("pixel-5x7").expect("built-in font"));
    let charset = a.charset.clone().unwrap_or_else(standard_charset);
    let chars: Vec<char> = charset.chars().collect();
    let cell = a.cell.unwrap_or([metrics.max_glyph_width() + 1, metrics.line_height.max(metrics.cell_height)]);
    let custom_cell = a.cell.is_some() && base.is_none();
    let def = SpriteFontDef {
        cell,
        charset,
        columns: a.columns,
        baseline: a.baseline.unwrap_or(if custom_cell { cell[1] - cell[1] / 4 } else { metrics.base.min(cell[1]) }),
        letter_spacing: a.letter_spacing.unwrap_or(metrics.letter_spacing),
        space_width: a.space_width.unwrap_or(if custom_cell { cell[0].div_ceil(2) } else { metrics.space_width }),
        line_height: a.line_height.unwrap_or(if a.cell.is_some() { cell[1] } else { metrics.line_height }),
        frame: None,
    };
    if def.columns == 0 || cell[0] == 0 || cell[1] == 0 {
        return Err(CommandError::invalid("`columns` e os lados da célula precisam ser pelo menos 1"));
    }
    let (w, h) = (def.columns.saturating_mul(cell[0]), def.rows().saturating_mul(cell[1]));
    let max = ctx.limits.max_canvas_side;
    if w == 0 || h == 0 || w > max || h > max {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("a grade daria um sprite de {w}x{h} px (limite {max} por lado): mude `columns`, `cell` ou o `charset`"),
        ));
    }
    def.validate(w, h)?;
    if let Some(f) = base {
        for c in chars.iter().filter(|c| **c != ' ') {
            if let Some(g) = f.glyph(*c) {
                let (gw, gh) = (g.xoffset.max(0) as u32 + g.width(), g.yoffset.max(0) as u32 + g.height());
                if gw > cell[0] || gh > cell[1] {
                    return Err(CommandError::invalid(format!(
                        "célula {}x{} pequena demais para a {}: '{c}' precisa de {gw}x{gh}",
                        cell[0], cell[1], f.name
                    )));
                }
            }
        }
    }
    let name = a.name.clone().unwrap_or_else(|| "Fonte".into());
    ensure_unique("sprite", &name, s.doc.model.sprites.iter().map(|x| x.name.as_str()))?;
    let si = add_sprite(&mut s.doc, &name, w, h);
    s.doc.model.sprites[si].layers[0].name = "glifos".into();
    let (glyph_layer, frame) = (sprite(&s.doc, si).layers[0].id, sprite(&s.doc, si).frames[0].id);
    let mut prefilled = 0;
    if let Some(f) = base {
        let palette = palette_colors(&s.doc, si);
        let color = a.color.clone().unwrap_or_else(|| ColorRef::Value("#ffffff".into())).resolve(&palette)?;
        let mut img = Image::new(w, h);
        for (i, c) in chars.iter().enumerate() {
            let Some(g) = f.glyph(*c).filter(|_| *c != ' ') else { continue };
            let r = def.cell_rect(i);
            for y in 0..g.height() as i32 {
                for x in 0..g.width() as i32 {
                    if !g.image.get_or_clear(x, y).is_transparent() {
                        img.set(r.x + g.xoffset + x, r.y + g.yoffset + y, color);
                    }
                }
            }
            prefilled += 1;
        }
        set_cel_image(&mut s.doc, si, glyph_layer, frame, img)?;
    }
    if a.guides {
        let id = s.doc.alloc_id();
        s.doc.model.sprites[si].layers.insert(
            0,
            Layer { id, name: "guias".into(), visible: true, locked: false, opacity: 255, blend: Default::default(), reference: true },
        );
        let mut img = Image::new(w, h);
        for i in 0..chars.len() {
            let r = def.cell_rect(i);
            let shade = if (i as u32 / def.columns + i as u32 % def.columns).is_multiple_of(2) {
                Color::rgb(36, 40, 48)
            } else {
                Color::rgb(46, 51, 61)
            };
            for p in r.points() {
                let on_base = def.baseline < cell[1] && p.y == r.y + def.baseline as i32;
                img.set(p.x, p.y, if on_base { Color::rgb(64, 84, 112) } else { shade });
            }
        }
        set_cel_image(&mut s.doc, si, id, frame, img)?;
        // Locked so strokes never land on the guides by accident.
        if let Some(l) = s.doc.model.sprites[si].layer_mut(id) {
            l.locked = true;
        }
    }
    let mut regions = 0;
    if a.regions {
        for (i, c) in chars.iter().enumerate() {
            let id = s.doc.alloc_id();
            s.doc.model.sprites[si].regions.push(Region {
                id,
                name: glyph_region_name(*c),
                part: Some("glifo".into()),
                layer: Some(glyph_layer),
                default: Some(RegionShape::Rect { rect: def.cell_rect(i) }),
                shapes: BTreeMap::new(),
                color: None,
            });
            regions += 1;
        }
    }
    s.doc.model.sprites[si].extras.texto.font = Some(def.clone());
    let sp = sprite(&s.doc, si);
    let sheet = compose_frame(sp, s.doc.images(), frame, &ComposeOptions { include_reference: true, ..Default::default() });
    let mut out = Outcome {
        data: json!({
            "sprite": sp.id, "name": sp.name, "size": [w, h], "cell": cell, "columns": def.columns, "rows": def.rows(),
            "layer": glyph_layer, "frame": frame, "regions": regions, "font": def,
            "prefilled_from": base.map(|f| f.name.clone()), "prefilled_glyphs": prefilled,
        }),
        changes: vec![structure_change(s, si)],
        ..Default::default()
    };
    let maxp = ctx.limits.max_preview_side;
    if let Ok(scale) = check_scale(w, h, auto_scale(w, h, maxp).min(4), maxp) {
        out.images.push(render::png(format!("modelo de fonte {}", sp.name), &preview(&sheet, scale, Background::Checker, false))?);
    }
    Ok(out)
}

// ---- exports --------------------------------------------------------------------------------

fn write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    write_atomic(path, bytes).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível gravar {}: {e}", path.display())))
}

fn png_bytes(img: &Image) -> Result<Vec<u8>, CommandError> {
    encode_png(img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

fn check_export_scale(scale: u32) -> Result<u32, CommandError> {
    if scale == 0 || scale > 16 {
        return Err(CommandError::invalid(format!("escala {scale} inválida (1 a 16)")));
    }
    Ok(scale)
}

/// Enlarged copy of an exported image for the agent, when it fits the preview limit.
fn push_preview(out: &mut Outcome, label: &str, img: &Image, ctx: &ExecContext) -> Result<(), CommandError> {
    let max = ctx.limits.max_preview_side;
    if img.width() <= max && img.height() <= max {
        let scale = auto_scale(img.width(), img.height(), max).min(4);
        out.images.push(render::png(label, &preview(img, scale, Background::Checker, false))?);
    }
    Ok(())
}

pub(crate) fn export_bitmap_font(s: &mut Session, a: ExportBitmapFont, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let target_si = sprite_index(&s.doc, a.target.sprite.as_ref());
    let r = match (&a.font, &target_si) {
        (Some(f), _) => f.clone(),
        (None, Ok(si)) => ItemRef::Id(sprite(&s.doc, *si).id),
        (None, Err(e)) => return Err(e.clone()),
    };
    let font = load_font(&s.doc, &r)?;
    let scale = check_export_scale(a.scale)?;
    if a.spacing > 16 {
        return Err(CommandError::invalid("`spacing` de 0 a 16"));
    }
    let palette = target_si.map(|si| palette_colors(&s.doc, si)).unwrap_or_default();
    let color = match &a.color {
        Some(c) => Some(c.resolve(&palette)?),
        None if font.builtin => Some(Color::WHITE),
        None => None,
    };
    let fnt_path = ctx.paths.resolve(&a.path)?;
    let png_path = fnt_path.with_extension("png");
    let face = a.face.clone().unwrap_or_else(|| font.name.clone());
    let opts =
        sprite_project::texto::BmFontOptions { face: face.clone(), page_file: file_name(&png_path), scale, color, spacing: a.spacing };
    let bm = sprite_project::texto::bitmap_font(&font, &opts)?;
    write(&png_path, &png_bytes(&bm.atlas)?)?;
    write(&fnt_path, bm.text.as_bytes())?;
    let mut out = Outcome::data(json!({
        "fnt": fnt_path.display().to_string(), "png": png_path.display().to_string(), "face": face,
        "chars": bm.chars.len(), "atlas_size": [bm.atlas.width(), bm.atlas.height()],
        "line_height": font.line_height * scale, "base": font.base * scale, "scale": scale,
    }));
    push_preview(&mut out, &format!("atlas da fonte {}", font.name), &bm.atlas, ctx)?;
    Ok(out)
}

// ---- 9-slice --------------------------------------------------------------------------------

pub(crate) fn nine_slice_set(s: &mut Session, a: NineSliceSet) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    if a.clear {
        let had = s.doc.model.sprites[si].extras.texto.nine_slice.take().is_some();
        let sp = sprite(&s.doc, si);
        return Ok(Outcome {
            data: json!({ "sprite": sp.id, "cleared": had }),
            changes: vec![structure_change(s, si)],
            ..Default::default()
        });
    }
    let sp = sprite(&s.doc, si);
    let cur = sp.extras.texto.nine_slice.unwrap_or_default();
    let base = a.all.map(|v| NineSlice { left: v, top: v, right: v, bottom: v }).unwrap_or(cur);
    let m = NineSlice {
        left: a.left.unwrap_or(base.left),
        top: a.top.unwrap_or(base.top),
        right: a.right.unwrap_or(base.right),
        bottom: a.bottom.unwrap_or(base.bottom),
    };
    m.validate(sp.width, sp.height)?;
    let (w, h) = (sp.width, sp.height);
    s.doc.model.sprites[si].extras.texto.nine_slice = Some(m);
    Ok(Outcome {
        data: json!({ "sprite": sprite(&s.doc, si).id, "margins": m, "center": [w - m.left - m.right, h - m.top - m.bottom] }),
        changes: vec![structure_change(s, si)],
        ..Default::default()
    })
}

/// Dashed guide lines on a preview: vertical at `xs`, horizontal at `ys` (preview pixels).
fn guide_lines(out: &mut Image, xs: &[i32], ys: &[i32], color: Color) {
    let (w, h) = (out.width() as i32, out.height() as i32);
    let dash = |i: i32| if (i / 3) % 2 == 0 { color } else { Color::BLACK };
    for &x in xs {
        let x = x.clamp(0, w - 1);
        for y in 0..h {
            out.set(x, y, dash(y));
        }
    }
    for &y in ys {
        let y = y.clamp(0, h - 1);
        for x in 0..w {
            out.set(x, y, dash(x));
        }
    }
}

const GUIDE: Color = Color::rgb(255, 64, 200);

fn margin_lines(m: NineSlice, w: u32, h: u32, scale: u32, origin: Point) -> (Vec<i32>, Vec<i32>) {
    let s = scale as i32;
    let xs = vec![(m.left as i32 - origin.x) * s, (w as i32 - m.right as i32 - origin.x) * s];
    let ys = vec![(m.top as i32 - origin.y) * s, (h as i32 - m.bottom as i32 - origin.y) * s];
    (xs, ys)
}

/// Draws the 9-slice guides of `sp` on a frame preview (`rect` is the previewed area).
pub(crate) fn nine_slice_overlay(sp: &Sprite, rect: Rect, scale: u32, out: &mut Image, legend: &mut Vec<serde_json::Value>) {
    let Some(m) = sp.extras.texto.nine_slice else { return };
    let (xs, ys) = margin_lines(m, sp.width, sp.height, scale, Point::new(rect.x, rect.y));
    guide_lines(out, &xs, &ys, GUIDE);
    legend.push(json!({ "nine_slice": m, "color": GUIDE }));
}

pub(crate) fn nine_slice_render(s: &mut Session, a: NineSliceRender, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = frame_id(sp, a.frame.as_ref())?;
    let m = a.margins.or(sp.extras.texto.nine_slice).ok_or_else(|| {
        CommandError::invalid(format!("o sprite \"{}\" não tem 9-slice: defina com nine_slice_set (ou passe `margins`)", sp.name))
    })?;
    let max = ctx.limits.max_preview_side;
    if a.width == 0 || a.height == 0 || a.width > max || a.height > max {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("tamanho {}x{} inválido (1 a {max} px por lado)", a.width, a.height),
        ));
    }
    let src = compose_frame(sp, s.doc.images(), frame, &ComposeOptions::default());
    let center = a.center.unwrap_or(a.edges);
    let img = nine_slice::render(&src, m, a.width, a.height, a.edges, center)?;
    let scale = check_scale(a.width, a.height, a.scale.unwrap_or_else(|| auto_scale(a.width, a.height, max)), max)?;
    let mut big = preview(&img, scale, a.background, false);
    if a.guides {
        let (xs, ys) = margin_lines(m, a.width, a.height, scale, Point::default());
        guide_lines(&mut big, &xs, &ys, GUIDE);
    }
    let mut out = Outcome::data(json!({
        "sprite": sp.id, "frame": frame, "size": [a.width, a.height], "margins": m, "edges": a.edges, "center": center,
        "scale": scale, "source_size": [sp.width, sp.height],
    }));
    out.images.push(render::png(format!("9-slice {}x{} (escala {scale}x)", a.width, a.height), &big)?);
    Ok(out)
}

pub(crate) fn export_nine_slice(s: &mut Session, a: ExportNineSlice, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let frame = frame_id(sp, a.frame.as_ref())?;
    let m =
        sp.extras.texto.nine_slice.ok_or_else(|| {
            CommandError::invalid(format!("o sprite \"{}\" não tem 9-slice: defina as margens com nine_slice_set", sp.name))
        })?;
    m.validate(sp.width, sp.height)?;
    let scale = check_export_scale(a.scale)?;
    let tex = scale_image(&compose_frame(sp, s.doc.images(), frame, &ComposeOptions::default()), scale);
    let ms = m.scaled(scale);
    let size = a.size.unwrap_or([tex.width() * 3, tex.height() * 3]);
    if size[0] < ms.left + ms.right || size[1] < ms.top + ms.bottom || size[0] > 16_384 || size[1] > 16_384 {
        return Err(CommandError::invalid(format!(
            "`size` {}x{} precisa caber os cantos ({}x{}) e ter no máximo 16384 px",
            size[0],
            size[1],
            ms.left + ms.right,
            ms.top + ms.bottom
        )));
    }
    let tscn = ctx.paths.resolve(&a.path)?;
    let png_path = tscn.with_extension("png");
    let json_path = tscn.with_extension("json");
    let png_name = file_name(&png_path);
    let res_path = a.res_path.clone().unwrap_or_else(|| format!("res://{png_name}"));
    let node = sprite_project::texto::godot_node_name(a.node_name.as_deref().unwrap_or(&sp.name));
    let (hm, vm) = (a.axis_stretch_horizontal, a.axis_stretch_vertical);
    let scene = sprite_project::texto::nine_patch_scene(&node, &res_path, ms, size, hm, vm);
    let meta = json!({
        "texture": png_name, "texture_res_path": res_path, "texture_size": [tex.width(), tex.height()], "scale": scale,
        "margins": ms, "axis_stretch_horizontal": hm, "axis_stretch_vertical": vm,
        "godot_axis_stretch": [hm.godot_axis_stretch(), vm.godot_axis_stretch()], "node": node, "node_size": size,
    });
    write(&png_path, &png_bytes(&tex)?)?;
    write(&tscn, scene.as_bytes())?;
    write(&json_path, serde_json::to_string_pretty(&meta).unwrap_or_default().as_bytes())?;
    let mut out = Outcome::data(json!({
        "tscn": tscn.display().to_string(), "png": png_path.display().to_string(), "json": json_path.display().to_string(),
        "node": node, "texture_res_path": res_path, "margins": ms, "node_size": size,
    }));
    if size[0] <= ctx.limits.max_preview_side && size[1] <= ctx.limits.max_preview_side {
        let shown = nine_slice::render_axes(&tex, ms, size[0], size[1], hm, vm)?;
        push_preview(&mut out, &format!("NinePatchRect {}x{}", size[0], size[1]), &shown, ctx)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
