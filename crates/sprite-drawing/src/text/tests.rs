use super::*;

const RED: Color = Color::rgb(255, 0, 0);
const BLUE: Color = Color::rgb(0, 0, 255);
const GREEN: Color = Color::rgb(0, 255, 0);

fn spec(text: &str, x: i32, y: i32) -> TextSpec {
    let mut s = TextSpec::new(text, Point::new(x, y));
    s.color = Some(ColorRef::Value("#ff0000".into()));
    s
}

fn p57() -> &'static BitmapFont {
    builtin_font("pixel-5x7").unwrap()
}

fn draw_on(img: &mut Image, s: &TextSpec) -> (TextLayout, crate::DrawStats) {
    let mut c = Canvas::new(img, None);
    let lay = draw(&mut c, s, op_font(s, None).unwrap(), &[]).unwrap();
    (lay, c.stats())
}

#[test]
fn letters_land_on_the_right_pixels() {
    let mut img = Image::new(20, 20);
    let (lay, _) = draw_on(&mut img, &spec("A", 2, 3));
    // The line box starts at `at`; capitals start 2 rows down (accent zone).
    assert_eq!(lay.rect, Rect::new(2, 3, 5, 11));
    assert_eq!(img.get(3, 5), Some(RED), "topo do A");
    assert_eq!(img.get(2, 5), Some(Color::TRANSPARENT));
    assert_eq!(img.get(2, 6), Some(RED));
    assert_eq!(img.get(6, 11), Some(RED), "perna direita na linha de base");
    assert_eq!(img.get(2, 3), Some(Color::TRANSPARENT), "zona dos acentos vazia");
    assert_eq!(img.opaque_count(), 3 + 2 * 6 + 3);
}

#[test]
fn alignment_without_and_with_max_width() {
    let f = p57();
    // "AB" = 5 + 1 + 5 = 11 px.
    let mut s = spec("AB", 10, 0);
    s.align = TextAlign::Center;
    assert_eq!(layout(&s, f).unwrap().rect.x, 5);
    s.align = TextAlign::Right;
    let lay = layout(&s, f).unwrap();
    assert_eq!((lay.rect.x, lay.rect.right() - 1), (0, 10), "termina na coluna 10");
    let mut s = spec("A", 4, 0);
    s.max_width = Some(20);
    s.align = TextAlign::Center;
    let lay = layout(&s, f).unwrap();
    assert_eq!(lay.rect, Rect::new(4, 0, 20, 11));
    assert_eq!(lay.lines[0].x, 4 + 7);
    s.align = TextAlign::Right;
    assert_eq!(layout(&s, f).unwrap().lines[0].x, 4 + 15);
}

#[test]
fn wraps_by_word_and_breaks_long_words() {
    let f = p57();
    let mut s = spec("AB CD", 0, 0);
    s.max_width = Some(11);
    let lay = layout(&s, f).unwrap();
    let texts: Vec<&str> = lay.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["AB", "CD"]);
    assert_eq!(lay.lines[1].y, 12);
    assert_eq!(lay.rect.h, 12 + 11);
    s.text = "ABCDEFG".into();
    let lay = layout(&s, f).unwrap();
    let texts: Vec<&str> = lay.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["AB", "CD", "EF", "G"]);
    s.text = "uma linha\nduas".into();
    s.max_width = None;
    assert_eq!(layout(&s, f).unwrap().lines.len(), 2);
    s.line_spacing = 2;
    assert_eq!(layout(&s, f).unwrap().lines[1].y, 14);
}

#[test]
fn scale_enlarges_every_pixel() {
    let mut img = Image::new(16, 30);
    let mut s = spec("I", 0, 0);
    s.scale = 2;
    let (lay, _) = draw_on(&mut img, &s);
    assert_eq!(lay.rect, Rect::new(0, 0, 6, 22));
    // Row 0 of the I ("###") is line row 2 → pixels y = 4..6.
    for (x, y) in [(0, 4), (5, 5), (2, 6), (3, 7), (0, 16), (5, 17)] {
        assert_eq!(img.get(x, y), Some(RED), "({x},{y})");
    }
    assert_eq!(img.get(0, 6), Some(Color::TRANSPARENT));
    assert_eq!(img.opaque_count(), (3 + 5 + 3) * 4);
}

