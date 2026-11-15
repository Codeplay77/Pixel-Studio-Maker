//! Tileset commands: tile grid, collisions, terrains, autotile generation, preview map and the
//! Godot 4 TileSet export. The algorithms live in `sprite_drawing::{tiles, autotile}` and
//! `sprite_project::tileset`; this module validates arguments and wires them to the project.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use sprite_animation::Sprite;
use sprite_core::codec::encode_png;
use sprite_core::image::MAX_IMAGE_SIDE;
use sprite_core::{Color, Image, ItemRef, PasteMode, Point, Rect};
use sprite_drawing::autotile::{AutotileOutput, QuarterKind, QuarterSet, blob47_masks};
use sprite_drawing::raster::{polygon_fill, polyline};
use sprite_drawing::tiles::{
    MAX_MAP_SIDE, MAX_POLYGONS, PeeringBit, PeeringBits, Terrain, TerrainMode, TerrainSet, TileCoord, TileData, TileGrid, TileMap,
    TileTerrain, all_tiles, collision_shape, pick_terrain_tile, render_map, tile_is_empty, validate_polygon,
};
use sprite_project::export::scale_image;
use sprite_project::format::write_atomic;
use sprite_project::tileset::{TileSetExport, godot_tileset};

use crate::args::*;
use crate::render::{self, auto_scale, check_scale, overlay_color, preview};
use crate::target::{frame_id, sprite, sprite_index};
use crate::{Change, CommandError, ErrorCode, ExecContext, Outcome, Session};

const MAX_TERRAIN_SETS: usize = 32;
const MAX_TERRAINS: usize = 64;
const GRID_COLOR: Color = Color::rgb(0, 229, 255);
const COLLISION_COLOR: Color = Color::rgb(255, 64, 129);

fn structure_change(s: &Session, si: usize) -> Change {
    Change { sprite: s.doc.model.sprites[si].id, layer: None, frame: None, rect: None, kind: "structure".into() }
}

fn no_grid(sp: &Sprite) -> CommandError {
    CommandError::invalid(format!(
        "o sprite \"{}\" não tem grade de tiles: defina com tile_grid_set (ex.: {{\"tile_width\": 16}})",
        sp.name
    ))
}

fn tile_data(sp: &Sprite) -> Result<&TileData, CommandError> {
    sp.extras.tiles.as_ref().ok_or_else(|| no_grid(sp))
}

fn tile_data_mut(s: &mut Session, si: usize) -> Result<&mut TileData, CommandError> {
    let sp = &mut s.doc.model.sprites[si];
    if sp.extras.tiles.is_none() {
        return Err(no_grid(sp));
    }
    Ok(sp.extras.tiles.as_mut().expect("checked"))
}

fn grid_size(sp: &Sprite, grid: &TileGrid) -> (u32, u32) {
    grid.columns_rows(sp.width, sp.height)
}

fn check_tile(tile: TileCoord, columns: u32, rows: u32) -> Result<(), CommandError> {
    if tile[0] >= columns || tile[1] >= rows {
        return Err(CommandError::new(
            ErrorCode::OutOfBounds,
            format!("o tile [{}, {}] fica fora da grade ({columns} colunas x {rows} linhas)", tile[0], tile[1]),
        ));
    }
    Ok(())
}

fn tiles_of(tile: Option<[u32; 2]>, tiles: &[[u32; 2]]) -> Result<Vec<TileCoord>, CommandError> {
    let mut list: Vec<TileCoord> = tile.into_iter().chain(tiles.iter().copied()).collect();
    list.sort_by_key(|t| (t[1], t[0]));
    list.dedup();
    if list.is_empty() {
        return Err(CommandError::invalid("informe `tile` ([coluna, linha]) ou `tiles`"));
    }
    Ok(list)
}

fn grid_json(sp: &Sprite, d: &TileData) -> serde_json::Value {
    let (c, r) = grid_size(sp, &d.grid);
    json!({
        "tile_width": d.grid.tile_width, "tile_height": d.grid.tile_height,
        "margin": d.grid.margin, "spacing": d.grid.spacing,
        "columns": c, "rows": r, "tiles": c * r, "leftover": d.grid.leftover(sp.width, sp.height),
    })
}

/// Composite of a frame (from the session cache).
fn composite(s: &mut Session, si: usize, frame: u32) -> Result<Image, CommandError> {
    let id = s.doc.model.sprites[si].id;
    s.composite(id, frame).ok_or_else(|| CommandError::not_found(format!("frame {frame} não existe")))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
    write_atomic(path, bytes).map_err(|e| CommandError::new(ErrorCode::Io, format!("não foi possível gravar {}: {e}", path.display())))
}

fn png(img: &Image) -> Result<Vec<u8>, CommandError> {
    encode_png(img).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))
}

// ---- grid -----------------------------------------------------------------------------------

