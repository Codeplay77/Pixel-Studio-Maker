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

### Converter uma foto ou ilustração em pixel art

`image_pixelate` transforma um PNG, JPG ou GIF (de qualquer tamanho; acima de 4096 px a imagem é reduzida na
leitura) em pixel art estática. É processamento de imagem determinístico: nada é gerado.

- Comece com `preview: true` (devolve a imagem, não cria projeto) e ajuste: `max_side` (ou `width`/`height`),
  `colors`, `dither` (`none`, `ordered`, `floyd_steinberg`), `sampling` (`dominant` para cores limpas,
  `average` para detalhes finos, `nearest` para pixel art já pronta).
- Fundo: `background: "auto"` remove quando as bordas são de uma cor só; `"remove"` força (com
  `background_color` e `background_tolerance`); `"keep"` mantém. `crop` recorta ao conteúdo e `region`
  ({x, y, w, h}, em pixels da imagem original) escolhe só uma parte.
- Pixel art ampliada (cada "pixel" com N px): use `cell: N` para recuperar os pixels exatos.
- `outline: true` adiciona contorno de 1 px; `palette` (lista de cores) prende o resultado a uma paleta.
- Sem `preview`, cria um projeto novo (sprite, layer "pixel art", paleta ativa). Para pôr num projeto aberto,
  use `import_image` com `pixelate: { ... }` (em layer, o resultado cabe no sprite).
- Depois, trate como qualquer sprite: inspecione, corrija pixels, crie frames.

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

## 12. Recursos para jogos (RPG Maker, Godot e tilesets)

Os dados destes recursos ficam em `extras` de cada sprite (veja `project_info`) e no `extras` do projeto.

### RPG Maker (MZ, MV e VX Ace)

- `rpgmaker_template` cria um sprite no tamanho exato de um arquivo do RPG Maker: tilesets A1–A5 e B–E,
  folha de personagens (`character`; `single` para a folha `$` de um personagem), faces, battler lateral
  (`sv_battler`, só MV/MZ) ou `window` (Window.png; `fill_text_colors` pinta as 32 cores do texto). Cada bloco
  vira uma região nomeada ("A2 bloco 3,1", "personagem 0 baixo", "andar", "moldura") que serve de `clip` ao
  desenhar. `engine` escolhe tiles de 48 px (`mz`/`mv`) ou 32 px (`vxace`).
- `export_rpgmaker_character` monta a folha de personagem a partir de animações. As linhas são, nesta ordem, baixo (S),
  esquerda (W), direita (E) e cima (N). Cada direção tem 3 colunas, e a do meio é a pose parada. O pivô de cada
  frame vai no centro da base da célula. São até 8 personagens, ou só um com `single`, que gera `$Nome.png`;
  `object` gera `!Nome.png`, e `mirror_missing` espelha W/E quando uma das duas falta.
- `export_rpgmaker_faces` monta até 8 faces de 144×144 (96 no VX Ace). `export_rpgmaker_battler` monta o
  battler lateral 9×6 com os 18 movimentos (walk, wait, chant, guard, damage, evade, thrust, swing, missile,
  skill, spell, item, escape, victory, dying, abnormal, sleep, dead) a partir de animações; movimentos sem
  animação ficam vazios e entram nos avisos.

### Tiles e tilesets

- `tile_grid_set` transforma o sprite em tileset: tamanho do tile, margem e espaçamento. Os tiles são endereçados por
  `[coluna, linha]`. `tile_info` resume a grade, os tiles vazios, as colisões e os terrenos.
- `tile_collision_set` define colisões por tile. Elas podem ser polígonos em pixels do tile, com vértices nos
  cantos dos pixels e origem no canto superior esquerdo, ou atalhos: `full`, `top_half`, `bottom_half`,
  `left_half` e `right_half`.
- `tile_terrain_set` define terrenos no modelo do Godot 4. Cada conjunto tem um modo: `match_corners_and_sides`,
  `match_corners` ou `match_sides`. Em cada tile ficam o terreno e os bits de vizinhança.
- `autotile_generate` monta autotiles por quartos de tile, a partir de um bloco A2 do RPG Maker (2×3 tiles) ou
  de uma moldura 3×3 com cantos internos. Gera o bloco A2 e/ou os 47 tiles do blob, já com os terrenos para o
  Godot, num sprite novo que traz um mapa de prévia.
- `tile_preview_map_set` monta um mapa de teste e pode pintar por terreno com `terrain_rows`.
  `tile_preview_render` mostra como os tiles se encaixam, e `frame_preview` com `tiles: true` desenha a grade
  e as colisões.
- `export_godot_tileset` grava o PNG e um TileSet `.tres` do Godot 4 com os tiles não vazios, as colisões na
  camada de física 0 e os terrain sets.

### Godot 4: hitboxes, cena animada e exportação automática

