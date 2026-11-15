//! Dados de projeto das ferramentas de jogo (tarefas de exportação, variantes de paleta...).
//! Cada área cuida do seu campo, sempre com valor padrão.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectExtras {
    /// Tarefas de exportação (rodam ao salvar e em lote com `psm export`).
    #[serde(default, skip_serializing_if = "crate::godot::GodotProject::is_empty")]
    pub godot: crate::godot::GodotProject,
    /// Variantes de paleta (recolorações) do projeto.
    #[serde(default, skip_serializing_if = "crate::variants::PaletasExtras::is_empty")]
    pub paletas: crate::variants::PaletasExtras,
}

impl ProjectExtras {
    pub fn is_empty(&self) -> bool {
        *self == ProjectExtras::default()
    }
}
