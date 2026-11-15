//! End-to-end over MCP: a tileset is built, autotiled, previewed and exported to Godot with the
//! real `psm mcp` binary (runtime started on demand, isolated endpoint and folders).

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
    let dir = std::env::temp_dir().join(format!("psm-mcp-tiles-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("work")).unwrap();
    Env { endpoint: format!("psm-mcp-tiles-{tag}-{}", std::process::id()), dir }
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
    ($client:expr, $name:expr, $args:expr $(,)?) => {{
        let r = $client.call_tool(CallToolRequestParams::new($name).with_arguments(args($args))).await.expect("tool call");
        assert_ne!(r.is_error, Some(true), "{} falhou: {:?}", $name, body(&r));
        r
    }};
}

fn image_of(r: &CallToolResult) -> Vec<u8> {
    let image = r.content.iter().find_map(|c| c.as_image()).expect("image content");
    assert_eq!(image.mime_type, "image/png");
    base64::engine::general_purpose::STANDARD.decode(&image.data).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tileset_autotile_and_godot_export_over_mcp() {
    let e = env("e2e");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    for name in [
        "tile_grid_set",
        "tile_info",
        "tile_collision_set",
        "tile_terrain_set",
        "autotile_generate",
        "tile_preview_map_set",
        "tile_preview_render",
        "export_godot_tileset",
    ] {
        let tool = tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("missing tool {name}"));
        assert!(!serde_json::to_string(&tool.input_schema).unwrap().contains("$ref"), "{name} schema is inline");
    }

    // A 3x3 frame + inner corners (4x3 tiles of 16x16): water inside, a darker border.
    call!(client, "project_create", json!({ "name": "Tiles MCP", "width": 64, "height": 48 }));
    call!(
        client,
        "draw_batch",
        json!({ "ops": [
            { "op": "rect", "x": 0, "y": 0, "w": 48, "h": 48, "fill": true, "color": "#2a6fdb" },
            { "op": "rect", "x": 0, "y": 0, "w": 48, "h": 48, "color": "#123a7a", "thickness": 2 },
            { "op": "rect", "x": 48, "y": 0, "w": 16, "h": 16, "fill": true, "color": "#2a6fdb" },
            { "op": "pixel", "x": 48, "y": 0, "color": "#123a7a" },
            { "op": "pixel", "x": 63, "y": 0, "color": "#123a7a" },
            { "op": "pixel", "x": 48, "y": 15, "color": "#123a7a" },
            { "op": "pixel", "x": 63, "y": 15, "color": "#123a7a" }
        ]}),
    );
    let grid = body(&call!(client, "tile_grid_set", json!({ "tile_width": 16 })));
    assert_eq!(grid["data"]["columns"], 4);
    assert_eq!(grid["data"]["rows"], 3);
    let info = body(&call!(client, "tile_info", json!({})));
    assert_eq!(info["data"]["non_empty"], 10, "two empty tiles under the inner corners");

    // Collisions and the preview overlay.
    call!(client, "tile_collision_set", json!({ "tiles": [[0, 0], [1, 0], [2, 0]], "shape": "full" }));
    let preview = call!(client, "frame_preview", json!({ "tiles": true, "scale": 4 }));
    assert_eq!(&image_of(&preview)[1..4], b"PNG");

    // Autotile → blob 47 with terrains, then the demo map rendered.
    let made = body(&call!(client, "autotile_generate", json!({ "input": "frame_3x3", "output": ["blob_47"], "terrain_name": "água" })));
    assert_eq!(made["data"]["size"], json!([128, 96]));
    assert_eq!(made["data"]["parts"]["blob_47"]["count"], 47);
    let sprite = made["data"]["name"].as_str().unwrap().to_string();
    let map = call!(client, "tile_preview_render", json!({ "sprite": sprite, "scale": 1 }));
    assert!(body(&map)["data"]["filled"].as_u64().unwrap() > 10);
    assert_eq!(&image_of(&map)[1..4], b"PNG");
    let auto_info = body(&call!(client, "tile_info", json!({ "sprite": sprite })));
    assert_eq!(auto_info["data"]["terrains"].as_array().unwrap().len(), 47);
    assert_eq!(auto_info["data"]["terrain_sets"][0]["terrains"][0]["name"], "água");

    // Godot TileSet export, written inside the work folder.
    let exported = body(&call!(client, "export_godot_tileset", json!({ "sprite": sprite, "path": "agua.tres" })));
    assert_eq!(exported["data"]["tiles"], 47);
    let tres = std::fs::read_to_string(e.dir.join("work/agua.tres")).unwrap();
    assert!(tres.starts_with("[gd_resource type=\"TileSet\" load_steps=3 format=3]"));
    assert!(tres.contains("[ext_resource type=\"Texture2D\" path=\"res://agua.png\" id=\"1_texture\"]"));
    assert!(tres.contains("6:5/0/terrains_peering_bit/top_left_corner = 0"), "last blob tile (mask 255) has every bit");
    assert!(tres.contains("terrain_set_0/terrain_0/name = \"água\""));
    assert!(e.dir.join("work/agua.png").exists());

    // Errors keep their codes.
    let bad = client
        .call_tool(CallToolRequestParams::new("tile_collision_set").with_arguments(args(json!({ "tile": [9, 9], "shape": "full" }))))
        .await
        .unwrap();
    assert_eq!(bad.is_error, Some(true));
    assert_eq!(body(&bad)["error"]["code"], "OUT_OF_BOUNDS");

    // Tile data survives save and reopen.
    call!(client, "project_save", json!({ "path": "tiles.psmproj" }));
    call!(client, "project_close", json!({}));
    call!(client, "project_open", json!({ "path": "tiles.psmproj" }));
    let reopened = body(&call!(client, "tile_info", json!({})));
    assert_eq!(reopened["data"]["collisions"].as_array().unwrap().len(), 3);
    client.cancel().await.ok();
}
