//! Saving through the runtime runs the project's `on_save` export jobs (Godot area), with an
//! isolated runtime (own endpoint and folders).

use std::time::Duration;

use serde_json::json;
use sprite_runtime::{ClientHello, RuntimeClient, RuntimeConfig};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn project_save_runs_on_save_exports_and_keeps_the_save_when_one_fails() {
    let base = std::env::temp_dir().join(format!("psm-rt-godot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mut cfg = RuntimeConfig::new(format!("psm-test-godot-save-{}", std::process::id()));
    cfg.autosave_dir = base.join("autosave");
    cfg.projects_dir = base.join("projects");
    cfg.idle_timeout = Duration::from_secs(5);
    std::fs::create_dir_all(&cfg.projects_dir).unwrap();
    let c2 = cfg.clone();
    tokio::spawn(async move {
        sprite_runtime::run(c2).await.expect("runtime");
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let ws = cfg.projects_dir.clone();
    let hello = ClientHello { kind: "mcp".into(), name: "Agente".into(), workspace: Some(ws.display().to_string()), allowed_dirs: vec![] };
    let a = RuntimeClient::connect(&cfg.endpoint, hello).await.unwrap();
    a.call(None, "project_create", json!({ "name": "heroi", "width": 16, "height": 16 }), None).await.unwrap();
    a.call(
        None,
        "draw_shape",
        json!({ "shape": "rect", "rect": { "x": 4, "y": 4, "w": 8, "h": 12 }, "color": "#3976d2", "fill": true }),
        None,
    )
    .await
    .unwrap();
    a.call(
        None,
        "hitbox_create",
        json!({ "name": "corpo", "kind": "hurtbox", "rect": { "x": 4, "y": 4, "w": 8, "h": 12 }, "all_frames": true }),
        None,
    )
    .await
    .unwrap();

    // Without jobs, saving answers as before.
    let plain = a.call(None, "project_save", json!({ "path": "sub/heroi" }), None).await.unwrap();
    assert!(plain.data.get("exports").is_none());
    assert!(plain.warnings.is_empty());

    a.call(
        None,
        "export_job_set",
        json!({ "name": "cena", "command": "export_godot_scene", "args": { "path": "godot/heroi.tscn" }, "on_save": true }),
        None,
    )
    .await
    .unwrap();
    a.call(
        None,
        "export_job_set",
        json!({ "name": "ruim", "command": "export_gif", "args": { "path": "x.gif", "animation": "nada" }, "on_save": true }),
        None,
    )
    .await
    .unwrap();
    a.call(None, "export_job_set", json!({ "name": "manual", "command": "export_png", "args": { "path": "manual.png" } }), None)
        .await
        .unwrap();

    let saved = a.call(None, "project_save", json!({}), None).await.unwrap();
    assert!(saved.data["saved"].as_str().unwrap().ends_with("heroi.psmproj"));
    let exports = &saved.data["exports"];
    assert_eq!(exports["ran"], 2, "{exports}");
    assert_eq!(exports["failed"], 1);
    assert_eq!(exports["results"][0]["name"], "cena");
    assert_eq!(exports["results"][0]["ok"], true);
    assert!(saved.warnings.iter().any(|w| w.contains("\"ruim\" falhou")), "{:?}", saved.warnings);
    // Relative paths start at the project folder.
    assert!(ws.join("sub/godot/heroi.tscn").exists());
    assert!(ws.join("sub/godot/heroi.tres").exists());
    assert!(!ws.join("sub/manual.png").exists(), "only on_save jobs run when saving");
    let tscn = std::fs::read_to_string(ws.join("sub/godot/heroi.tscn")).unwrap();
    assert!(tscn.contains("[node name=\"corpo\" type=\"Area2D\""));

    // The failed export did not undo the save: the project is clean and the file has the jobs.
    let listing = a.call(None, "project_list", json!({}), None).await.unwrap();
    assert_eq!(listing.data["open"][0]["dirty"], false);
    let doc = sprite_project::format::load(&ws.join("sub/heroi.psmproj")).unwrap();
    assert_eq!(doc.model.extras.godot.export_jobs.len(), 3);
    assert_eq!(doc.model.sprites[0].extras.godot.hitboxes[0].name, "corpo");
}
