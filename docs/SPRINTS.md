# Sprints — Omarchy Pomo

Este documento quebra o `docs/PLAN.md` em sprints pequenas o bastante para desenvolvimento autônomo. Cada sprint deve terminar com código testado, critérios de aceite verificados e documentação atualizada.

## Definição de pronto por sprint

Antes de considerar qualquer sprint concluída:

- `cargo fmt` executado sem diffs pendentes.
- `cargo clippy -- -D warnings` sem warnings.
- `cargo test` passando.
- Critérios de aceite da sprint verificados.
- Docs atualizados no mesmo ciclo de trabalho.
- Decisões novas registradas em `docs/PLAN.md`, `docs/SPRINTS.md`, `README.md` ou exemplos relevantes.

## Sprint 1 — Base de domínio, caminhos e persistência

Status: concluída.

### Objetivo

Trocar o protótipo atual por uma base testável: modelos do timer, resolução de caminhos e persistência local do estado.

### Tasks

- Criar módulos `state.rs` e ajustar `timer.rs` para conter lógica pura.
- Definir enums/structs:
  - `TimerStatus`: `Idle`, `Running`, `Paused`, `Finished`.
  - `SessionType`: `Focus`, `ShortBreak`, `Custom`.
  - `TimerState` com status, tipo, label, duração, início e restante pausado.
- Adicionar dependências mínimas: `serde`, `serde_json`, `chrono`, `dirs`, `anyhow`, `thiserror` se necessário.
- Implementar resolução de caminhos:
  - `~/.local/state/omarchy-pomo/state.json`
  - `~/.local/state/omarchy-pomo/history.jsonl`
  - `~/.local/state/omarchy-pomo/pomo.sock`
- Criar diretórios necessários com segurança.
- Implementar leitura/gravação de `state.json`.
- Retornar estado `Idle` quando `state.json` não existir.
- Manter CLI compilando com comandos existentes.

### Testes automatizados

- Serialização/deserialização de `TimerState`.
- Leitura retorna `Idle` quando arquivo não existe.
- Escrita seguida de leitura preserva campos do estado.
- Resolução de caminhos respeita diretório base injetável em teste.

### Critérios de aceite

- O projeto compila sem depender de daemon.
- Estado inicial é determinístico e testável.
- Nenhum teste escreve em diretórios reais do usuário.
- Docs explicam onde os arquivos locais ficam.

### Docs a atualizar ao fim

- `docs/PLAN.md`: marcar base/persistência como implementada.
- `README.md`: criar seção inicial de instalação, comandos atuais e arquivos locais.

## Sprint 2 — Lógica do timer e CLI local

Status: concluída.

### Objetivo

Implementar transições do timer e comandos CLI funcionando localmente via `state.json`, antes do daemon.

### Tasks

- Implementar cálculo por timestamp: `remaining = duration - elapsed`.
- Implementar funções puras para `start`, `pause`, `resume`, `stop`, `finish_if_due`.
- Expandir CLI:
  - `pomo start --profile 25-5`
  - `pomo start --profile 30-10`
  - `pomo start --break`
  - `pomo start --custom <minutos> --type <focus|break>`
  - `pomo status`
  - `pomo pause`
  - `pomo resume`
  - `pomo stop`
- Formatar saída humana simples para `status`.
- Validar entradas de tempo customizado.

### Testes automatizados

- Cálculo de restante para sessão em execução.
- Restante nunca fica negativo.
- Pause grava `paused_remaining_secs`.
- Resume recria `started_at` preservando restante.
- Stop retorna para `Idle`.
- Finish é idempotente.
- Parsing dos argumentos principais da CLI.

### Critérios de aceite

- É possível iniciar, consultar, pausar, retomar e parar uma sessão sem daemon.
- Tempo restante é baseado em relógio, não em loop por segundo.
- Finalização não duplica transição.
- Erros de CLI são claros para valores inválidos.

### Docs a atualizar ao fim

