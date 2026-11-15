//! End-to-end: RPG Maker tools through the real `psm mcp` binary over stdio (isolated runtime):
//! a template sprite with named regions, a character sheet and a side-view battler built from
//! animations, the files on disk, the preview images and the errors.

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
    let dir = std::env::temp_dir().join(format!("psm-mcp-rpgmaker-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("work")).unwrap();
    Env { endpoint: format!("psm-mcp-test-rpgmaker-{tag}-{}", std::process::id()), dir }
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

fn png(path: PathBuf) -> sprite_core::Image {
    sprite_core::codec::decode_png(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rpgmaker_tools_over_mcp() {
    let e = env("e2e");
    let client = ().serve(TokioChildProcess::new(command(&e)).unwrap()).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    for name in ["rpgmaker_template", "export_rpgmaker_character", "export_rpgmaker_faces", "export_rpgmaker_battler"] {
        let tool = tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("missing tool {name}"));
        assert!(tool.input_schema.get("properties").unwrap().get("project").is_some(), "{name}");
    }

    // A project in the size of a tileset, then the template applied to its sprite.
    let created = body(&call!(client, "project_create", json!({ "name": "Jogo", "width": 768, "height": 576 })));
    let sprite = created["data"]["sprite"]["id"].as_u64().unwrap();
    let t = body(&call!(client, "rpgmaker_template", json!({ "kind": "tileset_a2", "sprite": sprite })));
    assert_eq!(t["data"]["regions"].as_array().unwrap().len(), 32);
    assert_eq!(t["data"]["tile"], 48);
    let regions = body(&call!(client, "region_list", json!({ "sprite": sprite })));
    assert!(regions.to_string().contains("A2 bloco 7,3"), "{regions}");

    // A walking hero: 4 directions × 3 frames, a different color per direction at the pivot.
    call!(client, "sprite_create", json!({ "name": "heroi", "width": 32, "height": 32 }));
    call!(client, "animation_create", json!({ "sprite": "heroi", "name": "walk", "directions": ["S", "W", "E", "N"], "frames": 3 }));
    let colors = ["#ff0000", "#00ff00", "#0000ff", "#ffff00"];
    for (dir, color) in ["S", "W", "E", "N"].iter().zip(colors) {
        for i in 0..3 {
            call!(
                client,
                "draw_batch",
                json!({ "sprite": "heroi", "frame": { "animation": "walk", "direction": dir, "index": i }, "ops": [
                    { "op": "rect", "x": 12, "y": 20, "w": 8, "h": 12, "color": color, "fill": true }
                ]})
            );
        }
    }
    let sheet = call!(
        client,
        "export_rpgmaker_character",
        json!({ "path": "img/characters/Heroi.png", "single": true, "characters": [{ "sprite": "heroi", "animation": "walk" }] })
    );
    assert!(sheet.content.iter().any(|c| c.as_image().is_some()), "prévia em imagem");
    assert_eq!(body(&sheet)["data"]["file"], "$Heroi.png");
    let img = png(e.dir.join("work/img/characters/$Heroi.png"));
    assert_eq!((img.width(), img.height()), (144, 192));
    // Pivot (16, 31) -> bottom center (24, 47) of each 48x48 cell; rows S, W, E, N.
    for (row, color) in colors.iter().enumerate() {
        let c = sprite_core::Color::parse(color).unwrap();
        assert_eq!(img.get(24, row as i32 * 48 + 47), Some(c), "linha {row}");
        assert_eq!(img.get(2 * 48 + 24 - 4, row as i32 * 48 + 36), Some(c), "linha {row}, coluna 2");
    }

    // Side-view battler from the same animation (first track), and the VX Ace error.
    let battler = body(&call!(
        client,
        "export_rpgmaker_battler",
        json!({ "sprite": "heroi", "path": "img/sv_actors/Heroi.png", "motions": { "wait": "walk", "dead": { "animation": "walk", "direction": "N" } } })
    ));
    assert_eq!(battler["data"]["size"], json!([576, 384]));
    let img = png(e.dir.join("work/img/sv_actors/Heroi.png"));
    assert_eq!(img.get(32, 64 + 63), Some(sprite_core::Color::parse("#ff0000").unwrap()), "wait = linha 1");
    assert_eq!(img.get(6 * 64 + 32, 5 * 64 + 63), Some(sprite_core::Color::parse("#ffff00").unwrap()), "dead = coluna 6, linha 5");
    let bad = client
        .call_tool(
            CallToolRequestParams::new("export_rpgmaker_battler")
                .with_arguments(args(json!({ "sprite": "heroi", "path": "x.png", "engine": "vxace", "motions": { "walk": "walk" } }))),
        )
        .await
        .unwrap();
    assert_eq!(bad.is_error, Some(true));
    assert_eq!(body(&bad)["error"]["code"], "UNSUPPORTED");

    // Faces and the template survive save / reopen.
    call!(
        client,
        "export_rpgmaker_faces",
        json!({ "sprite": "heroi", "path": "img/faces/Heroi.png", "faces": [{ "frame": { "animation": "walk", "direction": "S", "index": 0 } }] })
    );
    assert_eq!(png(e.dir.join("work/img/faces/Heroi.png")).width(), 576);
    call!(client, "project_save", json!({ "path": "jogo.psmproj" }));
    call!(client, "project_close", json!({}));
    call!(client, "project_open", json!({ "path": "jogo.psmproj" }));
    let regions = body(&call!(client, "region_list", json!({ "sprite": sprite })));
    assert!(regions.to_string().contains("A2 bloco 0,0"), "{regions}");
    client.cancel().await.ok();
}