pub(crate) fn tile_grid_set(s: &mut Session, a: TileGridSet) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let (w, h) = (sprite(&s.doc, si).width, sprite(&s.doc, si).height);
    if a.remove {
        let had = s.doc.model.sprites[si].extras.tiles.take().is_some();
        return Ok(Outcome { data: json!({ "removed": had }), changes: vec![structure_change(s, si)], ..Default::default() });
    }
    let current = s.doc.model.sprites[si].extras.tiles.clone();
    let base = current.as_ref().map(|d| d.grid);
    let tw = a
        .tile_width
        .or(base.map(|g| g.tile_width))
        .ok_or_else(|| CommandError::invalid("informe `tile_width` (largura do tile em pixels)"))?;
    let th = match (a.tile_height, a.tile_width, base) {
        (Some(v), _, _) => v,
        (None, Some(_), _) => tw,
        (None, None, Some(g)) => g.tile_height,
        (None, None, None) => tw,
    };
    let margin = a.margin.or(base.map(|g| g.margin)).unwrap_or([0, 0]);
    let spacing = a.spacing.or(base.map(|g| g.spacing)).unwrap_or([0, 0]);
    if tw == 0 || th == 0 {
        return Err(CommandError::invalid("o tile precisa de pelo menos 1x1 pixel"));
    }
    if margin[0].saturating_add(tw) > w || margin[1].saturating_add(th) > h {
        return Err(CommandError::new(
            ErrorCode::OutOfBounds,
            format!("a grade não cabe no sprite {w}x{h}: margem [{}, {}] + tile {tw}x{th} passa do tamanho", margin[0], margin[1]),
        ));
    }
    if spacing[0] > w || spacing[1] > h {
        return Err(CommandError::invalid(format!("espaçamento [{}, {}] maior que o sprite {w}x{h}", spacing[0], spacing[1])));
    }
    let grid = TileGrid { tile_width: tw, tile_height: th, margin, spacing };
    let mut data = current.unwrap_or_else(|| TileData::new(grid));
    data.grid = grid;
    let (columns, rows) = grid.columns_rows(w, h);
    let removed = data.retain_valid(columns, rows);
    let mut warnings = Vec::new();
    let left = grid.leftover(w, h);
    if left != [0, 0] {
        warnings.push(format!("sobram {} px à direita e {} px embaixo que não formam um tile inteiro", left[0], left[1]));
    }
    if removed > 0 {
        warnings.push(format!(
            "{removed} dado(s) de tiles que não cabem na grade nova foram removidos (colisões, terrenos ou células do mapa)"
        ));
    }
    s.doc.model.sprites[si].extras.tiles = Some(data);
    let sp = sprite(&s.doc, si);
    Ok(Outcome {
        data: grid_json(sp, sp.extras.tiles.as_ref().expect("set")),
        changes: vec![structure_change(s, si)],
        warnings,
        ..Default::default()
    })
}

fn terrain_name(d: &TileData, set: u32, terrain: u32) -> serde_json::Value {
    d.terrain_sets.get(set as usize).and_then(|s| s.terrains.get(terrain as usize)).map(|t| json!(t.name)).unwrap_or(json!(terrain))
}

fn terrain_json(d: &TileData, t: &TileTerrain) -> serde_json::Value {
    let bits: BTreeMap<&str, serde_json::Value> = t.peering.iter().map(|(b, x)| (b.name(), terrain_name(d, t.set, x))).collect();
    json!({
        "tile": t.tile,
        "set": d.terrain_sets.get(t.set as usize).map(|s| s.name.clone()),
        "terrain": t.terrain.map(|x| terrain_name(d, t.set, x)),
        "bits": bits,
    })
}

pub(crate) fn tile_info(s: &mut Session, a: TileInfo) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    tile_data(sprite(&s.doc, si))?;
    let frame = frame_id(sprite(&s.doc, si), a.frame.as_ref())?;
    let img = composite(s, si, frame)?;
    let sp = sprite(&s.doc, si);
    let d = tile_data(sp)?;
    let (columns, rows) = grid_size(sp, &d.grid);
    let empty: Vec<TileCoord> = all_tiles(columns, rows).filter(|t| tile_is_empty(&img, &d.grid, *t)).collect();
    let mut data = grid_json(sp, d);
    data["sprite"] = json!(sp.name);
    data["frame"] = json!(frame);
    data["non_empty"] = json!(columns * rows - empty.len() as u32);
    data["empty"] = json!(empty);
    data["collisions"] = json!(d.collisions);
    data["terrain_sets"] = json!(d.terrain_sets);
    data["terrains"] = json!(d.terrains.iter().map(|t| terrain_json(d, t)).collect::<Vec<_>>());
    data["preview_map"] = match &d.preview_map {
        Some(m) => json!({ "width": m.width, "height": m.height, "filled": m.cells.iter().filter(|c| c.is_some()).count() }),
        None => serde_json::Value::Null,
    };
    Ok(Outcome::data(data))
}

// ---- collisions -----------------------------------------------------------------------------