#[test]
fn outline_and_shadow_stay_inside_the_box() {
    let f = p57();
    let mut s = spec(".", 0, 0);
    s.outline = Some(ColorRef::Value("#0000ff".into()));
    let mut img = Image::new(10, 20);
    let (lay, _) = draw_on(&mut img, &s);
    assert_eq!(lay.rect, Rect::new(0, 0, 3, 13));
    // The dot (line row 8) moves 1 px right and down to make room for the outline.
    assert_eq!(img.get(1, 9), Some(RED));
    for (x, y) in [(0, 8), (2, 8), (0, 10), (2, 10), (1, 8), (1, 10)] {
        assert_eq!(img.get(x, y), Some(BLUE), "contorno em ({x},{y})");
    }
    assert_eq!(img.opaque_count(), 9);

    let mut s = spec(".", 0, 0);
    s.shadow = Some(TextShadow { color: ColorRef::Value("#00ff00".into()), offset: Point::new(2, 1) });
    let lay = layout(&s, f).unwrap();
    assert_eq!(lay.rect, Rect::new(0, 0, 3, 12));
    let mut img = Image::new(10, 20);
    draw_on(&mut img, &s);
    assert_eq!(img.get(0, 8), Some(RED));
    assert_eq!(img.get(2, 9), Some(GREEN));
    // Negative offsets push the text right/down instead.
    s.shadow = Some(TextShadow { color: ColorRef::Value("#00ff00".into()), offset: Point::new(-1, -1) });
    let mut img = Image::new(10, 20);
    draw_on(&mut img, &s);
    assert_eq!(img.get(1, 9), Some(RED));
    assert_eq!(img.get(0, 8), Some(GREEN));
}

#[test]
fn measure_matches_what_is_drawn() {
    let mut s = spec("Olá, Mundo!\nçÑ ¿Qué? 42%", 3, 2);
    s.outline = Some(ColorRef::Value("#000000".into()));
    s.shadow = Some(TextShadow { color: ColorRef::Value("#00000080".into()), offset: Point::new(1, 2) });
    s.scale = 2;
    let font = op_font(&s, None).unwrap();
    let lay = layout(&s, font).unwrap();
    let colors = TextColors::resolve(&s, font, &[]).unwrap();
    let ink = render(&lay, font, &s, &colors).unwrap().ink().unwrap();
    let mut img = Image::new(256, 96);
    let (drawn, stats) = draw_on(&mut img, &s);
    assert_eq!(drawn, lay);
    assert_eq!(stats.rect(), Some(ink), "a caixa dos pixels desenhados é a medida");
    assert_eq!(ink.intersect(&lay.rect), ink, "nada sai da caixa");
    assert!(lay.missing.is_empty());
}

#[test]
fn missing_glyphs_draw_a_replacement_box() {
    let f = p57();
    let mut img = Image::new(20, 12);
    let (lay, _) = draw_on(&mut img, &spec("A€", 0, 0));
    assert_eq!(lay.missing, vec!['€']);
    assert_eq!(lay.rect.w, 5 + 1 + f.replacement.advance as u32);
    // Replacement: hollow 4x7 box right after the A and its spacing.
    assert_eq!(img.get(6, 2), Some(RED));
    assert_eq!(img.get(9, 8), Some(RED));
    assert_eq!(img.get(7, 4), Some(Color::TRANSPARENT));
}

#[test]
fn colors_and_errors() {
    let f = p57();
    let mut s = spec("A", 0, 0);
    s.color = None;
    let mut img = Image::new(8, 12);
    let mut c = Canvas::new(&mut img, None);
    assert!(draw(&mut c, &s, f, &[]).unwrap_err().to_string().contains("color"));
    s.color = Some(ColorRef::Index(1));
    draw(&mut c, &s, f, &[Color::BLACK, BLUE]).unwrap();
    assert_eq!(img.get(1, 2), Some(BLUE));
    let mut c = Canvas::new(&mut img, None);
    assert!(draw(&mut c, &spec("", 0, 0), f, &[]).is_err());
    let mut big = spec("A", 0, 0);
    big.scale = 17;
    assert!(layout(&big, f).is_err());
    let mut narrow = spec("A", 0, 0);
    narrow.max_width = Some(1);
    narrow.outline = Some(ColorRef::Value("#000".into()));
    assert!(layout(&narrow, f).is_err());
    assert!(op_font(&TextSpec { font: ItemRef::Name("Minha".into()), ..spec("A", 0, 0) }, None).is_err());
    let empty = layout(&spec("", 5, 5), f).unwrap();
    assert_eq!((empty.rect.w, empty.rect.h), (0, 0));
}

