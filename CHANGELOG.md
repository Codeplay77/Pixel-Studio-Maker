# Mudanças

## Não publicado

- **RPG Maker MZ/MV/VX Ace**:
  - `rpgmaker_template` cria modelos com regiões nomeadas (tilesets A1–E, personagens, faces, battler lateral, Window.png com as cores do texto).
  - Novas exportações: `export_rpgmaker_character`, `export_rpgmaker_faces` e `export_rpgmaker_battler`.
  - No editor: modelo no "Novo projeto" e as opções no Exportar.
- **Tilesets**:
  - grade de tiles (`tile_grid_set`, `tile_info`);
  - colisões e terrenos por tile (`tile_collision_set`, `tile_terrain_set`);
  - gerador de autotile (`autotile_generate`: bloco A2 ou moldura 3×3 → A2 e/ou os 47 tiles do blob, com os terrenos do Godot);
  - prévia de mapa (`tile_preview_map_set`, `tile_preview_render`);
  - TileSet do Godot 4 (`export_godot_tileset`).
  - No editor: aba Tiles, modo tile e os diálogos Gerar autotile e Prévia de mapa.
- **Godot 4**:
  - hitboxes por frame (`hitbox_create`, `hitbox_update`, `hitbox_set`, `hitbox_list`, sobreposição no `frame_preview`);
  - cena `.tscn` com `AnimationPlayer`, `Area2D`/`CollisionShape2D` por caixa e `Marker2D` por anchor (`export_godot_scene`);
  - tarefas de exportação (`export_job_*`, `export_jobs_run`) que rodam ao salvar;
  - `psm export` para exportar em lote.
  - No editor: ferramenta Hitbox (H), aba Hitboxes e "Exportar sempre que salvar".
- **Pincéis e seleção**:
  - op SDC `stroke` com simetria, volta nas bordas (`wrap`), dithering e sombreado por rampa;
  - seleção por máscara com `select_color` (varinha) e `select_polygon` (laço), combinável;
  - máscara em `pixel_clear`, `draw_transform` e `selection_read`.
  - No editor: pincéis de dithering (D) e sombreado (U), simetria, varinha (W) e laço (Q).
  - Correção: recortar e colar não colava nada.
- **Texto**:
  - op SDC `text` e `draw_text`/`text_measure` com as fontes originais `pixel-5x7` e `mini-3x5`;
  - fontes desenhadas num sprite (`font_template`, `font_define`, `font_info`);
  - BMFont (`export_bitmap_font`);
  - 9-slice (`nine_slice_set`, `nine_slice_render`, `export_nine_slice` com `NinePatchRect`).
  - No editor: ferramenta Texto (T) e aba Texto.
- **Paletas**:
  - importar e exportar `.gpl`, `.hex`, `.pal` (JASC e RIFF), `.txt` (Paint.NET), `.png` e `.gif`;
  - variantes de paleta (`palette_variant_*`, `export_palette_variants`);
  - importação de `.aseprite` com layers, cels vinculados, tags, paleta e slices;
  - normal map para o Godot (`export_normal_map`).
- O modelo ganhou `extras` por sprite e por projeto para os dados acima; projetos antigos abrem normalmente.
- `shade` (e `draw_shade`) ganhou `wrap`: em imagens que se repetem, o vizinho além da borda é o pixel do lado
  oposto, e a borda deixa de virar uma linha de luz em cada emenda.
- Abas de projeto com botão de fechar, clique do meio e Ctrl+W, com confirmação quando há alterações não salvas.
- Exemplo `demo-h-parallax`: cenário de entardecer em 7 camadas que se emendam, com projeto Godot 4 pronto
  (`Parallax2D`) e gravação do Godot rodando a cena.
- Converter imagem em pixel art (`image_pixelate` no MCP, e no editor pelo botão de grade na barra de cima ou
  na tela inicial): PNG, JPG ou GIF de qualquer tamanho (acima de 4096 px a imagem é reduzida na leitura,
  linha a linha), tamanho de saída por lado maior, largura × altura ou tamanho do pixel (pixel art ampliada),
  remoção de fundo pelas bordas com tolerância, recorte automático ao conteúdo e recorte manual (`region`,
  arrastando sobre a original), paleta calculada (k-means em OKLab) ou fixa, amostragem por cor dominante, média
  ou pixel central, dithering ordenado ou Floyd-Steinberg, saturação, contraste e brilho, limpeza de pixels
  soltos e contorno. Cria um projeto novo, ou entra como layer/sprite pelo `import_image` com `pixelate`.

## 0.1.0 — 2026-11-14

Primeira versão.

- Motor de pixels em Rust: RGBA e paletas, máscaras, composição de layers com 8 modos de mistura,
  diferenças entre imagens, matrizes de texto e PNG.
- Desenho pixel-perfect: linhas, polilinhas, retângulos, elipses, círculos, polígonos, curvas,
  preenchimento, contornos, dithering, degradês discretos, sombreamento pela direção da luz, sombra
  projetada, padrões, limpeza, quantização, ajuste HSV e transformações por vizinho mais próximo.
- SDC: linguagem de desenho em lote, atômica, com matrizes textuais, RLE e patches.
- Animação: frames, cels vinculadas, animações com trilhas por direção (8 padrão + personalizadas),
  anchors, regiões semânticas, poses, ciclos, onion skin e validação automática.
- Projeto `.psmproj` versionado, gravação atômica, histórico com undo/redo e transações, snapshots,
  recuperação automática.
- Importação de PNG, GIF e sprite sheets. Exportação de PNG, sequência, GIF, sprite sheet (5 layouts) com
  JSON e `SpriteFrames` do Godot 4.
- Runtime compartilhado por Named Pipe, com revisões, transações por cliente e eventos em tempo real.
- Servidor MCP nativo (`rmcp`, stdio) com 84 ferramentas, recursos e prompts.
- Editor Tauri 2 + React com ferramentas manuais, layers, paletas, linha do tempo, reprodução, regiões,
  anchors, inspeção e painel de atividade do agente.
- Demonstrações A–E feitas pelo MCP e teste de regressão por imagem.
