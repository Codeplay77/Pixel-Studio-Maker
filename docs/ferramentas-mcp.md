# Ferramentas MCP

Gerado por `psm tools` a partir da tabela de comandos (`crates/sprite-commands/src/lib.rs`).
São 84 ferramentas. Todas as que agem num projeto aceitam também `project` (id) e, se escrevem, `expected_revision`.

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
| `import_image` | escrita | Importa PNG/GIF como sprite novo, layer, frames ou referência; corta sprite sheets e cria animação com os frames. |
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

O schema completo de cada ferramenta (tipos, campos, descrições) é enviado pelo próprio servidor no `tools/list`.

