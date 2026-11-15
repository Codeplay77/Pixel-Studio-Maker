# Pixel Studio Maker
## Editor profissional de Pixel Art e Sprite Sheets com controle completo via MCP

> **Idioma: responda sempre em português (Brasil).** Vale para todas as respostas ao autor, explicações,
> resumos e perguntas, mesmo quando a pergunta for curta ou o assunto for técnico. Código, nomes de
> ferramentas e mensagens de commit seguem o padrão já usado no projeto.

### 1. Sua missão

Você é um arquiteto de software e engenheiro sênior especializado em Rust, Tauri 2, computação gráfica 2D, pixel art, animação de sprites, protocolos de comunicação entre processos e Model Context Protocol (MCP).

Sua missão é desenvolver do zero o **Pixel Studio Maker**, um aplicativo desktop open-source para Windows, capaz de criar, editar e animar pixel arts profissionalmente.

O aplicativo terá como referências funcionais o Aseprite e o PixelLab, mas deverá possuir arquitetura, código, interface e ferramentas próprias.

**Não integrar Aseprite, PixelLab, APIs de geração de imagens, modelos generativos locais ou serviços de geração gráfica.**

A criação artística automatizada será executada exclusivamente por agentes como o Claude Code, utilizando as ferramentas gráficas disponibilizadas pelo nosso servidor MCP.

O Claude deve conseguir:

- Criar pixel arts do zero.
- Importar sprites e continuar sua edição.
- Desenhar pixels individualmente ou em lotes.
- Construir personagens, objetos e cenários.
- Criar personagens de múltiplas perspectivas.
- Desenvolver animações frame a frame.
- Alterar poses e formas em cada frame.
- Inspecionar o resultado visual.
- Corrigir pixels individualmente.
- Gerar e organizar sprite sheets.
- Exportar assets para engines como Godot.

O editor visual e o MCP serão duas interfaces para o mesmo motor gráfico.

**O objetivo não é criar um editor que apenas recebe comandos de uma IA. O objetivo é construir um ambiente de produção artística inteiramente controlável por agentes de IA.**

---

### 2. Decisões definitivas do projeto

Estas decisões já estão aprovadas e não precisam ser questionadas novamente.

| Característica | Decisão |
|---|---|
| Plataforma inicial | Windows |
| Backend | Rust |
| Framework desktop | Tauri 2 |
| Interface | React + TypeScript |
| Protocolo de automação | MCP nativo em Rust |
| Gerador de imagens externo | Não utilizar |
| Criação artística | 100% via comandos MCP |
| Editor manual | Sim, completo |
| Resoluções principais | 16×16 até 128×128 |
| Resoluções personalizadas | Sim |
| Animação de sprites existentes | Sim |
| Criação de sprites do zero | Sim |
| Perspectivas | 8 direções e vistas personalizadas |
| Modelo de licença | Open-source |

Resoluções acima de 128×128 poderão ser suportadas mediante limites configuráveis de memória e desempenho.

Não implementar restrições artificiais que impeçam futuras expansões.

---

# 3. Arquitetura técnica

Construir o projeto como um Cargo Workspace modular.

Estrutura sugerida:

```text
spriteforge/
├── crates/
│   ├── sprite-core/
│   ├── sprite-drawing/
│   ├── sprite-animation/
│   ├── sprite-project/
│   ├── sprite-commands/
│   ├── sprite-mcp/
│   └── sprite-runtime/
├── apps/
│   └── desktop/
│       ├── src/
│       └── src-tauri/
├── tools/
│   └── sprite-cli/
├── tests/
├── examples/
├── docs/
└── Cargo.toml
```

### 3.1 Sprite Core

Responsável por:
- Armazenamento e leitura de pixels RGBA.
- Representação de pixels indexados por paletas.
- Transparência.
- Buffers de imagem.
- Operações determinísticas.
- Composição de camadas.
- Controle de coordenadas.
- Manipulação de regiões.
- Detecção e comparação de alterações.

Esse módulo não poderá depender do Tauri, React ou MCP.

### 3.2 Sprite Drawing

Implementar todos os algoritmos de desenho.

O módulo será responsável por transformar instruções estruturadas em alterações efetivas nos pixels.

### 3.3 Sprite Animation

Gerenciar:
- Timelines.
- Frames.
- Duração de frames.
- Layers e cels por frame.
- Onion skin.
- Tags de animação.
- Pontos de ancoragem.
- Poses.
- Direções.
- Composição de sprite sheets.

