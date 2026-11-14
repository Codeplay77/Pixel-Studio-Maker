# Arquitetura

O Pixel Studio Maker tem **um motor** e **duas portas de entrada**: o editor visual (Tauri) e o servidor
MCP (para agentes como o Claude Code). As duas usam exatamente os mesmos comandos; nenhuma operação de
edição existe só na interface.

```
 Claude Code ──stdio/MCP──► psm mcp ─┐
                                     ├──IPC (Named Pipe)──► psm runtime ──► sprite-commands ──► motor
 Editor (Tauri + React) ─────────────┘                       (estado oficial)
```

## Crates

| Crate | Papel | Depende de |
|---|---|---|
| `sprite-core` | Cores RGBA, imagens, paletas, máscaras, composição de layers, diferenças, matrizes de texto, PNG | — |
| `sprite-drawing` | Rasterizadores pixel-perfect, preenchimento, contorno, dithering, degradês, sombreamento, padrões, transformações e a linguagem SDC | core |
| `sprite-animation` | Modelo do sprite (layers, cels, frames, animações por direção, anchors, regiões, poses), composição de frames, onion skin, layout de sprite sheets, validação | core, drawing |
| `sprite-project` | Documento com histórico transacional e snapshots, formato `.psmproj`, importação PNG/GIF, exportações | core, drawing, animation |
| `sprite-commands` | A camada única de comandos (85), com schemas JSON, códigos de erro, limites e política de pastas | todos acima |
| `sprite-runtime` | Processo que guarda o estado oficial; protocolo IPC; cliente | commands, project |
| `sprite-mcp` | Servidor MCP (`rmcp`, stdio): ferramentas, recursos e prompts | runtime, commands |
| `apps/desktop/src-tauri` | App Tauri: cliente do runtime para a interface | runtime, commands |
| `tools/sprite-cli` | O executável `psm`: `mcp`, `runtime`, `call`, `script`, `mcp-run`, `render`, `info`, `tools` | runtime, mcp |

`sprite-core`, `sprite-drawing` e `sprite-animation` não conhecem Tauri, React nem MCP.

## Contratos

### Motor (`sprite-core` / `sprite-drawing`)

- Coordenadas inteiras, origem (0,0) no canto superior esquerdo.
- Cores RGBA não pré-multiplicadas; transparente é sempre `#00000000`.
- Toda operação é determinística (matemática inteira; onde há `f64`, as mesmas entradas dão as mesmas
  saídas). Nada cria cores intermediárias sem pedir.
- Toda escrita passa por um `Canvas`, que respeita recorte (máscara) e registra a área alterada.

### Comandos (`sprite-commands`)

- `Command` é um enum `{"cmd": nome, "args": {...}}`. A tabela de comandos gera o enum, os nomes, as
  descrições e os schemas JSON das ferramentas MCP (sem `$ref`).
- `Session::execute` roda leituras direto e escritas dentro de uma transação: implícita (um passo do
  histórico por comando) ou a transação explícita aberta por `transaction_begin`. Dentro dela, cada comando
  tem um *savepoint*: se falhar, só ele é desfeito.
- Resposta: `Outcome { data, images (PNG), changes, warnings, revision, transaction }`.
- Erros: `CommandError { code, message, details }`, com códigos fixos: `INVALID_ARGUMENT`, `NOT_FOUND`,
  `OUT_OF_BOUNDS`, `REVISION_CONFLICT`, `LIMIT_EXCEEDED`, `TRANSACTION_STATE`, `LOCKED`, `IO`,
  `PERMISSION_DENIED`, `UNSUPPORTED`, `NO_PROJECT`, `INTERNAL`.
- Limites configuráveis: lado máximo do canvas (1024), layers (256), frames (4096), operações por lote
  (20 000), lado máximo de prévia (2048).
- Política de pastas: caminhos relativos resolvem a partir da pasta de trabalho do cliente; o MCP só
  lê/grava dentro dela, de `Documentos/Pixel Studio Maker`, da pasta de recuperação e das pastas extras
  passadas com `--allow-dir`. O editor pode tudo, porque os caminhos vêm de diálogos do usuário.

### Runtime (`sprite-runtime`)

- Um processo por usuário (`psm runtime`), endpoint `\\.\pipe\pixel-studio-maker-<usuário>` no Windows
  (Unix socket no Linux/macOS). `PSM_RUNTIME_PIPE` muda o endpoint (os testes usam endpoints isolados).
