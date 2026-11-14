//! `psm`: the Pixel Studio Maker command line.
//!
//! - `psm mcp`      — MCP server over stdio (what Claude Code launches).
//! - `psm runtime`  — the runtime process (started automatically by the MCP server and the editor).
//! - `psm call`, `psm script`, `psm status` — talk to the runtime from scripts, headless.
//! - `psm render`, `psm info` — read `.psmproj` files directly, without a runtime.
//! - `psm mcp-config` — prints the Claude Code configuration for this executable.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use base64::Engine;
use serde_json::{Value, json};
use sprite_commands::{ExecContext, parse_command};
use sprite_runtime::{ClientHello, RuntimeClient, RuntimeConfig};

const HELP: &str = "Pixel Studio Maker — linha de comando (psm)

Uso:
  psm mcp [--workspace PASTA] [--allow-dir PASTA]...   servidor MCP por stdio (Claude Code)
  psm runtime [--idle SEGUNDOS]                        runtime (normalmente iniciado sozinho)
  psm status                                           estado do runtime
  psm call FERRAMENTA [JSON] [--project ID] [--images PASTA]
                                                       executa uma ferramenta no runtime
  psm script ARQUIVO.json [--images PASTA]             executa uma lista de passos {\"tool\", \"args\"}
  psm mcp-run ARQUIVO.json [--images PASTA]            o mesmo, mas como cliente MCP de verdade
                                                       (sobe `psm mcp` e chama as ferramentas por stdio)
  psm render ARQUIVO.psmproj [--frame ID] [--scale N] -o SAIDA.png
                                                       renderiza um frame sem runtime
  psm info ARQUIVO.psmproj                             resumo de um projeto sem runtime
  psm mcp-config                                       configuração para o Claude Code
  psm tools                                            lista das ferramentas MCP (Markdown)
  psm --version | --help

Opções gerais:
  --endpoint NOME    pipe/socket do runtime (padrão: por usuário; ou PSM_RUNTIME_PIPE)";

struct Args {
    positional: Vec<String>,
    flags: Vec<(String, String)>,
    switches: Vec<String>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Args {
        let value_flags = [
            "--endpoint",
            "--workspace",
            "--allow-dir",
            "--idle",
            "--project",
            "--images",
            "--frame",
            "--scale",
            "-o",
            "--output",
            "--sprite",
        ];
        let mut a = Args { positional: Vec::new(), flags: Vec::new(), switches: Vec::new() };
        let mut it = raw.into_iter();
        while let Some(x) = it.next() {
            if value_flags.contains(&x.as_str()) {
                let v = it.next().unwrap_or_default();
                a.flags.push((x, v));
            } else if x.starts_with("--") || (x.starts_with('-') && x.len() == 2) {
                a.switches.push(x);
            } else {
                a.positional.push(x);
            }
        }
        a
    }
    fn flag(&self, name: &str) -> Option<&str> {
        self.flags.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    fn flags(&self, name: &str) -> Vec<String> {
        self.flags.iter().filter(|(k, _)| k == name).map(|(_, v)| v.clone()).collect()
    }
}

fn endpoint(a: &Args) -> String {
    a.flag("--endpoint").map(String::from).unwrap_or_else(sprite_runtime::default_endpoint)
}

fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("psm"))
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn runtime_config(endpoint: String) -> RuntimeConfig {
    let mut c = RuntimeConfig::new(endpoint);
    if let Some(d) = std::env::var_os("PSM_DATA_DIR") {
        c.autosave_dir = PathBuf::from(d).join("autosave");
    }
    if let Some(d) = std::env::var_os("PSM_PROJECTS_DIR") {
        c.projects_dir = PathBuf::from(d);
    }
    if let Some(s) = std::env::var("PSM_RUNTIME_IDLE").ok().and_then(|v| v.parse::<u64>().ok()) {
        c.idle_timeout = Duration::from_secs(s);
    }
    c
}

fn main() -> ExitCode {
    // Children (the runtime, `psm mcp`) must not keep our stdio pipes open.
    sprite_runtime::disinherit_std_handles();
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let a = Args::parse(raw);
    if a.switches.iter().any(|s| s == "--version" || s == "-V") {
        println!("psm {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let Some(cmd) = a.positional.first().cloned() else {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    };
    if a.switches.iter().any(|s| s == "--help" || s == "-h") || cmd == "help" {
        println!("{HELP}");
        return ExitCode::SUCCESS;
    }
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("erro: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result: Result<(), String> = rt.block_on(async {
        match cmd.as_str() {
            "mcp" => cmd_mcp(&a).await,
            "runtime" => cmd_runtime(&a).await,
            "status" => cmd_call(&a, "editor_status", json!({})).await,
            "call" => {
                let tool = a.positional.get(1).cloned().ok_or("informe a ferramenta: psm call pixel_set '{...}'")?;
                let args: Value = match a.positional.get(2) {
                    Some(t) => serde_json::from_str(t).map_err(|e| format!("JSON inválido: {e}"))?,
                    None => json!({}),
                };
                cmd_call(&a, &tool, args).await
            }
            "script" => cmd_script(&a).await,
            "mcp-run" => cmd_mcp_run(&a).await,
            "render" => cmd_render(&a),
            "info" => cmd_info(&a),
            "tools" => {
                println!("{}", tools_markdown());
                Ok(())
            }
            "mcp-config" => {
                println!("{}", serde_json::to_string_pretty(&mcp_config()).unwrap_or_default());
                Ok(())
            }
            other => Err(format!("comando desconhecido: {other} (veja psm --help)")),
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("erro: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Markdown reference of every MCP tool, generated from the command table.
fn tools_markdown() -> String {
    let specs = sprite_commands::specs();
    let mut out =
        String::from("# Ferramentas MCP\n\nGerado por `psm tools` a partir da tabela de comandos (`crates/sprite-commands/src/lib.rs`).\n");
    out.push_str(&format!(
        "São {} ferramentas. Todas as que agem num projeto aceitam também `project` (id) e, se escrevem, `expected_revision`.\n\n",
        specs.len()
    ));
    out.push_str("Tipos: **leitura** não muda nada; **escrita** vira um passo do histórico (pode ser desfeita); **runtime** cuida de projetos, transações e histórico.\n\n");
    out.push_str("| Ferramenta | Tipo | O que faz |\n|---|---|---|\n");
    for s in &specs {
        let kind = match s.kind {
            sprite_commands::Kind::Read => "leitura",
            sprite_commands::Kind::Write => "escrita",
            sprite_commands::Kind::Runtime => "runtime",
        };
        out.push_str(&format!("| `{}` | {kind} | {} |\n", s.name, s.description.replace('|', "\\|")));
    }
    out.push_str("\nO schema completo de cada ferramenta (tipos, campos, descrições) é enviado pelo próprio servidor no `tools/list`.\n");
    out
}

fn mcp_config() -> Value {
    json!({
        "mcpServers": {
            "pixel-studio-maker": {
                "type": "stdio",
                "command": exe().display().to_string(),
                "args": ["mcp"],
                "env": {}
            }
        }
    })
}

async fn cmd_mcp(a: &Args) -> Result<(), String> {
    let workspace = a.flag("--workspace").map(PathBuf::from).unwrap_or_else(cwd);
    let mut allowed = a.flags("--allow-dir");
    if let Ok(extra) = std::env::var("PSM_ALLOWED_DIRS") {
        allowed.extend(std::env::split_paths(&extra).map(|p| p.display().to_string()));
    }
    let hello = ClientHello {
        kind: "mcp".into(),
        name: "Claude Code".into(),
        workspace: Some(workspace.display().to_string()),
        allowed_dirs: allowed,
    };
    let server = sprite_mcp::PsmServer::new(endpoint(a), exe(), hello);
    sprite_mcp::serve_stdio(server).await.map_err(|e| e.to_string())
}

async fn cmd_runtime(a: &Args) -> Result<(), String> {
    let mut config = runtime_config(endpoint(a));
    if let Some(s) = a.flag("--idle").and_then(|v| v.parse::<u64>().ok()) {
        config.idle_timeout = Duration::from_secs(s);
    }
    match sprite_runtime::run(config).await {
        Ok(()) => Ok(()),
        // Another runtime owns the endpoint: nothing to do.
        Err(e) if matches!(e.kind(), std::io::ErrorKind::AddrInUse | std::io::ErrorKind::PermissionDenied) => Ok(()),
        Err(e) => Err(format!("runtime: {e}")),
    }
}

async fn connect(a: &Args) -> Result<RuntimeClient, String> {
    let hello = ClientHello { kind: "cli".into(), name: "psm".into(), workspace: Some(cwd().display().to_string()), allowed_dirs: vec![] };
    RuntimeClient::connect_or_spawn(&endpoint(a), hello, &exe()).await.map_err(|e| format!("runtime indisponível: {e}"))
}

fn save_images(dir: &Path, step: usize, tool: &str, images: &[sprite_runtime::protocol::WireImage]) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (i, img) in images.iter().enumerate() {
        let ext = if img.mime == "image/gif" { "gif" } else { "png" };
        let path = dir.join(format!("{step:03}_{tool}_{i}.{ext}"));
        let bytes = base64::engine::general_purpose::STANDARD.decode(&img.base64).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        out.push(path.display().to_string());
    }
    Ok(out)
}

fn split_project(args: &mut Value) -> (Option<String>, Option<u64>) {
    let obj = args.as_object_mut();
    match obj {
        Some(o) => (o.remove("project").and_then(|v| v.as_str().map(String::from)), o.remove("expected_revision").and_then(|v| v.as_u64())),
        None => (None, None),
    }
}

async fn cmd_call(a: &Args, tool: &str, mut args: Value) -> Result<(), String> {
    let client = connect(a).await?;
    let (p, rev) = split_project(&mut args);
    let project = a.flag("--project").map(String::from).or(p);
    match client.call(project, tool, args, rev).await {
        Ok(r) => {
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            if let Some(dir) = a.flag("--images") {
                let files = save_images(Path::new(dir), 0, tool, &r.images)?;
                v["images"] = json!(files);
            } else if let Some(imgs) = v.get_mut("images").and_then(|x| x.as_array_mut()) {
                for i in imgs {
                    i["base64"] = json!("(omitido; use --images PASTA)");
                }
            }
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            Ok(())
        }
        Err(e) => Err(format!("{:?}: {}", e.code, e.message)),
    }
}

/// Runs `[{"tool": "...", "args": {...}}, ...]` in order, stopping at the first error.
async fn cmd_script(a: &Args) -> Result<(), String> {
    let file = a.positional.get(1).ok_or("informe o arquivo do script")?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let steps: Vec<Value> = serde_json::from_str(&text).map_err(|e| format!("script inválido: {e}"))?;
    let client = connect(a).await?;
    let images_dir = a.flag("--images").map(PathBuf::from);
    for (i, step) in steps.into_iter().enumerate() {
        let tool = step["tool"].as_str().ok_or(format!("passo {i}: falta \"tool\""))?.to_string();
        let mut args = step.get("args").cloned().unwrap_or(json!({}));
        let (project, rev) = split_project(&mut args);
        let started = std::time::Instant::now();
        match client.call(project, &tool, args, rev).await {
            Ok(r) => {
                let saved = match &images_dir {
                    Some(dir) if !r.images.is_empty() => save_images(dir, i, &tool, &r.images)?,
                    _ => Vec::new(),
                };
                let mut brief = r.data.clone();
                if let Some(o) = brief.as_object_mut() {
                    o.remove("rows");
                    o.remove("legend");
                    o.remove("patch_a_to_b");
                }
                let brief = serde_json::to_string(&brief).unwrap_or_default();
                let brief: String = brief.chars().take(220).collect();
                println!(
                    "[{i:03}] {tool} ok rev={} {}ms {brief}{}",
                    r.revision,
                    started.elapsed().as_millis(),
                    if saved.is_empty() { String::new() } else { format!(" -> {}", saved.join(", ")) }
                );
                for w in &r.warnings {
                    println!("      aviso: {w}");
                }
            }
            Err(e) => return Err(format!("passo {i} ({tool}) falhou: {:?}: {}", e.code, e.message)),
        }
    }
    Ok(())
}

/// Runs a script through the MCP protocol: spawns `psm mcp` and calls each tool over stdio.
async fn cmd_mcp_run(a: &Args) -> Result<(), String> {
    use rmcp::ServiceExt;
    use rmcp::model::CallToolRequestParams;
    let file = a.positional.get(1).ok_or("informe o arquivo do script")?;
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let steps: Vec<Value> = serde_json::from_str(&text).map_err(|e| format!("script inválido: {e}"))?;
    let mut cmd = tokio::process::Command::new(exe());
    cmd.arg("mcp");
    if let Some(ep) = a.flag("--endpoint") {
        cmd.arg("--endpoint").arg(ep);
    }
    let transport = rmcp::transport::TokioChildProcess::new(cmd).map_err(|e| format!("não foi possível iniciar psm mcp: {e}"))?;
    let client = ().serve(transport).await.map_err(|e| format!("handshake MCP falhou: {e}"))?;
    let images_dir = a.flag("--images").map(PathBuf::from);
    if let Some(dir) = &images_dir {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    for (i, step) in steps.into_iter().enumerate() {
        let tool = step["tool"].as_str().ok_or(format!("passo {i}: falta \"tool\""))?.to_string();
        let args = step.get("args").and_then(|v| v.as_object()).cloned().unwrap_or_default();
        let started = std::time::Instant::now();
        let r = client
            .call_tool(CallToolRequestParams::new(tool.clone()).with_arguments(args))
            .await
            .map_err(|e| format!("passo {i} ({tool}): {e}"))?;
        let text = r.content.iter().find_map(|c| c.as_text()).map(|t| t.text.clone()).unwrap_or_default();
        if r.is_error == Some(true) {
            let _ = client.cancel().await;
            return Err(format!("passo {i} ({tool}) falhou: {text}"));
        }
        let mut saved = Vec::new();
        if let Some(dir) = &images_dir {
            for (k, img) in r.content.iter().filter_map(|c| c.as_image()).enumerate() {
                let ext = if img.mime_type == "image/gif" { "gif" } else { "png" };
                let label = step.get("save_as").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| format!("{i:03}_{tool}_{k}"));
                let path = dir.join(format!("{label}.{ext}"));
                let bytes = base64::engine::general_purpose::STANDARD.decode(&img.data).map_err(|e| e.to_string())?;
                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                saved.push(path.display().to_string());
            }
        }
        let full = a.switches.iter().any(|s| s == "--full");
        let brief: String = if full { text.clone() } else { text.chars().take(240).collect() };
        println!(
            "[{i:03}] {tool} ok {}ms {brief}{}",
            started.elapsed().as_millis(),
            if saved.is_empty() { String::new() } else { format!(" -> {}", saved.join(", ")) }
        );
    }
    let _ = client.cancel().await;
    Ok(())
}

fn load(a: &Args) -> Result<sprite_commands::Session, String> {
    let file = a.positional.get(1).ok_or("informe o arquivo .psmproj")?;
    let doc = sprite_project::format::load(Path::new(file)).map_err(|e| e.to_string())?;
    Ok(sprite_commands::Session::new(doc, Some(PathBuf::from(file))))
}

fn cmd_info(a: &Args) -> Result<(), String> {
    let mut s = load(a)?;
    let out =
        s.execute(parse_command("project_info", json!({})).map_err(|e| e.message)?, &ExecContext::local(cwd())).map_err(|e| e.message)?;
    println!("{}", serde_json::to_string_pretty(&out.data).unwrap_or_default());
    Ok(())
}

fn cmd_render(a: &Args) -> Result<(), String> {
    let mut s = load(a)?;
    let output = a.flag("-o").or(a.flag("--output")).ok_or("informe a saída com -o arquivo.png")?;
    let mut args = json!({ "path": output, "scale": a.flag("--scale").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1) });
    if let Some(f) = a.flag("--frame").and_then(|v| v.parse::<u32>().ok()) {
        args["frame"] = json!(f);
    }
    if let Some(sp) = a.flag("--sprite") {
        args["sprite"] = json!(sp);
    }
    let out = s.execute(parse_command("export_png", args).map_err(|e| e.message)?, &ExecContext::local(cwd())).map_err(|e| e.message)?;
    println!("{}", out.data);
    Ok(())
}
