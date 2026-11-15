# Exportação

| Ferramenta | Saída |
|---|---|
| `export_png` | Um frame composto (só layers visíveis, sem as de referência), com escala inteira |
| `export_frames` | Um PNG por frame: `<prefixo>_<animação>_<direção>_<índice>.png` |
| `export_gif` | Uma trilha de animação, com as durações de cada frame e repetição conforme o modo |
| `export_spritesheet` | PNG da sheet + JSON de metadados ao lado |
| `export_godot_metadata` | PNG da sheet + `SpriteFrames` do Godot 4 (`.tres`) + JSON |

Toda ampliação é por vizinho mais próximo. GIFs usam a paleta exata quando há até 255 cores.

## Layouts de sprite sheet

`sheet.layout`: `horizontal`, `vertical`, `grid` (`columns`/`rows`), `rows` (uma linha por trilha de
animação/direção, o que a maioria dos motores espera) e `packed` (atlas: frames recortados e empacotados).

Outras opções: `spacing`, `margin`, `cell: [w, h]` (célula fixa), `trim` (recorta bordas transparentes) e
`align`: `pivot` (padrão: o pivô de todos os frames cai no mesmo ponto da célula), `center`,
`top_left`, `bottom_center`.

`selection` escolhe o que entra: `animations`, `directions` e `include_unassigned` (frames fora de
animações). A ordem é: animações na ordem do projeto, direções na ordem N, NE, E, SE, S, SW, W, NW.

## JSON da sheet (`psm-sheet`, versão 1)

```json
{
  "app": "Pixel Studio Maker", "format": "psm-sheet", "version": 1,
  "image": "guerreiro_walk.png", "size": { "w": 512, "h": 512 }, "scale": 1,
  "cell": { "w": 64, "h": 64 },
  "sprite": { "name": "guerreiro", "w": 64, "h": 64, "origin": [26, 59] },
  "frames": [
    { "frame": 36, "animation": "walk", "direction": "E", "index": 0, "duration": 100,
      "x": 0, "y": 128, "w": 64, "h": 64,
      "source": { "x": 0, "y": 0, "w": 64, "h": 64 }, "dest": [0, 128], "pivot": [26, 187] }
  ],
  "animations": [
    { "name": "walk", "direction": "E", "loop": "loop", "frames": [16, 17, 18, 19, 20, 21, 22, 23], "durations": [100, 100, 100, 100, 100, 100, 100, 100] }
  ]
}
```

- `x, y, w, h`: a célula na sheet. `source`: o pedaço do frame copiado; `dest`: onde ele começa na sheet.
- `pivot`: o pivô do frame em coordenadas da sheet (alinhamento no motor de jogo).
- `animations[].frames`: índices em `frames`, já na ordem de reprodução (ping-pong e reverso expandidos).

## Godot 4

`export_godot_metadata` grava o PNG e um `SpriteFrames` (`.tres`) com uma animação por trilha:
`walk_N`, `walk_NE`, …, ou só `idle` para animações sem direção. Cada frame é um `AtlasTexture` da sheet;
as durações viram durações relativas sobre a menor duração da trilha (e `speed` em quadros por segundo);
`once` vira `loop: false`. Com o layout `packed`, cada `AtlasTexture` ganha `margin` para manter o tamanho
original do frame.

Para usar: copie o PNG e o `.tres` para o projeto Godot (o caminho `res://` pode ser escolhido com
`res_path`), crie um `AnimatedSprite2D` e aponte `sprite_frames` para o `.tres`. A primeira linha de
comentário do arquivo traz o `offset` sugerido para o pivô cair na origem do nó.

### Cena com hitboxes (`export_godot_scene`)

Grava `<nome>.tscn` com a sheet `.png` e o SpriteFrames `.tres` ao lado. A cena tem:

- um `Node2D` raiz (`texture_filter = 1`, filtro nearest);
- um `AnimatedSprite2D` "Sprite" com `centered = false` e `offset` igual a menos o pivô, para que a posição do nó seja o pivô;
- um `Area2D` para cada hitbox, no grupo do tipo dela, com um `CollisionShape2D` "Shape" e um `RectangleShape2D` próprio;
- um `Marker2D` para cada anchor;
- um `AnimationPlayer` com uma animação por trilha, com o mesmo nome usado no SpriteFrames.

Cada animação tem trilhas discretas para `Sprite:animation`, `Sprite:frame`, `Caixa/Shape:position`,
`Caixa/Shape:shape:size`, `Caixa/Shape:disabled` e `Anchor:position`, todas relativas ao pivô. Se o `.tscn` cair
dentro de um projeto Godot (há `project.godot` numa pasta acima), os caminhos `res://` saem dessa pasta.

### TileSet (`export_godot_tileset`)

Grava o PNG e um `TileSet` `.tres` com um `TileSetAtlasSource`:

- só os tiles não vazios entram;
- as colisões vão para `physics_layer_0`, em coordenadas relativas ao centro do tile;
- os terrenos vão para `terrain_set_N`, com `terrain` e `terrains_peering_bit/*` em cada tile.

Os modos dos conjuntos de terreno são `0` (cantos e lados), `1` (cantos) e `2` (lados).

### Fonte bitmap e 9-slice

- `export_bitmap_font` grava BMFont texto (`.fnt`) e o atlas `.png`. O Godot 4 importa o `.fnt` como FontFile.
- `export_nine_slice` grava o PNG, um `.tscn` com `NinePatchRect` e um `.json` com as margens. O `NinePatchRect` vem com `patch_margin_*` e com `axis_stretch_*`: 0 estica, 1 repete, 2 repete encaixando.

### Normal map

`export_normal_map` grava um PNG RGB com o alfa copiado, para o campo Normal Map de um `CanvasTexture`. Ele usa
o mesmo layout da sheet quando recebe `sheet` + `selection`, então casa pixel a pixel com a textura. A
convenção é OpenGL (Y para cima), a do Godot; para DirectX, use `flip_y`.

### Tarefas de exportação

`export_job_set` guarda uma exportação no projeto: um comando `export_*` e seus argumentos. As tarefas com
`on_save` rodam depois de cada `project_save`. `psm export projeto.psmproj [--job NOME]...` roda as tarefas sem
runtime, para scripts de build, e sai com código 1 se alguma falhar.

## RPG Maker (MZ, MV e VX Ace)

| Arquivo | MZ/MV | VX Ace | Comando |
|---|---|---|---|
| Personagens | 12×8 células (48 px) = 576×384; `$` = 3×4 células | 32 px = 384×256 | `export_rpgmaker_character` |
| Faces | 4×2 de 144×144 = 576×288 | 96×96 = 384×192 | `export_rpgmaker_faces` |
| Battler lateral | 9×6 de 64×64 = 576×384 | não existe | `export_rpgmaker_battler` |
| Tilesets A1–A5, B–E e Window | tamanhos exatos de cada motor | idem | `rpgmaker_template` (modelo para desenhar) |

Na folha de personagem, as linhas são baixo, esquerda, direita e cima, com 3 colunas por direção. O RPG
Maker toca as colunas na ordem 0, 1, 2, 1. No battler, o movimento *i* com quadro *p* fica na coluna
`floor(i/6)*3 + p` e na linha `i % 6`.
