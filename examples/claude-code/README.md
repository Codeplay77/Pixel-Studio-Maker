# Conectar o Claude Code

O servidor MCP é o próprio executável `psm` com o argumento `mcp`. Não precisa de Node nem de Python.

## Jeito mais fácil

No editor, clique em **MCP** (canto superior direito): ele mostra o comando pronto com o caminho certo do
`psm.exe` da sua instalação. Copie e rode no terminal, na pasta do seu jogo:

```sh
claude mcp add pixel-studio-maker -- "C:\Users\SEU_USUARIO\AppData\Local\Pixel Studio Maker\psm.exe" mcp
```

Ou rode `psm mcp-config`, que imprime o JSON abaixo já com o caminho do executável.

## Pelo arquivo `.mcp.json`

Coloque um `.mcp.json` na pasta do projeto (veja o [exemplo](.mcp.json)) e troque `command` pelo caminho
do seu `psm.exe`:

- instalado: `%LOCALAPPDATA%\Pixel Studio Maker\psm.exe`;
- compilado do código: `<repositório>\target\release\psm.exe` (ou `target\debug\psm.exe`).

## Pastas que o agente pode usar

Por segurança, o MCP só lê e grava arquivos dentro da pasta onde o Claude Code está rodando, de
`Documentos\Pixel Studio Maker` e da pasta de recuperação. Para liberar outra pasta, adicione
`"--allow-dir", "D:\\Jogos\\MeuJogo\\assets"` em `args`, ou a variável `PSM_ALLOWED_DIRS`.

## Primeiro pedido

Com o editor aberto (opcional: tudo funciona sem ele), peça ao Claude Code:

> Crie um personagem de pixel art 64x64. Ele será um guerreiro medieval. Gere as animações idle, walk e
> attack em oito direções. Mantenha a identidade visual entre os frames. Exporte as sprite sheets para Godot.

O servidor entrega um guia (`psm://guide`) e prompts prontos (`criar_personagem`, `animar_caminhada`,
`oito_direcoes`, `importar_e_animar`) que ensinam o fluxo de trabalho ao agente.
