//! Local IPC: Windows named pipes, Unix domain sockets elsewhere. Both expose a byte stream
//! split into a reader and a writer.

use std::io;
#[cfg(unix)]
use std::path::PathBuf;

use tokio::io::{AsyncRead, AsyncWrite};

pub type BoxRead = Box<dyn AsyncRead + Send + Unpin>;
pub type BoxWrite = Box<dyn AsyncWrite + Send + Unpin>;

/// Default endpoint name for the current user. `PSM_RUNTIME_PIPE` overrides it (tests use this to
/// get an isolated runtime).
pub fn default_endpoint() -> String {
    if let Ok(v) = std::env::var("PSM_RUNTIME_PIPE")
        && !v.trim().is_empty()
    {
        return v;
    }
    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "user".into());
    let user: String = user.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    format!("pixel-studio-maker-{user}")
}

#[cfg(windows)]
fn pipe_path(name: &str) -> String {
    if name.starts_with(r"\\.\pipe\") { name.to_string() } else { format!(r"\\.\pipe\{name}") }
}

#[cfg(unix)]
fn socket_path(name: &str) -> PathBuf {
    if name.contains('/') {
        return PathBuf::from(name);
    }
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    dir.join(format!("{name}.sock"))
}

/// Human-readable endpoint (for status screens).
pub fn describe(name: &str) -> String {
    #[cfg(windows)]
    {
        pipe_path(name)
    }
    #[cfg(unix)]
    {
        socket_path(name).display().to_string()
    }
}

/// Connects to a running runtime.
pub async fn connect(name: &str) -> io::Result<(BoxRead, BoxWrite)> {
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        let path = pipe_path(name);
        let mut tries = 0;
        loop {
            match ClientOptions::new().open(&path) {
                Ok(client) => {
                    let (r, w) = tokio::io::split(client);
                    return Ok((Box::new(r), Box::new(w)));
                }
                // All instances busy: the server is creating the next one.
                Err(e) if e.raw_os_error() == Some(231) && tries < 50 => {
                    tries += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
    #[cfg(unix)]
    {
        let stream = tokio::net::UnixStream::connect(socket_path(name)).await?;
        let (r, w) = stream.into_split();
        Ok((Box::new(r), Box::new(w)))
    }
}

/// Server side: accepts connections one at a time.
pub struct Listener {
    #[cfg(windows)]
    path: String,
    #[cfg(windows)]
    next: tokio::net::windows::named_pipe::NamedPipeServer,
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(unix)]
    path: PathBuf,
}

impl Listener {
    /// Fails with `AddrInUse`/`PermissionDenied` when another runtime already owns the endpoint.
    pub fn bind(name: &str) -> io::Result<Listener> {
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            let path = pipe_path(name);
            let next = ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create(&path)?;
            Ok(Listener { path, next })
        }
        #[cfg(unix)]
        {
            let path = socket_path(name);
            if path.exists() {
                // A stale socket from a crashed runtime: remove it only if nobody answers.
                if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                    return Err(io::Error::new(io::ErrorKind::AddrInUse, "runtime já está rodando"));
                }
                let _ = std::fs::remove_file(&path);
            }
            let inner = tokio::net::UnixListener::bind(&path)?;
            Ok(Listener { inner, path })
        }
    }

    /// Replaces the pending pipe instance after a failed accept (Windows); no-op elsewhere.
    pub fn recreate(&mut self) -> io::Result<()> {
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            self.next = ServerOptions::new().reject_remote_clients(true).create(&self.path)?;
        }
        Ok(())
    }

    pub async fn accept(&mut self) -> io::Result<(BoxRead, BoxWrite)> {
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            self.next.connect().await?;
            let fresh = ServerOptions::new().reject_remote_clients(true).create(&self.path)?;
            let connected = std::mem::replace(&mut self.next, fresh);
            let (r, w) = tokio::io::split(connected);
            Ok((Box::new(r), Box::new(w)))
        }
        #[cfg(unix)]
        {
            let (stream, _) = self.inner.accept().await?;
            let (r, w) = stream.into_split();
            Ok((Box::new(r), Box::new(w)))
        }
    }
}

#[cfg(unix)]
impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
