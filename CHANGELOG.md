# Mudanças

## 0.1.0 — 2026-11-14

Primeira versão.

- Motor de pixels em Rust: RGBA e paletas, máscaras, composição de layers com 8 modos de mistura,
  diferenças entre imagens, matrizes de texto e PNG.
- Desenho pixel-perfect: linhas, polilinhas, retângulos, elipses, círculos, polígonos, curvas,
  preenchimento, contornos, dithering, degradês discretos, sombreamento pela direção da luz, sombra
  projetada, padrões, limpeza, quantização, ajuste HSV e transformações por vizinho mais próximo.
- SDC: linguagem de desenho em lote, atômica, com matrizes textuais, RLE e patches.
- Animação: frames, cels vinculadas, animações com trilhas por direção (8 padrão + personalizadas),
  anchors, regiões semânticas, poses, ciclos, onion skin e validação automática.
- Projeto `.psmproj` versionado, gravação atômica, histórico com undo/redo e transações, snapshots,
  recuperação automática.
- Importação de PNG, GIF e sprite sheets. Exportação de PNG, sequência, GIF, sprite sheet (5 layouts) com
  JSON e `SpriteFrames` do Godot 4.
- Runtime compartilhado por Named Pipe, com revisões, transações por cliente e eventos em tempo real.
- Servidor MCP nativo (`rmcp`, stdio) com 84 ferramentas, recursos e prompts.
- Editor Tauri 2 + React com ferramentas manuais, layers, paletas, linha do tempo, reprodução, regiões,
  anchors, inspeção e painel de atividade do agente.
- Demonstrações A–E feitas pelo MCP e teste de regressão por imagem.