### 3.4 Sprite Project

Gerenciar o formato nativo de arquivo, histórico, snapshots, referências, metadados e persistência.

### 3.5 Sprite Commands

Criar uma camada única de comandos que exponha todas as operações do editor.

Tanto a interface gráfica quanto o MCP devem utilizar essa camada.

Nenhuma funcionalidade de edição deverá existir exclusivamente no frontend.

### 3.6 Sprite MCP

Servidor nativo em Rust usando o SDK oficial `rmcp`.

Implementar inicialmente o transporte stdio, compatível com Claude Code.

### 3.7 Sprite Runtime

Implementar um processo local responsável por manter o estado autoritativo dos projetos abertos.

Para Windows, utilizar inicialmente IPC local com Named Pipes ou mecanismo equivalente.

O servidor MCP e a interface Tauri devem se conectar ao mesmo runtime.

Requisitos:

- Uma única fonte de verdade para cada projeto aberto.
- Controle de concorrência.
- Revisões de projeto.
- Operações atômicas.
- Notificações de alterações.
- Funcionamento sem interface gráfica.
- Reabertura de projetos existentes.

O Claude deverá conseguir iniciar o MCP e editar projetos mesmo com a interface Tauri fechada.

---

# 4. Motor próprio de geração pixel por pixel

## 4.1 Conceito fundamental

O SpriteForge não possuirá um modelo de geração de imagens.

O Claude será responsável pelas decisões artísticas e o motor Rust será responsável pela execução precisa dessas decisões.

Por exemplo, quando o Claude solicitar a criação de um cavaleiro, deverá planejar sua estrutura visual e utilizar operações MCP para materializá-la.

O aplicativo não deverá gerar um cavaleiro utilizando uma API externa.

O Claude poderá utilizar algoritmos locais de desenho, montagem e transformação, mas deverá manter controle sobre os pixels finais.

## 4.2 Modelo de pixels

Implementar um canvas com:

- Coordenadas inteiras.
- Origem definida em (0,0).
- Canais RGBA.
- Transparência real.
- Cores indexadas quando aplicável.
- Nenhum antialiasing involuntário.
- Nenhum redimensionamento suavizado.
- Operações pixel-perfect.

Suportar buffers de pixels e regiões retangulares.

Cada operação deverá produzir um resultado determinístico.

## 4.3 Operações elementares

Implementar:

**Pixels**
- Definir pixel.
- Obter pixel.
- Apagar pixel.
- Substituir cor.
- Editar múltiplos pixels.

**Geometria**
- Linhas.
- Polilinhas.
- Retângulos.
- Elipses.
- Polígonos.
- Curvas rasterizadas.
- Preenchimentos.
- Contornos.

**Transformações**
- Mover.
- Espelhar.
- Rotacionar.
- Escalar por nearest-neighbor.
- Recortar.
- Copiar e colar.
- Alterar regiões.

**Operações artísticas**
- Aplicar contornos.
- Criar gradientes discretos.
- Aplicar dithering.
- Construir sombras.
- Gerar highlights.
- Quantizar cores.
- Limpar pixels isolados.
- Aplicar padrões.

Todas as operações deverão ser implementadas em Rust.

---

# 5. Linguagem estruturada de desenho

Este é um dos componentes mais importantes do projeto.

O Claude não deverá precisar realizar milhares de chamadas MCP para criar uma imagem.

Implementar uma representação compacta de operações gráficas, chamada provisoriamente **Sprite Drawing Commands (SDC)**.

Essa representação será interpretada pelo motor Rust e poderá conter várias operações encadeadas.

## 5.1 Tipos de instruções

O formato deverá permitir:

- Pixel individual.
- Lista de pixels.
- Matriz de pixels.
- RLE por linhas.
- Regiões retangulares.
- Formas geométricas.
- Aplicação de paleta.
- Transformações.
- Operações sobre layers.
- Alterações parciais de frames.

Usar um schema JSON versionado, com validação rigorosa.

### Exemplo de desenho por matriz

```json
{
  "operation": "draw_pixel_matrix",
  "layer": "character",
  "frame": 0,
  "position": [10, 10],
  "palette": [
    "#000000",
    "#3976D2",
    "#93C5FD"
  ],
  "pixels": [
    [null, 0, 0, null],
    [0, 1, 1, 0],
    [0, 1, 2, 0],
    [null, 0, 0, null]
  ]
}
```

