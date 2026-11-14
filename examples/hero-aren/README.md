# Aren — Guardião de Esmeralda

Herói medieval original em pixel art de 128 × 128, com proporções naturais,
armadura de aço, couro, capa verde, bordados dourados e espada na mão direita.
Criado no Pixel Studio Maker pelo protocolo MCP real (`psm mcp-run` → `psm mcp`),
com polígonos e pixels SDC rasterizados pelo motor Rust. Nenhuma imagem importada.

## Abrir e reproduzir

O projeto `aren.psmproj` está salvo e aberto no runtime. No editor, selecione a aba
**Aren - Guardião de Esmeralda**. Na timeline, escolha uma animação e uma direção,
e clique em Reproduzir. `idle` e `walk` repetem; `attack` e `hurt` executam uma vez.
O projeto anterior `Heroi` foi preservado.

| Animação | Quadros por direção | Total | Ação |
| --- | ---: | ---: | --- |
| idle | 6 | 48 | Respiração, cabelo/capa e piscar |
| walk | 8 | 64 | Contato, passagem, elevação do pé e braços opostos |
| attack | 8 | 64 | Guarda, antecipação, golpe de espada e recuperação |
| hurt | 4 | 32 | Recuo, proteção e retorno |

São 208 quadros editáveis, em 32 trilhas. Direções: N, NE, E, SE, S, SW, W, NW.
As vistas foram redesenhadas com projeção consistente; não são cópias espelhadas.
O pivô permanece em [64, 111]. A vista é oblíqua, portanto pés em profundidades
opostas podem estar em alturas diferentes na imagem.

## Arquivos

- `aren.psmproj`: projeto nativo editável, com paleta e animações.
- `exports/aren-{idle,walk,attack,hurt}.png`: uma sprite sheet por animação,
  uma linha por direção, transparência real, células de 128 × 128, sem espaçamento.
- `exports/aren-completo.png`: todas as animações em uma única sheet (1024 × 4096).
- JSON ao lado de cada sheet: células, direção, animação, duração e pivô.
- `exports/gifs/`: 32 prévias animadas em escala 3×.
- `exports/poses/`: oito poses em PNG transparente, ampliadas 4× sem suavização.
- `gerar.mjs`: definição das poses, materiais e expansão dos comandos MCP.
- `validation.json`: relatório técnico final.

## Validação

208 quadros presentes, quatro animações e oito direções em cada uma. Nenhum
quadro vazio, nenhuma repetição consecutiva, nenhuma cor fora da paleta ou corte
nas bordas. Foram retocados 130 pixels transparentes isolados via `pixel_apply_patch`.

O validador mantém alertas heurísticos de mudança acentuada de pixels (48) e
mudança da caixa visual (6), sobretudo no golpe e no recuo. A espada altera a
caixa visual; o pivô do personagem é fixo. Os 85 avisos informativos de vãos
incluem espaços entre braços, corpo, espada e pernas. Esses alertas não são
uma certificação da qualidade estética; as vistas e tiras principais foram
inspecionadas visualmente.

## Recriar as poses

Na raiz do repositório, `node examples/hero-aren/gerar.mjs preview` escreve
`preview.json`. Execute com `target/debug/psm.exe mcp-run` para criar um novo
projeto; anote o id retornado. `node examples/hero-aren/gerar.mjs animate ID`
escreve o lote das 208 poses. `node examples/hero-aren/gerar.mjs export ID`
escreve o lote de exportações. Os retoques de pixels e durações finais estão
preservados no projeto entregue; o gerador representa as poses anteriores a
esse acabamento. Não reexecute a criação de animações sobre o projeto entregue.