- É iniciado sob demanda pelo primeiro cliente (MCP ou editor), sai do *job object* do host e encerra
  sozinho depois de 5 minutos sem clientes, gravando antes a recuperação dos projetos não salvos.
- Cada projeto aberto tem um mutex: comandos no mesmo projeto rodam em ordem; projetos diferentes em
  paralelo.
- **Revisões**: cada passo do histórico incrementa a revisão. Uma escrita com `expected_revision` diferente
  da atual falha com `REVISION_CONFLICT` e devolve `current_revision`.
- **Transações por cliente**: enquanto um cliente tem uma transação aberta, escritas de outros clientes no
  mesmo projeto esperam até 15 s e então falham com `LOCKED`. Se o dono cair, a transação é desfeita.
- **Eventos**: `changed` (projeto, revisão, quem, comando, cels alteradas), `operation` (log para o painel
  MCP do editor), `projects` e `clients`.
- **Autosave e recuperação**: a cada 10 s, projetos com alterações não salvas são gravados em
  `%LOCALAPPDATA%/Pixel Studio Maker/autosave`. `project_list` mostra o que pode ser recuperado e
  `project_open` com `recover: true` abre a cópia mais nova.

#### Protocolo IPC

Uma mensagem JSON por linha, nos dois sentidos.

```json
{"id": 1, "op": "hello", "client": {"kind": "mcp", "name": "Claude Code", "workspace": "C:/jogo"}, "protocol": 1}
{"id": 2, "op": "call", "project": null, "command": {"cmd": "pixel_set", "args": {"x": 1, "y": 2, "color": "#ff0000"}}, "expected_revision": 7}
{"id": 3, "op": "subscribe"}
{"id": 4, "op": "render", "frame": 12}
{"id": 5, "op": "state"}
```

Respostas: `{"id": 2, "ok": {...}}` ou `{"id": 2, "error": {"code": "...", "message": "..."}}`.
Eventos (sem `id`): `{"event": "changed", "project": "7f0f2d0b", "revision": 8, "source": "Claude Code", ...}`.

### MCP (`sprite-mcp`)

- Transporte stdio. O servidor não guarda estado: cada ferramenta vira um `call` no runtime.
- Toda ferramenta que age num projeto aceita `project` e, se escreve, `expected_revision`.
- Resultados: um bloco de texto JSON compacto (`revision`, `data`, avisos) e blocos de **imagem** (PNG ou
  GIF) quando há prévia. Erros voltam como resultado de ferramenta com `isError` e o código.
- Recursos: `psm://guide`, `psm://guide/sdc`, `psm://status` e modelos
  `psm://project/{id}/state|palettes|metadata`, `psm://project/{id}/frame/{frame}.png`,
  `psm://project/{id}/animation/{nome}.png`.
- Prompts: `criar_personagem`, `animar_caminhada`, `oito_direcoes`, `importar_e_animar`.

### Editor (`apps/desktop`)

- O lado nativo só conecta ao runtime, repassa eventos (`runtime-event`) e entrega pixels em bytes
  (`runtime_render`: 8 bytes de cabeçalho + RGBA).
- A interface (React + TypeScript + Tailwind + Zustand) desenha o canvas ampliado sem suavização e envia
  comandos. O lápis desenha ao vivo dentro de uma transação e, ao soltar, troca os segmentos por um único
  traço *pixel-perfect* (um passo do histórico). As prévias de linha/retângulo/elipse durante o arraste
  são só um guia: quem rasteriza é o Rust.

## Modelo de dados

```
Projeto ── sprites[] ── layers[] (ordem de baixo para cima)
        │            ├─ frames[] (linha do tempo, com duração e tags)
        │            ├─ cels[] (layer × frame → imagem; imagens iguais = cels vinculadas)
        │            ├─ animations[] ── tracks[] (uma por direção) ── frames (ids)
        │            ├─ directions[] (N, NE, E, SE, S, SW, W, NW + personalizadas)
        │            ├─ anchors[] (posição padrão + por frame)
        │            ├─ regions[] (forma padrão + por frame: retângulo, máscara ou polígono)
        │            └─ poses[] (anchors, chão, notas, frame)
        ├─ palettes[] e paleta ativa
        ├─ references[] (imagens de referência, não exportadas)
        └─ snapshots[] (cópias completas nomeadas)
```

Os pixels ficam num repositório de imagens por id; o resto é metadado leve, barato de copiar, o que
permite o histórico guardar o modelo inteiro antes/depois de cada passo e só as imagens tocadas.
