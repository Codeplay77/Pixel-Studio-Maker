//! Godot 4 `TileSet` resource (`.tres`) for a sprite used as a tileset: one atlas source over the
//! exported PNG, with the grid (tile size, margins, separation), one tile per non-empty cell,
//! collision polygons on physics layer 0 and the terrain sets with their peering bits.

use sprite_core::Color;
use sprite_drawing::tiles::{TileCoord, TileData};

/// What goes into the resource besides the tile data.
#[derive(Debug, Clone)]
pub struct TileSetExport<'a> {
    /// `res://` path of the texture.
    pub texture_res_path: &'a str,
    /// Tiles that get an entry (the non-empty ones), any order.
    pub tiles: &'a [TileCoord],
    /// Pixel scale of the exported PNG (1 = original size).
    pub scale: u32,
}

/// Number as Godot writes it: integers without decimals, otherwise up to 6 decimals.
pub fn godot_number(v: f64) -> String {
    if v.fract() == 0.0 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn godot_color(c: Color) -> String {
    let f = |x: u8| godot_number(x as f64 / 255.0);
    format!("Color({}, {}, {}, {})", f(c.r), f(c.g), f(c.b), f(c.a))
}

fn godot_string(s: &str) -> String {
    let escaped: String = s
        .chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            other => vec![other],
        })
        .collect();
    format!("\"{escaped}\"")
}

