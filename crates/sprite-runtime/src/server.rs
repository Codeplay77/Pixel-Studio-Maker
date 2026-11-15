//! The runtime process: the single source of truth for every open project.
//!
//! - One [`ProjectEntry`] per open project, behind its own mutex: commands on a project run one
//!   at a time, in arrival order; different projects run in parallel.
//! - A client may open an explicit transaction; other clients' writes on that project wait for
//!   it (up to [`TX_WAIT`]) and then fail with `LOCKED`.
//! - Writes may carry `expected_revision`; a mismatch fails with `REVISION_CONFLICT`.
//! - Every change is broadcast as an [`Event`] to subscribed clients (the editor).
//! - Dirty projects are autosaved; the runtime exits after a period without clients.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use sprite_animation::compose::{ComposeOptions, compose_frame, layer_image};
use sprite_commands::{Command, CommandError, ErrorCode, ExecContext, Kind, Limits, Outcome, PathPolicy, Session, parse_command};
use sprite_project::format;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc};

use crate::protocol::*;
use crate::transport::{BoxRead, BoxWrite, Listener};

/// How long a write waits for another client's transaction to finish.
pub const TX_WAIT: Duration = Duration::from_secs(15);
const MAX_LINE: usize = 64 * 1024 * 1024;
const OP_LOG: usize = 300;

pub struct ProjectEntry {
    pub id: String,
    pub session: Session,
    /// Client id owning the open explicit transaction.
    pub tx_owner: Option<u64>,
    pub autosaved_revision: u64,
}

#[derive(Debug, Clone)]
struct Client {
    id: u64,
    hello: ClientHello,
    current: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub endpoint: String,
    /// Exit after this long without connected clients (dirty projects are autosaved first).
    pub idle_timeout: Duration,
    pub autosave_every: Duration,
    pub autosave_dir: PathBuf,
    /// Default folder for projects (Documents/Pixel Studio Maker).
    pub projects_dir: PathBuf,
    pub limits: Limits,
}

impl RuntimeConfig {
    pub fn new(endpoint: impl Into<String>) -> Self {
        let data = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Pixel Studio Maker");
        let docs = dirs::document_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_else(std::env::temp_dir)).join("Pixel Studio Maker");
        RuntimeConfig {
            endpoint: endpoint.into(),
            idle_timeout: Duration::from_secs(300),
            autosave_every: Duration::from_secs(10),
            autosave_dir: data.join("autosave"),
            projects_dir: docs,
            limits: Limits::default(),
        }
    }
}

pub struct Runtime {
    pub config: RuntimeConfig,
    projects: Mutex<BTreeMap<String, Arc<Mutex<ProjectEntry>>>>,
    clients: Mutex<HashMap<u64, Client>>,
    events: broadcast::Sender<Event>,
    log: Mutex<VecDeque<OpRecord>>,
    next_client: AtomicU64,
    started: Instant,
    last_activity: Mutex<Instant>,
}

fn now_iso() -> String {
    sprite_project::time::now_iso()
}