- Cada sprite pode ter caixas de jogo, cada uma com um retângulo por frame. Um frame sem retângulo deixa a
  caixa desligada. Os tipos são:
  - `hitbox`: onde o golpe acerta;
  - `hurtbox`: onde recebe dano;
  - `collision`;
  - `custom`.
- Crie a caixa com `hitbox_create` e posicione com `hitbox_set`: num frame, numa lista, em toda uma trilha com
  `animation` + `direction`, ou com `all_frames`. Confira com `hitbox_list` e com `frame_preview` usando `hitboxes: true`. Duplicar
  frames ou criar animações com `copy_from` leva as caixas junto (espelhadas quando o frame é espelhado).
- `export_godot_scene` grava uma cena `.tscn` pronta para o Godot 4, com a sheet e o SpriteFrames ao lado. A cena tem:
  - um `Node2D` com um `AnimatedSprite2D`;
  - um `Area2D` com `CollisionShape2D` para cada caixa, no grupo do tipo dela;
  - um `Marker2D` para cada anchor;
  - um `AnimationPlayer` com uma animação por trilha, que troca os frames e move, redimensiona, liga e desliga as caixas e os pontos frame a frame. Tudo é relativo ao pivô.
  Gravando dentro de um projeto Godot (com `project.godot` acima), os caminhos `res://` saem certos sozinhos.
- Exportações podem ficar salvas no projeto como tarefas, com `export_job_set`, `export_job_list` e `export_job_delete`:
  - as marcadas com `on_save` rodam depois de cada `project_save`; o resultado volta em `data.exports`, e uma falha vira aviso sem desfazer o salvamento;
  - todas as tarefas rodam com `export_jobs_run` ou, sem runtime, com `psm export projeto.psmproj [--job NOME]...`, que sai com erro se alguma falhar.
  Caminhos relativos partem da pasta do `.psmproj`.

### Pincéis e seleção por máscara

- A op SDC `stroke` é o traço das ferramentas de pincel:
  - `symmetry` espelha o traço: `{"x": true}` usa o centro do sprite, e um eixo como `15.5` fica entre dois pixels; com os dois eixos saem 4 cópias;
  - `wrap: true` faz o traço dar a volta nas bordas, para texturas que se repetem;
  - `mode: "dither"` usa padrão Bayer ou xadrez ancorado no canvas, com `density` e `secondary`;
  - `mode: "shade"` anda `step` posições numa rampa de cores (por padrão, a paleta ativa) e muda cada pixel uma vez só.
- Seleções que não são retângulos vêm da varinha mágica (`select_color`) e do laço (`select_polygon`), no
  formato `bounds` + `rows`. Elas combinam com a seleção atual por `base` + `mode`, e a máscara pode ser
  passada a `pixel_clear`, `draw_transform` e `selection_read`.

### Texto, fontes bitmap e 9-slice

- `draw_text` (ou a op `text` do `draw_batch`) escreve com as fontes embutidas `pixel-5x7` (com os acentos do português
  e do espanhol) e `mini-3x5`, ou com uma fonte desenhada num sprite. Aceita alinhamento, quebra por palavra
  em `max_width`, escala inteira, contorno e sombra. `text_measure` dá a caixa exata antes de desenhar.
- Para criar uma fonte própria:
  1. `font_template` gera o sprite com a grade de células, uma região por caractere.
  2. Desenhe os glyphs.
  3. `font_define` ajusta as métricas.
  4. `font_info` mostra uma amostra.
  `export_bitmap_font` gera BMFont (`.fnt` + PNG), que o Godot 4 importa como FontFile.
- `nine_slice_set` guarda as margens de painéis e botões. `nine_slice_render` mostra a prévia esticada ou
  repetida, e `export_nine_slice` gera PNG + `.tscn` com um `NinePatchRect` pronto.

### Paletas, variantes e normal maps

- `palette_import`/`palette_export`: GIMP `.gpl`, Lospec `.hex`, JASC/RIFF `.pal`, Paint.NET `.txt`, `.png`/`.gif`.
- Variantes recolorem um personagem sem duplicar arte. A cor *i* da paleta base vira a cor *i* da variante.
  - Crie e ajuste com `palette_variant_set`, por índice (`set`) ou por cor (`replace`).
  - Confira com `palette_variant_preview` (`compare: true` mostra lado a lado).
  - Exporte com `export_palette_variants`: uma sheet, um PNG ou um GIF por variante. Os pixels do projeto não mudam.
- `import_image` lê arquivos do Aseprite (`.aseprite`/`.ase`): layers, frames com duração, cels vinculados,
  tags (viram animações), paleta e slices (viram regiões).
- `export_normal_map` gera o normal map de um frame ou de uma sheet inteira, no mesmo layout, para a
  iluminação 2D do Godot. A altura vem de `bevel`, `luminance` ou `mix`, na convenção OpenGL; use `flip_y` para DirectX.
