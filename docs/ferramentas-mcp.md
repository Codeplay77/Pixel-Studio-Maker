# Ferramentas MCP

Gerado por `psm tools` a partir da tabela de comandos (`crates/sprite-commands/src/lib.rs`).
São 126 ferramentas. Todas as que agem num projeto aceitam também `project` (id) e, se escrevem, `expected_revision`.

Tipos: **leitura** não muda nada; **escrita** vira um passo do histórico (pode ser desfeita); **runtime** cuida de projetos, transações e histórico.

| Ferramenta | Tipo | O que faz |
|---|---|---|
| `project_create` | runtime | Cria um projeto novo com um sprite (tamanho em pixels) e o torna o projeto atual. Retorna o id do projeto. |
| `project_open` | runtime | Abre um arquivo .psmproj e o torna o projeto atual (se já estiver aberto, reaproveita a mesma sessão). |
| `project_save` | runtime | Salva o projeto atual (salvamento atômico). Com `path`, salva como um arquivo novo. |
| `project_close` | runtime | Fecha o projeto atual. Recusa se houver alterações não salvas, a menos que `save` ou `discard`. |
| `project_list` | runtime | Lista os projetos abertos no runtime e as cópias de recuperação disponíveis. |
| `project_info` | leitura | Resumo do projeto: sprites, layers, frames, animações e direções, paletas, revisão, histórico. |
| `project_validate` | leitura | Verificações automáticas: frames vazios ou duplicados, saltos bruscos, mudanças anormais, cores fora da paleta, buracos, pivôs e chão desalinhados. Gera alertas, não bloqueia nada. |
| `sprite_create` | escrita | Cria outro sprite no projeto (personagem, item, cenário...). |
| `sprite_update` | escrita | Renomeia, define o pivô/origem, a paleta ou o tamanho de um sprite (canvas ou escala por vizinho mais próximo). |
| `reference_add` | escrita | Adiciona uma imagem de referência ao sprite (aparece no editor, nunca é exportada). |
| `reference_update` | escrita | Move, escala, mostra/esconde ou remove uma referência. |
| `snapshot_create` | escrita | Guarda uma cópia nomeada do projeto inteiro para restaurar depois. |
| `snapshot_restore` | escrita | Volta o projeto ao estado de um snapshot (pode ser desfeito). |
| `snapshot_list` | leitura | Lista os snapshots do projeto. |
| `pixel_get` | leitura | Lê a cor de um pixel (de uma layer ou da imagem composta). |
| `pixel_set` | escrita | Pinta um pixel. |
| `pixel_set_batch` | escrita | Pinta muitos pixels em uma chamada: [[x,y], ...] com `color`, ou [[x,y,cor], ...]. |
| `pixel_clear` | escrita | Apaga (deixa transparente) um pixel, um retângulo ou a cel inteira. |
| `pixel_read_region` | leitura | Lê uma região como matriz textual compacta: `legend` (char → cor) + `rows`. É o mesmo formato aceito por pixel_write_region. |
| `pixel_write_region` | escrita | Escreve uma matriz de pixels a partir de `at`: `rows` + `legend` (char → cor, `.` mantém, "clear" apaga), opcionalmente RLE, ou `pixels` numéricos com `palette`. |
| `pixel_apply_patch` | escrita | Aplica um patch de diferenças (matriz com `.` = manter e/ou lista [[x,y,cor]]) sem reenviar o resto. |
| `draw_batch` | escrita | Executa várias operações SDC em ordem, de forma atômica (um passo no histórico). É a forma recomendada de desenhar: silhueta, preenchimento, contorno, sombra e detalhes numa só chamada. Retorna pixels alterados, região alterada, revisão e transação. |
| `draw_line` | escrita | Linha reta pixel-perfect (Bresenham), com espessura. |
| `draw_shape` | escrita | Retângulo, elipse ou círculo (contorno ou cheio), sem antialiasing. |
| `draw_polygon` | escrita | Polígono pelos vértices (cheio por padrão) — bom para silhuetas. |
| `draw_fill` | escrita | Balde de tinta a partir de um pixel (contíguo ou global, conectividade 4/8, tolerância). |
| `draw_outline` | escrita | Contorno em volta (outside) ou na borda (inside) das formas visíveis da layer. |
| `draw_dither` | escrita | Dithering ordenado (Bayer 2/4/8, xadrez, linhas) com densidade 0..1. |
| `draw_gradient` | escrita | Degradê discreto (faixas) com as cores dadas, linear ou radial, com dithering opcional. |
| `draw_shade` | escrita | Sombras e luzes pela direção da luz: bordas viradas para a luz clareiam, as opostas escurecem (cores fixas ou automáticas com desvio de matiz). |
| `draw_transform` | escrita | Espelha, gira (90° exatos ou graus), move ou escala (vizinho mais próximo) os pixels de uma área ou região. |
| `draw_replace_color` | escrita | Troca uma cor por outra na cel, na layer, no frame ou no sprite inteiro. |
| `region_create` | escrita | Cria uma região semântica (cabeça, tronco, braço, arma...) por retângulo, máscara, polígono ou pelos pixels visíveis de uma layer. |
| `region_update` | escrita | Altera uma região: nome, parte, layer, forma padrão, formas por frame, ou apaga. |
| `region_list` | leitura | Lista as regiões semânticas (com a caixa de cada uma no frame pedido). |
| `region_copy` | escrita | Copia os pixels de uma região de um frame para outros frames (com deslocamento e espelho opcionais). |
| `region_transform` | escrita | Transforma (move, gira, espelha, escala) os pixels de uma região em um ou mais frames. |
| `anchor_create` | escrita | Cria um anchor (pivô, articulação, mão, pé, chão...) com posição padrão e/ou por frame. |
| `anchor_update` | escrita | Move, renomeia ou apaga um anchor; define posições por frame. |
| `anchor_list` | leitura | Lista os anchors e suas posições (no frame pedido, ou todas). |
| `pose_create` | escrita | Registra uma pose (inicial, final, chave, contato, passagem...) com posições de anchors, chão e notas. |
| `pose_update` | escrita | Altera ou apaga uma pose registrada. |
| `pose_list` | leitura | Lista as poses registradas. |
| `layer_create` | escrita | Cria uma layer (no topo ou acima de outra). |
| `layer_delete` | escrita | Apaga uma layer e suas cels. |
| `layer_duplicate` | escrita | Duplica uma layer com todas as cels. |
| `layer_reorder` | escrita | Muda a ordem de uma layer (índice, acima de ou abaixo de outra). |
| `layer_merge` | escrita | Funde uma layer na de baixo (ou em `into`) em todos os frames. |
| `layer_properties` | escrita | Lê e altera nome, visibilidade, bloqueio, opacidade, modo de mistura ou se é referência. |
| `animation_create` | escrita | Cria uma animação (idle, walk, attack...) com N frames por direção, opcionalmente copiando um frame base para redesenhar. |
| `animation_delete` | escrita | Apaga uma animação (e os frames que só ela usava). |
| `animation_info` | leitura | Lista animações ou detalha uma: trilhas por direção, frames, durações, modo de repetição, ciclo e métricas por frame. |
| `animation_update` | escrita | Renomeia, define tags ou o ciclo de movimento (quadros-chave, contatos, linha do chão). |
| `animation_set_direction` | escrita | Cria ou altera a trilha de uma direção: frames existentes, frames novos, cópia (opcionalmente espelhada) de outra direção, referência, ou remove. |
| `animation_set_timing` | escrita | Define durações (iguais, por frame ou por FPS) e o modo de repetição (loop, once, ping_pong, reverse). |
| `animation_preview` | leitura | Imagem da animação para inspeção: frames lado a lado (strip) ou GIF. |
| `direction_create` | escrita | Cria uma direção ou vista personalizada além das 8 padrão. |
| `direction_update` | escrita | Altera rótulo, ângulo, espelho sugerido ou referência de uma direção; apaga direções personalizadas. |
| `frame_create` | escrita | Cria frames vazios (ou cópias de um frame), opcionalmente dentro de uma animação/direção. |
| `frame_duplicate` | escrita | Duplica um frame logo depois dele (cópia, cópia espelhada ou cels vinculadas). |
| `frame_delete` | escrita | Apaga frames (de todas as animações). |
| `frame_move` | escrita | Muda a posição de um frame na trilha de uma animação ou na linha do tempo. |
| `frame_edit` | escrita | Altera duração/tags de um frame, desvincula cels e/ou desenha nele com operações SDC. |
| `frame_compare` | leitura | Compara dois frames: pixels alterados, caixa das diferenças, patch textual de A para B e imagem lado a lado com as diferenças. |
| `frame_preview` | leitura | Renderiza um frame (ou região) como imagem ampliada para inspeção, com grade, anchors, regiões e onion skin opcionais. |
| `frame_apply_patch` | escrita | Aplica um patch de diferenças num frame (só os pixels que mudam). |
| `frame_inspect` | leitura | Dados de um frame: caixa dos pixels visíveis, contagem, cores usadas (e se estão na paleta), semitransparência, buracos, anchors e regiões. |
| `palette_create` | escrita | Cria uma paleta (e a ativa). Índices da paleta ativa podem ser usados como cores em qualquer comando. |
| `palette_update` | escrita | Altera uma paleta: renomeia, troca/adiciona/remove cores, ativa ou apaga (não mexe nos pixels). |
| `palette_replace` | escrita | Troca uma cor por outra em todos os pixels do sprite (todas as layers e frames) e na paleta. |
| `palette_quantize` | escrita | Reduz as cores do sprite (median cut) ou as leva para uma paleta. |
| `import_image` | escrita | Importa PNG/GIF/Aseprite como sprite novo, layer, frames ou referência (.aseprite como sprite novo mantém layers, tags e paleta); corta sprite sheets e cria animação com os frames. Com `pixelate`, converte a imagem em pixel art antes (como image_pixelate). |
| `image_pixelate` | runtime | Converte uma imagem PNG, JPG ou GIF de qualquer tamanho (foto, ilustração, pixel art ampliada) em pixel art estática, no tamanho pedido: remove o fundo, recorta (ao conteúdo ou numa região), reduz por blocos, calcula a paleta (ou usa uma fixa), com dithering, limpeza e contorno opcionais. Cria um projeto novo com o resultado; com `preview`, só devolve a imagem. Processamento determinístico, nada é gerado por IA. |
| `export_png` | leitura | Exporta um frame como PNG (com escala). |
| `export_spritesheet` | leitura | Exporta uma sprite sheet PNG (horizontal, vertical, grade, uma linha por trilha ou atlas compactado), com JSON de metadados. |
| `export_frames` | leitura | Exporta cada frame como um PNG numerado. |
| `export_gif` | leitura | Exporta uma animação (uma direção) como GIF animado. |
| `export_godot_metadata` | leitura | Exporta sheet PNG + SpriteFrames .tres do Godot 4 (uma animação por trilha, durações e loop). |
| `history_undo` | runtime | Desfaz passos do histórico. |
| `history_redo` | runtime | Refaz passos desfeitos. |
| `transaction_begin` | runtime | Abre uma transação: os comandos seguintes viram um único passo no histórico e outros clientes esperam. Feche com commit ou rollback. |
| `transaction_commit` | runtime | Confirma a transação aberta. |
| `transaction_rollback` | runtime | Desfaz tudo o que foi feito desde transaction_begin. |
| `editor_status` | runtime | Estado do runtime: projetos abertos, projeto atual, clientes conectados (editor, MCP), transação aberta. |
| `editor_get_capabilities` | runtime | Limites, operações SDC disponíveis, formatos e versão do schema. |
| `rpgmaker_template` | escrita | Cria um sprite no tamanho exato de um arquivo do RPG Maker MZ/MV (tile 48) ou VX Ace (tile 32): tilesets A1–A5 e B–E, folha de personagens, faces, battler lateral ou Window.png, com uma região semântica nomeada para cada bloco (ex.: "A2 bloco 3,1", "personagem 0 baixo", "andar", "moldura"). Com `sprite`, aplica o modelo a um sprite existente do mesmo tamanho. Retorna o tamanho, a grade e as regiões. |
| `export_rpgmaker_character` | leitura | Grava a folha de personagem do RPG Maker (img/characters) a partir de animações: linhas baixo (S), esquerda (W), direita (E) e cima (N), 3 colunas por direção (o RPG Maker toca 0, 1, 2, 1; a do meio é a pose parada), cada frame com o pivô no centro da base da célula. Até 8 personagens (grade 12×8) ou um só com `single` (prefixo $); `object` põe o prefixo !. Devolve a imagem. |
| `export_rpgmaker_faces` | leitura | Grava uma imagem de faces do RPG Maker (img/faces): até 8 faces (frames) numa grade 4×2 de células 144×144 (96×96 no VX Ace), centralizadas, com escala inteira opcional. Devolve a imagem. |
| `export_rpgmaker_battler` | leitura | Grava um battler de visão lateral do RPG Maker MV/MZ (img/sv_actors): 9×6 células (64×64 por padrão) com os 18 movimentos (walk, wait, chant, guard, damage, evade, thrust, swing, missile, skill, spell, item, escape, victory, dying, abnormal, sleep, dead), 3 quadros cada, tirados de animações; o movimento i fica na coluna floor(i/6)*3 + quadro e na linha i%6. Devolve a imagem. |
| `tile_grid_set` | escrita | Transforma o sprite num tileset: define o tamanho do tile, a margem e o espaçamento da grade (tiles endereçados por [coluna, linha]). Também altera ou apaga a grade. |
| `tile_info` | leitura | Grade de tiles do sprite: colunas, linhas, tiles vazios (pela imagem composta do frame), colisões, terrenos e mapa de prévia. |
| `tile_collision_set` | escrita | Define a colisão de um ou mais tiles: polígonos em pixels do tile (origem no canto superior esquerdo) ou atalhos full, top_half, bottom_half, left_half, right_half; ou limpa. |
| `tile_terrain_set` | escrita | Define conjuntos de terreno (nome, modo e terrenos com cor) e, por tile, o terreno e os bits de vizinhança (lados e cantos), como os terrenos do Godot 4. |
| `autotile_generate` | escrita | Monta um autotile a partir de um bloco A2 do RPG Maker ou de uma moldura 3x3 com cantos internos: cria um sprite novo com o bloco A2 e/ou os 47 tiles do blob (com terrenos para o Godot), montados por quartos de tile. |
| `tile_preview_map_set` | escrita | Guarda um mapa pequeno de prévia (largura x altura em células, cada uma com um tile [coluna, linha] ou vazia); também pinta por terreno escolhendo os tiles pelos bits de vizinhança. |
| `tile_preview_render` | leitura | Imagem PNG do mapa de prévia montado com os tiles do sprite, para ver como eles se encaixam. |
| `export_godot_tileset` | leitura | Exporta o tileset para o Godot 4: PNG (imagem composta de um frame) + TileSet .tres com a grade, os tiles não vazios, colisões (camada de física 0) e terrenos. |
| `hitbox_create` | escrita | Cria uma caixa de jogo no sprite: hitbox (causa dano), hurtbox (recebe dano), collision ou custom. Opcionalmente já com um retângulo em frames escolhidos. Exportada para o Godot como Area2D + CollisionShape2D. |
| `hitbox_update` | escrita | Renomeia uma caixa, muda o tipo ou a cor, ou a apaga (`delete`). |
| `hitbox_set` | escrita | Define o retângulo de uma caixa (pixels do sprite) ou a desliga (`clear`) num frame, numa lista de frames, em todos os frames de uma animação/direção ou em todos os frames. Frame sem retângulo = caixa desligada nele. |
| `hitbox_list` | leitura | Lista as caixas (hitbox/hurtbox/collision) com tipo, cor e retângulos por frame (ou só os de um frame), com alertas de retângulos fora do canvas. |
| `export_godot_scene` | leitura | Exporta uma cena Godot 4 (.tscn) pronta: Node2D com AnimatedSprite2D (sheet PNG + SpriteFrames .tres gravados ao lado), um Area2D/CollisionShape2D por hitbox, um Marker2D por anchor e um AnimationPlayer com uma animação por trilha que troca os frames e move/liga/desliga as caixas e os pontos frame a frame. Posições relativas ao pivô. |
| `export_job_set` | escrita | Cria ou atualiza uma tarefa de exportação salva no projeto (um comando export_* com seus argumentos). Com `on_save`, roda sozinha depois de cada project_save. Caminhos relativos partem da pasta do .psmproj. |
| `export_job_delete` | escrita | Apaga uma tarefa de exportação. |
| `export_job_list` | leitura | Lista as tarefas de exportação do projeto (comando, argumentos, se roda ao salvar) e a pasta base dos caminhos relativos. |
| `export_jobs_run` | leitura | Roda as tarefas de exportação (todas, as nomeadas ou só as de `on_save`) e devolve o resultado de cada uma; uma falha não impede as outras. É a exportação em lote (também: `psm export projeto.psmproj`). |
| `select_color` | leitura | Varinha mágica: seleciona os pixels parecidos com o de (x, y), contíguos ou na imagem toda (tolerância por canal, conectividade 4/8, layer ou imagem composta). Combina com a seleção `base` (replace, add, subtract, intersect) e devolve a máscara `bounds` + `rows` (`#` = selecionado). |
| `select_polygon` | leitura | Laço: seleciona o polígono pelos pontos (pode ser côncavo; bordas incluídas). Combina com a seleção `base` (replace, add, subtract, intersect) e devolve a máscara `bounds` + `rows` (`#` = selecionado). |
| `selection_read` | leitura | Lê só os pixels selecionados (máscara e/ou retângulo) de uma layer ou da imagem composta, como matriz pronta para colar com pixel_write_region: `.` = fora da seleção ou transparente (mantém o pixel ao colar). |
| `draw_text` | escrita | Escreve texto com uma fonte bitmap ("pixel-5x7", "mini-3x5" ou uma fonte feita num sprite): alinhamento, quebra por palavra (`max_width`), escala inteira, espaçamentos, contorno e sombra. Atalho para um draw_batch com a op `text`. Caracteres sem glyph viram um retângulo e entram nos avisos. |
| `text_measure` | leitura | Mede um texto sem desenhar: largura, altura e caixa (igual à do desenho), linhas com posição e largura, caixa dos pixels e caracteres sem glyph. |
| `font_define` | escrita | Marca um sprite como fonte bitmap: grade de células (`cell`, `columns`), caracteres na ordem das células (`charset`), linha de base, espaçamentos e altura de linha. O avanço de cada glyph é a largura real (colunas opacas) da célula. Devolve uma amostra. |
| `font_info` | leitura | Detalha uma fonte (embutida ou de sprite): métricas, caracteres, avanço de cada glyph, células vazias, caracteres padrão que faltam e uma amostra renderizada. |
| `font_template` | escrita | Cria um sprite NOVO com a grade de células de uma fonte (uma região por caractere, layer de guias), já definido como fonte e, opcionalmente, preenchido com uma fonte embutida para servir de ponto de partida. |
| `export_bitmap_font` | leitura | Exporta uma fonte (embutida ou de sprite) como BMFont texto da AngelCode: <nome>.fnt + atlas <nome>.png, que o Godot 4 importa como FontFile. |
| `nine_slice_set` | escrita | Define (ou remove) as margens de 9-slice do sprite (left, top, right, bottom em pixels); o centro precisa sobrar com pelo menos 1 px. |
| `nine_slice_render` | leitura | Prévia do 9-slice: renderiza o frame esticado para width×height (cantos intactos; bordas e centro esticados ou repetidos) e devolve a imagem. |
| `export_nine_slice` | leitura | Exporta o 9-slice para o Godot 4: PNG + cena .tscn com um NinePatchRect configurado (textura, patch_margin_*, axis_stretch_*) + JSON com as margens. |
| `palette_import` | escrita | Importa um arquivo de paleta como paleta do projeto (e a ativa): .gpl (GIMP), .hex (Lospec), .pal (JASC-PAL ou RIFF PAL), .txt (Paint.NET) ou imagem .png/.gif (cores únicas na ordem de leitura). Até 256 cores; um erro no arquivo diz a linha. |
| `palette_export` | leitura | Grava uma paleta (ou as cores de uma variante) em arquivo, no formato da extensão: .gpl, .hex, .pal (JASC-PAL), .txt (Paint.NET) ou imagem .png/.gif com uma amostra por cor. |
| `palette_variant_set` | escrita | Cria ou altera uma variante de paleta (recoloração: inimigo vermelho, time azul, versão noturna): a cor i da paleta base vira a cor i da variante. Não mexe nos pixels; veja com palette_variant_preview e exporte com export_palette_variants. |
| `palette_variant_delete` | escrita | Apaga uma variante de paleta. |
| `palette_variant_list` | leitura | Lista as variantes de paleta: base, cores, quantas cores mudam e se ainda casam com a paleta base. |
| `palette_variant_preview` | leitura | Imagem de um frame recolorido por uma variante (ou por cores de teste, sem salvar), opcionalmente ao lado do original. Troca exata de cor sobre a imagem composta; diz quantos pixels mudaram e quais cores não estão na base. |
| `export_palette_variants` | leitura | Exporta um arquivo por variante de paleta (padrão: todas): PNG de um frame, sprite sheet (mesmas opções e seleção de export_spritesheet, com JSON) ou GIF de uma animação. Recolore as imagens compostas por troca exata de cor (alfa preservado), sem mudar o projeto. Nomes: <prefixo>_<variante>.<ext>. |
| `export_normal_map` | leitura | Exporta o normal map PNG de um frame ou de uma sprite sheet (mesmo layout de export_spritesheet, casa pixel a pixel) para a iluminação 2D do Godot (CanvasTexture): altura pela borda (bevel), pelo brilho (luminance) ou mix, Sobel com `strength`, convenção OpenGL (ou DirectX com flip_y). Área plana = (128,128,255); alfa copiado. |

O schema completo de cada ferramenta (tipos, campos, descrições) é enviado pelo próprio servidor no `tools/list`.