fn err(code: ErrorCode, msg: impl Into<String>) -> CommandError {
    CommandError::new(code, msg)
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Arc<Runtime> {
        let (events, _) = broadcast::channel(1024);
        Arc::new(Runtime {
            config,
            projects: Mutex::new(BTreeMap::new()),
            clients: Mutex::new(HashMap::new()),
            events,
            log: Mutex::new(VecDeque::new()),
            next_client: AtomicU64::new(1),
            started: Instant::now(),
            last_activity: Mutex::new(Instant::now()),
        })
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    fn touch(&self) {
        *self.last_activity.lock().expect("lock") = Instant::now();
    }

    // ---- clients ----------------------------------------------------------------------------

    pub fn register(&self, hello: ClientHello) -> u64 {
        let id = self.next_client.fetch_add(1, Ordering::Relaxed);
        self.clients.lock().expect("lock").insert(id, Client { id, hello, current: None });
        self.emit(Event::Clients { clients: self.client_summaries() });
        id
    }

    pub fn unregister(&self, id: u64) {
        self.clients.lock().expect("lock").remove(&id);
        // A vanished client's transaction is rolled back so nobody waits forever.
        let entries: Vec<Arc<Mutex<ProjectEntry>>> = self.projects.lock().expect("lock").values().cloned().collect();
        for e in entries {
            let mut p = e.lock().expect("lock");
            if p.tx_owner == Some(id) {
                let _ = p.session.doc.rollback();
                p.session.invalidate_cache();
                p.tx_owner = None;
                let (pid, rev) = (p.id.clone(), p.session.doc.revision());
                let changes = all_structure(&p.session);
                drop(p);
                self.emit(Event::Changed {
                    project: pid,
                    revision: rev,
                    source: "runtime".into(),
                    command: "transaction_rollback".into(),
                    changes,
                    transaction: None,
                });
            }
        }
        self.emit(Event::Clients { clients: self.client_summaries() });
        self.touch();
    }

    fn client_summaries(&self) -> Vec<ClientSummary> {
        let mut v: Vec<ClientSummary> = self
            .clients
            .lock()
            .expect("lock")
            .values()
            .map(|c| ClientSummary { id: c.id, kind: c.hello.kind.clone(), name: c.hello.name.clone(), project: c.current.clone() })
            .collect();
        v.sort_by_key(|c| c.id);
        v
    }

    pub fn client_count(&self) -> usize {
        self.clients.lock().expect("lock").len()
    }

    fn client(&self, id: u64) -> Client {
        self.clients.lock().expect("lock").get(&id).cloned().unwrap_or(Client { id, hello: ClientHello::default(), current: None })
    }

    fn set_current(&self, client: u64, project: Option<String>) {
        if let Some(c) = self.clients.lock().expect("lock").get_mut(&client) {
            c.current = project;
        }
        self.emit(Event::Clients { clients: self.client_summaries() });
    }

    fn context(&self, client: &Client) -> ExecContext {
        let workspace = client.hello.workspace.as_ref().map(PathBuf::from).unwrap_or_else(|| self.config.projects_dir.clone());
        let mut allowed: Vec<PathBuf> = client.hello.allowed_dirs.iter().map(PathBuf::from).collect();
        allowed.push(self.config.projects_dir.clone());
        allowed.push(self.config.autosave_dir.clone());
        allowed.push(std::env::temp_dir());
        ExecContext {
            paths: PathPolicy { base: workspace, allowed, allow_any: client.hello.kind == "desktop" },
            limits: self.config.limits.clone(),
            client: if client.hello.name.is_empty() { client.hello.kind.clone() } else { client.hello.name.clone() },
        }
    }

    // ---- projects ---------------------------------------------------------------------------

    fn project_summaries(&self) -> Vec<ProjectSummary> {
        let entries: Vec<Arc<Mutex<ProjectEntry>>> = self.projects.lock().expect("lock").values().cloned().collect();
        let clients = self.clients.lock().expect("lock").clone();
        entries
            .iter()
            .map(|e| {
                let p = e.lock().expect("lock");
                ProjectSummary {
                    id: p.id.clone(),
                    name: p.session.doc.model.name.clone(),
                    path: p.session.path.as_ref().map(|x| x.display().to_string()),
                    dirty: p.session.dirty(),
                    revision: p.session.doc.revision(),
                    transaction_owner: p.tx_owner.and_then(|o| clients.get(&o)).map(|c| c.hello.name.clone()),
                }
            })
            .collect()
    }

    fn entry(&self, id: &str) -> Result<Arc<Mutex<ProjectEntry>>, CommandError> {
        self.projects
            .lock()
            .expect("lock")
            .get(id)
            .cloned()
            .ok_or_else(|| err(ErrorCode::NoProject, format!("projeto {id} não está aberto")))
    }

    /// The project a request targets: explicit id, else the client's current one, else the only
    /// open project.
    fn target_project(&self, client: u64, explicit: Option<&str>) -> Result<String, CommandError> {
        if let Some(p) = explicit {
            self.entry(p)?;
            return Ok(p.to_string());
        }
        if let Some(c) = self.client(client).current
            && self.projects.lock().expect("lock").contains_key(&c)
        {
            return Ok(c);
        }
        let projects = self.projects.lock().expect("lock");
        match projects.len() {
            1 => Ok(projects.keys().next().expect("one").clone()),
            0 => Err(err(ErrorCode::NoProject, "nenhum projeto aberto: use project_create ou project_open")),
            _ => Err(err(
                ErrorCode::NoProject,
                format!(
                    "vários projetos abertos ({}); informe `project` ou use project_open",
                    projects.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            )),
        }
    }

    fn insert_project(&self, session: Session) -> String {
        let id = short_id(&session.doc.model.uid);
        let rev = session.doc.revision();
        let entry = ProjectEntry { id: id.clone(), session, tx_owner: None, autosaved_revision: rev };
        self.projects.lock().expect("lock").insert(id.clone(), Arc::new(Mutex::new(entry)));
        self.emit(Event::Projects { projects: self.project_summaries() });
        id
    }

    fn autosave_path(&self, uid: &str) -> PathBuf {
        self.config.autosave_dir.join(format!("{uid}.{}", format::EXTENSION))
    }

    /// Writes recovery copies of dirty projects (not in a transaction).
    pub fn autosave_all(&self) {
        let entries: Vec<Arc<Mutex<ProjectEntry>>> = self.projects.lock().expect("lock").values().cloned().collect();
        for e in entries {
            let mut p = e.lock().expect("lock");
            if p.tx_owner.is_some() || p.session.doc.in_transaction() {
                continue;
            }
            let rev = p.session.doc.revision();
            if !p.session.dirty() || rev == p.autosaved_revision {
                continue;
            }
            let path = self.autosave_path(&p.session.doc.model.uid);
            let meta = json!({
                "name": p.session.doc.model.name,
                "path": p.session.path.as_ref().map(|x| x.display().to_string()),
                "revision": rev,
                "saved_at": now_iso(),
            });
            if format::save(&p.session.doc, &path).is_ok() {
                let _ = std::fs::write(path.with_extension("json"), serde_json::to_vec_pretty(&meta).unwrap_or_default());
                p.autosaved_revision = rev;
            }
        }
    }

    fn remove_autosave(&self, uid: &str) {
        let path = self.autosave_path(uid);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json"));
    }

    fn recoverable(&self) -> Vec<serde_json::Value> {
        let open: Vec<String> = {
            let entries: Vec<Arc<Mutex<ProjectEntry>>> = self.projects.lock().expect("lock").values().cloned().collect();
            entries.iter().map(|e| e.lock().expect("lock").session.doc.model.uid.clone()).collect()
        };
        let mut out = Vec::new();
        if let Ok(dir) = std::fs::read_dir(&self.config.autosave_dir) {
            for e in dir.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some(format::EXTENSION) {
                    continue;
                }
                let uid = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                if open.contains(&uid) {
                    continue;
                }
                let meta: serde_json::Value =
                    std::fs::read(p.with_extension("json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
                out.push(json!({ "autosave": p.display().to_string(), "name": meta["name"], "original_path": meta["path"], "saved_at": meta["saved_at"] }));
            }
        }
        out
    }

    // ---- requests ---------------------------------------------------------------------------

    /// Handles one command for a client. Blocking (call from a blocking thread).
    pub fn call(
        &self,
        client: u64,
        project: Option<&str>,
        command: serde_json::Value,
        expected_revision: Option<u64>,
    ) -> Result<CallResult, CommandError> {
        self.touch();
        let started = Instant::now();
        let name = command.get("cmd").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let args = command.get("args").cloned().unwrap_or(serde_json::Value::Null);
        let cl = self.client(client);
        let result = parse_command(&name, args).and_then(|cmd| self.dispatch(&cl, project, cmd, expected_revision));
        let summary = match &result {
            Ok(r) => match r.data.get("changed_pixels").and_then(|v| v.as_u64()) {
                Some(n) => format!("{n} pixels alterados"),
                None => r.warnings.first().cloned().unwrap_or_else(|| "ok".into()),
            },
            Err(e) => format!("{:?}: {}", e.code, e.message),
        };
        let rec = OpRecord {
            time: now_iso(),
            client: if cl.hello.name.is_empty() { cl.hello.kind.clone() } else { cl.hello.name.clone() },
            client_kind: cl.hello.kind.clone(),
            project: result.as_ref().ok().and_then(|r| r.project.clone()).or(project.map(String::from)),
            command: name,
            ok: result.is_ok(),
            summary,
            millis: started.elapsed().as_millis() as u64,
        };
        {
            let mut log = self.log.lock().expect("lock");
            log.push_back(rec.clone());
            while log.len() > OP_LOG {
                log.pop_front();
            }
        }
        self.emit(Event::Operation(rec));
        result
    }

    fn dispatch(
        &self,
        cl: &Client,
        project: Option<&str>,
        cmd: Command,
        expected_revision: Option<u64>,
    ) -> Result<CallResult, CommandError> {
        let ctx = self.context(cl);
        match cmd {
            Command::ProjectCreate(a) => {
                let session = sprite_commands::create_project(&a, &ctx)?;
                let info = session.doc.model.sprites.first().map(|s| json!({ "id": s.id, "name": s.name, "width": s.width, "height": s.height, "layer": s.layers[0].id, "frame": s.frames[0].id }));
                let rev = session.doc.revision();
                let id = self.insert_project(session);
                self.set_current(cl.id, Some(id.clone()));
                Ok(CallResult {
                    project: Some(id.clone()),
                    data: json!({ "project": id, "name": a.name, "sprite": info, "note": "projeto em memória; use project_save para gravar" }),
                    images: vec![],
                    changes: vec![],
                    warnings: vec![],
                    revision: rev,
                    transaction: None,
                })
            }
            Command::ImagePixelate(a) => {
                let (session, out) = sprite_commands::picture::image_pixelate(&a, &ctx)?;
                let Some(session) = session else { return Ok(CallResult::from_outcome(None, out)) };
                let rev = session.doc.revision();
                let id = self.insert_project(session);
                self.set_current(cl.id, Some(id.clone()));
                let mut r = CallResult::from_outcome(Some(id.clone()), out);
                r.data["project"] = json!(id);
                r.data["note"] = json!("projeto em memória; use project_save para gravar");
                r.revision = rev;
                Ok(r)
            }
            Command::ProjectOpen(a) => self.open(cl, &ctx, &a.path, a.recover),
            Command::ProjectList(_) => Ok(CallResult {
                project: None,
                data: json!({ "open": self.project_summaries(), "current": self.client(cl.id).current, "recoverable": self.recoverable() }),
                images: vec![],
                changes: vec![],
                warnings: vec![],
                revision: 0,
                transaction: None,
            }),
            Command::EditorStatus(_) => Ok(CallResult {
                project: None,
                data: self.status(cl.id),
                images: vec![],
                changes: vec![],
                warnings: vec![],
                revision: 0,
                transaction: None,
            }),
            Command::EditorGetCapabilities(_) => Ok(CallResult {
                project: None,
                data: capabilities(&self.config.limits),
                images: vec![],
                changes: vec![],
                warnings: vec![],
                revision: 0,
                transaction: None,
            }),
            other => {
                let pid = self.target_project(cl.id, project)?;
                self.set_current_quiet(cl.id, &pid);
                self.on_project(cl, &ctx, &pid, other, expected_revision)
            }
        }
    }

    fn set_current_quiet(&self, client: u64, project: &str) {
        let changed = {
            let mut clients = self.clients.lock().expect("lock");
            match clients.get_mut(&client) {
                Some(c) if c.current.as_deref() != Some(project) => {
                    c.current = Some(project.to_string());
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.emit(Event::Clients { clients: self.client_summaries() });
        }
    }

    /// Locks a project, waiting while another client holds a transaction on it.
    fn acquire<'a>(
        &self,
        entry: &'a Mutex<ProjectEntry>,
        client: u64,
        write: bool,
    ) -> Result<std::sync::MutexGuard<'a, ProjectEntry>, CommandError> {
        let deadline = Instant::now() + TX_WAIT;
        loop {
            let guard = entry.lock().expect("lock");
            let blocked = write && guard.tx_owner.is_some_and(|o| o != client);
            if !blocked {
                return Ok(guard);
            }
            drop(guard);
            if Instant::now() >= deadline {
                return Err(err(
                    ErrorCode::Locked,
                    "outro cliente está com uma transação aberta neste projeto; tente de novo em instantes",
                ));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn on_project(
        &self,
        cl: &Client,
        ctx: &ExecContext,
        pid: &str,
        cmd: Command,
        expected_revision: Option<u64>,
    ) -> Result<CallResult, CommandError> {
        let write = !matches!(cmd.kind(), Kind::Read);
        let name = cmd.name();
        let entry = self.entry(pid)?;
        let mut p = self.acquire(&entry, cl.id, write)?;
        if let Some(expected) = expected_revision {
            let current = p.session.doc.revision();
            if write && expected != current {
                return Err(err(
                    ErrorCode::RevisionConflict,
                    format!("o projeto está na revisão {current}, não na {expected}: alguém mudou o projeto; leia de novo e refaça"),
                )
                .with_details(json!({ "current_revision": current })));
            }
        }
        let outcome: Outcome = match cmd {
            Command::ProjectSave(a) => {
                let path = match &a.path {
                    Some(raw) => {
                        let mut path = ctx.paths.resolve(raw)?;
                        if path.extension().is_none() {
                            path.set_extension(format::EXTENSION);
                        }
                        path
                    }
                    None => p
                        .session
                        .path
                        .clone()
                        .ok_or_else(|| err(ErrorCode::InvalidArgument, "o projeto ainda não tem arquivo: informe `path` (.psmproj)"))?,
                };
                if p.session.doc.in_transaction() {
                    return Err(err(ErrorCode::TransactionState, "confirme ou desfaça a transação antes de salvar"));
                }
                format::save(&p.session.doc, &path)?;
                p.session.path = Some(path.clone());
                p.session.saved_revision = p.session.doc.revision();
                self.remove_autosave(&p.session.doc.model.uid.clone());
                let rev = p.session.doc.revision();
                let mut data = json!({ "saved": path.display().to_string() });
                // Export jobs marked `on_save` run now; a failed export is only a warning and
                // never undoes the save.
                let (exports, warnings) = on_save_exports(&mut p.session, ctx);
                if let Some(e) = exports {
                    data["exports"] = e;
                }
                drop(p);
                self.emit(Event::Projects { projects: self.project_summaries() });
                return Ok(CallResult {
                    project: Some(pid.into()),
                    data,
                    images: vec![],
                    changes: vec![],
                    warnings,
                    revision: rev,
                    transaction: None,
                });
            }
            Command::ProjectClose(a) => {
                if p.tx_owner.is_some() {
                    return Err(err(ErrorCode::TransactionState, "há uma transação aberta neste projeto"));
                }
                if a.save {
                    let path =
                        p.session.path.clone().ok_or_else(|| {
                            err(ErrorCode::InvalidArgument, "o projeto não tem arquivo; salve com project_save {path} antes")
                        })?;
                    format::save(&p.session.doc, &path)?;
                    p.session.saved_revision = p.session.doc.revision();
                    self.remove_autosave(&p.session.doc.model.uid.clone());
                } else if p.session.dirty() && !a.discard {
                    return Err(err(ErrorCode::InvalidArgument, "há alterações não salvas: use save: true ou discard: true"));
                } else if p.session.dirty() {
                    // Discarded changes stay recoverable.
                    let uid = p.session.doc.model.uid.clone();
                    let _ = format::save(&p.session.doc, &self.autosave_path(&uid));
                }
                drop(p);
                self.projects.lock().expect("lock").remove(pid);
                for c in self.clients.lock().expect("lock").values_mut() {
                    if c.current.as_deref() == Some(pid) {
                        c.current = None;
                    }
                }
                self.emit(Event::Projects { projects: self.project_summaries() });
                self.emit(Event::Clients { clients: self.client_summaries() });
                return Ok(CallResult {
                    project: Some(pid.into()),
                    data: json!({ "closed": pid }),
                    images: vec![],
                    changes: vec![],
                    warnings: vec![],
                    revision: 0,
                    transaction: None,
                });
            }
            Command::TransactionBegin(a) => {
                if p.tx_owner.is_some() || p.session.doc.in_transaction() {
                    return Err(err(ErrorCode::TransactionState, "já existe uma transação aberta"));
                }
                let label = a.label.unwrap_or_else(|| format!("transação ({})", ctx.client));
                let id = p.session.doc.begin(label.clone())?;
                p.tx_owner = Some(cl.id);
                let rev = p.session.doc.revision();
                drop(p);
                self.emit(Event::Projects { projects: self.project_summaries() });
                return Ok(CallResult {
                    project: Some(pid.into()),
                    data: json!({ "transaction": id, "label": label }),
                    images: vec![],
                    changes: vec![],
                    warnings: vec![],
                    revision: rev,
                    transaction: Some(id),
                });
            }
            Command::TransactionCommit(_) | Command::TransactionRollback(_) => {
                if p.tx_owner != Some(cl.id) {
                    return Err(err(ErrorCode::TransactionState, "você não tem uma transação aberta neste projeto"));
                }
                let commit = matches!(cmd, Command::TransactionCommit(_));
                let data = if commit {
                    let info = p.session.doc.commit()?;
                    json!({ "committed": info.transaction, "label": info.label, "changed_images": info.changed_images.len(), "revision": info.revision })
                } else {
                    p.session.doc.rollback()?;
                    p.session.invalidate_cache();
                    json!({ "rolled_back": true })
                };
                p.tx_owner = None;
                let rev = p.session.doc.revision();
                let changes = all_structure(&p.session);
                drop(p);
                self.emit(Event::Changed {
                    project: pid.into(),
                    revision: rev,
                    source: ctx.client.clone(),
                    command: name.into(),
                    changes: changes.clone(),
                    transaction: None,
                });
                self.emit(Event::Projects { projects: self.project_summaries() });
                return Ok(CallResult {
                    project: Some(pid.into()),
                    data,
                    images: vec![],
                    changes,
                    warnings: vec![],
                    revision: rev,
                    transaction: None,
                });
            }
            Command::HistoryUndo(a) | Command::HistoryRedo(a) => {
                let undo = name == "history_undo";
                if p.session.doc.in_transaction() {
                    return Err(err(ErrorCode::TransactionState, "feche a transação antes de desfazer/refazer"));
                }
                let mut labels = Vec::new();
                for _ in 0..a.steps.clamp(1, 1000) {
                    let step = if undo { p.session.doc.undo()? } else { p.session.doc.redo()? };
                    match step {
                        Some((label, _)) => labels.push(label),
                        None => break,
                    }
                }
                p.session.invalidate_cache();
                let rev = p.session.doc.revision();
                let changes = all_structure(&p.session);
                let (u, r) = p.session.doc.history();
                drop(p);
                if !labels.is_empty() {
                    self.emit(Event::Changed {
                        project: pid.into(),
                        revision: rev,
                        source: ctx.client.clone(),
                        command: name.into(),
                        changes: changes.clone(),
                        transaction: None,
                    });
                }
                let key = if undo { "undone" } else { "redone" };
                return Ok(CallResult {
                    project: Some(pid.into()),
                    data: json!({ key: labels, "can_undo": u.last().map(|x| x.1.clone()), "can_redo": r.first().map(|x| x.1.clone()) }),
                    images: vec![],
                    changes,
                    warnings: vec![],
                    revision: rev,
                    transaction: None,
                });
            }
            other => {
                // A write inside someone's transaction joins it only for the owner (checked by
                // lock_for); outside transactions each command is its own undo step.
                p.session.execute(other, ctx)?
            }
        };
        let rev = outcome.revision;
        let tx = p.session.doc.transaction_id();
        drop(p);
        if write && (!outcome.changes.is_empty()) {
            self.emit(Event::Changed {
                project: pid.into(),
                revision: rev,
                source: ctx.client.clone(),
                command: name.into(),
                changes: outcome.changes.clone(),
                transaction: tx,
            });
            self.emit(Event::Projects { projects: self.project_summaries() });
        }
        Ok(CallResult::from_outcome(Some(pid.into()), outcome))
    }

    fn open(&self, cl: &Client, ctx: &ExecContext, raw: &str, recover: bool) -> Result<CallResult, CommandError> {
        let path = ctx.paths.resolve(raw)?;
        let canon = clean_path(std::fs::canonicalize(&path).unwrap_or(path.clone()));
        // Already open: reuse the session.
        let existing: Vec<Arc<Mutex<ProjectEntry>>> = self.projects.lock().expect("lock").values().cloned().collect();
        for e in existing {
            let p = e.lock().expect("lock");
            let same = p.session.path.as_ref().map(|x| clean_path(std::fs::canonicalize(x).unwrap_or(x.clone()))) == Some(canon.clone());
            if same {
                let id = p.id.clone();
                let rev = p.session.doc.revision();
                drop(p);
                self.set_current(cl.id, Some(id.clone()));
                return Ok(CallResult {
                    project: Some(id.clone()),
                    data: json!({ "project": id, "reused": true }),
                    images: vec![],
                    changes: vec![],
                    warnings: vec![],
                    revision: rev,
                    transaction: None,
                });
            }
        }
        let is_autosave =
            canon.starts_with(clean_path(std::fs::canonicalize(&self.config.autosave_dir).unwrap_or(self.config.autosave_dir.clone())));
        let mut doc = format::load(&canon)?;
        let mut warnings = Vec::new();
        let mut file_path = Some(canon.clone());
        if is_autosave {
            // Opening a recovery copy: keep the original path if known, else leave it unsaved.
            let meta: serde_json::Value =
                std::fs::read(canon.with_extension("json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(json!({}));
            file_path = meta["path"].as_str().map(PathBuf::from);
            warnings.push("projeto recuperado da cópia automática; salve para mantê-lo".into());
        } else {
            let auto = self.autosave_path(&doc.model.uid);
            let newer = match (std::fs::metadata(&auto).and_then(|m| m.modified()), std::fs::metadata(&canon).and_then(|m| m.modified())) {
                (Ok(a), Ok(f)) => a > f,
                _ => false,
            };
            if newer && recover {
                doc = format::load(&auto)?;
                warnings.push("aberta a cópia de recuperação, mais nova que o arquivo".into());
            } else if newer {
                warnings.push("existe uma cópia de recuperação mais nova que o arquivo; abra com recover: true para usá-la".into());
            }
        }
        let mut session = Session::new(doc, file_path);
        if is_autosave || recover && !warnings.is_empty() {
            // Recovered content is unsaved by definition.
            session.saved_revision = u64::MAX;
        }
        let rev = session.doc.revision();
        let summary = json!({
            "name": session.doc.model.name,
            "sprites": session.doc.model.sprites.iter().map(|s| json!({ "id": s.id, "name": s.name, "width": s.width, "height": s.height, "frames": s.frames.len(), "layers": s.layers.len(), "animations": s.animations.iter().map(|a| &a.name).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        });
        let id = self.insert_project(session);
        self.set_current(cl.id, Some(id.clone()));
        Ok(CallResult {
            project: Some(id.clone()),
            data: json!({ "project": id, "opened": canon.display().to_string(), "summary": summary }),
            images: vec![],
            changes: vec![],
            warnings,
            revision: rev,
            transaction: None,
        })
    }

    pub fn status(&self, client: u64) -> serde_json::Value {
        let log: Vec<OpRecord> = self.log.lock().expect("lock").iter().rev().take(20).cloned().collect();
        json!({
            "runtime": { "version": env!("CARGO_PKG_VERSION"), "protocol": PROTOCOL_VERSION, "endpoint": crate::transport::describe(&self.config.endpoint), "pid": std::process::id(), "uptime_s": self.started.elapsed().as_secs() },
            "clients": self.client_summaries(),
            "you": client,
            "current_project": self.client(client).current,
            "projects": self.project_summaries(),
            "recent_operations": log,
            "autosave_dir": self.config.autosave_dir.display().to_string(),
            "projects_dir": self.config.projects_dir.display().to_string(),
        })
    }

    pub fn recent_ops(&self) -> Vec<OpRecord> {
        self.log.lock().expect("lock").iter().cloned().collect()
    }

    /// Composited RGBA for the editor canvas.
    pub fn render(
        &self,
        client: u64,
        project: Option<&str>,
        sprite: Option<u32>,
        frame: u32,
        layer: Option<u32>,
        all_layers: bool,
    ) -> Result<serde_json::Value, CommandError> {
        let pid = self.target_project(client, project)?;
        let entry = self.entry(&pid)?;
        let mut p = entry.lock().expect("lock");
        let model = &p.session.doc.model;
        let sp = match sprite {
            Some(id) => model.sprites.iter().find(|s| s.id == id),
            None => model.sprites.first(),
        }
        .ok_or_else(|| err(ErrorCode::NotFound, "sprite não existe"))?;
        if sp.frame(frame).is_none() {
            return Err(err(ErrorCode::NotFound, format!("frame {frame} não existe")));
        }
        let sprite_id = sp.id;
        let img = match (layer, all_layers) {
            (Some(l), _) => layer_image(sp, p.session.doc.images(), l, frame),
            (None, true) => compose_frame(
                sp,
                p.session.doc.images(),
                frame,
                &ComposeOptions { include_hidden: true, include_reference: true, layers: None },
            ),
            // The editor canvas: served from the composition cache.
            (None, false) => p.session.composite(sprite_id, frame).ok_or_else(|| err(ErrorCode::NotFound, "frame não existe"))?,
        };
        use base64::Engine;
        Ok(
            json!({ "width": img.width(), "height": img.height(), "rgba": base64::engine::general_purpose::STANDARD.encode(img.to_rgba()), "revision": p.session.doc.revision() }),
        )
    }

    /// Raw RGBA of a stored image (reference images).
    pub fn render_image(&self, client: u64, project: Option<&str>, image: u32) -> Result<serde_json::Value, CommandError> {
        let pid = self.target_project(client, project)?;
        let entry = self.entry(&pid)?;
        let p = entry.lock().expect("lock");
        let img = p.session.doc.image(image).ok_or_else(|| err(ErrorCode::NotFound, format!("imagem {image} não existe")))?;
        use base64::Engine;
        Ok(json!({ "width": img.width(), "height": img.height(), "rgba": base64::engine::general_purpose::STANDARD.encode(img.to_rgba()) }))
    }

    /// Full editor state: model JSON (no pixels), path, revision, history labels.
    pub fn state(&self, client: u64, project: Option<&str>) -> Result<serde_json::Value, CommandError> {
        let pid = self.target_project(client, project)?;
        let entry = self.entry(&pid)?;
        let p = entry.lock().expect("lock");
        let (undo, redo) = p.session.doc.history();
        Ok(json!({
            "project": pid,
            "model": p.session.doc.model,
            "path": p.session.path.as_ref().map(|x| x.display().to_string()),
            "dirty": p.session.dirty(),
            "revision": p.session.doc.revision(),
            "transaction": p.session.doc.transaction_id(),
            "transaction_owner": p.tx_owner,
            "undo": undo.iter().map(|x| &x.1).collect::<Vec<_>>(),
            "redo": redo.iter().map(|x| &x.1).collect::<Vec<_>>(),
            "snapshots": p.session.doc.snapshots.iter().map(|s| json!({ "id": s.id, "name": s.name, "created": s.created })).collect::<Vec<_>>(),
        }))
    }

    pub fn idle_for(&self) -> Duration {
        self.last_activity.lock().expect("lock").elapsed()
    }
}

fn all_structure(s: &Session) -> Vec<sprite_commands::Change> {
    s.doc
        .model
        .sprites
        .iter()
        .map(|sp| sprite_commands::Change { sprite: sp.id, layer: None, frame: None, rect: None, kind: "structure".into() })
        .collect()
}

/// Runs the project's `on_save` export jobs (Godot area). Returns the per-job results (when
/// there are jobs) and warnings for the failures.
fn on_save_exports(session: &mut Session, ctx: &ExecContext) -> (Option<serde_json::Value>, Vec<String>) {
    if !session.doc.model.extras.godot.export_jobs.iter().any(|j| j.on_save) {
        return (None, Vec::new());
    }
    let cmd = Command::ExportJobsRun(sprite_commands::ExportJobsRun { jobs: Vec::new(), on_save: true });
    match session.execute(cmd, ctx) {
        Ok(out) => (Some(out.data), out.warnings),
        Err(e) => (None, vec![format!("projeto salvo, mas as exportações automáticas falharam: {}", e.message)]),
    }
}

fn short_id(uid: &str) -> String {
    uid.chars().take(8).collect()
}

pub fn capabilities(limits: &Limits) -> serde_json::Value {
    let tools: Vec<serde_json::Value> = sprite_commands::specs().iter().map(|s| json!({ "name": s.name, "kind": s.kind })).collect();
    json!({
        "app": "Pixel Studio Maker",
        "version": env!("CARGO_PKG_VERSION"),
        "sdc_version": sprite_drawing_version(),
        "limits": limits,
        "sdc_ops": ["pixel", "pixels", "matrix", "line", "polyline", "rect", "ellipse", "circle", "polygon", "curve", "fill", "replace_color", "clear", "outline", "dither", "gradient", "shade", "drop_shadow", "pattern", "transform", "copy", "cleanup", "quantize", "adjust", "stroke", "text"],
        "colors": "\"#rgb\", \"#rgba\", \"#rrggbb\", \"#rrggbbaa\", \"clear\" ou índice da paleta ativa",
        "matrix_format": { "rows": "linhas de caracteres", "legend": "char → cor | null (manter) | \"clear\"", "dot": "'.' mantém o pixel", "rle": "\"3k2.\" = \"kkk..\"" },
        "directions": ["N", "NE", "E", "SE", "S", "SW", "W", "NW"],
        "project_format": format!(".{} v{}", format::EXTENSION, format::FORMAT_VERSION),
        "import": ["png", "gif", "aseprite (.aseprite/.ase)", "sprite sheet (slice)", "jpg (só na conversão em pixel art)"],
        "palette_formats": ["gpl", "hex", "pal (JASC; RIFF só leitura)", "txt (Paint.NET)", "png", "gif"],
        "export": ["png", "png sequence", "gif", "sprite sheet (horizontal, vertical, grid, rows, packed)", "json", "godot SpriteFrames .tres", "godot cena .tscn (AnimationPlayer, hitboxes, pontos)", "godot TileSet .tres", "godot NinePatchRect .tscn", "BMFont .fnt", "normal map", "variantes de paleta", "rpg maker (personagem, faces, battler lateral)"],
        "tools": tools,
    })
}

fn sprite_drawing_version() -> u32 {
    1
}

// ---- connection handling --------------------------------------------------------------------

async fn write_line(w: &mut BoxWrite, v: &impl serde::Serialize) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(v).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    w.write_all(&bytes).await?;
    w.flush().await
}

/// Serves one connection until it closes.
pub async fn serve_connection(rt: Arc<Runtime>, read: BoxRead, write: BoxWrite) {
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<serde_json::Value>();
    let writer = tokio::spawn(async move {
        let mut write = write;
        while let Some(v) = out_rx.recv().await {
            if write_line(&mut write, &v).await.is_err() {
                break;
            }
        }
    });
    let mut reader = BufReader::new(read);
    let mut client_id: Option<u64> = None;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(n) if n > MAX_LINE => break,
            Ok(_) => {}
        }
        let req: Request = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                let _ =
                    out_tx.send(json!({ "id": 0, "error": { "code": "INVALID_ARGUMENT", "message": format!("mensagem inválida: {e}") } }));
                continue;
            }
        };
        let id = req.id;
        let cid = match (&req.body, client_id) {
            (RequestBody::Hello { client, .. }, _) => {
                let c = rt.register(client.clone());
                client_id = Some(c);
                let _ = out_tx
                    .send(json!({ "id": id, "ok": { "client": c, "protocol": PROTOCOL_VERSION, "runtime": env!("CARGO_PKG_VERSION") } }));
                continue;
            }
            (_, Some(c)) => c,
            (_, None) => {
                let c = rt.register(ClientHello { kind: "unknown".into(), name: "cliente".into(), ..Default::default() });
                client_id = Some(c);
                c
            }
        };
        let rt2 = rt.clone();
        let tx = out_tx.clone();
        match req.body {
            RequestBody::Subscribe => {
                let mut rx = rt.subscribe();
                let tx2 = out_tx.clone();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(ev) => {
                                if tx2.send(serde_json::to_value(&ev).unwrap_or_default()).is_err() {
                                    break;
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                });
                let _ = out_tx.send(json!({ "id": id, "ok": { "subscribed": true } }));
            }
            RequestBody::Ping => {
                let _ = out_tx.send(json!({ "id": id, "ok": { "pong": true } }));
            }
            RequestBody::Select { project } => {
                let r = rt.entry(&project).map(|_| {
                    rt.set_current(cid, Some(project.clone()));
                    json!({ "project": project })
                });
                let _ = out_tx.send(respond(id, r));
            }
            body => {
                // Commands can block (waiting on a transaction, exporting): run off the reactor,
                // but keep per-connection order by awaiting each one.
                let result = tokio::task::spawn_blocking(move || match body {
                    RequestBody::Call { project, command, expected_revision } => {
                        rt2.call(cid, project.as_deref(), command, expected_revision).map(|r| serde_json::to_value(r).unwrap_or_default())
                    }
                    RequestBody::Render { project, sprite, frame, layer, all_layers } => {
                        rt2.render(cid, project.as_deref(), sprite, frame, layer, all_layers)
                    }
                    RequestBody::State { project } => rt2.state(cid, project.as_deref()),
                    RequestBody::RenderImage { project, image } => rt2.render_image(cid, project.as_deref(), image),
                    _ => Err(err(ErrorCode::Unsupported, "operação desconhecida")),
                })
                .await
                .unwrap_or_else(|e| Err(err(ErrorCode::Internal, format!("falha interna: {e}"))));
                let _ = tx.send(respond(id, result));
            }
        }
    }
    if let Some(c) = client_id {
        rt.unregister(c);
    }
    drop(out_tx);
    let _ = writer.await;
}

fn respond(id: u64, r: Result<serde_json::Value, CommandError>) -> serde_json::Value {
    match r {
        Ok(v) => json!({ "id": id, "ok": v }),
        Err(e) => json!({ "id": id, "error": e }),
    }
}

/// Appends a line to `runtime.log` next to the autosave folder (diagnostics only).
pub fn log_line(config: &RuntimeConfig, msg: &str) {
    use std::io::Write;
    let path = config.autosave_dir.parent().map(|p| p.join("runtime.log")).unwrap_or_else(|| PathBuf::from("runtime.log"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} [{}] {msg}", now_iso(), std::process::id());
    }
}

/// Runs the runtime until it has been idle (no clients) for `idle_timeout`.
pub async fn run(config: RuntimeConfig) -> std::io::Result<()> {
    let _ = std::fs::create_dir_all(&config.autosave_dir);
    let mut listener = match Listener::bind(&config.endpoint) {
        Ok(l) => l,
        Err(e) => {
            log_line(&config, &format!("não iniciou em {}: {e}", config.endpoint));
            return Err(e);
        }
    };
    log_line(
        &config,
        &format!("iniciado em {} (inatividade {}s)", crate::transport::describe(&config.endpoint), config.idle_timeout.as_secs()),
    );
    let rt = Runtime::new(config);
    let autosave_rt = rt.clone();
    tokio::spawn(async move {
        let every = autosave_rt.config.autosave_every;
        loop {
            tokio::time::sleep(every).await;
            let r = autosave_rt.clone();
            let _ = tokio::task::spawn_blocking(move || r.autosave_all()).await;
        }
    });
    loop {
        let idle = if rt.client_count() == 0 { rt.config.idle_timeout.saturating_sub(rt.idle_for()) } else { rt.config.idle_timeout };
        tokio::select! {
            accepted = listener.accept() => {
                match accepted {
                    Ok((r, w)) => {
                        rt.touch();
                        tokio::spawn(serve_connection(rt.clone(), r, w));
                    }
                    Err(e) => {
                        // A client that vanished mid-handshake must not take the runtime down.
                        log_line(&rt.config, &format!("falha ao aceitar conexão: {e}"));
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        if let Err(e2) = listener.recreate() {
                            log_line(&rt.config, &format!("não foi possível recriar o pipe: {e2}; encerrando"));
                            let r = rt.clone();
                            let _ = tokio::task::spawn_blocking(move || r.autosave_all()).await;
                            return Err(e2);
                        }
                    }
                }
            }
            _ = tokio::time::sleep(idle.max(Duration::from_millis(200))) => {
                if rt.client_count() == 0 && rt.idle_for() >= rt.config.idle_timeout {
                    let r = rt.clone();
                    let _ = tokio::task::spawn_blocking(move || r.autosave_all()).await;
                    log_line(&rt.config, "encerrado por inatividade (sem clientes)");
                    return Ok(());
                }
            }
        }
    }
}

/// Strips the `\\?\` verbatim prefix Windows adds to canonical local paths.
pub fn clean_path(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\")
        && !rest.starts_with("UNC\\")
    {
        return PathBuf::from(rest.to_string());
    }
    p
}
