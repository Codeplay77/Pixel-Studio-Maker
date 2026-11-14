# Formato `.psmproj`, versão 1

Um projeto do Pixel Studio Maker é um arquivo ZIP comum (dá para abrir renomeando para `.zip`).

```
manifest.json
images/<id>.png
snapshots/<id>/project.json
snapshots/<id>/images/<id>.png
```

## `manifest.json`

```json
{
  "format": "pixel-studio-maker",
  "format_version": 1,
  "app_version": "0.1.0",
  "saved_at": "2026-11-14T07:20:00Z",
  "project": { "uid": "…", "name": "…", "sprites": [ … ], "palettes": [ … ], "active_palette": "…",
               "references": [ … ], "metadata": { … }, "export": { … }, "next_id": 120 },
  "images": { "5": { "file": "images/5.png", "width": 64, "height": 64 } },
  "snapshots": [ { "id": 1, "name": "antes", "created": "…", "file": "snapshots/1/project.json" } ]
}
```

- `project` é o modelo completo (ver [arquitetura.md](arquitetura.md#modelo-de-dados)). Os ids são
  únicos no projeto inteiro e menores ou iguais a `next_id`.
- Cada imagem é um PNG RGBA de 8 bits, sem perdas, guardado sem recompressão no ZIP.
- Cels que apontam para o mesmo id de imagem estão vinculadas.

## Leitura

1. `format` precisa ser `pixel-studio-maker`.
2. `format_version` maior que a suportada → erro claro ("arquivo de versão mais nova").
3. Versões antigas passam por migrações em `crates/sprite-project/src/format.rs` (`migrate`).
4. Verificação de integridade: ids repetidos, cels apontando para layers/frames/imagens inexistentes,
   animações com frames inexistentes, imagens de tamanho diferente do sprite. Qualquer problema recusa o
   arquivo, em vez de abrir um projeto quebrado.

## Gravação

Atômica: o ZIP é gravado num arquivo temporário na mesma pasta, sincronizado com o disco e renomeado por
cima do destino. Uma queda no meio nunca deixa um projeto pela metade.

## Recuperação

O runtime grava cópias de recuperação (no mesmo formato) em `%LOCALAPPDATA%/Pixel Studio Maker/autosave/<uid>.psmproj`,
com um `<uid>.json` ao lado (nome, caminho original, revisão, data). Salvar o projeto apaga a cópia.