pub(crate) fn tile_collision_set(s: &mut Session, a: TileCollisionSet) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let d = tile_data(sp)?;
    let (columns, rows) = grid_size(sp, &d.grid);
    let (tw, th) = (d.grid.tile_width, d.grid.tile_height);
    let tiles = tiles_of(a.tile, &a.tiles)?;
    for t in &tiles {
        check_tile(*t, columns, rows)?;
    }
    let polygons: Vec<Vec<Point>> = if a.clear {
        if a.shape.is_some() || !a.polygons.is_empty() {
            return Err(CommandError::invalid("`clear` remove a colisão: não combine com `shape` ou `polygons`"));
        }
        Vec::new()
    } else {
        let mut list: Vec<Vec<Point>> = a.shape.map(|sh| collision_shape(sh, tw, th)).into_iter().collect();
        list.extend(a.polygons.iter().cloned());
        if list.is_empty() {
            return Err(CommandError::invalid("informe `shape` (full, top_half...), `polygons` ou `clear`"));
        }
        for (i, p) in list.iter().enumerate() {
            validate_polygon(p, tw, th).map_err(|e| CommandError::invalid(format!("polígono {i}: {e}")))?;
        }
        list
    };
    let d = tile_data_mut(s, si)?;
    for t in &tiles {
        let mut list = if a.add { d.collision(*t).map(|c| c.polygons.clone()).unwrap_or_default() } else { Vec::new() };
        list.extend(polygons.iter().cloned());
        if list.len() > MAX_POLYGONS {
            return Err(CommandError::new(ErrorCode::LimitExceeded, format!("no máximo {MAX_POLYGONS} polígonos por tile")));
        }
        d.set_collision(*t, list);
    }
    let result: Vec<serde_json::Value> =
        tiles.iter().map(|t| json!({ "tile": t, "polygons": d.collision(*t).map(|c| c.polygons.clone()).unwrap_or_default() })).collect();
    let total = d.collisions.len();
    Ok(Outcome {
        data: json!({ "tiles": result, "tiles_with_collision": total }),
        changes: vec![structure_change(s, si)],
        ..Default::default()
    })
}

// ---- terrains -------------------------------------------------------------------------------

/// Names compare without case, accents included ("Água" = "água").
fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

