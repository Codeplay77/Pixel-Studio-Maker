//! The runtime: one local process holding the authoritative state of every open project.
//!
//! The MCP server and the desktop editor are both clients. They talk to the runtime over local
//! IPC (Windows named pipes; Unix domain sockets on Linux/macOS) with newline-delimited JSON
//! ([`protocol`]). The runtime keeps working with the editor closed, so agents can create and
//! edit projects headless; when the editor is open it receives change events and updates live.

pub mod client;
pub mod protocol;
pub mod server;
pub mod transport;

pub use client::{RuntimeClient, disinherit_std_handles, spawn_runtime};
pub use protocol::{CallResult, ClientHello, Event, OpRecord};
pub use server::{Runtime, RuntimeConfig, run};
pub use transport::default_endpoint;

#[cfg(test)]
mod tests;
