---
name: Omarchy Pomo
description: Timer de foco silencioso, nativo do tema ativo do Omarchy.
colors:
  terminal-background: "#f2f2f2"
  terminal-foreground: "#1a1a1a"
  terminal-muted: "#8a8a8a"
  terminal-primary: "#005f87"
  terminal-break: "#7a3f7a"
  terminal-success: "#006d00"
  terminal-warning: "#6f5200"
  terminal-error: "#9d0006"
typography:
  timer:
    fontFamily: "CaskaydiaMono Nerd Font, monospace"
    fontSize: "terminal-native"
    fontWeight: 700
    lineHeight: 1
  body:
    fontFamily: "CaskaydiaMono Nerd Font, monospace"
    fontSize: "terminal-native"
    fontWeight: 400
    lineHeight: 1
  label:
    fontFamily: "CaskaydiaMono Nerd Font, monospace"
    fontSize: "terminal-native"
    fontWeight: 600
    lineHeight: 1
spacing:
  compact: "1 cell"
  standard: "2 cells"
  relaxed: "3 cells"
components:
  timer-active:
    textColor: "{colors.terminal-primary}"
    typography: "{typography.timer}"
    padding: "2 cells"
  timer-break:
    textColor: "{colors.terminal-break}"
    typography: "{typography.timer}"
    padding: "2 cells"
  error-message:
    textColor: "{colors.terminal-error}"
    typography: "{typography.body}"
    padding: "1 cell"
---

# Design System: Omarchy Pomo

## Overview

**Creative North Star: "O mostrador que pertence ao desktop"**

Omarchy Pomo parece uma extensão natural do terminal, não um aplicativo pintado
por cima dele. O fundo, a fonte e a identidade cromática vêm do tema ativo do
Omarchy. No tema Corporate atual, isso significa PaperColor Light: superfície
clara, texto quase preto e azul sóbrio. Em outro tema, os mesmos papéis são
ocupados pelos slots ANSI equivalentes.

O cronômetro é o centro óptico. Estado, nome da sessão, progresso, resumo diário
e comandos aparecem em ordem decrescente de importância. A tela usa um único
contorno externo e separadores internos; não usa uma grade de cartões. Mudanças
de estado são imediatas, sem animação decorativa.

**Key Characteristics:**

- Cronômetro grande, central e legível à primeira vista.
- Paleta adaptativa baseada nas cores ANSI do terminal.
- Uma moldura, poucos separadores e amplo espaço vazio.
- Ações contextuais, mostrando primeiro o que pode ser feito agora.
- Três layouts por capacidade: compacto, padrão e amplo.

**The Native Theme Rule.** Nunca definir fundo RGB na TUI. Usar `Color::Reset`
para preservar a superfície do terminal e cores ANSI semânticas para acompanhar
o tema ativo.

**The One Glance Rule.** Tempo restante, estado e ação principal devem ser
compreendidos sem percorrer a tela inteira.

## Colors

A paleta normativa registra o tema Corporate atual. A implementação deve mapear
papéis para ANSI, permitindo que o terminal substitua esses valores quando o
usuário trocar de tema.

### Primary

- **Azul Corporate:** ação principal, sessão de foco, cursor do formulário e
  trecho concluído do progresso. Mapear para `Color::Blue`.

### Secondary

- **Ameixa de Pausa:** sessões de descanso e seu progresso. Mapear para
  `Color::Magenta`.
- **Verde de Conclusão:** confirmação curta de sessão concluída. Mapear para
  `Color::Green`.

### Neutral

- **Papel do Terminal:** fundo herdado; nunca pintar por widget.
- **Tinta Corporate:** conteúdo principal herdado com `Color::Reset`.
- **Cinza Silencioso:** ajuda, bordas, progresso restante e histórico secundário.
  Mapear para `Color::DarkGray`.
- **Âmbar Operacional:** sessão pausada e avisos recuperáveis. Mapear para
  `Color::Yellow`.
- **Vermelho de Falha:** erros de daemon, IPC e validação. Mapear para
  `Color::Red`.

**The Ten Percent Rule.** Cor de estado ocupa no máximo o cronômetro, o rótulo de
estado e a parte preenchida do progresso. O restante permanece neutro.

**The Semantic Slot Rule.** Foco é azul; pausa é amarelo; descanso é magenta;
conclusão é verde; erro é vermelho. Nunca trocar esses papéis por preferência
decorativa.

## Typography

**Display Font:** CaskaydiaMono Nerd Font, herdada do terminal
**Body Font:** CaskaydiaMono Nerd Font, herdada do terminal
**Label/Mono Font:** CaskaydiaMono Nerd Font, herdada do terminal

**Character:** uma única fonte mono mantém alinhamento preciso e integração com
o desktop. A hierarquia vem de escala em células, peso, posição e contraste, não
de famílias tipográficas concorrentes.

### Hierarchy

- **Display** (bold, 5 linhas, line-height 1): cronômetro em dígitos block quando
  houver pelo menos 52 colunas e 18 linhas.
- **Headline** (bold, terminal-native): cronômetro textual `24:37` no layout
  compacto ou como fallback sem caracteres block.
