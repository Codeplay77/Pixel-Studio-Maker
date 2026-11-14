# Sprite Drawing Commands (SDC), versão 1

A SDC é a linguagem de desenho em lote do Pixel Studio Maker. Uma operação é um objeto JSON com o campo
`"op"`. A ferramenta `draw_batch` recebe uma lista delas (`ops`) e aplica tudo em ordem, de forma atômica.

## Convenções

- **Coordenadas**: inteiras, (0,0) no canto superior esquerdo. Pontos são `[x, y]`. Retângulos são
  `{"x": 0, "y": 0, "w": 8, "h": 4}`.
- **Cores**: `"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`, `"clear"` (apagar) ou um **inteiro**, que é o
  índice na paleta ativa do sprite.
- **Recorte**: tudo é recortado ao canvas. Com `clip` (região semântica) a operação só pinta dentro da região.
- **Sem antialiasing**: nenhuma operação cria cores intermediárias, exceto as que pedem cor explicitamente
  (`shade` automático e `adjust`).
- **Alvo**: no `draw_batch`, cada operação pode ter `layer`, `frame` e `clip` próprios; sem eles vale o alvo do lote.

## Matrizes

`matrix` escreve uma grade a partir de `at`:

- **Textual**: `rows` (strings) + `legend` (char → cor). `.` e espaço mantêm o pixel, a menos que a legenda diga
  outra coisa. `null` na legenda também mantém; `"clear"` apaga. Com `"rle": true`, `"3k2."` = `"kkk.."`.
- **Numérica**: `pixels` (matriz de índices ou `null`) + `palette` opcional (senão, a paleta ativa).

```json
{ "op": "matrix", "position": [10, 10],
  "palette": ["#000000", "#3976D2", "#93C5FD"],
  "pixels": [[null, 0, 0, null], [0, 1, 1, 0], [0, 1, 2, 0], [null, 0, 0, null]] }
```

## Operações

| op | Campos | O que faz |
|---|---|---|
| `pixel` | `x`, `y`, `color` | um pixel |
| `pixels` | `pixels: [[x,y] ou [x,y,cor]]`, `color?` | lista de pixels |
| `matrix` | `at`, `rows`+`legend` (+`rle`) ou `pixels`(+`palette`) | grade de pixels |
| `line` | `from`, `to`, `color`, `thickness=1` | linha Bresenham |
| `polyline` | `points`, `color`, `closed`, `thickness`, `pixel_perfect` | linhas encadeadas; `pixel_perfect` remove cantos em "L" |
| `rect` | `x`,`y`,`w`,`h`, `color`, `fill`, `thickness` | retângulo |
| `ellipse` | `x`,`y`,`w`,`h`, `color`, `fill`, `thickness` | elipse inscrita na caixa, simétrica |
| `circle` | `center`, `radius`, `color`, `fill` | círculo |
| `polygon` | `points`, `color`, `fill=true` | polígono (regra par-ímpar) com arestas |
| `curve` | `points`, `color`, `thickness` | 3 pts = Bézier quadrática, 4 = cúbica, mais = spline pelos pontos |
| `fill` | `x`,`y`, `color`, `contiguous=true`, `connectivity=4`, `tolerance=0` | balde |
| `replace_color` | `from`, `to`, `tolerance`, `rect?` | troca de cor |
| `clear` | `rect?` | apaga (tudo, sem `rect`) |
| `outline` | `color`, `mode=outside/inside`, `connectivity`, `thickness`, `rect?` | contorno das formas visíveis |
| `dither` | `color`, `background?`, `density`, `pattern`, `rect?`, `only_opaque` | dithering ordenado |
| `gradient` | `colors`, `shape`, `dither?`, `rect?`, `only_opaque` | degradê em faixas |
| `shade` | `light`, `highlight?`, `shadow?`, `*_amount`, `*_depth`, `hue_shift`, `rect?` | luz e sombra pela direção da luz |
| `drop_shadow` | `dx`, `dy`, `color`, `rect?` | sombra projetada atrás da silhueta |
| `pattern` | `rows`, `legend`, `offset`, `rect?`, `only_opaque` | padrão ladrilhado |
| `transform` | `transform`, `rect?` | `flip_h`, `flip_v`, `rotate` (`quarter_turns` ou `degrees`), `move` (`dx`,`dy`,`wrap`), `scale` (`sx`,`sy`,`origin`) |
| `copy` | `from: {layer?, frame?, composite?}`, `rect`, `to`, `mode`, `flip_h` | copia de outra layer/frame |
| `cleanup` | `mode=orphans/noise`, `rect?` | remove pixels soltos ou isolados |
| `quantize` | `colors?`, `rect?` | leva cada pixel à cor mais próxima |
| `adjust` | `hue`, `saturation`, `value`, `rect?` | ajuste HSV |

Valores de `pattern` (dither): `bayer2`, `bayer4` (padrão), `bayer8`, `checker`, `lines_h`, `lines_v`, `diagonal`.
Valores de `light`: `top_left` (padrão), `top`, `top_right`, `left`, `right`, `bottom_left`, `bottom`, `bottom_right`.
`shape` do degradê: `{"type": "linear", "angle": 90}` (0 = esquerda→direita, 90 = cima→baixo) ou
`{"type": "radial", "center": [x, y], "radius": r}`.

## Atomicidade e respostas

`draw_batch` com `policy: "all_or_nothing"` (padrão) desfaz tudo se uma operação falhar, e o erro diz
`failed_index`. Com `"skip_invalid"`, as inválidas são puladas e listadas em `skipped`.

Toda escrita responde com `changed_pixels`, `region` (caixa alterada), `revision` e `transaction` (o id do
passo no histórico; dentro de uma transação explícita, o id dela).

## Limites

Até 20 000 operações por lote, 200 000 pontos por lista, 4096 px por lado de forma, espessura de 1 a 64.
