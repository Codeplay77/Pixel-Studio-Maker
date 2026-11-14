//! Headless end-to-end checks: the demonstration scripts in `examples/` are replayed through the
//! command layer (no UI, no runtime) and the resulting pixels are compared with golden images.
//! Determinism means the same script always produces exactly the same pixels.

use std::path::{Path, PathBuf};

use serde_json::Value;
use sprite_commands::{ExecContext, ProjectCreate, Session, create_project, parse_command};

/// Tools that only touch files or produce previews are skipped during replay.
const SKIP: [&str; 9] = [
    "project_open",
    "project_save",
    "frame_preview",
    "animation_preview",
    "frame_compare",
    "export_png",
    "export_spritesheet",
    "export_gif",
    "export_godot_metadata",
];

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root").to_path_buf()
}

/// Replays a `psm mcp-run` script. `project_create` starts a new session; transactions are
/// applied directly on the document.
pub fn replay(script: &Path, session: &mut Option<Session>, ctx: &ExecContext) {
    let steps: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(script).expect("script")).expect("json");
    for (i, step) in steps.iter().enumerate() {
        let tool = step["tool"].as_str().expect("tool");
        let args = step.get("args").cloned().unwrap_or(Value::Null);
        if SKIP.contains(&tool) {
            continue;
        }
        if tool == "project_create" {
            let a: ProjectCreate = serde_json::from_value(args).expect("args");
            *session = Some(create_project(&a, ctx).expect("create"));
            continue;
        }
        let s = session.as_mut().expect("project created first");
        match tool {
            "transaction_begin" => {
                s.doc.begin(args["label"].as_str().unwrap_or("tx")).expect("begin");
            }
            "transaction_commit" => {
                s.doc.commit().expect("commit");
            }
            _ => {
                let cmd = parse_command(tool, args).unwrap_or_else(|e| panic!("passo {i} ({tool}): {}", e.message));
                s.execute(cmd, ctx).unwrap_or_else(|e| panic!("passo {i} ({tool}): {}", e.message));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sprite_core::codec::decode_png;

    fn golden(name: &str) -> sprite_core::Image {
        decode_png(&std::fs::read(workspace_root().join("tests/golden").join(name)).expect("golden")).expect("png")
    }

    fn export(session: &mut Session, ctx: &ExecContext, tool: &str, args: Value) -> sprite_core::Image {
        let out = session.execute(parse_command(tool, args).expect("cmd"), ctx).expect("export");
        let path = out.data["path"].as_str().expect("path").to_string();
        decode_png(&std::fs::read(path).expect("read")).expect("png")
    }

    #[test]
    fn demo_a_and_b_replay_pixel_exact() {
        let tmp = std::env::temp_dir().join(format!("psm-golden-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let ctx = ExecContext::local(&tmp);
        let ex = workspace_root().join("examples");
        let mut session = None;
        replay(&ex.join("demo-a-guerreiro/1-desenhar.json"), &mut session, &ctx);
        replay(&ex.join("demo-a-guerreiro/3-corrigir.json"), &mut session, &ctx);
        let s = session.as_mut().unwrap();
        let warrior = export(s, &ctx, "export_png", serde_json::json!({ "path": "a.png" }));
        assert_eq!(warrior, golden("guerreiro_a.png"), "a Demonstração A gerou pixels diferentes da referência");

        replay(&ex.join("demo-a-guerreiro/4-estrutura-e-exportar.json"), &mut session, &ctx);
        replay(&ex.join("demo-b-caminhada/caminhada.json"), &mut session, &ctx);
        let s = session.as_mut().unwrap();
        let sheet = export(
            s,
            &ctx,
            "export_spritesheet",
            serde_json::json!({ "path": "walk.png", "selection": { "animations": ["walk"] }, "sheet": { "layout": "horizontal" }, "json": false }),
        );
        assert_eq!(sheet, golden("guerreiro_walk_E.png"), "a caminhada gerou pixels diferentes da referência");
        // The walk has 8 distinct poses: no two consecutive frames are identical.
        let issues = s.execute(parse_command("project_validate", serde_json::json!({})).unwrap(), &ctx).unwrap();
        let warnings: Vec<&Value> = issues.data["issues"].as_array().unwrap().iter().filter(|i| i["severity"] == "warning").collect();
        assert!(warnings.is_empty(), "alertas inesperados: {warnings:?}");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
