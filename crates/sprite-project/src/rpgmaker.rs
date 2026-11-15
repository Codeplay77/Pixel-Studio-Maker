//! RPG Maker (MZ, MV and VX Ace) image layouts and the helpers that assemble RPG Maker sheets.
//! Pure geometry, no project state: the commands (`rpgmaker_template`, `export_rpgmaker_*`) live
//! in `sprite_commands`.
//!
//! Every layout follows what the engines read at runtime:
//! - tilesets: `Tilemap` (MV/MZ) and the RGSS3 tilemap (VX Ace), in tiles of 48 px (32 on VX Ace);
//! - characters: `ImageManager.isBigCharacter` (`$` prefix) and `Sprite_Character` (anchor at the
//!   bottom center of the cell, patterns 0,1,2,1);
//! - side-view battlers: `Sprite_Actor.updateFrame` (`cx = floor(motion / 6) * 3 + pattern`,
//!   `cy = motion % 6`, 9×6 cells);
//! - window skins: `Window._refreshBack/_refreshFrame/_refreshCursor/_refreshArrows/_refreshPauseSign`
//!   and `textColor(n)` (MV/MZ read the pixel at `96 + (n % 8) * 12 + 6, 144 + floor(n / 8) * 12 + 6`).

use sprite_animation::rpgmaker::{RpgMakerEngine, RpgMakerKind};
use sprite_core::{Color, Image, PasteMode, Point, Rect};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RpgMakerError {
    /// The engine has no such file (e.g. side-view battlers on VX Ace).
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Invalid(String),
}

/// A named block of a template, turned into a semantic region of the sprite.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRegion {
    pub name: String,
    /// What the block is for (Portuguese, shown to the agent and in the editor).
    pub part: String,
    pub rect: Rect,
    pub color: Color,
}

/// Size, cells and named blocks of one RPG Maker file.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateLayout {
    pub kind: RpgMakerKind,
    pub engine: RpgMakerEngine,
    pub width: u32,
    pub height: u32,
    pub tile: u32,
    /// Cell size of character, face and battler sheets.
    pub cell: Option<[u32; 2]>,
    pub single: bool,
    /// Grid description for agents (Portuguese).
    pub grid: String,
    pub regions: Vec<TemplateRegion>,
}

/// Options that change a layout (characters and battlers only).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemplateOptions {
    /// Character sheet with a single character (3×4 cells, `$` prefix).
    pub single: bool,
    /// Cell size [w, h] of characters (default: the tile) and battlers (default 64×64).
    pub cell: Option<[u32; 2]>,
}

/// Directions of a character sheet, top to bottom: (direction name in the sprite, label).
pub const CHARACTER_DIRECTIONS: [(&str, &str); 4] = [("S", "baixo"), ("W", "esquerda"), ("E", "direita"), ("N", "cima")];

/// Side-view battler motions in `Sprite_Actor.MOTIONS` index order: (RPG Maker key, Portuguese name).
pub const SV_MOTIONS: [(&str, &str); 18] = [
    ("walk", "andar"),
    ("wait", "esperar"),
    ("chant", "entoar"),
    ("guard", "defender"),
    ("damage", "dano"),
    ("evade", "esquivar"),
    ("thrust", "estocar"),
    ("swing", "brandir"),
    ("missile", "disparar"),
    ("skill", "habilidade"),
    ("spell", "magia"),
    ("item", "item"),
    ("escape", "fugir"),
    ("victory", "vitória"),
    ("dying", "agonizando"),
    ("abnormal", "estado anormal"),
    ("sleep", "dormir"),
    ("dead", "morto"),
];

/// Default side-view battler cell.
pub const SV_CELL: [u32; 2] = [64, 64];