- `README.md`: comandos suportados e exemplos.
- `docs/PLAN.md`: atualizar estado real do MVP.

## Sprint 3 — Histórico JSONL e resumo diário

Status: concluída.

### Objetivo

Registrar sessões concluídas e exibir resumo diário via CLI.

### Tasks

- Criar `history.rs`.
- Definir `HistoryEntry`.
- Escrever em `history.jsonl` apenas quando sessão concluir com sucesso.
- Implementar resumo diário:
  - sessões de foco concluídas;
  - tempo total focado;
  - pausas concluídas.
- Implementar `pomo history`.
- Garantir idempotência: mesma sessão finalizada só registra uma vez.

### Testes automatizados

- Parsing de JSONL válido.
- Linhas inválidas são tratadas sem quebrar resumo, ou erro documentado.
- Resumo do dia soma foco e pausas corretamente.
- Sessão parada manualmente não entra como concluída.
- Finalização repetida não duplica histórico.

### Critérios de aceite

- `history.jsonl` é append-only.
- `pomo history` mostra resumo do dia atual.
- Histórico usa data local.
- Histórico não é escrito para `stop` manual.

### Docs a atualizar ao fim

- `README.md`: comando `history` e formato do arquivo.
- `docs/PLAN.md`: critérios de histórico atualizados.

## Sprint 4 — IPC e daemon como fonte de verdade

Status: concluída.

### Objetivo

Mover a fonte de verdade para o daemon via Unix Socket e fazer CLI conversar por IPC.

### Tasks

- Criar `ipc.rs` com requests/responses JSON.
- Criar `daemon.rs` com Unix Socket em `pomo.sock`.
- Implementar comandos IPC:
  - `status`
  - `start`
  - `pause`
  - `resume`
  - `stop`
  - `history`
- Fazer `pomo daemon` iniciar loop do daemon.
- Ajustar CLI para usar daemon quando disponível.
- Definir fallback amigável quando daemon estiver indisponível.
- Remover socket antigo no startup quando seguro.

### Testes automatizados

- Serialização/deserialização de request/response.
- Handler de comando altera estado esperado.
- Socket path usa diretório de estado.
- Erro amigável quando socket não existe.

### Critérios de aceite

- Daemon é a fonte de verdade.
- CLI não escreve estado diretamente quando usa daemon.
- `pomo status` funciona via IPC com daemon rodando.
- Daemon indisponível retorna erro compreensível.

### Docs a atualizar ao fim

- `README.md`: como iniciar daemon e usar CLI.
- `docs/PLAN.md`: arquitetura real do daemon.

## Sprint 5 — Waybar JSON

Status: concluída.

### Objetivo

Entregar integração consumível pela Waybar.

### Tasks

- Criar `waybar.rs`.
- Implementar `pomo status --waybar`.
- Gerar JSON com `text`, `tooltip`, `class`.
- Mapear classes:
  - `idle`
  - `running`
  - `paused`
  - `break`
  - `finished`
  - `error`
- Criar `examples/waybar.jsonc`.

### Testes automatizados

- JSON gerado é válido.
- Classe correta para cada estado/tipo.
- Formatação `MM:SS` correta.
- Erro de daemon gera JSON classe `error`.

### Critérios de aceite

- `pomo status --waybar` sempre imprime JSON válido.
- Waybar não escreve estado.
- JSON tem classe útil para styling.
- Exemplo pode ser copiado para config da Waybar.

### Docs a atualizar ao fim

- `README.md`: seção Waybar.
- `examples/waybar.jsonc`: exemplo atualizado.

## Sprint 6 — Notificação e som

Status: concluída.

### Objetivo

Notificar o fim de sessão e tocar som opcional sem quebrar em ambientes sem áudio/notificação.

### Tasks

- Criar `notify.rs`.
- Rodar `notify-send` ao finalizar sessão.
- Tocar `~/.config/omarchy-pomo/done.ogg` com `paplay` ou `mpv` se existir.
- Tolerar ausência dos comandos externos.
- Integrar chamada no daemon ao detectar finalização.

