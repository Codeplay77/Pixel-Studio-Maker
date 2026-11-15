//! End-to-end over MCP (the real `psm mcp` binary, stdio, runtime on demand): palette files,
//! palette variants, the layered Aseprite import and normal maps.

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
    let dir = std::env::temp_dir().join(format!("psm-mcp-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("work")).unwrap();
    Env { endpoint: format!("psm-mcp-test-{tag}-{}", std::process::id()), dir }
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

fn image(r: &CallToolResult) -> sprite_core::Image {
    let img = r.content.iter().find_map(|c| c.as_image()).expect("image content");
    assert_eq!(img.mime_type, "image/png");
    let png = base64::engine::general_purpose::STANDARD.decode(&img.data).unwrap();
    sprite_core::codec::decode_png(&png).unwrap()
}

macro_rules! call {
    ($client:expr, $name:expr, $args:expr) => {{
        let r = $client.call_tool(CallToolRequestParams::new($name).with_arguments(args($args))).await.expect("tool call");
        assert_ne!(r.is_error, Some(true), "{} falhou: {:?}", $name, body(&r));
        r
    }};
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn palettes_variants_aseprite_and_normal_maps_over_mcp() {
    let e = env("paletas");
    let work = e.dir.join("work");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    for name in [
        "palette_import",
        "palette_export",
        "palette_variant_set",
        "palette_variant_delete",
        "palette_variant_list",
        "palette_variant_preview",
        "export_palette_variants",
        "export_normal_map",
    ] {
        let tool = tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("missing tool {name}"));
        assert!(tool.input_schema.get("properties").unwrap().get("project").is_some(), "{name} takes `project`");
    }

    // Palette file in, sprite drawn with it.
    std::fs::write(work.join("azul.hex"), "1d4b8f\n3976d2\nffffff\n").unwrap();
    call!(client, "project_create", json!({ "name": "Paletas MCP", "width": 8, "height": 8 }));
    let imported = body(&call!(client, "palette_import", json!({ "path": "azul.hex" })));
    assert_eq!(imported["data"]["palette"], "azul");
    call!(client, "draw_batch", json!({ "ops": [{ "op": "rect", "x": 1, "y": 1, "w": 6, "h": 6, "color": 1, "fill": true }] }));
    call!(client, "palette_export", json!({ "path": "azul.gpl" }));
    assert!(std::fs::read_to_string(work.join("azul.gpl")).unwrap().starts_with("GIMP Palette"));
    let bad =
        client.call_tool(CallToolRequestParams::new("palette_import").with_arguments(args(json!({ "path": "nada.hex" })))).await.unwrap();
    assert_eq!(bad.is_error, Some(true));

    // Variant: preview as an MCP image, then one PNG per variant; the project keeps its pixels.
    call!(client, "palette_variant_set", json!({ "name": "Vermelho", "replace": [{ "from": "#3976d2", "to": "#d23939" }] }));
    let preview = call!(client, "palette_variant_preview", json!({ "variant": "Vermelho", "scale": 1, "background": "none" }));
    assert_eq!(image(&preview).get(3, 3), Some(sprite_core::Color::rgb(0xd2, 0x39, 0x39)));
    let exported = body(&call!(client, "export_palette_variants", json!({ "directory": "variantes", "format": "png" })));
    assert_eq!(exported["data"]["files"].as_array().unwrap().len(), 1);
    let red = sprite_core::codec::decode_png(&std::fs::read(work.join("variantes/Paletas_MCP_Vermelho.png")).unwrap()).unwrap();
    assert_eq!(red.get(3, 3), Some(sprite_core::Color::rgb(0xd2, 0x39, 0x39)));
    let px = body(&call!(client, "pixel_get", json!({ "x": 3, "y": 3 })));
    assert_eq!(px["data"]["color"], "#3976d2");
    let list = body(&call!(client, "palette_variant_list", json!({})));
    assert_eq!(list["data"]["variants"][0]["name"], "Vermelho");

    // Normal map of the frame.
    let normal = call!(client, "export_normal_map", json!({ "path": "normal.png", "bevel": 2 }));
    assert_eq!(image(&normal).get(4, 4), Some(sprite_core::Color::rgb(128, 128, 255)));
    assert!(work.join("normal.png").exists());

    // Aseprite file with layers as a new sprite.
    std::fs::write(work.join("heroi.aseprite"), sprite_project::aseprite::builder::sample()).unwrap();
    let ase = body(&call!(client, "import_image", json!({ "path": "heroi.aseprite" })));
    assert_eq!(ase["data"]["format"], "aseprite");
    assert_eq!(ase["data"]["layers"].as_array().unwrap().len(), 3);
    assert_eq!(ase["data"]["animations"][0]["loop_mode"], "ping_pong");
    let px = body(&call!(client, "pixel_get", json!({ "sprite": "heroi", "x": 2, "y": 1 })));
    assert_eq!(px["data"]["color"], "#ff0000");

    // Variants survive save and reopen.
    call!(client, "project_save", json!({ "path": "paletas.psmproj" }));
    call!(client, "project_close", json!({}));
    call!(client, "project_open", json!({ "path": "paletas.psmproj" }));
    let list = body(&call!(client, "palette_variant_list", json!({})));
    assert_eq!(list["data"]["count"], 1);
    client.cancel().await.ok();
}
