# Como contribuir

Obrigado pelo interesse! Algumas regras mantêm o projeto coerente.

## Princípios

- **Toda operação de edição mora no Rust.** O editor e o MCP só chamam comandos de `sprite-commands`.
  Nada de lógica de desenho duplicada em TypeScript.
- **Toda funcionalidade do editor também existe no MCP.** Um comando novo entra na tabela `commands!` de
  `crates/sprite-commands/src/lib.rs` e vira ferramenta automaticamente.
- **Nenhuma ferramenta retorna sucesso sem fazer a operação.** Nada de dados fictícios.
- **Determinismo.** As mesmas entradas geram os mesmos pixels; sem antialiasing involuntário.
- **Sem geração de imagens por IA** e sem integrar Aseprite, PixelLab ou serviços externos.
- Textos para o usuário e para o agente em português; comentários de código em inglês.

## Antes de abrir um pull request

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm install && pnpm check
```

- Testes novos para comportamento novo (unitários no crate e, para ferramentas MCP, no teste de stdio em
  `tools/sprite-cli/tests`).
- Se mudar o motor de propósito e uma imagem de referência em `tests/golden` mudar, explique no PR e
  atualize a imagem.
- Ferramenta MCP nova: rode `psm tools > docs/ferramentas-mcp.md`.
- Mudou o formato `.psmproj`: suba `FORMAT_VERSION` e escreva a migração em `format.rs`.

## Relatar um problema

Abra uma issue com o que você fez, o que esperava e o que aconteceu. Se envolver um projeto, anexe o
`.psmproj` (ou o roteiro MCP) que reproduz o problema.
