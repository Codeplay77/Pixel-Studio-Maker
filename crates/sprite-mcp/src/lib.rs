//! Native MCP server for Pixel Studio Maker (official `rmcp` SDK, stdio transport).
//!
//! Every editor command is a tool with a typed JSON schema generated from the same Rust types
//! the editor uses. The server holds no project state: it forwards each call to the runtime
//! (starting it if needed), so the agent and the open editor always share one project state.

mod prompts;
mod resources;

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value, json};
use sprite_commands::{CommandError, Kind, specs};
use sprite_runtime::{CallResult, ClientHello, RuntimeClient};
use tokio::sync::Mutex;

pub const SERVER_NAME: &str = "pixel-studio-maker";

const INSTRUCTIONS: &str = "Pixel Studio Maker: editor de pixel art e animação. Você desenha pixel por pixel com ferramentas determinísticas (sem geração de imagens por IA).\n\
Fluxo: project_create (tamanho em px) → palette_create → layer_create → draw_batch (várias operações SDC numa chamada; use `matrix` com `rows`+`legend` para formas orgânicas) → frame_preview para ver o resultado → corrigir com pixel_apply_patch/draw_batch → project_save (.psmproj) → export_png/export_spritesheet/export_gif/export_godot_metadata.\n\
Animação: animation_create com copy_from (frames nascem como cópia para redesenhar), region_transform para mover partes, frame_compare e animation_preview para inspecionar. Animar é redesenhar poses, não só mover o sprite inteiro.\n\
Cores: \"#rrggbb\", \"clear\" ou índice da paleta ativa. Coordenadas: (0,0) no canto superior esquerdo.\n\
Leia o recurso psm://guide para o guia completo e psm://guide/sdc para a referência das operações. Se o editor estiver aberto, tudo aparece lá em tempo real.";

/// Tools that act on a project receive these extra optional arguments.
const PROJECT_FREE: [&str; 6] =
    ["project_create", "project_open", "project_list", "editor_status", "editor_get_capabilities", "image_pixelate"];

fn is_destructive(name: &str) -> bool {
    name.ends_with("_delete")
        || matches!(name, "project_close" | "pixel_clear" | "layer_merge" | "snapshot_restore" | "transaction_rollback")
}

/// Builds the MCP tool list from the command table.
pub fn build_tools() -> Vec<Tool> {
    specs()
        .into_iter()
        .map(|spec| {
            let mut schema = serde_json::to_value((spec.schema)()).unwrap_or(json!({ "type": "object" }));
            let obj = schema.as_object_mut().expect("object schema");
            obj.remove("$schema");
            obj.remove("title");
            obj.insert("type".into(), json!("object"));
            if !PROJECT_FREE.contains(&spec.name) {
                let props = obj.entry("properties").or_insert_with(|| json!({}));
                if let Some(p) = props.as_object_mut() {
                    p.insert("project".into(), json!({ "type": "string", "description": "Id do projeto (padrão: o projeto atual desta sessão)." }));
                    if spec.kind != Kind::Read {
                        p.insert(
                            "expected_revision".into(),
                            json!({ "type": "integer", "minimum": 0, "description": "Revisão esperada do projeto; se outro cliente mudou o projeto, a chamada falha com REVISION_CONFLICT." }),
                        );
                    }
                }
            }
            let map: Map<String, Value> = obj.clone();
            let annotations = ToolAnnotations::new()
                .read_only(spec.kind == Kind::Read)
                .destructive(is_destructive(spec.name))
                .open_world(false);
            Tool::new(spec.name, spec.description, Arc::new(map)).annotate(annotations)
        })
        .collect()
}

/// Turns a runtime result into MCP content: compact JSON text plus image blocks.
pub fn to_tool_result(r: CallResult) -> CallToolResult {
    let mut body = json!({ "revision": r.revision, "data": r.data });
    if let Some(p) = &r.project {
        body["project"] = json!(p);
    }
    if let Some(t) = r.transaction {
        body["transaction"] = json!(t);
    }
    if !r.warnings.is_empty() {
        body["warnings"] = json!(r.warnings);
    }
    if !r.changes.is_empty() {
        body["changed_cels"] = json!(r.changes.iter().filter(|c| c.kind == "pixels").count());
    }
    for img in &r.images {
        body.as_object_mut()
            .expect("object")
            .entry("images")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("array")
            .push(json!({ "label": img.label, "mime": img.mime, "size": [img.width, img.height] }));
    }
    let mut content = vec![ContentBlock::text(serde_json::to_string(&body).unwrap_or_default())];
    for img in r.images {
        content.push(ContentBlock::image(img.base64, img.mime));
    }
    CallToolResult::success(content)
}