Os números representam índices da paleta. `null` indica ausência de alteração, preservando o pixel existente.

Para apagar um pixel deverá existir uma operação explícita de limpeza, evitando ambiguidade.

O exemplo é conceitual. Defina o schema definitivo antes da implementação.

## 5.2 Operações em lote

Implementar o comando:

`draw_batch`

Esse comando receberá um conjunto de operações ordenadas.

Exemplo de uso:

1. Criar silhueta.
2. Preencher corpo.
3. Adicionar armadura.
4. Aplicar contornos.
5. Aplicar sombras.
6. Adicionar detalhes.
7. Executar a operação de forma atômica.

Se uma operação inválida comprometer o lote, o sistema deverá rejeitar ou reverter a transação conforme a política definida.

O comando retornará:
- Quantidade de pixels modificados.
- Região modificada.
- Revisão do projeto.
- Identificador da transação.
- Erros ou avisos.

## 5.3 Operações por diferenças

Implementar um mecanismo de patches.

O Claude deverá poder modificar apenas os pixels necessários, sem retransmitir todo o frame.

Isso será especialmente importante nas animações.

Oferecer representação compacta de diferenças entre duas imagens.

---

# 6. Sistema de construção assistida de personagens

Além de ferramentas básicas, criar um conjunto de recursos estruturais para auxiliar o Claude a construir pixel arts mais complexos.

Esses recursos devem ser determinísticos e editáveis.

## 6.1 Regiões semânticas

Permitir que o Claude organize um personagem em partes:

- Cabeça.
- Tronco.
- Braço esquerdo.
- Braço direito.
- Perna esquerda.
- Perna direita.
- Cabelo.
- Equipamentos.
- Armas.
- Acessórios.

Cada parte poderá ser associada a uma layer, região ou máscara.

Não é necessário que o aplicativo identifique automaticamente essas partes usando inteligência artificial.

O Claude deverá poder definir e modificar suas regiões.

## 6.2 Pontos de referência

Criar suporte a anchors, pivôs e articulações:

- Centro da cabeça.
- Pescoço.
- Ombros.
- Cotovelos.
- Mãos.
- Quadril.
- Joelhos.
- Pés.
- Origem do sprite.
- Ponto de contato com o chão.

Esses pontos devem auxiliar transformações e planejamento de animações.

Não implementar um sistema 3D.

Tudo deverá funcionar em coordenadas 2D e pixels discretos.

## 6.3 Construção em etapas

O Claude poderá seguir o fluxo:

1. Definir dimensões e perspectiva.
2. Criar silhueta.
3. Dividir o personagem em regiões.
4. Desenhar volumes.
5. Definir paleta.
6. Aplicar cores principais.
7. Construir iluminação.
8. Refinar detalhes.
9. Inspecionar a imagem.
10. Corrigir inconsistências.

Disponibilizar operações que facilitem esse processo.

**O objetivo não é limitar o Claude a templates prontos, mas oferecer primitivas gráficas que permitam construir diferentes estilos e personagens.**

---

# 7. Sistema avançado de animação

O SpriteForge deverá permitir a criação de animações inteiramente novas, inclusive a partir de imagens estáticas importadas.

## 7.1 Modelo de dados

Utilizar uma estrutura que diferencie:

- Projeto.
- Sprite.
- Layer.
- Cel.
- Frame.
- Animação.
- Direção.
- Tag.
- Pontos de ancoragem.

Uma animação conterá uma sequência ordenada de frames com duração individual.

Um frame será composto pelas cels correspondentes às suas layers.

Permitir reutilização de conteúdo entre frames quando apropriado.

## 7.2 Criação frame a frame

O Claude deverá conseguir:

- Criar um frame vazio.
- Duplicar um frame existente.
- Copiar regiões de outros frames.
- Alterar pixels específicos.
- Modificar partes do personagem.
- Reorganizar layers.
- Aplicar transformações.
- Criar poses intermediárias.
- Comparar frames.
- Pré-visualizar a animação.

As transformações geométricas serão ferramentas auxiliares.

Não considerar a simples movimentação ou rotação de um sprite inteiro equivalente a uma animação completa.

O Claude precisa conseguir redesenhar regiões para criar mudanças reais nas poses.

## 7.3 Planejamento de poses

