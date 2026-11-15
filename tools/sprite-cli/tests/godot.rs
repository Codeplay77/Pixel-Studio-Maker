//! Godot area, end to end: hitboxes, the scene export and export jobs through the real `psm mcp`
//! binary (MCP over stdio, runtime started on demand), and `psm export` from the command line.

use std::path::PathBuf;
use std::process::Command;

use base64::Engine;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::transport::TokioChildProcess;
use serde_json::{Value, json};

fn temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("psm-cli-godot-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("work")).unwrap();
    dir
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
async fn mcp_hitboxes_scene_and_export_on_save() {
    let dir = temp("mcp");
    let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_psm"));
    cmd.arg("mcp")
        .current_dir(dir.join("work"))
        .env("PSM_RUNTIME_PIPE", format!("psm-cli-godot-test-{}", std::process::id()))
        .env("PSM_DATA_DIR", dir.join("data"))
        .env("PSM_PROJECTS_DIR", dir.join("projects"))
        .env("PSM_RUNTIME_IDLE", "3");
    let client = ().serve(TokioChildProcess::new(cmd).unwrap()).await.expect("MCP handshake");
    let tools = client.list_all_tools().await.unwrap();
    for name in [
        "hitbox_create",
        "hitbox_update",
        "hitbox_set",
        "hitbox_list",
        "export_godot_scene",
        "export_job_set",
        "export_job_delete",
        "export_job_list",
        "export_jobs_run",
    ] {
        assert!(tools.iter().any(|t| t.name == name), "falta a ferramenta {name}");
    }
    let set = tools.iter().find(|t| t.name == "hitbox_set").unwrap();
    let props = set.input_schema.get("properties").unwrap().as_object().unwrap();
    for p in ["hitbox", "rect", "clear", "frame", "frames", "animation", "direction", "all_frames", "sprite", "project"] {
        assert!(props.contains_key(p), "hitbox_set sem `{p}`");
    }

    call!(client, "project_create", json!({ "name": "Ninja", "width": 16, "height": 16 }));
    call!(client, "draw_shape", json!({ "shape": "rect", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "color": "#3976d2", "fill": true }));
    let info = body(&call!(client, "project_info", json!({})));
    let base = info["data"]["sprites"][0]["frames"][0].as_u64().unwrap();
    call!(client, "animation_create", json!({ "name": "attack", "directions": ["E"], "frames": 3, "copy_from": base }));

    let made = body(&call!(
        client,
        "hitbox_create",
        json!({ "name": "golpe", "rect": { "x": 11, "y": 6, "w": 4, "h": 3 }, "frame": { "animation": "attack", "direction": "E", "index": 1 } })
    ));
    assert_eq!(made["data"]["hitbox"]["name"], "golpe");
    call!(client, "hitbox_create", json!({ "name": "corpo", "kind": "hurtbox" }));
    call!(client, "hitbox_set", json!({ "hitbox": "corpo", "rect": { "x": 5, "y": 3, "w": 6, "h": 13 }, "animation": "attack" }));
    let listed = body(&call!(client, "hitbox_list", json!({ "frame": { "animation": "attack", "direction": "E", "index": 1 } })));
    assert_eq!(listed["data"]["hitboxes"][0]["rect"]["x"], 11);

    // The preview shows the boxes as an MCP image.
    let preview = call!(
        client,
        "frame_preview",
        json!({ "frame": { "animation": "attack", "direction": "E", "index": 1 }, "scale": 4, "background": "none", "hitboxes": true })
    );
    let image = preview.content.iter().find_map(|c| c.as_image()).expect("image content");
    let png = base64::engine::general_purpose::STANDARD.decode(&image.data).unwrap();
    let img = sprite_core::codec::decode_png(&png).unwrap();
    assert_eq!(img.get(44, 24), Some(sprite_core::Color::rgb(255, 59, 48)), "contorno da hitbox na cor padrão");

    // Scene export.
    let scene = body(&call!(client, "export_godot_scene", json!({ "path": "godot/ninja.tscn" })));
    assert_eq!(scene["data"]["animations"][0]["name"], "attack_E");
    let tscn = std::fs::read_to_string(dir.join("work/godot/ninja.tscn")).unwrap();
    assert!(tscn.contains("[node name=\"golpe\" type=\"Area2D\""));
    assert!(tscn.contains("NodePath(\"golpe/Shape:disabled\")"));

    // Export on save.
    call!(
        client,
        "export_job_set",
        json!({ "name": "sheet", "command": "export_spritesheet", "args": { "path": "build/ninja.png" }, "on_save": true })
    );
    let jobs = body(&call!(client, "export_job_list", json!({})));
    assert_eq!(jobs["data"]["jobs"][0]["name"], "sheet");
    let saved = body(&call!(client, "project_save", json!({ "path": "proj/ninja.psmproj" })));
    assert_eq!(saved["data"]["exports"]["ran"], 1, "{saved}");
    assert!(dir.join("work/proj/build/ninja.png").exists());
    assert!(dir.join("work/proj/build/ninja.json").exists());
    let ran = body(&call!(client, "export_jobs_run", json!({ "jobs": ["sheet"] })));
    assert_eq!(ran["data"]["failed"], 0);
    client.cancel().await.ok();
}

#[test]
fn psm_export_runs_jobs_from_the_command_line() {
    use sprite_commands::{ExecContext, ProjectCreate, create_project, parse_command};
    let dir = temp("cli");
    let ctx = ExecContext::local(&dir);
    let mut s = create_project(
        &ProjectCreate {
            name: "lote".into(),
            width: 8,
            height: 8,
            sprite_name: Some("bau".into()),
            palette: vec![],
            palette_name: None,
            background: None,
        },
        &ctx,
    )
    .unwrap();
    let mut run = |name: &str, a: Value| s.execute(parse_command(name, a).unwrap(), &ctx).unwrap();
    run("pixel_set", json!({ "x": 1, "y": 1, "color": "#ff0000" }));
    run("hitbox_create", json!({ "name": "tampa", "kind": "collision", "rect": { "x": 0, "y": 0, "w": 8, "h": 3 }, "all_frames": true }));
    run(
        "export_job_set",
        json!({ "name": "cena", "command": "export_godot_scene", "args": { "path": "godot/bau.tscn", "res_dir": "res://itens" } }),
    );
    run("export_job_set", json!({ "name": "png", "command": "export_png", "args": { "path": "png/bau.png", "scale": 4 } }));
    run("export_job_set", json!({ "name": "quebrada", "command": "export_gif", "args": { "path": "bau.gif", "animation": "nada" } }));
    let file = dir.join("work/bau.psmproj");
    sprite_project::format::save(&s.doc, &file).unwrap();

    // Named jobs, run from another folder: paths start at the project's folder.
    let ok = Command::new(env!("CARGO_BIN_EXE_psm"))
        .args(["export", file.to_str().unwrap(), "--job", "cena", "--job", "png"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let out: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(out["ran"], 2);
    assert!(dir.join("work/godot/bau.tscn").exists());
    let tscn = std::fs::read_to_string(dir.join("work/godot/bau.tscn")).unwrap();
    assert!(tscn.contains("path=\"res://itens/bau.tres\""));
    assert!(tscn.contains("groups=[\"collision\"]"));
    let png = sprite_core::codec::decode_png(&std::fs::read(dir.join("work/png/bau.png")).unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (32, 32));

    // All jobs: one fails → exit code 1, the others still ran.
    let _ = std::fs::remove_file(dir.join("work/png/bau.png"));
    let bad = Command::new(env!("CARGO_BIN_EXE_psm")).args(["export", file.to_str().unwrap()]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("1 de 3"), "{}", String::from_utf8_lossy(&bad.stderr));
    assert!(dir.join("work/png/bau.png").exists());
    // Unknown job name.
    let unknown = Command::new(env!("CARGO_BIN_EXE_psm")).args(["export", file.to_str().unwrap(), "--job", "nada"]).output().unwrap();
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("nada"));
    // The help lists the command.
    let help = Command::new(env!("CARGO_BIN_EXE_psm")).arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("psm export ARQUIVO.psmproj"));
}
