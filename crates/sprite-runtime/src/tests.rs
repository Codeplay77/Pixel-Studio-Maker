//! Runtime tests over a real named pipe / socket, with isolated endpoints and folders.

use std::time::Duration;

use serde_json::json;

use crate::protocol::{ClientHello, Event};
use crate::{RuntimeClient, RuntimeConfig};

fn config(tag: &str) -> RuntimeConfig {
    let base = std::env::temp_dir().join(format!("psm-rt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let mut c = RuntimeConfig::new(format!("psm-test-{tag}-{}", std::process::id()));
    c.autosave_dir = base.join("autosave");
    c.projects_dir = base.join("projects");
    c.idle_timeout = Duration::from_secs(5);
    c.autosave_every = Duration::from_millis(200);
    c
}

fn hello(kind: &str, name: &str, ws: &std::path::Path) -> ClientHello {
    ClientHello { kind: kind.into(), name: name.into(), workspace: Some(ws.display().to_string()), allowed_dirs: vec![] }
}

async fn start(tag: &str) -> RuntimeConfig {
    let cfg = config(tag);
    std::fs::create_dir_all(&cfg.projects_dir).unwrap();
    let c2 = cfg.clone();
    tokio::spawn(async move {
        crate::run(c2).await.expect("runtime");
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    cfg
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shared_state_events_and_revisions() {
    let cfg = start("shared").await;
    let ws = cfg.projects_dir.clone();
    let agent = RuntimeClient::connect(&cfg.endpoint, hello("mcp", "Claude Code", &ws)).await.unwrap();
    let editor = RuntimeClient::connect(&cfg.endpoint, hello("desktop", "Editor", &ws)).await.unwrap();
    let mut events = editor.subscribe().await.unwrap();

    let created = agent.call(None, "project_create", json!({ "name": "Herói", "width": 16, "height": 16 }), None).await.unwrap();
    let pid = created.project.clone().unwrap();
    let base = created.revision;
    let r = agent.call(None, "pixel_set", json!({ "x": 1, "y": 1, "color": "#ff0000" }), None).await.unwrap();
    assert_eq!(r.revision, base + 1);

    // The editor sees the change as an event and reads the same state.
    let mut saw_change = false;
    for _ in 0..20 {
        match tokio::time::timeout(Duration::from_secs(2), events.recv()).await {
            Ok(Ok(Event::Changed { project, revision, source, .. })) if project == pid => {
                assert_eq!(revision, base + 1);
                assert_eq!(source, "Claude Code");
                saw_change = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(saw_change, "editor did not get the change event");
    let px = editor.call(Some(pid.clone()), "pixel_get", json!({ "x": 1, "y": 1 }), None).await.unwrap();
    assert_eq!(px.data["color"], "#ff0000");

    // Editor edits a pixel; the agent reads it.
    editor.call(Some(pid.clone()), "pixel_set", json!({ "x": 2, "y": 2, "color": "#00ff00" }), None).await.unwrap();
    let px = agent.call(None, "pixel_get", json!({ "x": 2, "y": 2 }), None).await.unwrap();
    assert_eq!(px.data["color"], "#00ff00");

    // Stale revision is rejected.
    let err = agent.call(None, "pixel_set", json!({ "x": 3, "y": 3, "color": "#000" }), Some(base + 1)).await.unwrap_err();
    assert_eq!(err.code, sprite_commands::ErrorCode::RevisionConflict);
    assert_eq!(err.details.unwrap()["current_revision"], base + 2);
    agent.call(None, "pixel_set", json!({ "x": 3, "y": 3, "color": "#000" }), Some(base + 2)).await.unwrap();

    // Raw render for the canvas.
    let frame = created.data["sprite"]["frame"].as_u64().unwrap() as u32;
    let render = editor
        .request(crate::protocol::RequestBody::Render { project: None, sprite: None, frame, layer: None, all_layers: false })
        .await
        .unwrap();
    assert_eq!(render["width"], 16);

    // Status lists both clients.
    let st = agent.call(None, "editor_status", json!({}), None).await.unwrap();
    assert_eq!(st.data["clients"].as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transactions_block_other_clients_and_roll_back_on_disconnect() {
    let cfg = start("tx").await;
    let ws = cfg.projects_dir.clone();
    let a = RuntimeClient::connect(&cfg.endpoint, hello("mcp", "A", &ws)).await.unwrap();
    let b = RuntimeClient::connect(&cfg.endpoint, hello("desktop", "B", &ws)).await.unwrap();
    let made = a.call(None, "project_create", json!({ "name": "tx", "width": 8, "height": 8 }), None).await.unwrap();
    let (pid, base) = (made.project.clone().unwrap(), made.revision);
    a.call(None, "transaction_begin", json!({ "label": "personagem" }), None).await.unwrap();
    a.call(None, "pixel_set", json!({ "x": 0, "y": 0, "color": "#ff0000" }), None).await.unwrap();
    a.call(None, "pixel_set", json!({ "x": 1, "y": 0, "color": "#ff0000" }), None).await.unwrap();

    // B's write waits until A commits.
    let pid_b = pid.clone();
    let waiting = tokio::spawn(async move {
        b.call(Some(pid_b), "pixel_set", json!({ "x": 5, "y": 5, "color": "#0000ff" }), None).await.map(|r| r.revision)
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!waiting.is_finished(), "B should wait for A's transaction");
    let commit = a.call(None, "transaction_commit", json!({}), None).await.unwrap();
    assert_eq!(commit.data["changed_images"], 1);
    let rev_b = waiting.await.unwrap().unwrap();
    assert_eq!(rev_b, base + 2);
    // One undo step for the whole transaction.
    let undo = a.call(None, "history_undo", json!({ "steps": 2 }), None).await.unwrap();
    assert_eq!(undo.data["undone"].as_array().unwrap().len(), 2);
    let px = a.call(None, "pixel_get", json!({ "x": 1, "y": 0 }), None).await.unwrap();
    assert_eq!(px.data["color"], "#00000000");

    // A transaction left open by a client that disconnects is rolled back.
    let c = RuntimeClient::connect(&cfg.endpoint, hello("mcp", "C", &ws)).await.unwrap();
    c.call(Some(pid.clone()), "transaction_begin", json!({}), None).await.unwrap();
    c.call(Some(pid.clone()), "pixel_set", json!({ "x": 7, "y": 7, "color": "#ffffff" }), None).await.unwrap();
    drop(c);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let px = a.call(Some(pid.clone()), "pixel_get", json!({ "x": 7, "y": 7 }), None).await.unwrap();
    assert_eq!(px.data["color"], "#00000000");
    a.call(Some(pid), "pixel_set", json!({ "x": 6, "y": 6, "color": "#ffffff" }), None).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn save_open_reuse_close_and_autosave() {
    let cfg = start("files").await;
    let ws = cfg.projects_dir.clone();
    let a = RuntimeClient::connect(&cfg.endpoint, hello("mcp", "A", &ws)).await.unwrap();
    let pid = a.call(None, "project_create", json!({ "name": "arquivo", "width": 8, "height": 8 }), None).await.unwrap().project.unwrap();
    a.call(None, "pixel_set", json!({ "x": 0, "y": 0, "color": "#123456" }), None).await.unwrap();
    // Unsaved project gets an autosave.
    tokio::time::sleep(Duration::from_millis(700)).await;
    let listing = a.call(None, "project_list", json!({}), None).await.unwrap();
    assert_eq!(listing.data["open"][0]["dirty"], true);
    assert!(std::fs::read_dir(&cfg.autosave_dir).unwrap().count() >= 1, "autosave written");

    let saved = a.call(None, "project_save", json!({ "path": "heroi" }), None).await.unwrap();
    assert!(saved.data["saved"].as_str().unwrap().ends_with("heroi.psmproj"));
    assert!(ws.join("heroi.psmproj").exists());
    // Opening the same file reuses the open session.
    let again = a.call(None, "project_open", json!({ "path": "heroi.psmproj" }), None).await.unwrap();
    assert_eq!(again.data["reused"], true);
    assert_eq!(again.project.unwrap(), pid);
    // Paths outside allowed folders are refused.
    let err = a.call(None, "project_save", json!({ "path": "C:/Windows/nope.psmproj" }), None).await.unwrap_err();
    assert_eq!(err.code, sprite_commands::ErrorCode::PermissionDenied);

    a.call(None, "pixel_set", json!({ "x": 1, "y": 0, "color": "#123456" }), None).await.unwrap();
    let refused = a.call(None, "project_close", json!({}), None).await.unwrap_err();
    assert!(refused.message.contains("não salvas"));
    a.call(None, "project_close", json!({ "save": true }), None).await.unwrap();
    let reopened = a.call(None, "project_open", json!({ "path": "heroi.psmproj" }), None).await.unwrap();
    assert_eq!(reopened.data["summary"]["sprites"][0]["width"], 8);
    let px = a.call(None, "pixel_get", json!({ "x": 1, "y": 0 }), None).await.unwrap();
    assert_eq!(px.data["color"], "#123456");
}
