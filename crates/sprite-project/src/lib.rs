//! Projects: the [`Document`] (model + pixels + transactional history + snapshots), the native
//! `.psmproj` file format, import of PNG/GIF/sheets and every export (PNG, frame sequence, GIF,
//! sprite sheet, atlas, JSON metadata, Godot SpriteFrames).

pub mod document;
pub mod export;
pub mod format;
pub mod import;
pub mod time;

pub use document::{CommitInfo, Document, ProjectModel, Reference, Snapshot};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("arquivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("formato inválido: {0}")]
    Format(String),
    #[error("este arquivo é da versão {found} do formato; esta versão do programa lê até a {supported}")]
    NewerVersion { found: u32, supported: u32 },
    #[error("projeto corrompido: {0}")]
    Integrity(String),
    #[error("{0}")]
    Transaction(String),
    #[error("imagem: {0}")]
    Image(String),
}

impl From<sprite_core::codec::CodecError> for ProjectError {
    fn from(e: sprite_core::codec::CodecError) -> Self {
        ProjectError::Image(e.to_string())
    }
}
