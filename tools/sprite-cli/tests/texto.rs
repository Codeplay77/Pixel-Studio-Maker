//! End-to-end over MCP (real `psm mcp` over stdio, runtime on demand): text, fonts drawn in a
//! sprite, BMFont export and 9-slice export to Godot.

use std::path::PathBuf;

use base64::Engine;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::transport::TokioChildProcess;
use serde_json::{Value, json};

struct Env {
    dir: PathBuf,
    endpoint: String,
}

fn env(tag: &str) -> Env {
    let dir = std::env::temp_dir().join(format!("psm-mcp-texto-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("work")).unwrap();
    Env { endpoint: format!("psm-mcp-texto-{tag}-{}", std::process::id()), dir }
}

fn command(e: &Env) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_psm"));
    cmd.arg("mcp")
        .current_dir(e.dir.join("work"))
        .env("PSM_RUNTIME_PIPE", &e.endpoint)
        .env("PSM_DATA_DIR", e.dir.join("data"))
        .env("PSM_PROJECTS_DIR", e.dir.join("projects"))
        .env("PSM_RUNTIME_IDLE", "3");
    cmd
}

fn args(v: Value) -> serde_json::Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn body(r: &CallToolResult) -> Value {
    let text = r.content.iter().find_map(|c| c.as_text()).map(|t| t.text.clone()).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

macro_rules! call {
    ($client:expr, $name:expr, $args:expr) => {{
        let r = $client.call_tool(CallToolRequestParams::new($name).with_arguments(args($args))).await.expect("tool call");
        assert_ne!(r.is_error, Some(true), "{} falhou: {:?}", $name, body(&r));
        r
    }};
}

fn png_of(r: &CallToolResult) -> Vec<u8> {
    let image = r.content.iter().find_map(|c| c.as_image()).expect("image content");
    assert_eq!(image.mime_type, "image/png");
    base64::engine::general_purpose::STANDARD.decode(&image.data).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_text_fonts_and_nine_slice() {
    let e = env("e2e");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    for name in [
        "draw_text",
        "text_measure",
        "font_define",
        "font_info",
        "font_template",
        "export_bitmap_font",
        "nine_slice_set",
        "nine_slice_render",
        "export_nine_slice",
    ] {
        assert!(tools.iter().any(|t| t.name == name), "missing tool {name}");
    }
    let draw = tools.iter().find(|t| t.name == "draw_text").unwrap();
    let props = draw.input_schema.get("properties").unwrap();
    for field in ["text", "at", "font", "max_width", "outline", "shadow", "project", "expected_revision"] {
        assert!(props.get(field).is_some(), "draw_text sem {field}");
    }

    // Text with a built-in font: measure, then draw; pixels where the measure says.
    call!(client, "project_create", json!({ "name": "Texto MCP", "width": 64, "height": 32 }));
    let m = body(&call!(client, "text_measure", json!({ "text": "HP 10", "at": [2, 1], "font": "mini-3x5" })));
    assert_eq!(m["data"]["box"], json!({ "x": 2, "y": 1, "w": 17, "h": 9 }));
    let drawn = body(&call!(client, "draw_text", json!({ "text": "HP 10", "at": [2, 1], "font": "mini-3x5", "color": "#ffcc00" })));
    assert_eq!(drawn["data"]["region"], m["data"]["ink"]);
    let px = body(&call!(client, "pixel_get", json!({ "x": 2, "y": 3 })));
    assert_eq!(px["data"]["color"], "#ffcc00", "topo do H");
    // The same text as an SDC op inside draw_batch, with a missing glyph warning.
    let batch = body(&call!(
        client,
        "draw_batch",
        json!({ "ops": [{ "op": "text", "text": "★", "at": [40, 1], "color": "#ffffff", "outline": "#000000" }] })
    ));
    assert!(batch["warnings"].to_string().contains("U+2605"), "{batch}");

    // A font drawn in a sprite: template from the built-in font, then use it.
    let tpl = body(&call!(client, "font_template", json!({ "name": "Titulo", "base_font": "pixel-5x7", "color": "#33aaff" })));
    assert_eq!(tpl["data"]["cell"], json!([6, 12]));
    let info = call!(client, "font_info", json!({ "font": "Titulo" }));
    assert_eq!(body(&info)["data"]["kind"], "sprite");
    assert_eq!(&png_of(&info)[1..4], b"PNG");
    call!(client, "draw_text", json!({ "sprite": "Texto MCP", "text": "Oi", "at": [2, 14], "font": "Titulo" }));
    let px = body(&call!(client, "pixel_get", json!({ "sprite": "Texto MCP", "x": 3, "y": 16 })));
    assert_eq!(px["data"]["color"], "#33aaff", "O da fonte de sprite com a cor dela");

    // BMFont export for Godot.
    let out = body(&call!(client, "export_bitmap_font", json!({ "font": "Titulo", "path": "fontes/titulo.fnt" })));
    assert!(out["data"]["chars"].as_u64().unwrap() > 140);
    let fnt = std::fs::read_to_string(e.dir.join("work/fontes/titulo.fnt")).unwrap();
    assert!(fnt.starts_with("info face=\"Titulo\" "));
    assert!(fnt.contains("page id=0 file=\"titulo.png\""));
    assert!(fnt.contains(&format!("char id={} ", 'ç' as u32)));
    assert!(e.dir.join("work/fontes/titulo.png").exists());

    // 9-slice: margins, preview image, Godot scene.
    call!(client, "sprite_create", json!({ "name": "painel", "width": 12, "height": 12 }));
    call!(
        client,
        "draw_batch",
        json!({ "sprite": "painel", "ops": [
            { "op": "rect", "x": 0, "y": 0, "w": 12, "h": 12, "color": "#222034", "fill": true },
            { "op": "rect", "x": 0, "y": 0, "w": 12, "h": 12, "color": "#cbdbfc" }
        ]})
    );
    let bad = client
        .call_tool(CallToolRequestParams::new("nine_slice_set").with_arguments(args(json!({ "sprite": "painel", "all": 6 }))))
        .await
        .unwrap();
    assert_eq!(bad.is_error, Some(true));
    assert_eq!(body(&bad)["error"]["code"], "INVALID_ARGUMENT");
    call!(client, "nine_slice_set", json!({ "sprite": "painel", "all": 3 }));
    let r = call!(client, "nine_slice_render", json!({ "sprite": "painel", "width": 48, "height": 24, "edges": "tile", "scale": 2 }));
    assert_eq!(body(&r)["data"]["size"], json!([48, 24]));
    assert_eq!(&png_of(&r)[1..4], b"PNG");
    call!(client, "export_nine_slice", json!({ "sprite": "painel", "path": "ui/painel.tscn", "axis_stretch_horizontal": "tile" }));
    let scene = std::fs::read_to_string(e.dir.join("work/ui/painel.tscn")).unwrap();
    assert!(scene.contains("type=\"NinePatchRect\""));
    assert!(scene.contains("patch_margin_left = 3"));
    assert!(scene.contains("axis_stretch_horizontal = 1"));
    assert!(e.dir.join("work/ui/painel.png").exists() && e.dir.join("work/ui/painel.json").exists());

    // Everything survives save and reopen.
    call!(client, "project_save", json!({ "path": "texto.psmproj" }));
    call!(client, "project_close", json!({}));
    call!(client, "project_open", json!({ "path": "texto.psmproj" }));
    let info = body(&call!(client, "font_info", json!({ "font": "Titulo", "widths": false })));
    assert_eq!(info["data"]["definition"]["cell"], json!([6, 12]));
    let r = call!(client, "nine_slice_render", json!({ "sprite": "painel", "width": 24, "height": 24 }));
    assert_eq!(body(&r)["data"]["margins"]["left"], 3);

    client.cancel().await.ok();
}
