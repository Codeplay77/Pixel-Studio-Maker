# Demonstrações

Cada pasta tem os roteiros de chamadas MCP usados para fazer a demonstração, e o resultado. Os roteiros
rodam pelo protocolo MCP de verdade com `psm mcp-run`, que sobe o `psm mcp` e chama as ferramentas por
stdio, como o Claude Code faz. Nenhuma imagem veio de serviço generativo: cada pixel saiu de uma operação
do motor.

Rodar (nesta pasta, `examples/`):

```sh
psm mcp-run demo-a-guerreiro/1-desenhar.json --images demo-a-guerreiro/saida
```

`--full` mostra as respostas inteiras; `--images` grava as imagens devolvidas (prévias, comparações, GIFs).

| Pasta | Teste da especificação | O que mostra |
|---|---|---|
| [demo-a-guerreiro](demo-a-guerreiro) | A — criação do zero | Guerreiro 64x64 com paleta de 18 cores e 8 layers; inspeção, leitura da região como texto, correção do penacho e da mão da espada; regiões semânticas, anchors e pose |
| [demo-b-caminhada](demo-b-caminhada) | B — animação | Caminhada de 8 frames para a direita com poses reais (contato, descida, passagem, subida), corpo subindo e descendo, braços em oposição; sprite sheet e GIF |
| [demo-c-importar](demo-c-importar) | C — importação e animação | Um slime 32x32 feito fora do editor é importado sem perder nenhum pixel; o idle de 6 frames é criado editando linhas e colunas da matriz lida pelo MCP (achatar, esticar, piscar) |
| [demo-d-direcoes](demo-d-direcoes) | D — oito direções | A caminhada nas 8 direções: S, N, SE e NE desenhadas, W/SW/NW a partir de espelhos com o equipamento refeito (o escudo continua no braço esquerdo); sheet com uma linha por direção e `SpriteFrames` do Godot |
| [demo-e-sincronizacao](demo-e-sincronizacao) | E — sincronização | Com o editor aberto, o agente muda a cor da armadura, aumenta o escudo e corrige os três primeiros frames; o editor atualiza sem recarregar e uma edição feita à mão no editor é lida pelo MCP |
| [demo-f-gaivota](demo-f-gaivota) | — | Gaivota voando: batida de asas em 8 frames com a ponta da asa atrasada e o corpo subindo quando as asas descem |
| [demo-g-jacare](demo-g-jacare) | — | Jacaré nadando na superfície: patas remando em pares diagonais, cauda ondulando, parte submersa recolorida |
| [demo-h-parallax](demo-h-parallax) | — | Cenário de entardecer em 7 camadas que se emendam, pronto para parallax, com projeto Godot 4 (`Parallax2D`) e gravação do Godot rodando |

Os scripts `gerar*.mjs` só expandem as decisões de pose (tabelas escritas à mão) em operações SDC; quem
desenha é o motor, e as decisões artísticas estão nas tabelas e matrizes dos próprios arquivos.

O teste `tests/` da raiz reexecuta as demonstrações A e B sem interface e confere, pixel a pixel, que o
resultado é idêntico às imagens de referência.