/// A readable default for the 32 text colors of Window.png. Indices follow the meaning the engines
/// give them: 0 normal text, 16 system, 17 crisis, 18 knockout, 19 gauge background,
/// 20–21 HP gauge, 22–23 MP gauge (23 MP cost), 24 power up, 25 power down, 26–27 time gauge (MZ),
/// 28–29 TP gauge (29 TP cost).
pub const TEXT_COLORS: [Color; 32] = [
    Color::rgb(0xff, 0xff, 0xff),
    Color::rgb(0x20, 0xa0, 0xd6),
    Color::rgb(0xff, 0x78, 0x4c),
    Color::rgb(0x66, 0xcc, 0x40),
    Color::rgb(0x99, 0xcc, 0xff),
    Color::rgb(0xcc, 0xc0, 0xff),
    Color::rgb(0xff, 0xff, 0xa0),
    Color::rgb(0x80, 0x80, 0x80),
    Color::rgb(0xc0, 0xc0, 0xc0),
    Color::rgb(0x20, 0x80, 0xcc),
    Color::rgb(0xff, 0x38, 0x10),
    Color::rgb(0x00, 0xa0, 0x10),
    Color::rgb(0x3e, 0x9a, 0xde),
    Color::rgb(0xa0, 0x98, 0xff),
    Color::rgb(0xff, 0xcc, 0x20),
    Color::rgb(0x00, 0x00, 0x00),
    Color::rgb(0x84, 0xaa, 0xff),
    Color::rgb(0xff, 0xff, 0x40),
    Color::rgb(0xff, 0x20, 0x20),
    Color::rgb(0x20, 0x20, 0x40),
    Color::rgb(0xe0, 0x80, 0x40),
    Color::rgb(0xf0, 0xc0, 0x40),
    Color::rgb(0x40, 0x80, 0xc0),
    Color::rgb(0x40, 0xc0, 0xf0),
    Color::rgb(0x80, 0xff, 0x80),
    Color::rgb(0xc0, 0x80, 0x80),
    Color::rgb(0x80, 0x80, 0xff),
    Color::rgb(0xff, 0x80, 0xff),
    Color::rgb(0x00, 0xa0, 0x40),
    Color::rgb(0x00, 0xe0, 0x60),
    Color::rgb(0xa0, 0x60, 0xe0),
    Color::rgb(0xc0, 0x80, 0xff),
];

const COLORS: [Color; 8] = [
    Color::rgb(0x00, 0xe5, 0xff),
    Color::rgb(0xff, 0x40, 0x81),
    Color::rgb(0xff, 0xd6, 0x00),
    Color::rgb(0x76, 0xff, 0x03),
    Color::rgb(0xd5, 0x00, 0xf9),
    Color::rgb(0xff, 0x6d, 0x00),
    Color::rgb(0x29, 0x79, 0xff),
    Color::rgb(0x1d, 0xe9, 0xb6),
];

fn color(i: usize) -> Color {
    COLORS[i % COLORS.len()]
}

fn region(name: impl Into<String>, part: impl Into<String>, rect: Rect, color: Color) -> TemplateRegion {
    TemplateRegion { name: name.into(), part: part.into(), rect, color }
}

/// Role of an A1 block (column `c`, row `l`, in 2×3-tile blocks), from `Tilemap._drawAutotile`:
/// animated water kinds have 3 frames side by side; kinds 2 and 3 are static decorations; the
/// last column of each half holds waterfalls.
fn a1_role(c: u32, l: u32) -> String {
    let water = |kinds: [u32; 4], frame: u32| format!("água animada (tipo {}, quadro {} de 3)", kinds[l as usize], frame + 1);
    match c {
        0..=2 => water([0, 1, 8, 10], c),
        3 => match l {
            0 => "decoração sobre a água (tipo 2, sem animação)".into(),
            1 => "decoração sobre a água (tipo 3, sem animação)".into(),
            2 => "cascata (tipo 9, 3 quadros na vertical)".into(),
            _ => "cascata (tipo 11, 3 quadros na vertical)".into(),
        },
        4..=6 => water([4, 6, 12, 14], c - 4),
        _ => format!("cascata (tipo {}, 3 quadros na vertical)", [5, 7, 13, 15][l as usize]),
    }
}

