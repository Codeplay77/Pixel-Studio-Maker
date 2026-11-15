//! Command implementations, grouped by area.

mod draw;
mod ferramentas;
pub(crate) mod godot;
mod inspect;
mod io;
pub(crate) mod paletas;
mod rpgmaker;
pub(crate) mod structure;
pub(crate) mod texto;
pub(crate) mod tiles;

use crate::{Command, CommandError, ErrorCode, ExecContext, Outcome, Session};

/// History label for a write command.
pub(crate) fn label(cmd: &Command) -> String {
    match cmd {
        Command::DrawBatch(a) => a.label.clone().unwrap_or_else(|| format!("draw_batch ({} ops)", a.ops.len())),
        other => other.name().to_string(),
    }
}

pub(crate) fn run(s: &mut Session, cmd: Command, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    use Command as C;
    match cmd {
        C::ProjectInfo(a) => inspect::project_info(s, a),
        C::ProjectValidate(a) => inspect::validate(s, a),
        C::SpriteCreate(a) => structure::sprite_create(s, a, ctx),
        C::SpriteUpdate(a) => structure::sprite_update(s, a, ctx),
        C::ReferenceAdd(a) => io::reference_add(s, a, ctx),
        C::ReferenceUpdate(a) => io::reference_update(s, a),
        C::SnapshotCreate(a) => io::snapshot_create(s, a),
        C::SnapshotRestore(a) => io::snapshot_restore(s, a),
        C::SnapshotList(_) => inspect::snapshot_list(s),
        C::PixelGet(a) => draw::pixel_get(s, a),
        C::PixelSet(a) => draw::pixel_set(s, a),
        C::PixelSetBatch(a) => draw::pixel_set_batch(s, a),
        C::PixelClear(a) => draw::pixel_clear(s, a),
        C::PixelReadRegion(a) => draw::pixel_read_region(s, a),
        C::PixelWriteRegion(a) => draw::pixel_write_region(s, a),
        C::PixelApplyPatch(a) => draw::apply_patch(s, a.target, a.patch),
        C::FrameApplyPatch(a) => draw::apply_patch(s, a.target, a.patch),
        C::DrawBatch(a) => draw::draw_batch(s, a, ctx),
        C::DrawLine(a) => draw::draw_line(s, a),
        C::DrawShape(a) => draw::draw_shape(s, a),
        C::DrawPolygon(a) => draw::draw_polygon(s, a),
        C::DrawFill(a) => draw::draw_fill(s, a),
        C::DrawOutline(a) => draw::draw_outline(s, a),
        C::DrawDither(a) => draw::draw_dither(s, a),
        C::DrawGradient(a) => draw::draw_gradient(s, a),
        C::DrawShade(a) => draw::draw_shade(s, a),
        C::DrawTransform(a) => draw::draw_transform(s, a),
        C::DrawReplaceColor(a) => draw::draw_replace_color(s, a),
        C::RegionCreate(a) => structure::region_create(s, a),
        C::RegionUpdate(a) => structure::region_update(s, a),
        C::RegionList(a) => inspect::region_list(s, a),
        C::RegionCopy(a) => draw::region_copy(s, a),
        C::RegionTransform(a) => draw::region_transform(s, a),
        C::AnchorCreate(a) => structure::anchor_create(s, a),
        C::AnchorUpdate(a) => structure::anchor_update(s, a),
        C::AnchorList(a) => inspect::anchor_list(s, a),
        C::PoseCreate(a) => structure::pose_create(s, a),
        C::PoseUpdate(a) => structure::pose_update(s, a),
        C::PoseList(a) => inspect::pose_list(s, a),
        C::LayerCreate(a) => structure::layer_create(s, a, ctx),
        C::LayerDelete(a) => structure::layer_delete(s, a),
        C::LayerDuplicate(a) => structure::layer_duplicate(s, a, ctx),
        C::LayerReorder(a) => structure::layer_reorder(s, a),
        C::LayerMerge(a) => structure::layer_merge(s, a),
        C::LayerProperties(a) => structure::layer_properties(s, a),
        C::AnimationCreate(a) => structure::animation_create(s, a, ctx),
        C::AnimationDelete(a) => structure::animation_delete(s, a),
        C::AnimationInfo(a) => inspect::animation_info(s, a),
        C::AnimationUpdate(a) => structure::animation_update(s, a),
        C::AnimationSetDirection(a) => structure::animation_set_direction(s, a, ctx),
        C::AnimationSetTiming(a) => structure::animation_set_timing(s, a),
        C::AnimationPreview(a) => inspect::animation_preview(s, a, ctx),
        C::DirectionCreate(a) => structure::direction_create(s, a),
        C::DirectionUpdate(a) => structure::direction_update(s, a),
        C::FrameCreate(a) => structure::frame_create(s, a, ctx),
        C::FrameDuplicate(a) => structure::frame_duplicate(s, a, ctx),
        C::FrameDelete(a) => structure::frame_delete(s, a),
        C::FrameMove(a) => structure::frame_move(s, a),
        C::FrameEdit(a) => draw::frame_edit(s, a, ctx),
        C::FrameCompare(a) => inspect::frame_compare(s, a, ctx),
        C::FramePreview(a) => inspect::frame_preview(s, a, ctx),
        C::FrameInspect(a) => inspect::frame_inspect(s, a),
        C::PaletteCreate(a) => io::palette_create(s, a),
        C::PaletteUpdate(a) => io::palette_update(s, a),
        C::PaletteReplace(a) => io::palette_replace(s, a),
        C::PaletteQuantize(a) => io::palette_quantize(s, a),
        C::ImportImage(a) => io::import_image(s, a, ctx),
        C::ExportPng(a) => io::export_png(s, a, ctx),
        C::ExportSpritesheet(a) => io::export_spritesheet(s, a, ctx),
        C::ExportFrames(a) => io::export_frames(s, a, ctx),
        C::ExportGif(a) => io::export_gif(s, a, ctx),
        C::ExportGodot(a) => io::export_godot(s, a, ctx),
        C::RpgMakerTemplate(a) => rpgmaker::template(s, a, ctx),
        C::ExportRpgMakerCharacter(a) => rpgmaker::export_character(s, a, ctx),
        C::ExportRpgMakerFaces(a) => rpgmaker::export_faces(s, a, ctx),
        C::ExportRpgMakerBattler(a) => rpgmaker::export_battler(s, a, ctx),
        C::TileGridSet(a) => tiles::tile_grid_set(s, a),
        C::TileInfo(a) => tiles::tile_info(s, a),
        C::TileCollisionSet(a) => tiles::tile_collision_set(s, a),
        C::TileTerrainSet(a) => tiles::tile_terrain_set(s, a),
        C::AutotileGenerate(a) => tiles::autotile_generate(s, a, ctx),
        C::TilePreviewMapSet(a) => tiles::tile_preview_map_set(s, a, ctx),
        C::TilePreviewRender(a) => tiles::tile_preview_render(s, a, ctx),
        C::ExportGodotTileset(a) => tiles::export_godot_tileset(s, a, ctx),
        C::HitboxCreate(a) => godot::hitbox_create(s, a),
        C::HitboxUpdate(a) => godot::hitbox_update(s, a),
        C::HitboxSet(a) => godot::hitbox_set(s, a),
        C::HitboxList(a) => godot::hitbox_list(s, a),
        C::ExportGodotScene(a) => godot::export_godot_scene(s, a, ctx),
        C::ExportJobSet(a) => godot::export_job_set(s, a),
        C::ExportJobDelete(a) => godot::export_job_delete(s, a),
        C::ExportJobList(_) => godot::export_job_list(s, ctx),
        C::ExportJobsRun(a) => godot::export_jobs_run(s, a, ctx),
        C::SelectColor(a) => ferramentas::select_color(s, a),
        C::SelectPolygon(a) => ferramentas::select_polygon(s, a),
        C::SelectionRead(a) => ferramentas::selection_read(s, a),
        C::DrawText(a) => texto::draw_text(s, a),
        C::TextMeasure(a) => texto::text_measure(s, a),
        C::FontDefine(a) => texto::font_define(s, a, ctx),
        C::FontInfo(a) => texto::font_info(s, a, ctx),
        C::FontTemplate(a) => texto::font_template(s, a, ctx),
        C::ExportBitmapFont(a) => texto::export_bitmap_font(s, a, ctx),
        C::NineSliceSet(a) => texto::nine_slice_set(s, a),
        C::NineSliceRender(a) => texto::nine_slice_render(s, a, ctx),
        C::ExportNineSlice(a) => texto::export_nine_slice(s, a, ctx),
        C::PaletteImport(a) => paletas::palette_import(s, a, ctx),
        C::PaletteExport(a) => paletas::palette_export(s, a, ctx),
        C::PaletteVariantSet(a) => paletas::palette_variant_set(s, a),
        C::PaletteVariantDelete(a) => paletas::palette_variant_delete(s, a),
        C::PaletteVariantList(a) => paletas::palette_variant_list(s, a),
        C::PaletteVariantPreview(a) => paletas::palette_variant_preview(s, a, ctx),
        C::ExportPaletteVariants(a) => paletas::export_palette_variants(s, a, ctx),
        C::ExportNormalMap(a) => paletas::export_normal_map(s, a, ctx),
        other => Err(CommandError::new(ErrorCode::Unsupported, format!("{} é tratado pelo runtime", other.name()))),
    }
}

/// Unique-name check shared by layers, animations, regions, anchors, poses, palettes.
pub(crate) fn ensure_unique<'a>(kind: &str, name: &str, mut existing: impl Iterator<Item = &'a str>) -> Result<(), CommandError> {
    if name.trim().is_empty() {
        return Err(CommandError::invalid(format!("{kind} precisa de um nome")));
    }
    if existing.any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(CommandError::invalid(format!("já existe {kind} chamado \"{name}\"")));
    }
    Ok(())
}