Desenvolver estruturas para registrar:

- Pose inicial.
- Pose final.
- Poses-chave.
- Ciclo de movimento.
- Pontos de contato.
- Deslocamento de membros.
- Alinhamento ao chão.
- Variação de silhueta.

Esses registros serão utilizados pelo Claude durante a criação.

Implementar ferramentas para copiar e transformar regiões entre poses, preservando a possibilidade de correção pixel por pixel.

## 7.4 Animações suportadas

O editor deverá suportar qualquer animação criada pelo usuário.

Exemplos:

- Idle.
- Walk.
- Run.
- Jump.
- Attack.
- Hit.
- Death.
- Cast.
- Interact.

Esses nomes não representam animações pré-programadas.

O Claude deverá desenhar os frames necessários para cada movimento.

---

# 8. Sistema de 8 direções

Implementar suporte nativo para sprites multidirecionais.

Direções padrão:

1. Norte.
2. Nordeste.
3. Leste.
4. Sudeste.
5. Sul.
6. Sudoeste.
7. Oeste.
8. Noroeste.

Também permitir direções e perspectivas personalizadas.

Cada direção deverá possuir seus próprios frames.

O Claude deverá ser capaz de criar os oito ângulos mantendo consistência entre:

- Proporções.
- Cores.
- Equipamentos.
- Silhueta.
- Altura do personagem.
- Estilo.
- Identidade visual.

Espelhamento horizontal poderá ser utilizado para acelerar a construção de determinadas perspectivas, mas deverá ser opcional.

Nunca presumir que duas vistas opostas são perfeitamente simétricas.

Equipamentos assimétricos, iluminação, armas e acessórios poderão exigir redesenho.

Permitir definir sprites de referência para auxiliar a construção das demais direções.

---

# 9. Inspeção visual pelo Claude

Criar um sistema de feedback visual nativo para que o Claude consiga avaliar o resultado das operações.

O MCP deverá disponibilizar:

- Renderização PNG de um frame.
- Renderização de regiões específicas.
- Renderização da spritesheet.
- Pré-visualização da animação.
- Comparação entre frames.
- Informações sobre paleta.
- Informações sobre transparência.
- Diferenças de pixels.
- Bounding boxes.
- Pontos de referência.

Sempre que possível, retornar conteúdo de imagem através dos mecanismos de imagem do próprio MCP, não apenas caminhos de arquivos.

O Claude deverá conseguir solicitar uma imagem, inspecioná-la e executar correções posteriormente.

## Validação visual

Implementar verificações programáticas para identificar:

- Frames com dimensões inconsistentes.
- Pivôs desalinhados.
- Pixels fora dos limites.
- Uso inesperado de cores.
- Frames duplicados.
- Regiões transparentes inesperadas.
- Deslocamentos bruscos.
- Diferenças anormais entre frames.

Essas verificações devem gerar alertas, não impedir alterações artísticas intencionais.

Uma validação programática não deve ser tratada como garantia de qualidade estética.

---

# 10. Servidor MCP

Implementar um servidor MCP completamente funcional em Rust.

A integração inicial deverá ser com Claude Code, usando transporte stdio.

O servidor deverá operar independentemente da interface.

## 10.1 Ferramentas de projetos

- `project_create`
- `project_open`
- `project_save`
- `project_close`
- `project_info`
- `project_list`

## 10.2 Ferramentas de pixels

- `pixel_get`
- `pixel_set`
- `pixel_set_batch`
- `pixel_clear`
- `pixel_read_region`
- `pixel_write_region`
- `pixel_apply_patch`

## 10.3 Ferramentas de desenho

- `draw_batch`
- `draw_line`
- `draw_shape`
- `draw_polygon`
- `draw_fill`
- `draw_outline`
- `draw_dither`
- `draw_transform`
- `draw_replace_color`

## 10.4 Ferramentas estruturais

- `region_create`
- `region_update`
- `region_copy`
- `region_transform`
- `anchor_create`
- `anchor_update`
- `anchor_list`

## 10.5 Ferramentas de layers

- `layer_create`
- `layer_delete`
- `layer_duplicate`
- `layer_reorder`
- `layer_merge`
- `layer_properties`

## 10.6 Ferramentas de animação

- `animation_create`
- `animation_delete`
- `animation_info`
- `animation_set_direction`
- `animation_set_timing`
- `animation_preview`

