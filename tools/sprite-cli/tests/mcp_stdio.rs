//! End-to-end: the real `psm mcp` binary over stdio, driven by an MCP client, with the runtime
//! started on demand. Verifies tools, pixels, images, resources, prompts and errors.

use std::path::PathBuf;

use base64::Engine;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, GetPromptRequestParams, ReadResourceRequestParams, ResourceContents};
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
        r
    }};
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_over_stdio_creates_inspects_and_saves_a_sprite() {
    let e = env("e2e");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");

    let info = client.peer_info().expect("server info");
    assert_eq!(info.server_info.as_ref().map(|s| s.name.as_str()), Some("pixel-studio-maker"));
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.len() >= 80, "only {} tools", tools.len());
    for name in
        ["project_create", "pixel_set", "draw_batch", "frame_preview", "animation_create", "export_godot_metadata", "transaction_begin"]
    {
        assert!(tools.iter().any(|t| t.name == name), "missing tool {name}");
    }
    let draw = tools.iter().find(|t| t.name == "draw_batch").unwrap();
    assert!(draw.input_schema.get("properties").unwrap().get("project").is_some());

    // Create and draw.
    let created = body(&call!(client, "project_create", json!({ "name": "Teste MCP", "width": 16, "height": 16 })));
    let project = created["project"].as_str().unwrap().to_string();
    call!(client, "palette_create", json!({ "name": "p", "colors": ["#1a1c2c", "#3976d2", "#ffffff"] }));
    let drawn = body(&call!(
        client,
        "draw_batch",
        json!({ "ops": [
            { "op": "circle", "center": [8, 8], "radius": 5, "color": 1, "fill": true },
            { "op": "outline", "color": 0 },
            { "op": "matrix", "at": [6, 6], "legend": { "w": 2 }, "rows": ["w..w"] }
        ]})
    ));
    assert!(drawn["data"]["changed_pixels"].as_u64().unwrap() > 60);
    let px = body(&call!(client, "pixel_get", json!({ "x": 6, "y": 6 })));
    assert_eq!(px["data"]["color"], "#ffffff");
    let edge = body(&call!(client, "pixel_get", json!({ "x": 8, "y": 2 })));
    assert_eq!(edge["data"]["color"], "#1a1c2c", "outline above the circle");

    // Visual inspection comes back as an MCP image.
    let preview = call!(client, "frame_preview", json!({ "scale": 8, "grid": true }));
    let image = preview.content.iter().find_map(|c| c.as_image()).expect("image content");
    assert_eq!(image.mime_type, "image/png");
    let png = base64::engine::general_purpose::STANDARD.decode(&image.data).unwrap();
    assert_eq!(&png[1..4], b"PNG");

    // Errors are tool errors with a code, not protocol failures.
    let bad = client
        .call_tool(CallToolRequestParams::new("pixel_set").with_arguments(args(json!({ "x": 99, "y": 0, "color": "#fff" }))))
        .await
        .unwrap();
    assert_eq!(bad.is_error, Some(true));
    assert_eq!(body(&bad)["error"]["code"], "OUT_OF_BOUNDS");
    let stale = client
        .call_tool(
            CallToolRequestParams::new("pixel_set")
                .with_arguments(args(json!({ "x": 0, "y": 0, "color": "#fff", "expected_revision": 0 }))),
        )
        .await
        .unwrap();
    assert_eq!(body(&stale)["error"]["code"], "REVISION_CONFLICT");

    // Save, close, reopen: pixels survive.
    call!(client, "project_save", json!({ "path": "teste.psmproj" }));
    assert!(e.dir.join("work/teste.psmproj").exists());
    call!(client, "project_close", json!({}));
    let reopened = body(&call!(client, "project_open", json!({ "path": "teste.psmproj" })));
    assert_ne!(reopened["project"], Value::Null);
    let px = body(&call!(client, "pixel_get", json!({ "x": 6, "y": 6 })));
    assert_eq!(px["data"]["color"], "#ffffff");

    // Picture to pixel art: a preview (image content, no project), then a new project.
    call!(client, "export_png", json!({ "path": "circulo.png", "scale": 8 }));
    let preview = call!(client, "image_pixelate", json!({ "path": "circulo.png", "max_side": 12, "colors": 3, "preview": true }));
    assert!(preview.content.iter().any(|c| c.as_image().is_some()), "prévia em imagem");
    assert_eq!(body(&preview)["data"]["size"], json!([12, 12]));
    assert!(body(&preview).get("project").is_none());
    let made = body(&call!(client, "image_pixelate", json!({ "path": "circulo.png", "max_side": 12, "colors": 3, "name": "circulo" })));
    assert_ne!(made["project"], Value::Null);
    assert_eq!(made["data"]["sprite"]["width"], 12);
    let px = body(&call!(client, "pixel_get", json!({ "x": 6, "y": 6 })));
    assert_eq!(px["project"], made["project"], "o projeto convertido virou o atual");

    // Resources and prompts.
    let guide = client.read_resource(ReadResourceRequestParams::new("psm://guide")).await.unwrap();
    match &guide.contents[0] {
        ResourceContents::TextResourceContents { text, .. } => assert!(text.contains("Guia do agente")),
        other => panic!("unexpected {other:?}"),
    }
    let frame = created["data"]["sprite"]["frame"].as_u64().unwrap();
    let blob = client
        .read_resource(ReadResourceRequestParams::new(format!("psm://project/{}/frame/{frame}.png", reopened["project"].as_str().unwrap())))
        .await
        .unwrap();
    assert!(matches!(&blob.contents[0], ResourceContents::BlobResourceContents { .. }));
    let prompt = client
        .get_prompt(GetPromptRequestParams::new("criar_personagem").with_arguments(args(json!({ "descricao": "um slime verde" }))))
        .await
        .unwrap();
    assert!(format!("{:?}", prompt.messages[0]).contains("slime verde"));

    let status = body(&call!(client, "editor_status", json!({})));
    assert!(status["data"]["clients"].as_array().unwrap().iter().any(|c| c["kind"] == "mcp"));
    let _ = project;
    client.cancel().await.ok();
}
