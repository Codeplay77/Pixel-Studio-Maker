# Guia do agente: pixel art e animação pelo MCP do Pixel Studio Maker

Este guia é para agentes de IA (como o Claude Code) que desenham pelo servidor MCP `pixel-studio-maker`.
Você toma as decisões artísticas; o motor em Rust executa cada pixel exatamente como pedido.
Nada aqui gera imagens com IA: tudo é desenho determinístico.

## 1. Fluxo básico

1. `project_create` com nome e tamanho (16x16 a 128x128 é o comum; 64x64 para personagens detalhados).
2. `palette_create` com uma paleta limitada (8 a 24 cores). A partir daí, cores podem ser índices (`0`, `1`, ...).
3. `layer_create` para separar partes (contorno, corpo, roupa, arma, efeitos).
4. Desenhe com `draw_batch` (várias operações numa chamada, um passo no histórico).
5. Inspecione com `frame_preview` (imagem ampliada) e `frame_inspect` (números).
6. Corrija com `pixel_apply_patch`, `pixel_write_region` ou outro `draw_batch`.
7. `project_save` com um caminho `.psmproj`.
8. Exporte: `export_png`, `export_spritesheet`, `export_gif`, `export_godot_metadata`.

Coordenadas: (0,0) é o pixel do canto superior esquerdo; x cresce para a direita, y para baixo.

## 2. Economize chamadas e tokens

- Prefira **um `draw_batch` por etapa** (silhueta, cores base, sombras, detalhes) a dezenas de chamadas.
- Use **matrizes textuais** (`op: "matrix"`) para formas orgânicas: cada caractere é um pixel, a `legend` diz a cor,
  `.` mantém o pixel e `"clear"` apaga. Exemplo de um olho 4x3:

  ```json
  { "op": "matrix", "at": [20, 14], "legend": { "k": 0, "w": "#ffffff" }, "rows": [".kk.", "kwwk", ".kk."] }
  ```

- Linhas longas repetitivas: `"rle": true` e escreva `"6k3.2w"` em vez de `"kkkkkk...ww"`.
- Use **polígonos** para silhuetas grandes e **elipses/círculos** para cabeças, olhos, escudos.
- Para mudar poucos pixels de um frame use **patches** (`pixel_apply_patch`): só os pixels que mudam.
- `pixel_read_region` devolve a região no mesmo formato de matriz que você escreve: leia, ajuste o texto, escreva de volta.
- Inspecione **regiões pequenas** (`frame_preview` com `rect`) em vez do sprite inteiro quando for detalhe.

## 3. Planejar antes de desenhar

Defina por escrito, antes da primeira chamada:

- **Tamanho e enquadramento**: o personagem ocupa ~70–85% da altura do canvas, pés numa linha de chão fixa
  (ex.: y = altura − 2). Deixe 1–2 px de margem para o contorno.
- **Proporções**: em 64x64, um personagem estilizado costuma ter cabeça de 14–18 px e corpo de 3 a 3,5 cabeças.
- **Direção da luz**: escolha uma (ex.: `top_left`) e mantenha em todos os frames e direções.
- **Paleta**: rampas de 3–4 tons por material (sombra, base, luz, brilho). Sombras puxam para azul/roxo, luzes para
  amarelo. Um tom escuro quase preto para o contorno (evite preto puro em sprites coloridos).
- **Silhueta**: a forma tem de ser legível em preto chapado. Pose com braços e pernas separados do tronco.

## 4. Construção em etapas (personagem)

1. **Silhueta**: numa layer `base`, `polygon`/`ellipse` cheios com uma cor neutra. Confira com `frame_preview`.
2. **Regiões semânticas**: `region_create` para cabeça, tronco, braços, pernas, arma (por retângulo, máscara ou
   pelos pixels de uma layer). Elas permitem recortar operações (`clip`) e mover partes entre frames.
3. **Anchors**: `anchor_create` para pescoço, ombros, mãos, quadril, joelhos, pés, chão e pivô (`kind: "pivot"`).
4. **Cores base**: `fill` ou `replace_color` por região; ou redesenhe com matrizes.
5. **Volume e luz**: `shade` com a direção da luz (bordas iluminadas e sombreadas), `dither` com moderação.
6. **Contorno**: `outline` (`mode: "outside"`, conectividade 4 dá cantos arredondados; 8 fecha as diagonais).
   Depois retoque o contorno interno escurecendo onde as partes se sobrepõem.
7. **Detalhes**: olhos, fivelas, brilho de metal (1–2 px de cor clara), costuras.
8. **Limpeza**: `cleanup` com `orphans`; confira cores com `frame_inspect` (cores fora da paleta) e corrija.
9. **Inspeção final**: `frame_preview` com escala alta e `grid: true` nos detalhes; `project_validate`.

