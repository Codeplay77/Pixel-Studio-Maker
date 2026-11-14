//! MCP resources: usage guides, runtime status, project state, palettes, metadata and frame
//! previews (PNG blobs).

use rmcp::ErrorData as McpError;
use rmcp::model::*;
use serde_json::{Value, json};
use sprite_runtime::protocol::RequestBody;

use crate::PsmServer;

pub const GUIDE: &str = include_str!("../../../docs/guia-do-agente.md");
pub const SDC: &str = include_str!("../../../docs/sdc.md");

pub async fn list(server: &PsmServer) -> Vec<Resource> {
    let mut out = vec![
        Resource::new("psm://guide", "guia-do-agente")
            .with_title("Guia do agente")
            .with_description("Como planejar, desenhar, animar e corrigir pixel art com estas ferramentas.")
            .with_mime_type("text/markdown"),
        Resource::new("psm://guide/sdc", "referencia-sdc")
            .with_title("Referência SDC")
            .with_description("Todas as operações de desenho em lote.")
            .with_mime_type("text/markdown"),
        Resource::new("psm://status", "status")
            .with_title("Estado do runtime")
            .with_description("Projetos abertos, clientes conectados, operações recentes.")
            .with_mime_type("application/json"),
    ];
    if let Ok(r) = server.run_tool("project_list", Default::default()).await
        && let Some(open) = r.data["open"].as_array()
    {
        for p in open {
            let id = p["id"].as_str().unwrap_or_default();
            let name = p["name"].as_str().unwrap_or(id);
            out.push(
                Resource::new(format!("psm://project/{id}/state"), format!("{name}-estado"))
                    .with_title(format!("{name}: estado"))
                    .with_mime_type("application/json"),
            );
            out.push(
                Resource::new(format!("psm://project/{id}/palettes"), format!("{name}-paletas"))
                    .with_title(format!("{name}: paletas"))
                    .with_mime_type("application/json"),
            );
        }
    }
    out
}

pub fn templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate::new("psm://project/{project}/state", "estado-do-projeto")
            .with_description("Resumo completo do projeto (sprites, layers, frames, animações, paletas, histórico).")
            .with_mime_type("application/json"),
        ResourceTemplate::new("psm://project/{project}/palettes", "paletas")
            .with_description("Paletas do projeto.")
            .with_mime_type("application/json"),
        ResourceTemplate::new("psm://project/{project}/metadata", "metadados")
            .with_description("Metadados do projeto.")
            .with_mime_type("application/json"),
        ResourceTemplate::new("psm://project/{project}/frame/{frame}.png", "previa-do-frame")
            .with_description("Prévia ampliada de um frame (PNG).")
            .with_mime_type("image/png"),
        ResourceTemplate::new("psm://project/{project}/animation/{animation}.png", "previa-da-animacao")
            .with_description("Frames de uma animação lado a lado (PNG).")
            .with_mime_type("image/png"),
    ]
}

fn text(uri: &str, mime: &str, body: String) -> ReadResourceResult {
    ReadResourceResult::new(vec![ResourceContents::text(body, uri).with_mime_type(mime)])
}

fn not_found(uri: &str) -> McpError {
    McpError::resource_not_found(format!("recurso desconhecido: {uri}"), None)
}

fn args(pairs: Value) -> serde_json::Map<String, Value> {
    pairs.as_object().cloned().unwrap_or_default()
}

pub async fn read(server: &PsmServer, uri: &str) -> Result<ReadResourceResult, McpError> {
    let fail = |e: sprite_commands::CommandError| McpError::internal_error(e.message, None);
    match uri {
        "psm://guide" => return Ok(text(uri, "text/markdown", GUIDE.to_string())),
        "psm://guide/sdc" => return Ok(text(uri, "text/markdown", SDC.to_string())),
        "psm://status" => {
            let r = server.run_tool("editor_status", Default::default()).await.map_err(fail)?;
            return Ok(text(uri, "application/json", serde_json::to_string_pretty(&r.data).unwrap_or_default()));
        }
        _ => {}
    }
    let rest = uri.strip_prefix("psm://project/").ok_or_else(|| not_found(uri))?;
    let (project, what) = rest.split_once('/').ok_or_else(|| not_found(uri))?;
    match what {
        "state" => {
            let r = server.run_tool("project_info", args(json!({ "project": project, "full": true }))).await.map_err(fail)?;
            Ok(text(uri, "application/json", serde_json::to_string_pretty(&r.data).unwrap_or_default()))
        }
        "palettes" => {
            let r = server.run_tool("project_info", args(json!({ "project": project }))).await.map_err(fail)?;
            Ok(text(
                uri,
                "application/json",
                serde_json::to_string_pretty(&json!({ "palettes": r.data["palettes"], "active": r.data["active_palette"] }))
                    .unwrap_or_default(),
            ))
        }
        "metadata" => {
            let v = server.request(RequestBody::State { project: Some(project.into()) }).await.map_err(fail)?;
            Ok(text(
                uri,
                "application/json",
                serde_json::to_string_pretty(&json!({ "metadata": v["model"]["metadata"], "path": v["path"], "revision": v["revision"] }))
                    .unwrap_or_default(),
            ))
        }
        other => {
            let (kind, file) = other.split_once('/').ok_or_else(|| not_found(uri))?;
            let key = file.strip_suffix(".png").ok_or_else(|| not_found(uri))?;
            let r = match kind {
                "frame" => {
                    let frame: u32 = key.parse().map_err(|_| not_found(uri))?;
                    server.run_tool("frame_preview", args(json!({ "project": project, "frame": frame }))).await.map_err(fail)?
                }
                "animation" => {
                    server.run_tool("animation_preview", args(json!({ "project": project, "animation": key }))).await.map_err(fail)?
                }
                _ => return Err(not_found(uri)),
            };
            let img = r.images.into_iter().next().ok_or_else(|| not_found(uri))?;
            Ok(ReadResourceResult::new(vec![ResourceContents::blob(img.base64, uri).with_mime_type(img.mime)]))
        }
    }
}
