//! The runtime wire protocol: one JSON object per line, both ways (see `docs/arquitetura.md`).
//!
//! Requests carry an `id`; the response echoes it. Events have no `id`.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sprite_commands::{Change, CommandError, Outcome};

pub const PROTOCOL_VERSION: u32 = 1;

/// Who is connecting.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ClientHello {
    /// `mcp`, `desktop` or `cli`.
    pub kind: String,
    /// Human-readable name ("Claude Code", "Pixel Studio Maker").
    pub name: String,
    /// Working folder: base for relative paths and an allowed root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// Extra folders this client may read/write.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_dirs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RequestBody {
    Hello {
        client: ClientHello,
        #[serde(default)]
        protocol: u32,
    },
    /// Runs a command (`{"cmd": ..., "args": ...}`).
    Call {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        command: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_revision: Option<u64>,
    },
    /// Makes a project this client's current one.
    Select {
        project: String,
    },
    /// Starts receiving events.
    Subscribe,
    /// Raw RGBA of a composited frame (fast path for the editor canvas).
    Render {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sprite: Option<u32>,
        frame: u32,
        /// Only this layer (otherwise the composite of visible layers).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layer: Option<u32>,
        /// Include hidden and reference layers in the composite.
        #[serde(default)]
        all_layers: bool,
    },
    /// Raw RGBA of any stored image (reference images for the editor).
    RenderImage {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
        image: u32,
    },
    /// Full project snapshot for the editor: model + revision + path.
    State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        project: Option<String>,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub body: RequestBody,
}

/// An image inside a response (base64 encoded).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WireImage {
    pub label: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub base64: String,
}

/// Result of a `call`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CallResult {
    /// Project the command ran on (or created/opened).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub data: serde_json::Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<WireImage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<Change>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction: Option<u64>,
}

impl CallResult {
    pub fn from_outcome(project: Option<String>, out: Outcome) -> Self {
        CallResult {
            project,
            data: out.data,
            images: out
                .images
                .into_iter()
                .map(|i| WireImage {
                    label: i.label,
                    mime: i.mime,
                    width: i.width,
                    height: i.height,
                    base64: base64::engine::general_purpose::STANDARD.encode(&i.data),
                })
                .collect(),
            changes: out.changes,
            warnings: out.warnings,
            revision: out.revision,
            transaction: out.transaction,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ok: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CommandError>,
}

/// Something happened in the runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A project changed (pixels or structure).
    Changed {
        project: String,
        revision: u64,
        source: String,
        command: String,
        changes: Vec<Change>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transaction: Option<u64>,
    },
    /// A command ran (for the editor's MCP activity panel).
    Operation(OpRecord),
    /// Projects were opened, closed, saved or renamed.
    Projects { projects: Vec<ProjectSummary> },
    /// Clients connected or disconnected.
    Clients { clients: Vec<ClientSummary> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpRecord {
    pub time: String,
    pub client: String,
    pub client_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub command: String,
    pub ok: bool,
    /// Short human summary (error message, changed pixels...).
    pub summary: String,
    pub millis: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub dirty: bool,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientSummary {
    pub id: u64,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Either a response or an event, as read from the wire.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Response(Response),
    Event(Event),
}