/// The `.tres` text. Collision points are relative to the tile center, as Godot expects.
pub fn godot_tileset(data: &TileData, export: &TileSetExport) -> String {
    let s = export.scale.max(1) as f64;
    let g = &data.grid;
    let (tw, th) = (g.tile_width as f64 * s, g.tile_height as f64 * s);
    let mut tiles: Vec<TileCoord> = export.tiles.to_vec();
    tiles.sort_by_key(|t| (t[1], t[0]));
    tiles.dedup();

    let mut atlas = String::new();
    atlas.push_str("[sub_resource type=\"TileSetAtlasSource\" id=\"TileSetAtlasSource_1\"]\n");
    atlas.push_str("texture = ExtResource(\"1_texture\")\n");
    atlas.push_str(&format!("margins = Vector2i({}, {})\n", g.margin[0] * export.scale.max(1), g.margin[1] * export.scale.max(1)));
    atlas.push_str(&format!("separation = Vector2i({}, {})\n", g.spacing[0] * export.scale.max(1), g.spacing[1] * export.scale.max(1)));
    atlas.push_str(&format!("texture_region_size = Vector2i({}, {})\n", godot_number(tw), godot_number(th)));
    let mut has_collision = false;
    for t in &tiles {
        let key = format!("{}:{}/0", t[0], t[1]);
        atlas.push_str(&format!("{key} = 0\n"));
        if let Some(tt) = data.terrain(*t)
            && (tt.set as usize) < data.terrain_sets.len()
        {
            atlas.push_str(&format!("{key}/terrain_set = {}\n", tt.set));
            if let Some(terrain) = tt.terrain {
                atlas.push_str(&format!("{key}/terrain = {terrain}\n"));
            }
            for (bit, terrain) in tt.peering.iter() {
                atlas.push_str(&format!("{key}/terrains_peering_bit/{} = {terrain}\n", bit.name()));
            }
        }
        if let Some(c) = data.collision(*t) {
            for (i, poly) in c.polygons.iter().enumerate() {
                has_collision = true;
                let points: Vec<String> =
                    poly.iter().flat_map(|p| [godot_number(p.x as f64 * s - tw / 2.0), godot_number(p.y as f64 * s - th / 2.0)]).collect();
                atlas.push_str(&format!("{key}/physics_layer_0/polygon_{i}/points = PackedVector2Array({})\n", points.join(", ")));
            }
        }
    }

    let mut out = String::new();
    out.push_str("[gd_resource type=\"TileSet\" load_steps=3 format=3]\n\n");
    out.push_str(&format!("[ext_resource type=\"Texture2D\" path=\"{}\" id=\"1_texture\"]\n\n", export.texture_res_path));
    out.push_str(&atlas);
    out.push('\n');
    out.push_str("[resource]\n");
    out.push_str(&format!("tile_size = Vector2i({}, {})\n", godot_number(tw), godot_number(th)));
    if has_collision {
        out.push_str("physics_layer_0/collision_layer = 1\n");
    }
    for (i, set) in data.terrain_sets.iter().enumerate() {
        out.push_str(&format!("terrain_set_{i}/mode = {}\n", set.mode.godot()));
        for (j, terrain) in set.terrains.iter().enumerate() {
            out.push_str(&format!("terrain_set_{i}/terrain_{j}/name = {}\n", godot_string(&terrain.name)));
            out.push_str(&format!(
                "terrain_set_{i}/terrain_{j}/color = {}\n",
                godot_color(Color::rgb(terrain.color.r, terrain.color.g, terrain.color.b))
            ));
        }
    }
    out.push_str("sources/0 = SubResource(\"TileSetAtlasSource_1\")\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::Point;
    use sprite_drawing::tiles::{
        CollisionShape, PeeringBit, PeeringBits, Terrain, TerrainMode, TerrainSet, TileGrid, TileTerrain, collision_shape,
    };

    #[test]
    fn numbers_like_godot() {
        assert_eq!(godot_number(-8.0), "-8");
        assert_eq!(godot_number(7.5), "7.5");
        assert_eq!(godot_number(0.2), "0.2");
        assert_eq!(godot_number(60.0 / 255.0), "0.235294");
        assert_eq!(godot_string("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn resource_has_tiles_collisions_and_terrains() {
        let mut d = TileData::new(TileGrid { tile_width: 16, tile_height: 16, margin: [1, 1], spacing: [2, 2] });
        d.set_collision([1, 0], vec![collision_shape(CollisionShape::Full, 16, 16)]);
        d.set_collision([0, 0], vec![vec![Point::new(0, 8), Point::new(16, 8), Point::new(16, 16)]]);
        d.terrain_sets.push(TerrainSet {
            name: "chão".into(),
            mode: TerrainMode::MatchSides,
            terrains: vec![Terrain { name: "grama".into(), color: Color::rgb(0, 255, 0) }],
        });
        let mut bits = PeeringBits::default();
        bits.set(PeeringBit::RightSide, Some(0));
        d.set_terrain([1, 0], Some(TileTerrain { tile: [1, 0], set: 0, terrain: Some(0), peering: bits }));
        let tres = godot_tileset(&d, &TileSetExport { texture_res_path: "res://t.png", tiles: &[[1, 0], [0, 0]], scale: 1 });
        let lines: Vec<&str> = tres.lines().collect();
        assert_eq!(lines[0], "[gd_resource type=\"TileSet\" load_steps=3 format=3]");
        assert!(lines.contains(&"[ext_resource type=\"Texture2D\" path=\"res://t.png\" id=\"1_texture\"]"));
        assert!(lines.contains(&"margins = Vector2i(1, 1)"));
        assert!(lines.contains(&"separation = Vector2i(2, 2)"));
        assert!(lines.contains(&"texture_region_size = Vector2i(16, 16)"));
        let i0 = lines.iter().position(|l| *l == "0:0/0 = 0").unwrap();
        let i1 = lines.iter().position(|l| *l == "1:0/0 = 0").unwrap();
        assert!(i0 < i1, "row-major order");
        assert!(lines.contains(&"0:0/0/physics_layer_0/polygon_0/points = PackedVector2Array(-8, 0, 8, 0, 8, 8)"));
        assert!(lines.contains(&"1:0/0/physics_layer_0/polygon_0/points = PackedVector2Array(-8, -8, 8, -8, 8, 8, -8, 8)"));
        assert!(lines.contains(&"1:0/0/terrain_set = 0"));
        assert!(lines.contains(&"1:0/0/terrain = 0"));
        assert!(lines.contains(&"1:0/0/terrains_peering_bit/right_side = 0"));
        assert!(lines.contains(&"tile_size = Vector2i(16, 16)"));
        assert!(lines.contains(&"physics_layer_0/collision_layer = 1"));
        assert!(lines.contains(&"terrain_set_0/mode = 2"));
        assert!(lines.contains(&"terrain_set_0/terrain_0/name = \"grama\""));
        assert!(lines.contains(&"terrain_set_0/terrain_0/color = Color(0, 1, 0, 1)"));
        assert_eq!(lines.last(), Some(&"sources/0 = SubResource(\"TileSetAtlasSource_1\")"));
    }

    #[test]
    fn scale_and_odd_sizes() {
        let mut d = TileData::new(TileGrid::new(15, 15));
        d.set_collision([0, 0], vec![collision_shape(CollisionShape::Full, 15, 15)]);
        let tres = godot_tileset(&d, &TileSetExport { texture_res_path: "res://t.png", tiles: &[[0, 0]], scale: 1 });
        assert!(tres.contains("PackedVector2Array(-7.5, -7.5, 7.5, -7.5, 7.5, 7.5, -7.5, 7.5)"), "{tres}");
        let tres = godot_tileset(&d, &TileSetExport { texture_res_path: "res://t.png", tiles: &[[0, 0]], scale: 2 });
        assert!(tres.contains("texture_region_size = Vector2i(30, 30)"));
        assert!(tres.contains("PackedVector2Array(-15, -15, 15, -15, 15, 15, -15, 15)"));
        assert!(!tres.contains("terrain_set_0"));
    }
}