- **Title** (bold, terminal-native): nome curto da sessão, uma linha, truncado com
  reticências quando necessário.
- **Body** (regular, terminal-native): resumo diário, mensagens e campos.
- **Label** (bold, terminal-native): estado e teclas de ação; sem caixa alta em
  frases completas.

**The Numeric Calm Rule.** O cronômetro usa largura estável e nunca muda de
posição quando os dígitos mudam.

**The One Font Rule.** Não introduzir fonte display, ASCII art ornamental ou
ícones que não existam na Nerd Font ativa.

## Elevation

A TUI é totalmente plana. Profundidade vem de espaço vazio, um contorno externo
e separadores horizontais discretos. Não existem sombras, fundos simulados,
bordas duplas ou painéis aninhados.

**The Single Frame Rule.** Uma única moldura contém a experiência. Formulário,
histórico e comandos são regiões da mesma superfície, não cartões independentes.

## Components

### Timer Stage

- Centralizar vertical e horizontalmente o estado, o tempo e o nome da sessão.
- Em execução, renderizar tempo em cinco linhas com dígitos block; em terminais
  menores, usar `MM:SS` em uma linha e peso bold.
- Mostrar estado como texto curto: `FOCO`, `PAUSA`, `DESCANSO`, `CONCLUÍDO` ou
  `PRONTO`. Cor reforça o texto, nunca o substitui.
- Não repetir `tipo`, `categoria` e `status` como campos técnicos.

### Progress Track

- Usar uma linha fina, sem bloco próprio: `━━━━━━━━━━━━╺────────`.
- Trecho concluído recebe a cor da sessão; restante usa cinza silencioso.
- Exibir porcentagem somente no layout amplo. O tempo já comunica o essencial.

### Context Actions

- Exibir apenas ações válidas para o estado atual.
- `Running`: `[p] Pausar  [s] Parar  [q] Sair`.
- `Paused`: `[p] Retomar  [s] Parar  [q] Sair`.
- `Idle` ou `Finished`: `[1] 25 min  [2] 30 min  [3] Pausa  [4] Personalizar`.
- Tecla fica bold; descrição fica neutra. A ação principal usa azul apenas quando
  seleção explícita for adicionada.

### Daily Summary

- Uma única linha discreta abaixo de um separador: `Hoje  3 focos · 1h15 · 2 pausas`.
- Omitir no layout compacto quando faltar altura; nunca competir com o timer.
- Quando indisponível, mostrar `Histórico indisponível` em cinza, sem skeleton ou
  indicador permanente.

### Custom Timer Form

- Substituir o corpo do timer sem abrir modal ou moldura aninhada.
- Estrutura: título `Nova sessão`, campo `Duração  [45__] min`, escolha
  `Tipo  [Foco]  Pausa` e ajuda `Enter iniciar · Esc voltar`.
- O campo ativo usa cursor e azul ANSI. A opção escolhida usa reverse video, não
  um fundo RGB fixo.
- Erro aparece logo abaixo do campo correspondente, em vermelho, preservando os
  valores já digitados.

### Errors and Connection States

- Erro transitório ocupa uma linha no rodapé e não apaga o último timer válido.
- Sem estado anterior, mostrar `Daemon indisponível` e a ação
  `omarchy-pomo daemon` em vez de permanecer indefinidamente em “Conectando”.
- A mensagem deve indicar fonte quando houver mais de uma falha: ação, status ou
  histórico.

### Responsive Layout

- **Compacto, abaixo de 52×16:** sem dígitos block, sem histórico, bordas simples
  e ações quebradas em duas linhas.
- **Padrão, de 52×16 até 84×23:** dígitos block quando houver altura, resumo em
  uma linha e comandos contextuais.
- **Amplo, a partir de 85×24:** manter o timer central; usar largura extra para
  detalhe de progresso e histórico, nunca para criar colunas de cartões.
- Abaixo de 32×10, renderizar fallback seguro: `24:37 · FOCO` e uma linha de
  comandos essenciais, sem panic.

## Do's and Don'ts

### Do:

- **Do** herdar fundo e foreground do terminal com `Color::Reset`.
- **Do** usar os slots ANSI para acompanhar automaticamente o tema do Omarchy.
- **Do** manter o cronômetro como elemento dominante em todos os layouts.
- **Do** mostrar somente ações válidas para o estado atual.
- **Do** testar explicitamente 32×10, 52×16, 70×24 e 100×30 células.
- **Do** manter rótulo textual junto de toda mudança de cor semântica.

### Don't:

- **Don't** criar interfaces gamificadas, com celebrações, pontuações ou estímulos constantes.
- **Don't** criar dashboards cheios de cartões e métricas secundárias.
- **Don't** usar estética de terminal técnico denso, com informação operacional desnecessária.
- **Don't** usar decoração neon, gradientes, bordas excessivas ou cor sem função.
- **Don't** fixar `Color::Green` para todo progresso; a cor depende do estado e da sessão.
- **Don't** repetir status, sessão, tipo e categoria como uma ficha de banco de dados.
- **Don't** deixar atalhos em uma frase longa quando podem ser ações contextuais.