fn find_set(d: &TileData, r: &ItemRef) -> Result<usize, CommandError> {
    let by_index = |i: u32| (i as usize) < d.terrain_sets.len();
    match r {
        ItemRef::Id(i) if by_index(*i) => Ok(*i as usize),
        ItemRef::Name(n) if d.terrain_sets.iter().any(|s| same_name(&s.name, n)) => {
            Ok(d.terrain_sets.iter().position(|s| same_name(&s.name, n)).expect("any"))
        }
        ItemRef::Name(n) if n.parse::<u32>().is_ok_and(by_index) => Ok(n.parse::<usize>().expect("parsed")),
        _ => Err(CommandError::not_found(format!(
            "conjunto de terreno {r} não existe (conjuntos: {})",
            d.terrain_sets.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn find_terrain(set: &TerrainSet, r: &ItemRef) -> Result<u32, CommandError> {
    let by_index = |i: u32| (i as usize) < set.terrains.len();
    match r {
        ItemRef::Id(i) if by_index(*i) => Ok(*i),
        ItemRef::Name(n) if set.terrains.iter().any(|t| same_name(&t.name, n)) => {
            Ok(set.terrains.iter().position(|t| same_name(&t.name, n)).expect("any") as u32)
        }
        ItemRef::Name(n) if n.parse::<u32>().is_ok_and(by_index) => Ok(n.parse().expect("parsed")),
        _ => Err(CommandError::not_found(format!(
            "terreno {r} não existe no conjunto \"{}\" (terrenos: {})",
            set.name,
            set.terrains.iter().map(|t| t.name.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

pub(crate) fn tile_terrain_set(s: &mut Session, a: TileTerrainSet) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let (columns, rows) = grid_size(sp, &tile_data(sp)?.grid);
    if a.sets.is_empty() && a.remove_sets.is_empty() && a.tiles.is_empty() {
        return Err(CommandError::invalid("nada a fazer: informe `sets`, `remove_sets` ou `tiles`"));
    }
    let d = tile_data_mut(s, si)?;
    let mut warnings = Vec::new();
    // Removed sets: tiles of later sets shift down.
    let mut removed: Vec<usize> = a.remove_sets.iter().map(|r| find_set(d, r)).collect::<Result<_, _>>()?;
    removed.sort_unstable();
    removed.dedup();
    for idx in removed.into_iter().rev() {
        d.terrain_sets.remove(idx);
        d.terrains.retain(|t| t.set as usize != idx);
        for t in &mut d.terrains {
            if t.set as usize > idx {
                t.set -= 1;
            }
        }
    }
    for set in &a.sets {
        let name = set.name.trim();
        if name.is_empty() {
            return Err(CommandError::invalid("o conjunto de terreno precisa de um nome"));
        }
        if set.terrains.len() > MAX_TERRAINS {
            return Err(CommandError::new(ErrorCode::LimitExceeded, format!("no máximo {MAX_TERRAINS} terrenos por conjunto")));
        }
        let mut terrains: Vec<Terrain> = Vec::new();
        for (j, t) in set.terrains.iter().enumerate() {
            let tn = t.name.trim();
            if tn.is_empty() {
                return Err(CommandError::invalid(format!("o terreno {j} do conjunto \"{name}\" precisa de um nome")));
            }
            if terrains.iter().any(|x| same_name(&x.name, tn)) {
                return Err(CommandError::invalid(format!("terreno \"{tn}\" repetido no conjunto \"{name}\"")));
            }
            terrains.push(Terrain { name: tn.to_string(), color: t.color.unwrap_or(overlay_color(j)) });
        }
        let new = TerrainSet { name: name.to_string(), mode: set.mode, terrains };
        match d.terrain_sets.iter().position(|x| same_name(&x.name, name)) {
            Some(i) => d.terrain_sets[i] = new,
            None => d.terrain_sets.push(new),
        }
    }
    if d.terrain_sets.len() > MAX_TERRAIN_SETS {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("no máximo {MAX_TERRAIN_SETS} conjuntos de terreno")));
    }
    let pruned = d.prune_terrains();
    if pruned > 0 {
        warnings.push(format!("{pruned} referência(s) a terrenos que não existem mais foram removidas dos tiles"));
    }
    let mut updated = Vec::new();
    for entry in &a.tiles {
        let tiles = tiles_of(entry.tile, &entry.tiles)?;
        for t in &tiles {
            check_tile(*t, columns, rows)?;
        }
        if entry.clear {
            for t in &tiles {
                d.set_terrain(*t, None);
            }
            updated.extend(tiles);
            continue;
        }
        if d.terrain_sets.is_empty() {
            return Err(CommandError::invalid("o sprite não tem conjuntos de terreno: crie um em `sets` (nome, modo e terrenos)"));
        }
        let set_index = match &entry.set {
            Some(r) => find_set(d, r)?,
            None => 0,
        };
        let set = d.terrain_sets[set_index].clone();
        let terrain = entry.terrain.as_ref().map(|r| find_terrain(&set, r)).transpose()?;
        let mut peering = PeeringBits::default();
        let check_mode = |bit: PeeringBit| -> Result<(), CommandError> {
            if set.mode.allows(bit) {
                Ok(())
            } else {
                Err(CommandError::invalid(format!(
                    "o bit {} não vale no modo {} do conjunto \"{}\"",
                    bit.name(),
                    set.mode.name(),
                    set.name
                )))
            }
        };
        if !entry.bits.is_empty() {
            let t = terrain.ok_or_else(|| CommandError::invalid("`bits` usa o terreno do próprio tile: informe `terrain`"))?;
            for bit in &entry.bits {
                check_mode(*bit)?;
                peering.set(*bit, Some(t));
            }
        }
        for (bit, r) in entry.bit_terrains.iter() {
            check_mode(bit)?;
            peering.set(bit, Some(find_terrain(&set, r)?));
        }
        if terrain.is_none() && peering.is_empty() {
            return Err(CommandError::invalid("nada a definir no tile: informe `terrain`, `bits`, `bit_terrains` ou `clear`"));
        }
        for t in &tiles {
            d.set_terrain(*t, Some(TileTerrain { tile: *t, set: set_index as u32, terrain, peering }));
        }
        updated.extend(tiles);
    }
    let d = sprite(&s.doc, si).extras.tiles.as_ref().expect("checked");
    let tiles: Vec<serde_json::Value> =
        updated.iter().map(|t| d.terrain(*t).map(|x| terrain_json(d, x)).unwrap_or(json!({ "tile": t, "terrain": null }))).collect();
    Ok(Outcome {
        data: json!({ "terrain_sets": d.terrain_sets, "tiles": tiles, "tiles_with_terrain": d.terrains.len() }),
        changes: vec![structure_change(s, si)],
        warnings,
        ..Default::default()
    })
}

// ---- autotile -------------------------------------------------------------------------------

/// Most common opaque color of the interior pieces (ties: the smallest color), for the terrain.
fn dominant_color(set: &QuarterSet) -> Option<Color> {
    let mut counts: BTreeMap<Color, u32> = BTreeMap::new();
    for q in 0..4 {
        for (c, n) in set.piece(q, QuarterKind::Interior).color_counts() {
            if c.is_opaque() {
                *counts.entry(c).or_insert(0) += n;
            }
        }
    }
    counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(c, _)| c)
}

/// A small island with a bay, a hole and a one-tile path: shows most blob tiles at once.
const DEMO_MAP: [&str; 6] = ["........", ".####...", ".#.####.", ".####.#.", "...##.#.", "........"];

fn paint_terrain(d: &TileData, map: &mut TileMap, rows: &[String], at: (u32, u32), set: u32, terrain: u32) -> Result<usize, CommandError> {
    let grid: Vec<Vec<bool>> = rows.iter().map(|r| r.chars().map(|c| c != '.' && c != ' ').collect()).collect();
    let is_terrain = |x: i32, y: i32| -> bool {
        y >= 0 && (y as usize) < grid.len() && x >= 0 && grid[y as usize].get(x as usize).copied().unwrap_or(false)
    };
    let mut painted = 0;
    for (y, row) in grid.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let tile = if *cell {
                let (x, y) = (x as i32, y as i32);
                let pick = pick_terrain_tile(d, set, terrain, |b| {
                    let (dx, dy) = b.offset();
                    is_terrain(x + dx, y + dy)
                });
                painted += 1;
                Some(pick.ok_or_else(|| {
                    CommandError::invalid(
                        "nenhum tile tem esse terreno no centro: defina com tile_terrain_set ou gere tiles com autotile_generate",
                    )
                })?)
            } else {
                None
            };
            map.set(at.0 + x as u32, at.1 + y as u32, tile);
        }
    }
    Ok(painted)
}

pub(crate) fn autotile_generate(s: &mut Session, a: AutotileGenerate, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let grid = tile_data(sp)?.grid;
    let (columns, rows) = grid_size(sp, &grid);
    let frame = frame_id(sp, a.frame.as_ref())?;
    let source_name = sp.name.clone();
    let source_palette = sp.palette.clone();
    let (bw, bh) = a.input.size();
    let [sx, sy] = a.source;
    if sx + bw > columns || sy + bh > rows {
        return Err(CommandError::new(
            ErrorCode::OutOfBounds,
            format!("a entrada precisa de {bw}x{bh} tiles a partir de [{sx}, {sy}], mas a grade tem {columns} colunas x {rows} linhas"),
        ));
    }
    if a.output.is_empty() {
        return Err(CommandError::invalid("informe ao menos uma saída em `output`: rpgmaker_a2 e/ou blob_47"));
    }
    let (tw, th) = (grid.tile_width, grid.tile_height);
    let img = composite(s, si, frame)?;
    let mut block = Image::new(bw * tw, bh * th);
    for dy in 0..bh {
        for dx in 0..bw {
            let tile = img.crop(grid.tile_rect([sx + dx, sy + dy]));
            block.paste(&tile, Point::new((dx * tw) as i32, (dy * th) as i32), PasteMode::Replace);
        }
    }
    if block.is_blank() {
        return Err(CommandError::invalid(format!("a área de entrada ({bw}x{bh} tiles a partir de [{sx}, {sy}]) está vazia")));
    }
    let set = QuarterSet::from_block(a.input, &block, tw, th).map_err(CommandError::invalid)?;
    let want_a2 = a.output.contains(&AutotileOutput::RpgmakerA2);
    let want_blob = a.output.contains(&AutotileOutput::Blob47);
    let a2_rows = if want_a2 { 3 } else { 0 };
    let out_cols: u32 = if want_blob { 8 } else { 2 };
    let out_rows = a2_rows + if want_blob { 6 } else { 0 };
    let (w, h) = (out_cols * tw, out_rows * th);
    if w > ctx.limits.max_canvas_side || h > ctx.limits.max_canvas_side {
        return Err(CommandError::new(
            ErrorCode::LimitExceeded,
            format!("o resultado teria {w}x{h} px, acima do limite de {} px por lado", ctx.limits.max_canvas_side),
        ));
    }
    let mut out = Image::new(w, h);
    let mut parts = serde_json::Map::new();
    if want_a2 {
        out.paste(&set.rpgmaker_a2(), Point::new(0, 0), PasteMode::Replace);
        parts.insert("rpgmaker_a2".into(), json!({ "origin": [0, 0], "tiles": [2, 3] }));
    }
    let terrain_name = a.terrain_name.clone().unwrap_or_else(|| source_name.clone());
    if terrain_name.trim().is_empty() {
        return Err(CommandError::invalid("`terrain_name` não pode ser vazio"));
    }
    let color = a.terrain_color.or_else(|| dominant_color(&set)).unwrap_or(overlay_color(0));
    let mut data = TileData::new(TileGrid::new(tw, th));
    data.terrain_sets.push(TerrainSet {
        name: terrain_name.clone(),
        mode: TerrainMode::MatchCornersAndSides,
        terrains: vec![Terrain { name: terrain_name.clone(), color: Color::rgb(color.r, color.g, color.b) }],
    });
    if want_blob {
        let (blob, list) = set.blob47(8);
        out.paste(&blob, Point::new(0, (a2_rows * th) as i32), PasteMode::Replace);
        for ([c, r], mask) in &list {
            let mut peering = PeeringBits::default();
            for bit in PeeringBit::ALL {
                if mask & bit.mask() != 0 {
                    peering.set(bit, Some(0));
                }
            }
            let tile = [*c, r + a2_rows];
            data.set_terrain(tile, Some(TileTerrain { tile, set: 0, terrain: Some(0), peering }));
        }
        parts.insert("blob_47".into(), json!({ "origin": [0, a2_rows], "tiles": [8, 6], "count": list.len(), "masks": blob47_masks() }));
        let mut map = TileMap::new(8, 6);
        let demo: Vec<String> = DEMO_MAP.iter().map(|r| r.to_string()).collect();
        paint_terrain(&data, &mut map, &demo, (0, 0), 0, 0)?;
        data.preview_map = Some(map);
    }
    // The new sprite (or the replaced one, keeping its id and place).
    let name = a.name.clone().unwrap_or_else(|| format!("{source_name}_autotile"));
    if name.trim().is_empty() {
        return Err(CommandError::invalid("o sprite precisa de um nome"));
    }
    let existing = s.doc.model.sprites.iter().position(|x| x.name.eq_ignore_ascii_case(&name));
    let ni = match existing {
        Some(i) if i == si => {
            return Err(CommandError::invalid("o resultado não pode substituir o próprio sprite de origem: use outro `name`"));
        }
        Some(i) if a.replace => {
            let id = s.doc.model.sprites[i].id;
            let lid = s.doc.alloc_id();
            let fid = s.doc.alloc_id();
            let mut ids = [lid, fid].into_iter();
            s.doc.model.sprites[i] = Sprite::new(id, name.clone(), w, h, &mut || ids.next().expect("two ids"));
            i
        }
        Some(_) => {
            return Err(CommandError::invalid(format!("já existe um sprite \"{name}\": escolha outro `name` ou use `replace: true`")));
        }
        None => super::structure::add_sprite(&mut s.doc, &name, w, h),
    };
    let (layer, first) = (s.doc.model.sprites[ni].layers[0].id, s.doc.model.sprites[ni].frames[0].id);
    super::structure::set_cel_image(&mut s.doc, ni, layer, first, out.clone())?;
    let nsp = &mut s.doc.model.sprites[ni];
    nsp.palette = source_palette;
    nsp.extras.tiles = Some(data);
    let new_id = nsp.id;
    let max = ctx.limits.max_preview_side;
    let scale = auto_scale(w, h, max).min(8);
    let mut shown = preview(&out, scale, Background::Checker, false);
    draw_grid(&mut shown, &TileGrid::new(tw, th), out_cols, out_rows, Rect::new(0, 0, w, h), scale);
    let mut result = Outcome {
        data: json!({
            "sprite": new_id, "name": name, "size": [w, h], "tile_size": [tw, th],
            "parts": parts, "terrain": { "set": terrain_name, "name": terrain_name, "color": Color::rgb(color.r, color.g, color.b) },
            "replaced": existing.is_some(),
            "note": "blob_47: tile i (0..46) fica em [i % 8, linha inicial + i / 8], máscara de vizinhos N=1, NE=2, E=4, SE=8, S=16, SW=32, W=64, NW=128 (cantos só contam com os dois lados). O mapa de prévia do sprite novo já mostra o terreno montado (tile_preview_render).",
        }),
        changes: vec![structure_change(s, ni)],
        ..Default::default()
    };
    result.images.push(render::png(format!("autotile {name} (escala {scale}x)"), &shown)?);
    Ok(result)
}

// ---- preview map ----------------------------------------------------------------------------

fn render_preview_map(
    s: &mut Session,
    si: usize,
    frame: u32,
    scale: Option<u32>,
    background: Background,
    grid: bool,
    ctx: &ExecContext,
) -> Result<(Image, serde_json::Value), CommandError> {
    let img = composite(s, si, frame)?;
    let sp = sprite(&s.doc, si);
    let d = tile_data(sp)?;
    let map =
        d.preview_map.as_ref().ok_or_else(|| CommandError::invalid("o sprite não tem mapa de prévia: crie com tile_preview_map_set"))?;
    let (w, h) = (map.width * d.grid.tile_width, map.height * d.grid.tile_height);
    if w > MAX_IMAGE_SIDE || h > MAX_IMAGE_SIDE {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("o mapa teria {w}x{h} px (máximo {MAX_IMAGE_SIDE} por lado)")));
    }
    let raw = render_map(&img, &d.grid, map);
    let max = ctx.limits.max_preview_side;
    let scale = check_scale(w, h, scale.unwrap_or_else(|| auto_scale(w, h, max)), max)?;
    let mut out = preview(&raw, scale, background, false);
    if grid {
        draw_grid(&mut out, &TileGrid::new(d.grid.tile_width, d.grid.tile_height), map.width, map.height, Rect::new(0, 0, w, h), scale);
    }
    let filled = map.cells.iter().filter(|c| c.is_some()).count();
    let info = json!({ "map_size": [map.width, map.height], "filled": filled, "empty": map.cells.len() - filled, "image_size": [out.width(), out.height()], "scale": scale });
    Ok((out, info))
}

pub(crate) fn tile_preview_map_set(s: &mut Session, a: TilePreviewMapSet, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let sp = sprite(&s.doc, si);
    let d = tile_data(sp)?;
    let (columns, rows) = grid_size(sp, &d.grid);
    if a.delete {
        let had = tile_data_mut(s, si)?.preview_map.take().is_some();
        return Ok(Outcome { data: json!({ "deleted": had }), changes: vec![structure_change(s, si)], ..Default::default() });
    }
    let check_side = |w: u32, h: u32| -> Result<(), CommandError> {
        if w == 0 || h == 0 || w > MAX_MAP_SIDE || h > MAX_MAP_SIDE {
            return Err(CommandError::new(
                ErrorCode::LimitExceeded,
                format!("mapa {w}x{h} inválido: de 1 a {MAX_MAP_SIDE} células por lado"),
            ));
        }
        Ok(())
    };
    let mut map = match (a.size, &d.preview_map) {
        (Some([w, h]), Some(m)) => {
            check_side(w, h)?;
            m.resized(w, h)
        }
        (Some([w, h]), None) => {
            check_side(w, h)?;
            TileMap::new(w, h)
        }
        (None, Some(m)) => m.clone(),
        (None, None) if !a.terrain_rows.is_empty() => {
            let (w, h) = (a.terrain_rows.iter().map(|r| r.chars().count() as u32).max().unwrap_or(0), a.terrain_rows.len() as u32);
            check_side(w, h)?;
            TileMap::new(w, h)
        }
        (None, None) => return Err(CommandError::invalid("informe `size` [largura, altura] do mapa na primeira vez")),
    };
    let check = |t: TileCoord| check_tile(t, columns, rows);
    if let Some(cells) = &a.cells {
        if cells.len() != (map.width * map.height) as usize {
            return Err(CommandError::invalid(format!(
                "`cells` tem {} itens, mas o mapa {}x{} precisa de {}",
                cells.len(),
                map.width,
                map.height,
                map.width * map.height
            )));
        }
        for t in cells.iter().flatten() {
            check(*t)?;
        }
        map.cells = cells.clone();
    }
    if a.clear {
        map.cells.iter_mut().for_each(|c| *c = None);
    }
    if let Some(t) = a.fill {
        check(t)?;
        map.cells.iter_mut().for_each(|c| *c = Some(t));
    }
    let mut painted = 0;
    if !a.terrain_rows.is_empty() {
        if a.terrain_rows.len() as u32 > map.height || a.terrain_rows.iter().any(|r| r.chars().count() as u32 > map.width) {
            return Err(CommandError::new(ErrorCode::OutOfBounds, format!("`terrain_rows` passa do mapa {}x{}", map.width, map.height)));
        }
        if d.terrain_sets.is_empty() {
            return Err(CommandError::invalid("o sprite não tem terrenos: use tile_terrain_set ou autotile_generate"));
        }
        let set = match &a.terrain_set {
            Some(r) => find_set(d, r)?,
            None => 0,
        };
        let terrain = match &a.terrain {
            Some(r) => find_terrain(&d.terrain_sets[set], r)?,
            None => 0,
        };
        painted = paint_terrain(d, &mut map, &a.terrain_rows, (0, 0), set as u32, terrain)?;
    }
    for cell in &a.paint {
        if cell.at[0] >= map.width || cell.at[1] >= map.height {
            return Err(CommandError::new(
                ErrorCode::OutOfBounds,
                format!("célula [{}, {}] fora do mapa {}x{}", cell.at[0], cell.at[1], map.width, map.height),
            ));
        }
        if let Some(t) = cell.tile {
            check(t)?;
        }
        map.set(cell.at[0], cell.at[1], cell.tile);
    }
    tile_data_mut(s, si)?.preview_map = Some(map);
    let frame = frame_id(sprite(&s.doc, si), None)?;
    let (img, mut info) = render_preview_map(s, si, frame, None, Background::Checker, false, ctx)?;
    info["terrain_cells"] = json!(painted);
    let mut out = Outcome { data: info, changes: vec![structure_change(s, si)], ..Default::default() };
    out.images.push(render::png("mapa de prévia", &img)?);
    Ok(out)
}

pub(crate) fn tile_preview_render(s: &mut Session, a: TilePreviewRender, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    let frame = frame_id(sprite(&s.doc, si), a.frame.as_ref())?;
    let (img, mut info) = render_preview_map(s, si, frame, a.scale, a.background, a.grid, ctx)?;
    info["frame"] = json!(frame);
    let mut out = Outcome::data(info);
    out.images.push(render::png(format!("mapa de prévia ({}x{})", img.width(), img.height()), &img)?);
    Ok(out)
}

// ---- Godot export ---------------------------------------------------------------------------

pub(crate) fn export_godot_tileset(s: &mut Session, a: ExportGodotTileset, ctx: &ExecContext) -> Result<Outcome, CommandError> {
    let si = sprite_index(&s.doc, a.target.sprite.as_ref())?;
    tile_data(sprite(&s.doc, si))?;
    let frame = frame_id(sprite(&s.doc, si), a.frame.as_ref())?;
    let img = composite(s, si, frame)?;
    let sp = sprite(&s.doc, si);
    let d = tile_data(sp)?;
    let scale = a.scale.max(1);
    if scale > 64 || sp.width * scale > MAX_IMAGE_SIDE || sp.height * scale > MAX_IMAGE_SIDE {
        return Err(CommandError::new(ErrorCode::LimitExceeded, format!("escala grande demais (máximo {MAX_IMAGE_SIDE} px por lado)")));
    }
    let (columns, rows) = grid_size(sp, &d.grid);
    let tiles: Vec<TileCoord> = all_tiles(columns, rows).filter(|t| !tile_is_empty(&img, &d.grid, *t)).collect();
    let mut warnings = Vec::new();
    let skipped: Vec<TileCoord> = d
        .collisions
        .iter()
        .map(|c| c.tile)
        .chain(d.terrains.iter().map(|t| t.tile))
        .filter(|t| !tiles.contains(t))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if !skipped.is_empty() {
        warnings.push(format!("tiles vazios com colisão ou terreno ficaram de fora do TileSet: {skipped:?}"));
    }
    if tiles.is_empty() {
        warnings.push("todos os tiles estão vazios: o TileSet não tem tiles".into());
    }
    let tres = ctx.paths.resolve(&a.path)?;
    let png_path = tres.with_extension("png");
    let png_name = png_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let res_path = a.res_path.clone().unwrap_or_else(|| format!("res://{png_name}"));
    let text = godot_tileset(d, &TileSetExport { texture_res_path: &res_path, tiles: &tiles, scale });
    let scaled = scale_image(&img, scale);
    write(&png_path, &png(&scaled)?)?;
    write(&tres, text.as_bytes())?;
    let mut out = Outcome::data(json!({
        "tres": tres.display().to_string(), "png": png_path.display().to_string(), "texture_res_path": res_path,
        "frame": frame, "tile_size": [d.grid.tile_width * scale, d.grid.tile_height * scale],
        "columns": columns, "rows": rows, "tiles": tiles.len(), "collision_tiles": d.collisions.iter().filter(|c| tiles.contains(&c.tile)).count(),
        "terrain_sets": d.terrain_sets.len(), "png_size": [scaled.width(), scaled.height()],
    }));
    out.warnings = warnings;
    let max = ctx.limits.max_preview_side;
    if scaled.width() <= max && scaled.height() <= max {
        out.images.push(render::png("tileset do Godot", &scaled)?);
    }
    Ok(out)
}

// ---- preview overlay ------------------------------------------------------------------------

fn blend(img: &mut Image, x: i32, y: i32, c: Color) {
    if let Some(dst) = img.get(x, y) {
        img.set(x, y, c.over(dst));
    }
}

/// Outlines every tile of the grid (in sprite pixels, shifted by `rect`) on a preview at `scale`.
fn draw_grid(out: &mut Image, grid: &TileGrid, columns: u32, rows: u32, rect: Rect, scale: u32) {
    let s = scale as i32;
    let color = Color::rgba(GRID_COLOR.r, GRID_COLOR.g, GRID_COLOR.b, 190);
    for t in all_tiles(columns, rows) {
        let r = grid.tile_rect(t).translate(-rect.x, -rect.y);
        let (x0, y0, x1, y1) = (r.x * s, r.y * s, r.right() * s - 1, r.bottom() * s - 1);
        for x in x0..=x1 {
            blend(out, x, y0, color);
            blend(out, x, y1, color);
        }
        for y in (y0 + 1)..y1 {
            blend(out, x0, y, color);
            blend(out, x1, y, color);
        }
    }
}

/// Tile grid and collision polygons for `frame_preview` (`tiles: true`).
pub(crate) fn preview_overlay(sp: &Sprite, out: &mut Image, rect: Rect, scale: u32, legend: &mut Vec<serde_json::Value>) {
    let Some(d) = &sp.extras.tiles else {
        legend.push(json!({ "tiles": "o sprite não tem grade de tiles (tile_grid_set)" }));
        return;
    };
    let (columns, rows) = grid_size(sp, &d.grid);
    draw_grid(out, &d.grid, columns, rows, rect, scale);
    let s = scale as i32;
    let fill = Color::rgba(COLLISION_COLOR.r, COLLISION_COLOR.g, COLLISION_COLOR.b, 70);
    for c in &d.collisions {
        let origin = d.grid.tile_rect(c.tile);
        for poly in &c.polygons {
            // Vertices are pixel corners: in the preview they land on pixel boundaries.
            let pts: Vec<Point> = poly.iter().map(|p| Point::new((origin.x + p.x - rect.x) * s, (origin.y + p.y - rect.y) * s)).collect();
            // A vertex on the far edge would land one pixel outside the tile: pull it back in.
            let (w, h) = (out.width() as i32, out.height() as i32);
            let inner: Vec<Point> =
                pts.iter().map(|p| Point::new(if p.x == w { w - 1 } else { p.x }, if p.y == h { h - 1 } else { p.y })).collect();
            for p in polygon_fill(&inner) {
                blend(out, p.x, p.y, fill);
            }
            for p in polyline(&inner, true) {
                out.set(p.x, p.y, COLLISION_COLOR);
            }
        }
    }
    legend.push(json!({
        "tiles": { "columns": columns, "rows": rows, "grid_color": GRID_COLOR, "collision_color": COLLISION_COLOR, "collision_tiles": d.collisions.len() },
    }));
}
