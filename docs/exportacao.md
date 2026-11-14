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