/// Layout of an RPG Maker file for an engine.
pub fn template_layout(kind: RpgMakerKind, engine: RpgMakerEngine, opts: TemplateOptions) -> Result<TemplateLayout, RpgMakerError> {
    let t = engine.tile();
    let ti = t as i32;
    let mut regions = Vec::new();
    let mut cell = None;
    let mut single = false;
    let (width, height, grid) = match kind {
        RpgMakerKind::TilesetA1 | RpgMakerKind::TilesetA2 => {
            let a1 = kind == RpgMakerKind::TilesetA1;
            let prefix = if a1 { "A1" } else { "A2" };
            for l in 0..4u32 {
                for c in 0..8u32 {
                    let part = if a1 { a1_role(c, l) } else { "chão (autotile)".to_string() };
                    let col = match (a1, part.split(' ').next()) {
                        (false, _) => color((c + l) as usize),
                        (true, Some("água")) => color(0),
                        (true, Some("cascata")) => color(6),
                        _ => color(2),
                    };
                    regions.push(region(
                        format!("{prefix} bloco {c},{l}"),
                        part,
                        Rect::new(c as i32 * 2 * ti, l as i32 * 3 * ti, 2 * t, 3 * t),
                        col,
                    ));
                }
            }
            (16 * t, 12 * t, format!("16×12 tiles de {t} px; 8×4 blocos de autotile de 2×3 tiles ({}×{} px)", 2 * t, 3 * t))
        }
        RpgMakerKind::TilesetA3 => {
            for l in 0..4u32 {
                for c in 0..8u32 {
                    let part = if l % 2 == 0 { "telhado (autotile)" } else { "parede de construção (autotile)" };
                    regions.push(region(
                        format!("A3 bloco {c},{l}"),
                        part,
                        Rect::new(c as i32 * 2 * ti, l as i32 * 2 * ti, 2 * t, 2 * t),
                        color((l % 2) as usize * 2 + (c % 2) as usize),
                    ));
                }
            }
            (16 * t, 8 * t, format!("16×8 tiles de {t} px; 8×4 blocos de 2×2 tiles; linhas pares = telhados, ímpares = paredes"))
        }
        RpgMakerKind::TilesetA4 => {
            // Row l starts at tile y [0, 3, 5, 8, 10, 13]: wall tops are 3 tiles tall, wall faces 2.
            let mut y = 0;
            for l in 0..6u32 {
                let top = l % 2 == 0;
                let h = if top { 3 } else { 2 };
                for c in 0..8u32 {
                    let part = if top { "topo de parede (autotile 2×3)" } else { "face de parede (autotile 2×2)" };
                    regions.push(region(
                        format!("A4 bloco {c},{l}"),
                        part,
                        Rect::new(c as i32 * 2 * ti, y * ti, 2 * t, h * t),
                        color(if top { 0 } else { 2 } + (c % 2) as usize),
                    ));
                }
                y += h as i32;
            }
            (
                16 * t,
                15 * t,
                format!(
                    "16×15 tiles de {t} px; 8 colunas; linhas 0, 2, 4 = topo de parede (2×3 tiles), 1, 3, 5 = face de parede (2×2 tiles)"
                ),
            )
        }
        RpgMakerKind::TilesetA5 => {
            regions.push(region("A5", "tiles comuns (8×16)", Rect::new(0, 0, 8 * t, 16 * t), color(0)));
            (8 * t, 16 * t, format!("8×16 tiles de {t} px"))
        }
        RpgMakerKind::TilesetB | RpgMakerKind::TilesetC | RpgMakerKind::TilesetD | RpgMakerKind::TilesetE => {
            let letter = &kind.id()["tileset_".len()..].to_uppercase();
            regions.push(region(
                format!("{letter} esquerda"),
                "objetos: metade esquerda (tiles 0–127 na paleta)",
                Rect::new(0, 0, 8 * t, 16 * t),
                color(0),
            ));
            regions.push(region(
                format!("{letter} direita"),
                "objetos: metade direita (tiles 128–255 na paleta)",
                Rect::new(8 * ti, 0, 8 * t, 16 * t),
                color(1),
            ));
            (16 * t, 16 * t, format!("16×16 tiles de {t} px, em duas metades de 8×16"))
        }
        RpgMakerKind::Character => {
            let [cw, ch] = check_cell(opts.cell.unwrap_or([t, t]))?;
            cell = Some([cw, ch]);
            single = opts.single;
            let (cols, rows) = if single { (3, 4) } else { (12, 8) };
            let characters = if single { 1 } else { 8 };
            for k in 0..characters {
                for (row, (dir, label)) in CHARACTER_DIRECTIONS.iter().enumerate() {
                    let r = character_cell(k, row, 0, [cw, ch], single);
                    let name = if single { (*label).to_string() } else { format!("personagem {k} {label}") };
                    regions.push(region(name, *dir, Rect::new(r.x, r.y, 3 * cw, ch), color(row)));
                }
            }
            let grid = if single {
                format!("3×4 células de {cw}×{ch} px (um personagem, arquivo com prefixo $); linhas: baixo, esquerda, direita, cima")
            } else {
                format!("12×8 células de {cw}×{ch} px: 4×2 personagens de 3×4 células; linhas de cada um: baixo, esquerda, direita, cima")
            };
            (cols * cw, rows * ch, grid)
        }
        RpgMakerKind::Face => {
            let size = 3 * t;
            cell = Some([size, size]);
            for n in 0..8 {
                regions.push(region(format!("face {n}"), "face", face_cell(n, size), color(n)));
            }
            (4 * size, 2 * size, format!("4×2 faces de {size}×{size} px (índices 0–3 na primeira linha, 4–7 na segunda)"))
        }
        RpgMakerKind::SvBattler => {
            if engine == RpgMakerEngine::Vxace {
                return Err(RpgMakerError::Unsupported(
                    "o RPG Maker VX Ace não tem battlers de visão lateral; use engine \"mz\" ou \"mv\"".into(),
                ));
            }
            let [cw, ch] = check_cell(opts.cell.unwrap_or(SV_CELL))?;
            cell = Some([cw, ch]);
            for (i, (key, name)) in SV_MOTIONS.iter().enumerate() {
                let r = battler_cell(i, 0, [cw, ch]);
                regions.push(region(*name, *key, Rect::new(r.x, r.y, 3 * cw, ch), color(i)));
            }
            (
                9 * cw,
                6 * ch,
                format!(
                    "9×6 células de {cw}×{ch} px; o movimento i ocupa 3 células na coluna floor(i/6)*3 e na linha i%6 (ordem: {})",
                    SV_MOTIONS.iter().map(|m| m.0).collect::<Vec<_>>().join(", ")
                ),
            )
        }
        RpgMakerKind::Window => {
            for (i, (name, part, r)) in window_parts(engine).into_iter().enumerate() {
                regions.push(region(name, part, r, color(i)));
            }
            let s = window_size(engine);
            let corner = scaled(24, t);
            (s, s, format!("Window.png de {s}×{s} px; moldura com cantos de {corner} px; 32 cores de texto em 8×4 amostras"))
        }
    };
    Ok(TemplateLayout { kind, engine, width, height, tile: t, cell, single, grid, regions })
}

