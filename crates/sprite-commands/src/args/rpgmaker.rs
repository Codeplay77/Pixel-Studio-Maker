//! Arguments of the RPG Maker commands (`rpgmaker_template`, `export_rpgmaker_*`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprite_animation::rpgmaker::{RpgMakerEngine, RpgMakerKind};
use sprite_core::{FrameRef, ItemRef};

use super::SpriteTarget;

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RpgMakerTemplate {
    /// Arquivo do RPG Maker: tileset_a1 (água animada e cascatas), tileset_a2 (chão), tileset_a3
    /// (telhados e paredes de construções), tileset_a4 (paredes), tileset_a5 (tiles comuns),
    /// tileset_b/c/d/e (objetos), character (folha de personagens), face (faces), sv_battler (battler
    /// lateral, só MV/MZ) ou window (Window.png).
    pub kind: RpgMakerKind,
    /// Versão: "mz" (padrão) e "mv" usam tiles de 48 px; "vxace" usa 32 px.
    #[serde(default)]
    pub engine: RpgMakerEngine,
    /// Aplicar o modelo a este sprite existente (id ou nome), que precisa ter exatamente o tamanho do
    /// modelo. Omita para criar um sprite novo no projeto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<ItemRef>,
    /// Nome do sprite novo (padrão: o nome do modelo, ex.: "Tileset A2").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Só `character`: folha de um personagem só (3×4 células; o arquivo exportado leva o prefixo `$`).
    #[serde(default)]
    pub single: bool,
    /// Só `character` e `sv_battler`: tamanho da célula [largura, altura] em px. Padrão: o tile do
    /// motor (48 ou 32) para personagens e 64×64 para battlers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Só `window`: pinta as 32 amostras de cor do texto com uma paleta padrão legível (0 = texto
    /// normal branco, 16 = sistema, 17 = crise, 18 = nocaute, 19 = fundo dos medidores, 20–21 = HP,
    /// 22–23 = MP, 24 = melhora, 25 = piora, 28–29 = TP), na layer de cima, no primeiro frame.
    #[serde(default)]
    pub fill_text_colors: bool,
}

/// Um personagem da folha.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RpgMakerCharacterItem {
    /// Sprite do personagem (id ou nome). Padrão: o `sprite` do comando, ou o primeiro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<ItemRef>,
    /// Animação (id ou nome) com trilhas nas direções S (baixo), W (esquerda), E (direita) e N (cima).
    /// Uma trilha sem direção serve para as direções que faltarem.
    pub animation: ItemRef,
    /// 3 índices de frame dentro da trilha de cada direção, para as colunas 0, 1 e 2 (o RPG Maker
    /// toca 0, 1, 2, 1: a coluna do meio é a pose parada). Padrão: [0, 1, 2]; se a trilha tiver menos
    /// de 3 frames, o último se repete (com aviso).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportRpgMakerCharacter {
    /// Sprite padrão dos personagens que não informam `sprite`.
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png de saída (ex.: "img/characters/Heroi.png"). O nome recebe os prefixos do RPG
    /// Maker: `!` com `object`, `$` com `single` (ex.: "!$Heroi.png").
    pub path: String,
    /// De 1 a 8 personagens. Folha comum: grade de 12×8 células, personagem k no bloco 3×4 da coluna
    /// k % 4 e da linha k / 4.
    pub characters: Vec<RpgMakerCharacterItem>,
    /// Versão: "mz" (padrão), "mv" (tile 48) ou "vxace" (tile 32).
    #[serde(default)]
    pub engine: RpgMakerEngine,
    /// Tamanho da célula [largura, altura]. Padrão: o tile do motor (48×48 ou 32×32); com `single`,
    /// o tamanho do sprite se ele for maior que o tile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Folha de um personagem só (3×4 células, prefixo `$`): aceita células de qualquer tamanho.
    #[serde(default)]
    pub single: bool,
    /// Objeto (prefixo `!`): o RPG Maker não o desloca para cima (6 px no MV/MZ, 4 no VX Ace) nem o
    /// deixa semitransparente no mato. Use para portas, baús, tochas...
    #[serde(default)]
    pub object: bool,
    /// Quando faltar W ou E na animação, gera a direção espelhando a outra.
    #[serde(default)]
    pub mirror_missing: bool,
    /// Ampliação inteira dos frames (vizinho mais próximo) antes de montar a folha.
    #[serde(default = "one")]
    pub scale: u32,
}

/// Uma face da folha.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RpgMakerFaceItem {
    /// Sprite (id ou nome). Padrão: o `sprite` do comando, ou o primeiro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<ItemRef>,
    /// Frame da face: id, ou {"animation": "faces", "index": 0}.
    pub frame: FrameRef,
    /// Posição na grade (0–3 na primeira linha, 4–7 na segunda). Padrão: a ordem da lista.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportRpgMakerFaces {
    /// Sprite padrão das faces que não informam `sprite`.
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png de saída (ex.: "img/faces/Heroi.png").
    pub path: String,
    /// De 1 a 8 faces, centralizadas nas células.
    pub faces: Vec<RpgMakerFaceItem>,
    /// Versão: "mz" (padrão) e "mv" usam células de 144×144; "vxace" usa 96×96.
    #[serde(default)]
    pub engine: RpgMakerEngine,
    /// Ampliação inteira das faces (vizinho mais próximo). A face ampliada precisa caber na célula.
    #[serde(default = "one")]
    pub scale: u32,
}

/// De onde vêm os quadros de um movimento do battler.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RpgMakerMotionSource {
    /// Só a animação (id ou nome).
    Animation(ItemRef),
    /// Animação com direção e frames próprios.
    Detailed {
        /// Animação (id ou nome).
        animation: ItemRef,
        /// Trilha da animação (padrão: a `direction` do comando, ou a primeira trilha).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<String>,
        /// 3 índices de frame na trilha (padrão: os `frames` do comando, ou [0, 1, 2]).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        frames: Vec<usize>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportRpgMakerBattler {
    #[serde(flatten)]
    pub target: SpriteTarget,
    /// Arquivo .png de saída (ex.: "img/sv_actors/Heroi.png").
    pub path: String,
    /// Movimento → animação (nome/id, ou {"animation", "direction", "frames"}). Movimentos: walk,
    /// wait, chant, guard, damage, evade, thrust, swing, missile, skill, spell, item, escape, victory,
    /// dying, abnormal, sleep, dead (também valem os nomes em português das regiões do modelo:
    /// andar, esperar, entoar...). Movimentos sem animação ficam vazios e entram nos avisos.
    pub motions: BTreeMap<String, RpgMakerMotionSource>,
    /// Trilha usada em todos os movimentos (padrão: a primeira trilha de cada animação).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// 3 índices de frame em cada trilha (padrão: [0, 1, 2]; se faltar frame, o último se repete).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<usize>,
    /// Versão: "mz" (padrão) ou "mv". O VX Ace não tem battlers laterais.
    #[serde(default)]
    pub engine: RpgMakerEngine,
    /// Tamanho da célula [largura, altura] (padrão 64×64). A folha tem 9×6 células.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cell: Option<[u32; 2]>,
    /// Ampliação inteira dos frames (vizinho mais próximo) antes de montar a folha.
    #[serde(default = "one")]
    pub scale: u32,
}