## 5. Layers

- Uma layer por parte que se move de forma independente facilita animar (braço da frente, braço de trás, arma).
- Layers de referência (`reference: true`) não saem na exportação: use para esboços e guias.
- `layer_merge` junta partes quando a animação estiver pronta.

## 6. Animação frame a frame

Animar **não** é mover ou girar o sprite inteiro. Cada frame precisa de pose diferente.

1. `animation_create` com `copy_from` apontando para o frame-base: os frames nascem como cópia para você redesenhar.
2. Registre o ciclo: `animation_update` com `cycle` (quadros-chave, contatos com o chão, `ground_y`).
3. Desenhe primeiro as **poses-chave** (contato e passagem numa caminhada), depois os intermediários.
4. Para cada frame: mova partes com `region_transform` (move/rotate da região do braço, da perna) e depois
   **redesenhe** as junções com `draw_batch`/patches — a transformação é só o ponto de partida.
5. Use `frame_preview` com `onion: { "before": 1 }` para ver o frame anterior como fantasma.
6. `frame_compare` entre frames vizinhos: confira se a mudança é a esperada (pernas mudando, cabeça estável).
7. `animation_preview` (`strip`) para ver todos os frames lado a lado; `gif` para o movimento.
8. `project_validate`: aponta frames duplicados, saltos bruscos, chão desalinhado.

### Caminhada de 8 frames (lado)

| Frame | Pose |
|---|---|
| 0 | Contato: perna da frente esticada à frente tocando o chão, perna de trás atrás, braços opostos às pernas |
| 1 | Descida: peso na perna da frente, corpo 1 px mais baixo |
| 2 | Passagem: pernas cruzando, perna de trás dobrada subindo, corpo na altura normal |
| 3 | Subida: corpo 1 px mais alto, perna de trás indo para a frente |
| 4 | Contato oposto (espelho da pose 0 trocando as pernas) |
| 5–7 | Repetem 1–3 com as pernas trocadas |

Mantenha os pés no chão (`ground_y`), a cabeça subindo/descendo no máximo 1–2 px e os braços balançando
em oposição às pernas.

## 7. Oito direções

- Desenhe primeiro **S** (frente), **E** (lado) e **N** (costas): elas definem a identidade.
- **SE/NE** são vistas 3/4 entre essas; **W/SW/NW** podem começar como espelho de **E/SE/NE**
  (`animation_set_direction` com `copy_from: { direction, mirror: true }`), mas **não** suponha simetria:
  arma numa mão, escudo na outra, iluminação e detalhes assimétricos precisam ser redesenhados.
- Use `animation_set_direction` com `reference` para registrar qual direção serve de base para qual.
- Mesma altura, mesma paleta, mesmo chão em todas as direções; compare com `frame_compare`.

## 8. Sprites existentes (importação)

- `import_image` com `mode: "new_sprite"` preserva os pixels exatamente. Para sheets, use `slice`.
- Leia a paleta com `frame_inspect` e crie uma paleta com essas cores antes de desenhar frames novos.
- Para uma animação idle de um sprite importado: `animation_create` com `copy_from` no frame importado e redesenhe
  pequenas mudanças (respiração: tronco e cabeça descendo 1 px, piscar, cabelo/capa balançando).

## 9. Corrigir problemas comuns

| Problema | Correção |
|---|---|
| Linhas serrilhadas com "degraus" duplos | redesenhe com `polyline` + `pixel_perfect: true` ou ajuste com patch |
| Pixels soltos | `cleanup` modo `orphans` |
| Cores demais / fora da paleta | `palette_quantize` com a paleta, ou `frame_inspect` e `replace_color` |
| Personagem "flutuando" entre frames | confira `ground_y` em `animation_info` com `metrics: true` e mova com `draw_transform` |
| Animação "deslizando" | redesenhe poses; não use só `move` no sprite inteiro |

## 10. Transações, revisões e o editor aberto

- Cada comando de escrita é um passo do histórico. Para agrupar muitos comandos num passo só:
  `transaction_begin` → comandos → `transaction_commit` (ou `transaction_rollback`).
- Se o usuário estiver editando no editor ao mesmo tempo, envie `expected_revision` (a `revision` da sua última
  resposta). Se der `REVISION_CONFLICT`, leia o estado de novo antes de continuar.
- Tudo o que você faz aparece no editor em tempo real, e o que o usuário pinta lá você lê pelo MCP.

## 11. Verificação não é estética

`project_validate` acha problemas técnicos. Ele não diz se ficou bonito. Sempre olhe as imagens
(`frame_preview`, `animation_preview`) e critique: silhueta legível, proporções, contraste, consistência.