/// Validates a cell size (1 to 512 px per side).
pub fn check_cell([w, h]: [u32; 2]) -> Result<[u32; 2], RpgMakerError> {
    if w == 0 || h == 0 || w > 512 || h > 512 {
        return Err(RpgMakerError::Invalid(format!("célula {w}x{h} inválida: cada lado de 1 a 512 px")));
    }
    Ok([w, h])
}

/// MV/MZ window coordinates scaled to the engine (VX Ace = 2/3; every value is a multiple of 3).
fn scaled(v: i32, tile: u32) -> i32 {
    v * tile as i32 / 48
}

fn scaled_rect(x: i32, y: i32, w: i32, h: i32, tile: u32) -> Rect {
    Rect::new(scaled(x, tile), scaled(y, tile), scaled(w, tile) as u32, scaled(h, tile) as u32)
}

/// Side of Window.png (192 on MV/MZ, 128 on VX Ace).
pub fn window_size(engine: RpgMakerEngine) -> u32 {
    4 * engine.tile()
}

/// The parts of Window.png: (region name, description, rectangle).
pub fn window_parts(engine: RpgMakerEngine) -> Vec<(&'static str, &'static str, Rect)> {
    let t = engine.tile();
    let corner = if t == 32 { "moldura (cantos de 16 px)" } else { "moldura (cantos de 24 px)" };
    let pause = if t == 32 { "sinal de pausa: 4 quadros de 16×16 em 2×2" } else { "sinal de pausa: 4 quadros de 24×24 em 2×2" };
    let colors =
        if t == 32 { "cores do texto: 32 amostras de 8×8, 8 por linha" } else { "cores do texto: 32 amostras de 12×12, 8 por linha" };
    vec![
        ("fundo", "fundo da janela (esticado)", scaled_rect(0, 0, 96, 96, t)),
        ("padrão do fundo", "padrão ladrilhado sobre o fundo", scaled_rect(0, 96, 96, 96, t)),
        ("moldura", corner, scaled_rect(96, 0, 96, 96, t)),
        ("seta para cima", "seta de rolagem para cima (dentro da moldura)", scaled_rect(132, 24, 24, 12, t)),
        ("seta para baixo", "seta de rolagem para baixo (dentro da moldura)", scaled_rect(132, 60, 24, 12, t)),
        ("cursor", "cursor de seleção", scaled_rect(96, 96, 48, 48, t)),
        ("sinal de pausa", pause, scaled_rect(144, 96, 48, 48, t)),
        ("cores do texto", colors, scaled_rect(96, 144, 96, 48, t)),
    ]
}