pub fn to_tool_error(e: CommandError) -> CallToolResult {
    let body = json!({ "error": { "code": e.code, "message": e.message, "details": e.details } });
    CallToolResult::error(vec![ContentBlock::text(serde_json::to_string(&body).unwrap_or_default())])
}

/// The MCP server. Cheap to clone; all clones share one runtime connection.
#[derive(Clone)]
pub struct PsmServer {
    inner: Arc<Inner>,
}

struct Inner {
    endpoint: String,
    exe: PathBuf,
    hello: ClientHello,
    tools: Vec<Tool>,
    client: Mutex<Option<Arc<RuntimeClient>>>,
}

impl PsmServer {
    /// `exe` is the `psm` executable used to start the runtime when none is running.
    pub fn new(endpoint: impl Into<String>, exe: PathBuf, hello: ClientHello) -> Self {
        PsmServer { inner: Arc::new(Inner { endpoint: endpoint.into(), exe, hello, tools: build_tools(), client: Mutex::new(None) }) }
    }

    async fn client(&self) -> Result<Arc<RuntimeClient>, CommandError> {
        let mut guard = self.inner.client.lock().await;
        if let Some(c) = guard.as_ref()
            && c.is_alive()
        {
            return Ok(c.clone());
        }
        let c = RuntimeClient::connect_or_spawn(&self.inner.endpoint, self.inner.hello.clone(), &self.inner.exe).await.map_err(|e| {
            CommandError::new(
                sprite_commands::ErrorCode::Internal,
                format!("não foi possível conectar ao runtime ({}): {e}", self.inner.endpoint),
            )
        })?;
        let c = Arc::new(c);
        *guard = Some(c.clone());
        Ok(c)
    }

    /// Runs a tool by name; reconnects once if the runtime went away.
    pub async fn run_tool(&self, name: &str, mut args: Map<String, Value>) -> Result<CallResult, CommandError> {
        let project = args.remove("project").and_then(|v| v.as_str().map(String::from));
        let expected = args.remove("expected_revision").and_then(|v| v.as_u64());
        let args = Value::Object(args);
        for attempt in 0..2 {
            let client = self.client().await?;
            match client.call(project.clone(), name, args.clone(), expected).await {
                Err(e) if attempt == 0 && !client.is_alive() => {
                    let _ = e;
                    *self.inner.client.lock().await = None;
                }
                other => return other,
            }
        }
        Err(CommandError::new(sprite_commands::ErrorCode::Internal, "runtime indisponível"))
    }

    pub(crate) async fn request(&self, body: sprite_runtime::protocol::RequestBody) -> Result<Value, CommandError> {
        self.client().await?.request(body).await
    }
}

fn implementation() -> Implementation {
    Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION"))
}

impl ServerHandler for PsmServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_prompts().enable_resources().enable_tools().build())
            .with_server_info(implementation())
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.inner.tools.clone()))
    }

    async fn call_tool(&self, request: CallToolRequestParams, _context: RequestContext<RoleServer>) -> Result<CallToolResponse, McpError> {
        let name = request.name.to_string();
        if !self.inner.tools.iter().any(|t| t.name == name) {
            return Ok(to_tool_error(CommandError::not_found(format!("ferramenta desconhecida: {name}"))).into());
        }
        let args = request.arguments.unwrap_or_default();
        Ok(match self.run_tool(&name, args).await {
            Ok(r) => to_tool_result(r),
            Err(e) => to_tool_error(e),
        }
        .into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(resources::list(self).await))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult::with_all_items(resources::templates()))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        resources::read(self, &request.uri).await.map(Into::into)
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(prompts::list()))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        prompts::get(&request.name, request.arguments.unwrap_or_default()).map(Into::into)
    }
}

/// Serves MCP over stdin/stdout until the client disconnects.
pub async fn serve_stdio(server: PsmServer) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use rmcp::ServiceExt;
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
