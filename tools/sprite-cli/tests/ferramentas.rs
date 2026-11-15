//! End-to-end through the real `psm mcp` binary: brush strokes with symmetry, wrap and shading,
//! magic wand and lasso selections, and the masked selection actions.

use std::path::PathBuf;

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

macro_rules! call {
    ($client:expr, $name:expr, $args:expr) => {{
        let r = $client.call_tool(CallToolRequestParams::new($name).with_arguments(args($args))).await.expect("tool call");
        assert_ne!(r.is_error, Some(true), "{} falhou: {:?}", $name, body(&r));
        body(&r)
    }};
}

macro_rules! color_at {
    ($client:expr, $x:expr, $y:expr) => {{
        let px = call!($client, "pixel_get", json!({ "x": $x, "y": $y }));
        px["data"]["color"].as_str().unwrap().to_string()
    }};
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_brush_strokes_and_mask_selections() {
    let e = env("ferramentas");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");

    let tools = client.list_all_tools().await.unwrap();
    for name in ["select_color", "select_polygon", "selection_read"] {
        let tool = tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("missing tool {name}"));
        assert!(!serde_json::to_string(&tool.input_schema).unwrap().contains("$ref"));
    }
    let draw = tools.iter().find(|t| t.name == "draw_batch").unwrap();
    let schema = serde_json::to_string(&draw.input_schema).unwrap();
    assert!(schema.contains("\"stroke\"") && schema.contains("symmetry") && schema.contains("wrap"), "stroke no schema do draw_batch");

    call!(client, "project_create", json!({ "name": "Ferramentas MCP", "width": 8, "height": 8 }));
    call!(client, "palette_create", json!({ "name": "azuis", "colors": ["#102040", "#2050a0", "#40a0ff", "#c0e0ff"] }));

    // A stroke mirrored on both axes (center 3.5) and one that wraps across the right edge.
    let out = call!(
        client,
        "draw_batch",
        json!({ "ops": [
            { "op": "stroke", "points": [[0, 0], [1, 0]], "color": 1, "symmetry": { "x": true, "y": true } },
            { "op": "stroke", "points": [[6, 3], [9, 3]], "color": 1, "wrap": true }
        ]})
    );
    assert_eq!(out["data"]["changed_pixels"], 8 + 4);
    for (x, y) in [(0, 0), (1, 0), (6, 0), (7, 0), (0, 7), (7, 7), (0, 3), (1, 3), (6, 3), (7, 3)] {
        assert_eq!(color_at!(client, x, y), "#2050a0", "pixel ({x},{y})");
    }
    assert_eq!(color_at!(client, 2, 3), "#00000000");

    // Shade: lighten once along the active palette, even over a back-and-forth stroke.
    call!(client, "draw_batch", json!({ "ops": [{ "op": "stroke", "points": [[0, 3], [7, 3], [0, 3]], "mode": "shade", "wrap": true }] }));
    assert_eq!(color_at!(client, 0, 3), "#40a0ff");
    assert_eq!(color_at!(client, 3, 3), "#00000000", "transparent stays");

    // Magic wand on row 3, then delete only that selection.
    let wand = call!(client, "select_color", json!({ "x": 0, "y": 3 }));
    assert_eq!(wand["data"]["count"], 2, "contiguous: (0,3) and (1,3)");
    let global = call!(client, "select_color", json!({ "x": 0, "y": 3, "contiguous": false }));
    assert_eq!(global["data"]["count"], 4);
    let mask = json!({ "bounds": global["data"]["bounds"], "rows": global["data"]["rows"] });
    call!(client, "pixel_clear", json!({ "mask": mask }));
    assert_eq!(color_at!(client, 7, 3), "#00000000");
    assert_eq!(color_at!(client, 0, 0), "#2050a0", "outside the mask stays");

    // Lasso around the top-left corner, minus a pixel, then move only those pixels down.
    let lasso = call!(client, "select_polygon", json!({ "points": [[0, 0], [1, 0], [1, 1], [0, 1]] }));
    assert_eq!(lasso["data"]["count"], 4);
    let base = json!({ "bounds": lasso["data"]["bounds"], "rows": lasso["data"]["rows"] });
    let cut = call!(client, "select_polygon", json!({ "points": [[1, 0]], "base": base, "mode": "subtract" }));
    assert_eq!(cut["data"]["count"], 3);
    let mask = json!({ "bounds": cut["data"]["bounds"], "rows": cut["data"]["rows"] });
    let copied = call!(client, "selection_read", json!({ "mask": mask }));
    assert_eq!(copied["data"]["count"], 1, "only (0,0) is painted inside the selection");
    let moved = call!(client, "draw_transform", json!({ "transform": { "kind": "move", "dx": 0, "dy": 4 }, "mask": mask }));
    assert_eq!(moved["data"]["selection"]["bounds"], json!({ "x": 0, "y": 4, "w": 2, "h": 2 }));
    assert_eq!(color_at!(client, 0, 4), "#2050a0");
    assert_eq!(color_at!(client, 0, 0), "#00000000");
    assert_eq!(color_at!(client, 1, 0), "#2050a0", "the subtracted pixel did not move");

    // Paste what was read (before the move) back at the corner.
    call!(client, "pixel_write_region", json!({ "at": [0, 0], "rows": copied["data"]["rows"], "legend": copied["data"]["legend"] }));
    assert_eq!(color_at!(client, 0, 0), "#2050a0");

    client.cancel().await.ok();
}