/// Sample square of text color `n` (0..32) in Window.png.
pub fn text_color_sample(engine: RpgMakerEngine, n: usize) -> Rect {
    let t = engine.tile();
    let (c, r) = ((n % 8) as i32, (n / 8) as i32);
    scaled_rect(96 + c * 12, 144 + r * 12, 12, 12, t)
}

/// Pixel the engine reads for text color `n`: the center of the sample on MV/MZ
/// (`96 + (n % 8) * 12 + 6, 144 + floor(n / 8) * 12 + 6`), the top-left pixel on VX Ace
/// (`64 + (n % 8) * 8, 96 + (n / 8) * 8`).
pub fn text_color_point(engine: RpgMakerEngine, n: usize) -> Point {
    let r = text_color_sample(engine, n);
    match engine {
        RpgMakerEngine::Vxace => Point::new(r.x, r.y),
        _ => Point::new(r.x + 6, r.y + 6),
    }
}

/// Cell of a character sheet: character `index` (0..8, 4 per row), direction row (0 = down,
/// 1 = left, 2 = right, 3 = up) and pattern (column 0..3).
pub fn character_cell(index: usize, dir_row: usize, pattern: usize, [cw, ch]: [u32; 2], single: bool) -> Rect {
    let (bx, by) = if single { (0, 0) } else { (index % 4, index / 4) };
    let col = bx * 3 + pattern;
    let row = by * 4 + dir_row;
    Rect::new((col as u32 * cw) as i32, (row as u32 * ch) as i32, cw, ch)
}

/// Cell of a side-view battler: motion index (0..18) and pattern (0..3), as `Sprite_Actor.updateFrame`
/// reads it: column `floor(motion / 6) * 3 + pattern`, row `motion % 6`.
pub fn battler_cell(motion: usize, pattern: usize, [cw, ch]: [u32; 2]) -> Rect {
    let col = (motion / 6) * 3 + pattern;
    let row = motion % 6;
    Rect::new((col as u32 * cw) as i32, (row as u32 * ch) as i32, cw, ch)
}