#[test]
fn letter_spacing_override_and_sprite_font_colors() {
    let mut s = spec("AB", 0, 0);
    s.letter_spacing = Some(3);
    let mut img = Image::new(20, 12);
    draw_on(&mut img, &s);
    assert_eq!(img.get(8, 3), Some(RED), "B começa em 5 + 3");
    assert_eq!(img.get(6, 3), Some(Color::TRANSPARENT));

    // A sprite font without `color` keeps its own colors.
    let mut sheet = Image::new(4, 4);
    sheet.set(0, 0, GREEN);
    sheet.set(1, 0, BLUE);
    let def = SpriteFontDef {
        cell: [2, 2],
        charset: "xy".into(),
        columns: 2,
        baseline: 2,
        letter_spacing: 1,
        space_width: 1,
        line_height: 3,
        frame: None,
    };
    let (font, empty) = font_from_sheet("cores", &sheet, &def).unwrap();
    assert_eq!(empty, vec!['y']);
    let mut s = spec("xx", 0, 0);
    s.font = ItemRef::Name("cores".into());
    s.color = None;
    let mut img = Image::new(8, 4);
    let mut c = Canvas::new(&mut img, None);
    draw(&mut c, &s, &font, &[]).unwrap();
    assert_eq!(img.get(0, 0), Some(GREEN));
    assert_eq!(img.get(1, 0), Some(BLUE));
    assert_eq!(img.get(3, 0), Some(GREEN), "avanço 2 + espaçamento 1");
}

#[test]
fn mini_font_is_compact() {
    let f = builtin_font("mini-3x5").unwrap();
    let mut s = spec("HI", 0, 0);
    s.font = ItemRef::Name("mini-3x5".into());
    let lay = layout(&s, f).unwrap();
    assert_eq!(lay.rect, Rect::new(0, 0, 7, 9));
    let mut img = Image::new(8, 9);
    draw_on(&mut img, &s);
    assert_eq!(img.get(0, 2), Some(RED));
    assert_eq!(img.get(1, 2), Some(Color::TRANSPARENT));
    assert_eq!(img.get(4, 2), Some(RED));
}

#[test]
fn sdc_text_op_parses_and_draws() {
    let ops: Vec<crate::sdc::DrawOp> = serde_json::from_str(
        r##"[{"op":"text","text":"Oi","at":[1,0],"color":0,"font":"mini-3x5"},
             {"op":"text","text":"!","at":[12,0],"color":"#ff0000","outline":"#0000ff","shadow":{"color":"#00ff00"}}]"##,
    )
    .unwrap();
    assert_eq!(ops[0].name(), "text");
    let mut img = Image::new(20, 14);
    let ctx = crate::sdc::DrawContext { palette: &[BLUE], source: None };
    let stats = crate::sdc::apply_all(&mut img, None, &ops, &ctx).unwrap();
    assert!(stats.changed > 10);
    assert_eq!(img.get(2, 2), Some(BLUE), "topo do O da mini-3x5");
    // "!" with outline: the bar starts at line row 2, 1 px in.
    assert_eq!(img.get(13, 3), Some(RED));
    assert_eq!(img.get(12, 3), Some(BLUE));
    // A font that is neither built-in nor loaded fails with a clear message.
    let bad: Vec<crate::sdc::DrawOp> = serde_json::from_str(r##"[{"op":"text","text":"x","font":"MinhaFonte","color":0}]"##).unwrap();
    let err = crate::sdc::apply_all(&mut img, None, &bad, &ctx).unwrap_err().1;
    assert!(err.to_string().contains("font_define"), "{err}");
    // Round trip: the loaded font never shows up in JSON.
    let text = serde_json::to_string(&ops[1]).unwrap();
    assert!(text.contains("\"op\":\"text\"") && !text.contains("font_data"), "{text}");
    let schema = serde_json::to_string(&schemars::schema_for!(Vec<crate::sdc::DrawOp>)).unwrap();
    assert!(schema.contains("max_width") && schema.contains("letter_spacing") && !schema.contains("font_data"));
}