## 10.7 Ferramentas de frames

- `frame_create`
- `frame_duplicate`
- `frame_delete`
- `frame_move`
- `frame_edit`
- `frame_compare`
- `frame_preview`
- `frame_apply_patch`

## 10.8 Ferramentas de paletas

- `palette_create`
- `palette_update`
- `palette_replace`
- `palette_quantize`

## 10.9 Ferramentas de exportação

- `export_png`
- `export_spritesheet`
- `export_frames`
- `export_gif`
- `export_godot_metadata`

## 10.10 Ferramentas de controle

- `editor_status`
- `editor_get_capabilities`
- `history_undo`
- `history_redo`
- `transaction_begin`
- `transaction_commit`
- `transaction_rollback`
- `project_validate`

A lista representa os recursos mínimos planejados. Criar ferramentas adicionais sempre que uma funcionalidade do editor não estiver acessível pelo MCP.

### Contratos MCP

Cada ferramenta precisa ter:

- Schema de entrada tipado.
- Documentação.
- Validação.
- Resposta estruturada.
- Identificação do projeto.
- Identificação de revisão.
- Códigos de erro consistentes.
- Limites de operações.

Operações de escrita devem suportar controle de revisão para prevenir alterações concorrentes.

Criar testes automatizados que invoquem ferramentas MCP reais e verifiquem os pixels resultantes.

### Recursos MCP

Além das ferramentas, fornecer recursos para:

- Estado atual do projeto.
- Frames existentes.
- Paletas.
- Metadados.
- Guias de uso do editor.
- Pré-visualizações.

### Guias para o agente

Desenvolver instruções técnicas ensinando o Claude a:

- Planejar pixel arts.
- Escolher paletas.
- Criar silhuetas.
- Organizar layers.
- Utilizar operações em lote.
- Construir poses.
- Criar frames eficientes.
- Comparar resultados.
- Corrigir problemas.

Fornecer workflows de criação como documentação e prompts MCP, quando suportados.

---

# 11. Otimização de geração

O maior desafio do projeto será permitir que o Claude construa imagens e animações sem consumo excessivo de tokens ou chamadas MCP.

Portanto, desenvolver estratégias de otimização desde o início.

## Técnicas obrigatórias

**1. Operações vetoriais rasterizadas**

Permitir a construção de silhuetas por polígonos e outras formas, convertendo o resultado para pixels.

**2. Matrizes compactas**

Representar regiões inteiras por índices de paleta.

**3. Patches diferenciais**

Modificar apenas pixels alterados entre frames.

**4. Reutilização estrutural**

Copiar partes do personagem entre frames mantendo sua editabilidade.

**5. Operações em lote**

Executar várias ações em uma única chamada.

**6. Histórico transacional**

Agrupar alterações logicamente relacionadas.

**7. Renderização seletiva**

Permitir que o Claude inspecione pequenas regiões, não apenas imagens completas.

**8. Cache de composição**

Evitar recompor frames inteiros quando apenas uma região mudou.

**9. Templates estruturais opcionais**

Permitir criar templates próprios de poses e estruturas, sem depender deles para gerar arte.

O Claude não deverá ficar restrito a um único método de construção.

---

# 12. Editor visual

Desenvolver uma interface moderna, simples e profissional.

A aplicação deve transmitir a sensação de um software de criação artística, não de um painel administrativo.

Usar uma identidade visual própria, preferencialmente dark.

## Layout

### Barra superior
- Novo projeto.
- Abrir.
- Salvar.
- Importar.
- Exportar.
- Undo/redo.
- Configurações.
- Status MCP.

### Ferramentas à esquerda
- Lápis.
- Borracha.
- Preenchimento.
- Linha.
- Retângulo.
- Elipse.
- Seleção.
- Conta-gotas.
- Transformação.

### Canvas central
- Grid.
- Zoom nearest-neighbor.
- Transparência quadriculada.
- Pan.
- Guias.
- Preview.
- Referências.
- Onion skin.

### Painel direito
- Layers.
- Paletas.
- Propriedades.
- Regiões.
- Anchors.
- Inspeção.

### Timeline inferior
- Frames.
- Duração.
- Playback.
- Tags.
- Direções.
- Onion skin.
- Controle de loops.

### Painel MCP

Criar um painel discreto onde o usuário poderá visualizar:

- Estado da conexão.
- Operações recentes executadas pelo Claude.
- Projeto atual.
- Ferramentas disponíveis.
- Erros.
- Histórico de alterações.

Permitir observar em tempo real a construção realizada pelo agente.

A interface deverá atualizar automaticamente quando o MCP alterar o projeto.

---

# 13. Formato de projeto

Criar um formato nativo chamado provisoriamente `.spriteforge`.

O formato deverá ser versionado e documentado.

Armazenar:

- Pixels.
- Layers.
- Cels.
- Frames.
- Animações.
- Paletas.
- Direções.
- Anchors.
- Regiões.
- Metadados.
- Configurações de exportação.

Utilizar manifesto JSON e dados binários compactados quando apropriado.

Implementar:

- Salvamento atômico.
- Autosave.
- Recuperação de falhas.
- Compatibilidade de versões.
- Importação e exportação.
- Testes de integridade.

O projeto salvo deverá ser completamente editável.

---

# 14. Exportação de Sprite Sheets

Permitir exportar animações em diferentes layouts.

### Configurações
- Horizontal.
- Vertical.
- Grid.
- Quantidade de colunas.
- Quantidade de linhas.
- Espaçamento.
- Margens.
- Fundo transparente.
- Tamanho de célula.
- Alinhamento por pivô.
- Ordem de animação.
- Ordem de direção.

### Formatos
- PNG.
- Sequência PNG.
- GIF animado.
- JSON de metadados.
- Sprite atlas.
- Metadados de animação para Godot.

Garantir que os frames exportados mantenham posições e dimensões consistentes.

---

# 15. Controle de qualidade e testes

Implementar testes unitários e de integração desde o começo.

Testar:

- Precisão de pixels.
- Transparência.
- Desenho geométrico.
- Preenchimentos.
- Manipulação de regiões.
- Composição de layers.
- Edição de frames.
- Patches.
- Exportação.
- Undo/redo.
- Persistência.
- Concorrência.
- Ferramentas MCP.

Criar testes determinísticos comparando imagens geradas com resultados esperados.

Testar também o funcionamento headless.

Nenhuma ferramenta MCP deverá retornar sucesso sem executar e validar sua operação correspondente.

---

# 16. Desenvolvimento incremental

Desenvolver o projeto em etapas verificáveis.

## Etapa 1 — Motor de pixels

Entregar:

- Workspace Rust.
- Modelo de projeto.
- Canvas RGBA.
- Paletas.
- Operações individuais.
- Operações em lote.
- Renderização PNG.
- Testes.

**Critério:** gerar uma imagem exclusivamente utilizando comandos do motor.

## Etapa 2 — MCP funcional

Entregar:

- Servidor rmcp.
- Transporte stdio.
- Criação de projetos.
- Manipulação de pixels.
- Desenho de formas.
- Renderização e inspeção.
- Salvamento.

**Critério:** o Claude Code deverá conseguir criar e salvar um sprite do zero através de MCP.

Priorizar essa etapa antes de desenvolver toda a interface, pois ela valida a principal inovação do projeto.

## Etapa 3 — Editor Tauri

Entregar:

- Interface moderna.
- Canvas interativo.
- Ferramentas manuais.
- Layers.
- Paletas.
- Histórico.
- Integração com runtime compartilhado.

**Critério:** alterações feitas pelo Claude deverão aparecer no editor e vice-versa.

## Etapa 4 — Animações

Entregar:

- Timeline.
- Frames.
- Cels.
- Animações.
- Onion skin.
- Playback.
- Comparação visual.

**Critério:** o Claude deverá conseguir criar uma animação de caminhada com 8 frames distintos.

## Etapa 5 — Construção assistida

Entregar:

- Regiões semânticas.
- Anchors.
- Ferramentas estruturais.
- Patches diferenciais.
- Operações de transformação.
- Ferramentas de validação.

**Critério:** o Claude deverá conseguir animar um personagem existente, alterando suas poses e mantendo características visuais.

## Etapa 6 — Múltiplas direções

Entregar:

- Sistema de 8 direções.
- Vistas personalizadas.
- Referências entre direções.
- Organização de animações por direção.
- Exportação multidirecional.

## Etapa 7 — Finalização

Entregar:

- Otimizações.
- Documentação completa.
- Exemplos.
- Testes de integração.
- Instalador Windows.
- Publicação open-source.

---

# 17. Demonstrações obrigatórias

