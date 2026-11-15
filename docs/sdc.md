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
| `stroke` | `points`, `color`, `thickness`, `pixel_perfect`, `symmetry`, `wrap`, `mode` (+ `pattern`, `density`, `secondary`, `step`, `ramp`) | traço de pincel com simetria, volta nas bordas, dithering ou sombreado por rampa (ver abaixo) |
| `text` | `text`, `at`, `font`, `color`, `align`, `max_width`, `letter_spacing`, `line_spacing`, `scale`, `outline`, `shadow` | texto com fonte bitmap (ver abaixo) |
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

## Traço de pincel (`stroke`)

É o traço das ferramentas de pincel do editor (lápis, borracha, dithering, sombreado). Os `points` são
ligados por linhas; com `pixel_perfect: true` os cantos em "L" saem (traço à mão limpo). O pincel é um
quadrado de `thickness` px. **Cada pixel coberto muda uma única vez por operação**, mesmo que o traço passe
duas vezes por ele.

- `symmetry`: `{"x": eixo, "y": eixo}`. `x` espelha esquerda↔direita num eixo vertical; `y` espelha
  cima↔baixo num eixo horizontal; com os dois, o traço sai 4 vezes. O eixo é `true` (centro do sprite:
  `(largura-1)/2`, que num sprite de 32 px é 15.5, entre os pixels 15 e 16) ou a coordenada em pixels:
  inteiro passa por um pixel, `.5` fica entre dois.
- `wrap: true`: pixels fora do canvas dão a volta (módulo largura/altura), para desenhar texturas que se
  repetem. Sem `wrap`, o que sai do canvas é descartado.
- `mode`:
  - `paint` (padrão): pinta `color` (`"clear"` apaga — é a borracha).
  - `dither`: pinta `color` onde o padrão ordenado `pattern` (`bayer2`, `bayer4` padrão, `bayer8`,
    `checker`; também `lines_h`, `lines_v`, `diagonal`) manda pela `density` (0..1, padrão 0.5), e
    `secondary` no resto; sem `secondary`, o resto fica como está. O padrão é ancorado no canvas (não no
    traço), então traços vizinhos se encaixam.
  - `shade`: cada pixel anda `step` posições (padrão +1 = clareia; -1 = escurece) na rampa `ramp` (da
    cor mais escura para a mais clara; padrão: a paleta ativa, na ordem) e para no fim dela. Pixels
    transparentes ou com cor fora da rampa ficam iguais. `color` é ignorada.

```json
{ "op": "stroke", "points": [[4, 10], [9, 6], [12, 6]], "color": 3, "pixel_perfect": true,
  "symmetry": { "x": true } }
{ "op": "stroke", "points": [[0, 4], [31, 4]], "color": "#2f6ad0", "mode": "dither",
  "pattern": "checker", "density": 0.5, "secondary": "#1a1c2c", "thickness": 3, "wrap": true }
{ "op": "stroke", "points": [[8, 8], [14, 12]], "mode": "shade", "step": -1, "thickness": 2 }
```

## Seleção por máscara

Seleções não retangulares viajam como texto: `{"bounds": {"x", "y", "w", "h"}, "rows": ["#.#", ...]}`
(`#` = selecionado, `.` = fora; as linhas começam no canto `x`,`y` de `bounds`). `select_color`
(varinha mágica) e `select_polygon` (laço) devolvem esse formato e aceitam `base` (a seleção atual) +
`mode` (`replace`, `add`, `subtract`, `intersect`) para combinar. A máscara pode ser passada em
`pixel_clear` (apagar só os pixels selecionados), `draw_transform` (mover, espelhar, girar só eles; a
resposta traz `selection`, a máscara já transformada) e `selection_read` (ler só eles, numa matriz que
`pixel_write_region` ou a op `matrix` colam: `.` mantém o pixel de destino).

## Texto (`text`)

`text` escreve com uma fonte bitmap. A caixa do texto começa em `at` e inclui a zona dos acentos acima
das maiúsculas, as descidas, o contorno e a sombra. Nada é desenhado fora dela, e `text_measure` devolve
a mesma caixa sem desenhar.

| Campo | Padrão | O que faz |
|---|---|---|
| `text` | — | o texto; `\n` quebra a linha, tabulação vira 4 espaços |
| `at` | `[0, 0]` | canto superior esquerdo; sem `max_width`, com `center`/`right` é o centro ou a última coluna (inclusive) |
| `font` | `"pixel-5x7"` | `"pixel-5x7"` (maiúsculas 5×7, minúsculas com descidas), `"mini-3x5"` (compacta) ou nome/id de um sprite definido com `font_define` |
| `color` | — | obrigatória nas fontes embutidas; numa fonte de sprite, sem ela os glyphs mantêm as cores desenhadas; `"clear"` recorta |
| `align` | `left` | `left`, `center`, `right` (dentro de `max_width`, quando houver) |
| `max_width` | — | largura máxima da caixa; quebra por palavra (palavra longa demais é quebrada por letra) |
| `letter_spacing` | da fonte (1) | pixels da fonte entre letras; pode ser negativo |
| `line_spacing` | 0 | pixels da fonte a mais (ou a menos) entre linhas |
| `scale` | 1 | ampliação inteira 1–16 (vizinho mais próximo); espaçamentos crescem junto |
| `outline` | — | cor do contorno de 1 px por fora (8 vizinhos) |
| `shadow` | — | `{"color": "#000", "offset": [1, 1]}`, sob o texto e o contorno |

As fontes embutidas cobrem ASCII 32–126, `áàâãäéèêëíìîïóòôõöúùûüçñ`, as maiúsculas correspondentes e
`¡ ¿ º ª °`. Caractere sem glyph vira um retângulo e aparece nos avisos.

```json
{ "op": "text", "text": "Olá, Mundo!", "at": [2, 2], "color": "#ffffff",
  "outline": "#1a1c2c", "shadow": { "color": "#5d275d" } }
```

## Atomicidade e respostas

`draw_batch` com `policy: "all_or_nothing"` (padrão) desfaz tudo se uma operação falhar, e o erro diz
`failed_index`. Com `"skip_invalid"`, as inválidas são puladas e listadas em `skipped`.

Toda escrita responde com `changed_pixels`, `region` (caixa alterada), `revision` e `transaction` (o id do
passo no histórico; dentro de uma transação explícita, o id dela).

## Limites

Até 20 000 operações por lote, 200 000 pontos por lista, 4096 px por lado de forma, espessura de 1 a 64.
