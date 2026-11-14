//! Client side of the runtime protocol: used by the MCP server, the desktop app and the CLI.

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use sprite_commands::{CommandError, ErrorCode};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::protocol::*;
use crate::transport;

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Response>>>>;

pub struct RuntimeClient {
    out: mpsc::UnboundedSender<Vec<u8>>,
    pending: Pending,
    events: broadcast::Sender<Event>,
    next: AtomicU64,
    alive: Arc<AtomicBool>,
    shutdown: Arc<tokio::sync::Notify>,
    pub client_id: u64,
}

impl Drop for RuntimeClient {
    fn drop(&mut self) {
        // Closes the read half too, so the runtime sees the disconnect right away.
        self.shutdown.notify_one();
    }
}

fn disconnected() -> CommandError {
    CommandError::new(ErrorCode::Internal, "a conexão com o runtime caiu")
}

impl RuntimeClient {
    /// Connects to a running runtime and introduces itself.
    pub async fn connect(endpoint: &str, hello: ClientHello) -> io::Result<RuntimeClient> {
        let (read, mut write) = transport::connect(endpoint).await?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (events, _) = broadcast::channel(1024);
        let alive = Arc::new(AtomicBool::new(true));
        let (out, mut out_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let alive_w = alive.clone();
        tokio::spawn(async move {
            while let Some(bytes) = out_rx.recv().await {
                if write.write_all(&bytes).await.is_err() || write.flush().await.is_err() {
                    break;
                }
            }
            alive_w.store(false, Ordering::SeqCst);
        });
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let (p2, e2, a2, s2) = (pending.clone(), events.clone(), alive.clone(), shutdown.clone());
        tokio::spawn(async move {
            let mut reader = BufReader::new(read);
            let mut line = String::new();
            loop {
                line.clear();
                let read = tokio::select! {
                    r = reader.read_line(&mut line) => r,
                    _ = s2.notified() => break,
                };
                match read {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                match serde_json::from_str::<Incoming>(line.trim()) {
                    Ok(Incoming::Response(r)) => {
                        if let Some(tx) = p2.lock().expect("lock").remove(&r.id) {
                            let _ = tx.send(r);
                        }
                    }
                    Ok(Incoming::Event(ev)) => {
                        let _ = e2.send(ev);
                    }
                    Err(_) => {}
                }
            }
            a2.store(false, Ordering::SeqCst);
            // Fail everything still waiting.
            p2.lock().expect("lock").clear();
        });
        let mut client = RuntimeClient { out, pending, events, next: AtomicU64::new(1), alive, shutdown, client_id: 0 };
        let ok = client
            .request(RequestBody::Hello { client: hello, protocol: PROTOCOL_VERSION })
            .await
            .map_err(|e| io::Error::other(e.message))?;
        client.client_id = ok["client"].as_u64().unwrap_or(0);
        Ok(client)
    }

    /// Connects, starting `exe runtime --endpoint <endpoint>` in the background if no runtime
    /// answers. Two clients racing to start it is fine: the second runtime cannot bind and exits.
    pub async fn connect_or_spawn(endpoint: &str, hello: ClientHello, exe: &Path) -> io::Result<RuntimeClient> {
        if let Ok(c) = RuntimeClient::connect(endpoint, hello.clone()).await {
            return Ok(c);
        }
        spawn_runtime(exe, endpoint)?;
        let mut last = io::Error::other("runtime não respondeu");
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            match RuntimeClient::connect(endpoint, hello.clone()).await {
                Ok(c) => return Ok(c),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Sends a request and waits for its response.
    pub async fn request(&self, body: RequestBody) -> Result<Value, CommandError> {
        if !self.is_alive() {
            return Err(disconnected());
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().expect("lock").insert(id, tx);
        let mut bytes = serde_json::to_vec(&Request { id, body }).map_err(|e| CommandError::new(ErrorCode::Internal, e.to_string()))?;
        bytes.push(b'\n');
        self.out.send(bytes).map_err(|_| disconnected())?;
        let resp = rx.await.map_err(|_| disconnected())?;
        match (resp.ok, resp.error) {
            (_, Some(e)) => Err(e),
            (Some(v), None) => Ok(v),
            (None, None) => Ok(Value::Null),
        }
    }

    /// Runs a command by name with JSON arguments.
    pub async fn call(
        &self,
        project: Option<String>,
        name: &str,
        args: Value,
        expected_revision: Option<u64>,
    ) -> Result<CallResult, CommandError> {
        let v = self.request(RequestBody::Call { project, command: json!({ "cmd": name, "args": args }), expected_revision }).await?;
        serde_json::from_value(v).map_err(|e| CommandError::new(ErrorCode::Internal, format!("resposta inválida do runtime: {e}")))
    }

    /// Starts receiving events.
    pub async fn subscribe(&self) -> Result<broadcast::Receiver<Event>, CommandError> {
        let rx = self.events.subscribe();
        self.request(RequestBody::Subscribe).await?;
        Ok(rx)
    }
}

/// Windows: a child inherits every inheritable handle, including this process's own stdio
/// pipes. A runtime holding the MCP server's stdout open would make the host wait for EOF long
/// after the server exited, so our standard handles stop being inheritable first.
#[cfg(windows)]
pub fn disinherit_std_handles() {
    type Handle = *mut core::ffi::c_void;
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn SetHandleInformation(handle: Handle, mask: u32, flags: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 1;
    // STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE
    for which in [-10i32 as u32, -11i32 as u32, -12i32 as u32] {
        // SAFETY: plain Win32 calls on this process's own standard handles; invalid or null
        // handles are skipped and failures are harmless.
        unsafe {
            let h = GetStdHandle(which);
            if !h.is_null() && h as isize != -1 {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn disinherit_std_handles() {}

/// Starts a detached runtime process.
pub fn spawn_runtime(exe: &Path, endpoint: &str) -> io::Result<()> {
    #[cfg(windows)]
    disinherit_std_handles();
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("runtime")
        .arg("--endpoint")
        .arg(endpoint)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // MCP hosts often run servers inside a job object that kills the whole tree when the
        // server exits. The runtime must outlive its first client, so it leaves the job when the
        // job allows it.
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
        cmd.creation_flags(base | CREATE_BREAKAWAY_FROM_JOB);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
        cmd.creation_flags(base);
    }
    cmd.spawn().map(|_| ())
}