/// Cell of face `n` (0..8) in a 4×2 face sheet.
pub fn face_cell(n: usize, size: u32) -> Rect {
    Rect::new(((n % 4) as u32 * size) as i32, ((n / 4) as u32 * size) as i32, size, size)
}

/// Lowercase without Portuguese accents, for lenient name matching.
fn fold(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Index of a battler motion by its RPG Maker key (`walk`) or Portuguese name (`andar`).
pub fn motion_index(name: &str) -> Option<usize> {
    let f = fold(name);
    SV_MOTIONS.iter().position(|(key, pt)| f == *key || f == fold(pt))
}

/// Character sheet file name: leading `!`/`$` removed, then `!` (object) and `$` (single
/// character) added in the order RPG Maker expects (`!$Name.png`). Returns the name and whether
/// a prefix written by the caller was changed.
pub fn character_file_name(file_name: &str, single: bool, object: bool) -> (String, bool) {
    let bare = file_name.trim_start_matches(['!', '$']);
    let given = &file_name[..file_name.len() - bare.len()];
    let prefix = format!("{}{}", if object { "!" } else { "" }, if single { "$" } else { "" });
    let changed = !given.is_empty() && given != prefix;
    (format!("{prefix}{bare}"), changed)
}

/// Puts a frame in a cell with the pivot pixel on the cell's bottom center (`x = w / 2`,
/// `y = h - 1`), scaled by `scale` (nearest neighbor). Returns the cell image and how many visible
/// pixels fell outside the cell.
pub fn place_on_baseline(frame: &Image, pivot: Point, scale: u32, [cw, ch]: [u32; 2]) -> (Image, u32) {
    let s = scale.max(1);
    let big = if s == 1 { frame.clone() } else { frame.scale_nearest(frame.width() * s, frame.height() * s) };
    let (px, py) = (pivot.x * s as i32, pivot.y * s as i32 + s as i32 - 1);
    let at = Point::new(cw as i32 / 2 - px, ch as i32 - 1 - py);
    paste_counting(&big, at, [cw, ch])
}

/// Puts a frame centered in a cell, scaled by `scale`. Returns the cell image and how many
/// visible pixels fell outside the cell.
pub fn place_centered(frame: &Image, scale: u32, [cw, ch]: [u32; 2]) -> (Image, u32) {
    let s = scale.max(1);
    let big = if s == 1 { frame.clone() } else { frame.scale_nearest(frame.width() * s, frame.height() * s) };
    let at = Point::new((cw as i32 - big.width() as i32) / 2, (ch as i32 - big.height() as i32) / 2);
    paste_counting(&big, at, [cw, ch])
}

fn paste_counting(img: &Image, at: Point, [cw, ch]: [u32; 2]) -> (Image, u32) {
    let mut cell = Image::new(cw.max(1), ch.max(1));
    cell.paste(img, at, PasteMode::Replace);
    let lost = img.opaque_count().saturating_sub(cell.opaque_count());
    (cell, lost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(kind: RpgMakerKind, engine: RpgMakerEngine) -> TemplateLayout {
        template_layout(kind, engine, TemplateOptions::default()).unwrap()
    }

    #[test]
    fn sizes_match_the_engines() {
        use RpgMakerKind as K;
        // (kind, MV/MZ size, VX Ace size, regions)
        type Row = (K, (u32, u32), (u32, u32), usize);
        let expected: [Row; 13] = [
            (K::TilesetA1, (768, 576), (512, 384), 32),
            (K::TilesetA2, (768, 576), (512, 384), 32),
            (K::TilesetA3, (768, 384), (512, 256), 32),
            (K::TilesetA4, (768, 720), (512, 480), 48),
            (K::TilesetA5, (384, 768), (256, 512), 1),
            (K::TilesetB, (768, 768), (512, 512), 2),
            (K::TilesetC, (768, 768), (512, 512), 2),
            (K::TilesetD, (768, 768), (512, 512), 2),
            (K::TilesetE, (768, 768), (512, 512), 2),
            (K::Character, (576, 384), (384, 256), 32),
            (K::Face, (576, 288), (384, 192), 8),
            (K::SvBattler, (576, 384), (576, 384), 18),
            (K::Window, (192, 192), (128, 128), 8),
        ];
        for (kind, mz, vx, regions) in expected {
            for engine in [RpgMakerEngine::Mz, RpgMakerEngine::Mv] {
                let l = layout(kind, engine);
                assert_eq!((l.width, l.height), mz, "{kind:?} {engine:?}");
                assert_eq!(l.regions.len(), regions, "{kind:?}");
            }
            if kind == K::SvBattler {
                assert!(matches!(
                    template_layout(kind, RpgMakerEngine::Vxace, TemplateOptions::default()),
                    Err(RpgMakerError::Unsupported(_))
                ));
                continue;
            }
            let l = layout(kind, RpgMakerEngine::Vxace);
            assert_eq!((l.width, l.height), vx, "{kind:?} vxace");
            assert_eq!(l.regions.len(), regions, "{kind:?} vxace");
            // Every block lies inside the image and names are unique.
            let mut names: Vec<&str> = l.regions.iter().map(|r| r.name.as_str()).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), l.regions.len(), "{kind:?}: nomes repetidos");
            for r in &l.regions {
                assert!(r.rect.x >= 0 && r.rect.y >= 0, "{kind:?} {}", r.name);
                assert!(r.rect.x as u32 + r.rect.w <= l.width && r.rect.y as u32 + r.rect.h <= l.height, "{kind:?} {}", r.name);
            }
        }
        let single = template_layout(K::Character, RpgMakerEngine::Mz, TemplateOptions { single: true, cell: Some([64, 96]) }).unwrap();
        assert_eq!((single.width, single.height, single.regions.len()), (192, 384, 4));
    }

    #[test]
    fn tilesets_blocks_are_exact() {
        let a4 = layout(RpgMakerKind::TilesetA4, RpgMakerEngine::Mz);
        let row_y: Vec<i32> = (0..6).map(|l| a4.regions[l * 8].rect.y).collect();
        assert_eq!(row_y, vec![0, 144, 240, 384, 480, 624]);
        assert_eq!(a4.regions[8].rect, Rect::new(0, 144, 96, 96));
        assert_eq!(a4.regions[47].rect, Rect::new(672, 624, 96, 96));
        assert_eq!(a4.regions[0].name, "A4 bloco 0,0");
        let a2 = layout(RpgMakerKind::TilesetA2, RpgMakerEngine::Vxace);
        assert_eq!(a2.regions[9].name, "A2 bloco 1,1");
        assert_eq!(a2.regions[9].rect, Rect::new(64, 96, 64, 96));
        let a1 = layout(RpgMakerKind::TilesetA1, RpgMakerEngine::Mz);
        assert!(a1.regions[0].part.contains("tipo 0, quadro 1"));
        assert!(a1.regions[7].part.contains("cascata (tipo 5"));
        assert!(a1.regions[8 + 3].part.contains("tipo 3"));
        let b = layout(RpgMakerKind::TilesetB, RpgMakerEngine::Mz);
        assert_eq!(b.regions[1].name, "B direita");
        assert_eq!(b.regions[1].rect, Rect::new(384, 0, 384, 768));
    }

    #[test]
    fn window_parts_match_the_engines() {
        let mz: Vec<Rect> = window_parts(RpgMakerEngine::Mz).into_iter().map(|p| p.2).collect();
        assert_eq!(
            mz,
            vec![
                Rect::new(0, 0, 96, 96),
                Rect::new(0, 96, 96, 96),
                Rect::new(96, 0, 96, 96),
                Rect::new(132, 24, 24, 12),
                Rect::new(132, 60, 24, 12),
                Rect::new(96, 96, 48, 48),
                Rect::new(144, 96, 48, 48),
                Rect::new(96, 144, 96, 48),
            ]
        );
        let vx: Vec<Rect> = window_parts(RpgMakerEngine::Vxace).into_iter().map(|p| p.2).collect();
        assert_eq!(vx[0], Rect::new(0, 0, 64, 64));
        assert_eq!(vx[1], Rect::new(0, 64, 64, 64));
        assert_eq!(vx[2], Rect::new(64, 0, 64, 64));
        assert_eq!(vx[5], Rect::new(64, 64, 32, 32));
        assert_eq!(vx[6], Rect::new(96, 64, 32, 32));
        assert_eq!(vx[7], Rect::new(64, 96, 64, 32));
        assert_eq!(text_color_point(RpgMakerEngine::Mz, 0), Point::new(102, 150));
        assert_eq!(text_color_point(RpgMakerEngine::Mz, 17), Point::new(96 + 12 + 6, 144 + 24 + 6));
        assert_eq!(text_color_point(RpgMakerEngine::Vxace, 31), Point::new(64 + 7 * 8, 96 + 3 * 8));
        assert_eq!(text_color_sample(RpgMakerEngine::Vxace, 9), Rect::new(72, 104, 8, 8));
    }

    #[test]
    fn cells_follow_the_engine_formulas() {
        // Sprite_Actor.updateFrame: cx = floor(i / 6) * 3 + pattern, cy = i % 6.
        assert_eq!(battler_cell(0, 0, SV_CELL), Rect::new(0, 0, 64, 64));
        assert_eq!(battler_cell(7, 2, SV_CELL), Rect::new(5 * 64, 64, 64, 64));
        assert_eq!(battler_cell(17, 1, SV_CELL), Rect::new(7 * 64, 5 * 64, 64, 64));
        assert_eq!(character_cell(5, 3, 1, [48, 48], false), Rect::new(4 * 48, 7 * 48, 48, 48));
        assert_eq!(character_cell(0, 2, 2, [32, 48], true), Rect::new(64, 96, 32, 48));
        assert_eq!(face_cell(6, 144), Rect::new(288, 144, 144, 144));
        assert_eq!(motion_index("dead"), Some(17));
        assert_eq!(motion_index("Vitoria"), Some(13));
        assert_eq!(motion_index("ESTADO ANORMAL"), Some(15));
        assert_eq!(motion_index("voar"), None);
    }

    #[test]
    fn character_names_get_prefixes() {
        assert_eq!(character_file_name("Heroi.png", true, true), ("!$Heroi.png".into(), false));
        assert_eq!(character_file_name("$Heroi.png", true, false), ("$Heroi.png".into(), false));
        assert_eq!(character_file_name("!Porta.png", false, false), ("Porta.png".into(), true));
        assert_eq!(character_file_name("Povo.png", false, false), ("Povo.png".into(), false));
    }

    #[test]
    fn frames_sit_on_the_baseline() {
        let mut f = Image::new(8, 8);
        f.set(4, 7, Color::WHITE); // pivot pixel (default origin of an 8x8 sprite)
        f.set(0, 0, Color::BLACK);
        let (cell, lost) = place_on_baseline(&f, Point::new(4, 7), 1, [16, 12]);
        assert_eq!(lost, 0);
        assert_eq!(cell.get(8, 11), Some(Color::WHITE));
        assert_eq!(cell.get(4, 4), Some(Color::BLACK));
        let (cell, _) = place_on_baseline(&f, Point::new(4, 7), 2, [16, 16]);
        assert_eq!(cell.get(8, 15), Some(Color::WHITE));
        assert_eq!(cell.get(0, 0), Some(Color::BLACK));
        let (_, lost) = place_on_baseline(&f, Point::new(4, 7), 1, [4, 4]);
        assert_eq!(lost, 1, "the black pixel falls outside");
        let (cell, lost) = place_centered(&f, 1, [12, 12]);
        assert_eq!((cell.get(6, 9), lost), (Some(Color::WHITE), 0));
    }
}