## Teste A — Criação do zero

Solicitação ao Claude:

"Crie um pequeno guerreiro medieval em pixel art 64x64, usando uma paleta limitada, armadura azul, espada e escudo."

Resultado esperado:

- Projeto editável.
- Sprite construído por operações MCP.
- Layers organizadas.
- Paleta definida.
- PNG válido.

## Teste B — Animação

Solicitação:

"Utilizando o personagem criado, desenvolva uma animação de caminhada para a direita com 8 frames."

Resultado esperado:

- Oito frames.
- Poses realmente diferentes.
- Movimento dos membros.
- Consistência do personagem.
- Playback funcional.
- Sprite sheet exportada.

## Teste C — Importação e animação

Solicitação:

"Importe este sprite PNG, preserve sua aparência e crie uma animação idle com 6 frames."

Resultado esperado:

- Importação.
- Preservação do sprite original.
- Construção de novos frames.
- Edição pixel por pixel.
- Exportação.

## Teste D — Oito direções

Solicitação:

"Crie as oito direções de caminhada do personagem, mantendo a mesma identidade visual."

Resultado esperado:

- Oito conjuntos direcionais.
- Frames editáveis.
- Estrutura organizada.
- Sprite sheets.
- Metadados.

## Teste E — Sincronização

Abrir o Pixel Studio Maker e conectar o Claude Code.

Solicitar:

"Modifique a cor da armadura, aumente o escudo e corrija os três primeiros frames."

As alterações deverão aparecer no editor sem recarregar o projeto manualmente.

Em seguida, editar um pixel manualmente e verificar que o MCP consegue ler o novo estado.

---

# 18. Restrições obrigatórias

Não utilizar serviços externos para geração de imagens.

Não integrar ferramentas prontas como Aseprite ou PixelLab.

Não criar um editor cujo MCP apenas simula ações do mouse.

Não implementar a automação por screenshots e cliques na interface.

Não armazenar pixels exclusivamente no frontend.

Não duplicar a lógica de edição entre Rust e TypeScript.

Não implementar ferramentas MCP que retornem resultados fictícios.

Não declarar um módulo concluído sem testes.

Não depender da interface visual para executar operações MCP.

Não criar uma arquitetura monolítica difícil de expandir.

Não confundir transformações simples com animação artística real.

---

# 19. Critério de sucesso

O projeto será considerado funcional quando o seguinte cenário for executável:

Um desenvolvedor abre o Claude Code e solicita:

"Crie um personagem de pixel art 64x64. Ele será um guerreiro medieval. Gere as animações idle, walk e attack em oito direções. Mantenha a identidade visual entre os frames. Exporte as sprite sheets para Godot."

O Claude deverá utilizar exclusivamente as ferramentas MCP do Pixel Studio Maker para construir o projeto.

Nenhum arquivo de imagem deverá ser obtido de um serviço generativo externo.

O resultado deverá ser:
- Visualizável.
- Editável.
- Animável.
- Exportável.
- Reproduzível.
- Organizado em um projeto nativo.

A consistência estética e a qualidade das animações deverão ser avaliadas por inspeção visual e testes, não presumidas apenas porque os arquivos foram gerados corretamente.

---

# 20. Instrução inicial de execução

Comece analisando a arquitetura proposta e definindo os contratos fundamentais entre o motor de pixels, o runtime e o servidor MCP.

Em seguida, implemente imediatamente a primeira versão funcional.

Sua primeira entrega deverá conter:

1. Workspace Rust compilável.
2. Modelo de projeto e canvas.
3. Motor de manipulação de pixels.
4. Sistema de paletas.
5. Comandos de desenho em lote.
6. Exportação PNG.
7. Servidor MCP com ferramentas reais.
8. Testes automatizados.
9. Exemplo de configuração para Claude Code.
10. Demonstração de criação de um sprite exclusivamente via MCP.

Evite começar com uma interface visual elaborada antes de validar a arquitetura de geração.

**Prioridade absoluta: provar que o Claude consegue criar, visualizar, corrigir e salvar uma pixel art através do MCP.**

Depois dessa validação, evolua progressivamente para o editor visual completo e para o motor de animação.

O objetivo final é criar uma ferramenta profissional de produção de pixel art em que o Claude atue como artista e animador através de comandos programáticos, e o SpriteForge forneça a infraestrutura gráfica necessária para materializar cada pixel de cada frame.