### Testes automatizados

- Construção dos comandos externos esperados.
- Ausência de arquivo de som não gera erro fatal.
- Finalização chama notificação uma única vez via interface mockável.

### Critérios de aceite

- Sessão concluída gera tentativa de notificação.
- Som é opcional.
- Falhas de `notify-send`, `paplay` ou `mpv` não derrubam daemon.
- Histórico continua sendo registrado mesmo sem som.

### Docs a atualizar ao fim

- `README.md`: dependências opcionais e som customizado.
- `docs/PLAN.md`: comportamento real de notificação/som.

## Sprint 7 — TUI Ratatui mínima

Status: concluída.

### Objetivo

Criar TUI funcional para controlar o daemon sem manter timer próprio.

### Tasks

- Adicionar `ratatui` e `crossterm`.
- Criar `src/tui/mod.rs`, `app.rs`, `ui.rs`, `events.rs`.
- Implementar tela com:
  - status;
  - tempo restante;
  - barra de progresso;
  - sessão atual;
  - atalhos.
- Implementar atalhos:
  - `1`: foco 25 min;
  - `2`: foco 30 min;
  - `3`: break 5 min;
  - `p`: pause/resume;
  - `s`: stop;
  - `q`: sair sem parar timer.
- Atualizar status periodicamente via IPC.

### Testes automatizados

- Estado local da TUI interpreta responses corretamente.
- Eventos de tecla geram comandos IPC esperados.
- `q` não envia stop.

### Critérios de aceite

- `pomo tui` abre e controla daemon.
- Fechar TUI não para o timer.
- TUI não calcula tempo como fonte de verdade; apenas exibe status do daemon.
- Tela continua usável se daemon cair, mostrando erro.

### Docs a atualizar ao fim

- `README.md`: uso da TUI e atalhos.
- `docs/PLAN.md`: estado da TUI.

## Sprint 8 — Custom time na TUI e exemplos Omarchy/Hyprland

### Objetivo

Completar fluxo interativo e documentação de integração desktop.

### Tasks

- Implementar tecla `4` para tempo customizado simples.
- Permitir escolher tipo `focus` ou `break` no fluxo customizado.
- Criar `examples/hyprland.conf`.
- Documentar terminal flutuante com classe `omarchy-pomo`.
- Revisar UX mínima da TUI.

### Testes automatizados

- Input customizado aceita minutos válidos.
- Input customizado rejeita zero, negativos e texto inválido.
- Evento customizado gera request correta.

### Critérios de aceite

- Usuário inicia sessão customizada pela TUI.
- Exemplo Hyprland abre janela flutuante centralizada.
- Integração Waybar + clique + TUI está documentada.

### Docs a atualizar ao fim

- `README.md`: integração Omarchy/Hyprland completa.
- `examples/hyprland.conf`: exemplo final.

## Sprint 9 — Hardening MVP e release local

### Objetivo

Fechar MVP com revisão de qualidade, docs completas e build release.

### Tasks

- Revisar erros e mensagens da CLI.
- Rodar verificação manual do fluxo completo.
- Corrigir flakes e edge cases.
- Garantir `cargo build --release`.
- Criar troubleshooting básico.
- Validar que fora de escopo não entrou no MVP.

### Testes automatizados

- Rodar suíte completa.
- Adicionar testes de regressão para bugs encontrados na verificação manual.

### Critérios de aceite

- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` e `cargo build --release` passam.
- Fluxo manual do `docs/PLAN.md` passa.
- README permite instalar e usar o MVP sem ler código.
- Docs refletem exatamente o que foi implementado.

### Docs a atualizar ao fim

- `README.md`: versão MVP final.
- `docs/PLAN.md`: status final e próximos passos.
- `docs/SPRINTS.md`: marcar sprints concluídas ou pendências.
