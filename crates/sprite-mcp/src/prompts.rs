//! MCP prompts: complete creation workflows the agent can follow.

use rmcp::ErrorData as McpError;
use rmcp::model::*;
use serde_json::{Map, Value};

struct Workflow {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    args: &'static [(&'static str, &'static str, bool)],
    template: &'static str,
}

const WORKFLOWS: &[Workflow] = &[
    Workflow {
        name: "criar_personagem",
        title: "Criar um personagem do zero",
        description: "Planeja e desenha um personagem em pixel art usando só as ferramentas do editor.",
        args: &[
            ("descricao", "O personagem (ex.: guerreiro medieval com armadura azul, espada e escudo)", true),
            ("tamanho", "Lado do canvas em px (padrão 64)", false),
            ("vista", "Direção da vista (padrão: lado, olhando para a direita)", false),
        ],
        template: "Crie em pixel art: {descricao}. Canvas {tamanho}x{tamanho}, vista: {vista}.\n\
1. Leia psm://guide. Escreva o plano: proporções, linha do chão, direção da luz, paleta (8–20 cores em rampas), layers.\n\
2. project_create e palette_create (cores por índice daqui em diante).\n\
3. layer_create para: contorno/corpo, roupa/armadura, arma, escudo (o que fizer sentido).\n\
4. draw_batch da silhueta (polygon/ellipse/matrix). frame_preview para conferir a legibilidade.\n\
5. region_create para cabeça, tronco, braços, pernas, arma; anchor_create para pescoço, ombros, mãos, quadril, pés, chão e pivô.\n\
6. draw_batch com cores base, shade (luz {luz}), detalhes com matrix; outline no fim.\n\
7. frame_preview com escala alta e grid; frame_inspect (cores fora da paleta); corrija com pixel_apply_patch.\n\
8. project_validate, project_save e export_png. Mostre a imagem final.",
    },
    Workflow {
        name: "animar_caminhada",
        title: "Animação de caminhada",
        description: "Cria uma caminhada frame a frame com poses reais a partir de um personagem existente.",
        args: &[
            ("frames", "Quantidade de frames (padrão 8)", false),
            ("direcao", "Direção (padrão E)", false),
            ("frame_base", "Frame com o personagem parado (padrão: o primeiro)", false),
        ],
        template: "Anime uma caminhada de {frames} frames na direção {direcao}, a partir do frame {frame_base}.\n\
1. animation_info e region_list/anchor_list para conhecer partes e anchors.\n\
2. animation_create {\"name\": \"walk\", \"directions\": [\"{direcao}\"], \"frames\": {frames}, \"copy_from\": {frame_base}}.\n\
3. animation_update com cycle: key_frames = contatos e passagens, ground_y = linha do chão.\n\
4. Redesenhe as poses-chave primeiro (contato, passagem), depois os intermediários. Para cada frame: region_transform nas pernas/braços e redesenho das junções com draw_batch; cabeça e tronco sobem/descem no máximo 1–2 px.\n\
5. frame_preview com onion {\"before\": 1} enquanto desenha; frame_compare entre vizinhos.\n\
6. animation_preview (strip e gif), project_validate (sem duplicados, sem saltos, chão estável).\n\
7. project_save e export_spritesheet.",
    },
    Workflow {
        name: "oito_direcoes",
        title: "Oito direções",
        description: "Constrói as 8 direções de uma animação mantendo a identidade do personagem.",
        args: &[("animacao", "Animação existente (padrão walk)", false)],
        template: "Crie as 8 direções da animação {animacao}.\n\
1. Garanta S (frente), E (lado) e N (costas) desenhadas por inteiro: elas definem a identidade.\n\
2. SE e NE: vistas 3/4 entre S/E e N/E. Use animation_set_direction com create e redesenhe usando E e S como referência (reference).\n\
3. W, SW, NW: animation_set_direction com copy_from {direction, mirror: true} como ponto de partida e corrija tudo que é assimétrico (arma, escudo, luz, detalhes).\n\
4. Compare altura, chão e paleta entre direções com frame_compare e animation_info (metrics).\n\
5. animation_preview de cada direção; project_validate; export_godot_metadata com layout rows.",
    },
    Workflow {
        name: "importar_e_animar",
        title: "Importar e animar",
        description: "Importa um sprite PNG preservando a aparência e cria uma animação nova.",
        args: &[
            ("caminho", "Arquivo PNG/GIF", true),
            ("animacao", "Nome da animação (padrão idle)", false),
            ("frames", "Frames (padrão 6)", false),
        ],
        template: "Importe {caminho} e crie a animação {animacao} com {frames} frames.\n\
1. import_image com mode new_sprite (os pixels são preservados exatamente).\n\
2. frame_inspect para ler as cores; palette_create com elas.\n\
3. Defina regiões (cabeça, tronco, braços) e anchors sobre o sprite importado.\n\
4. animation_create com copy_from no frame importado e {frames} frames.\n\
5. Redesenhe mudanças sutis por frame (respiração: tronco e cabeça 1 px; piscar; cabelo/capa), com pixel_apply_patch para economizar.\n\
6. animation_preview, project_validate, project_save, export_spritesheet e export_gif.",
    },
];

pub fn list() -> Vec<Prompt> {
    WORKFLOWS
        .iter()
        .map(|w| {
            let args = w.args.iter().map(|(n, d, req)| PromptArgument::new(*n).with_description(*d).with_required(*req)).collect();
            let mut p = Prompt::new(w.name, Some(w.description), Some(args));
            p.title = Some(w.title.into());
            p
        })
        .collect()
}

fn default_for(name: &str) -> &'static str {
    match name {
        "tamanho" => "64",
        "vista" => "lado, olhando para a direita",
        "frames" => "8",
        "direcao" => "E",
        "frame_base" => "o primeiro frame",
        "animacao" => "walk",
        "luz" => "top_left",
        _ => "",
    }
}

pub fn get(name: &str, args: Map<String, Value>) -> Result<GetPromptResult, McpError> {
    let w =
        WORKFLOWS.iter().find(|w| w.name == name).ok_or_else(|| McpError::invalid_params(format!("prompt desconhecido: {name}"), None))?;
    let mut text = w.template.to_string();
    for (arg, _, required) in w.args.iter().chain([("luz", "", false)].iter()) {
        let value = args.get(*arg).and_then(|v| v.as_str().map(String::from).or_else(|| Some(v.to_string()))).filter(|v| !v.is_empty());
        if *required && value.is_none() {
            return Err(McpError::invalid_params(format!("o argumento \"{arg}\" é obrigatório"), None));
        }
        let mut value = value.unwrap_or_else(|| default_for(arg).to_string());
        if name == "animar_caminhada" && *arg == "frames" && value.is_empty() {
            value = "8".into();
        }
        text = text.replace(&format!("{{{arg}}}"), &value);
    }
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]).with_description(w.description))
}
