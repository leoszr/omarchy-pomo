# Plano — Omarchy Pomodoro TUI (`omarchy-pomo`)

## Contexto atual

Criar um aplicativo Pomodoro em Rust para Omarchy/Hyprland que funcione como utilitário de sistema: um daemon mantém o timer em segundo plano, a Waybar exibe o tempo restante e uma TUI flutuante permite controlar as sessões.

Objetivo do MVP: entregar um Pomodoro simples, estável e integrado ao desktop Linux, sem ciclos automáticos complexos nem recursos de estatísticas avançadas.

Estado do projeto: as Sprints 1 e 2 implementaram base de domínio, caminhos locais, persistência em `state.json`, lógica de timer por timestamp e CLI local para start/status/pause/resume/stop. Ainda não há daemon, histórico, Waybar ou TUI. A implementação deve seguir as próximas sprints definidas em `docs/SPRINTS.md`.

Fluxo desejado:

```text
Waybar custom module
  └── pomo status --waybar
        ↓ clique
Terminal flutuante Hyprland
  └── pomo tui
        ↓ IPC
Daemon Pomodoro
  ├── estado do timer
  ├── histórico
  ├── notificação
  └── som
```

A fonte de verdade deve ser sempre o daemon. A TUI e a Waybar apenas consultam ou enviam comandos via IPC.

---

## Plano atual

O backlog executável está dividido em sprints autônomas em `docs/SPRINTS.md`. Cada sprint possui tasks, testes automatizados, critérios de aceite e documentação obrigatória ao final.

### Arquitetura recomendada

Implementar um binário Rust único chamado `pomo` com subcomandos via `clap`:

```bash
pomo daemon
pomo status
pomo status --waybar
pomo tui
pomo start --profile 25-5
pomo start --profile 30-10
pomo start --break
pomo start --custom 45 --type focus
pomo pause
pomo resume
pomo stop
pomo history
```

O daemon deve expor um Unix Socket em:

```bash
~/.local/state/omarchy-pomo/pomo.sock
```

Clientes CLI/TUI/Waybar devem falar com o daemon usando mensagens JSON simples.

Exemplo de request:

```json
{ "command": "status" }
```

Exemplo de response:

```json
{
  "status": "running",
  "session_type": "focus",
  "label": "25/5 Focus",
  "remaining_secs": 1122,
  "duration_secs": 1500
}
```

### Estados do timer

Estados mínimos:

```text
Idle
Running
Paused
Finished
```

Tipos de sessão:

```text
Focus
ShortBreak
Custom
```

O timer não deve depender de decremento por segundo. O tempo restante deve ser calculado por horário:

```text
remaining = duration - (now - started_at)
```

Ao pausar, gravar `paused_remaining_secs`. Ao retomar, recriar `started_at` usando o restante salvo como nova duração operacional.

### Presets do MVP

```text
25/5      -> foco de 25 min; break manual sugerido de 5 min
30/10     -> foco de 30 min; break manual sugerido de 10 min
Break     -> pausa manual padrão de 5 min
Custom    -> tempo em minutos + tipo focus/break
```

Para o MVP, ao terminar uma sessão de foco, notificar e aguardar ação manual. Não iniciar break automaticamente.

### Persistência

Usar arquivos locais do usuário:

```bash
~/.local/state/omarchy-pomo/state.json
~/.local/state/omarchy-pomo/history.jsonl
~/.local/state/omarchy-pomo/pomo.sock
~/.config/omarchy-pomo/config.toml        # reservado para evolução futura
~/.config/omarchy-pomo/done.ogg           # som opcional do usuário
```

Exemplo de `state.json`:

```json
{
  "status": "running",
  "session_type": "focus",
  "label": "25/5 Focus",
  "duration_secs": 1500,
  "started_at": "2026-05-04T14:30:00-03:00",
  "paused_remaining_secs": null
}
```

Exemplo de `history.jsonl`:

```json
{"date":"2026-05-04","type":"focus","label":"25/5","duration_secs":1500,"completed":true,"finished_at":"2026-05-04T15:00:00-03:00"}
```

### Waybar

Implementar `pomo status --waybar` retornando JSON compatível com módulo customizado da Waybar:

```json
{
  "text": "󰔟 18:42",
  "tooltip": "Pomodoro: foco 25/5",
  "class": "running"
}
```

Classes sugeridas:

```text
idle
running
paused
break
finished
error
```

Ícones sugeridos:

```text
idle      -> 󰔟
running   -> 󰔟 18:42
paused    -> 󰏤 18:42
break     -> ☕ 04:22
finished  ->  pronto
error     ->  daemon
```

Exemplo de Waybar:

```jsonc
"custom/pomodoro": {
  "exec": "pomo status --waybar",
  "return-type": "json",
  "interval": 1,
  "on-click": "kitty --class omarchy-pomo -e pomo tui",
  "tooltip": true
}
```

### Hyprland

A TUI deve abrir em terminal com classe `omarchy-pomo`.

Exemplo:

```conf
windowrulev2 = float, class:^(omarchy-pomo)$
windowrulev2 = center, class:^(omarchy-pomo)$
windowrulev2 = size 700 420, class:^(omarchy-pomo)$
```

### TUI

Usar `ratatui` + `crossterm`.

A TUI deve mostrar:

- status atual;
- tempo restante;
- barra de progresso;
- sessão atual;
- presets;
- controles;
- resumo simples do histórico diário.

Atalhos mínimos:

```text
1 = iniciar foco 25 min
2 = iniciar foco 30 min
3 = iniciar break padrão
4 = tempo customizado
p = pause/resume
s = stop/reset
h = histórico
q = fechar TUI sem parar o timer
```

Fechar a TUI nunca deve parar o timer.

### Notificação e som

Ao finalizar uma sessão, o daemon deve:

1. registrar histórico se a sessão foi concluída;
2. enviar notificação com `notify-send`;
3. tocar som com comando externo.

Comandos sugeridos:

```bash
notify-send "Pomodoro finalizado" "Hora de fazer uma pausa."
paplay ~/.config/omarchy-pomo/done.ogg
```

Fallback aceitável para som:

```bash
mpv --no-video ~/.config/omarchy-pomo/done.ogg
```

Para o MVP, não implementar áudio nativo dentro do binário.

---

## Files to modify

Como o repositório ainda contém apenas o plano, a implementação deve criar a estrutura abaixo:

```text
Cargo.toml
README.md
src/main.rs
src/cli.rs
src/daemon.rs
src/ipc.rs
src/state.rs
src/timer.rs
src/history.rs
src/notify.rs
src/waybar.rs
src/tui/mod.rs
src/tui/app.rs
src/tui/ui.rs
src/tui/events.rs
assets/done.ogg                  # opcional; pode ser documentado em vez de versionado
examples/waybar.jsonc
examples/hyprland.conf
```

Responsabilidades:

- `main.rs`: ponto de entrada e roteamento dos comandos.
- `cli.rs`: definição da CLI com `clap`.
- `daemon.rs`: loop do daemon, socket, finalização de sessões.
- `ipc.rs`: protocolo JSON e cliente Unix Socket.
- `state.rs`: structs de estado, caminhos e persistência.
- `timer.rs`: cálculo de tempo restante e transições.
- `history.rs`: escrita/leitura de JSONL e resumo diário.
- `notify.rs`: `notify-send`, `paplay`/`mpv`.
- `waybar.rs`: formatação JSON para Waybar.
- `tui/*`: estado local da UI, renderização e eventos.

---

## Reuse

Não há código existente no repositório além deste `PLAN.md`. A implementação deve reaproveitar bibliotecas consolidadas do ecossistema Rust:

```toml
[dependencies]
ratatui = "..."
crossterm = "..."
serde = { version = "...", features = ["derive"] }
serde_json = "..."
chrono = { version = "...", features = ["serde"] }
clap = { version = "...", features = ["derive"] }
dirs = "..."
tokio = { version = "...", features = ["full"] }
anyhow = "..."
thiserror = "..."
```

Uso esperado:

- `ratatui`: layout e widgets da TUI.
- `crossterm`: input de teclado e controle do terminal.
- `serde`/`serde_json`: protocolo IPC, estado e histórico.
- `chrono`: datas, horários e serialização temporal.
- `clap`: comandos CLI.
- `dirs`: diretórios locais do usuário.
- `tokio`: daemon assíncrono e Unix Socket.
- `anyhow`/`thiserror`: tratamento de erros limpo.

---

## Steps

- [x] Criar projeto Rust, `Cargo.toml`, módulos principais e CLI inicial com `clap`.
- [x] Definir modelos base: `TimerStatus`, `SessionType` e `TimerState`.
- [x] Implementar resolução de caminhos em `~/.local/state/omarchy-pomo` e criação segura dos diretórios necessários.
- [x] Implementar persistência de `state.json` e leitura de estado inicial `Idle` quando não existir arquivo.
- [x] Implementar lógica de timer em `timer.rs`: start, pause, resume, stop, finish e cálculo por timestamp.
- [x] Implementar `pomo status` usando estado local.
- [ ] Implementar daemon com Unix Socket e protocolo JSON para `STATUS`, `START`, `PAUSE`, `RESUME`, `STOP` e `HISTORY`.
- [ ] Fazer os subcomandos CLI conversarem com o daemon via `ipc.rs`.
- [ ] Implementar detecção de sessão finalizada no daemon, com transição para `Finished` apenas uma vez.
- [ ] Implementar notificação e som em `notify.rs`, tolerando ausência de `notify-send`, `paplay`, `mpv` ou arquivo de som.
- [ ] Implementar histórico JSONL apenas para sessões concluídas com sucesso.
- [ ] Implementar `pomo history` com resumo do dia: sessões de foco, tempo focado e pausas concluídas.
- [ ] Implementar `pomo status --waybar` com JSON válido, classes por estado e saída de erro útil se o daemon estiver indisponível.
- [ ] Implementar TUI com Ratatui: tela principal, atualização periódica por status, atalhos e histórico básico.
- [ ] Implementar fluxo de tempo customizado na TUI de forma simples, por exemplo modal/input numérico.
- [ ] Criar exemplos de configuração para Waybar e Hyprland.
- [ ] Atualizar `README.md` com instalação, execução do daemon, módulo Waybar, regras Hyprland e comandos principais.

---

## Verification

### Verificação automatizada

Executar durante o desenvolvimento:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Adicionar testes unitários para:

- cálculo de tempo restante;
- pausa e retomada;
- finalização idempotente;
- serialização/deserialização de estado;
- parsing e resumo do histórico;
- formatação JSON da Waybar.

### Verificação manual do MVP

Fluxo final esperado:

```text
1. Rodar `pomo daemon`.
2. Rodar `pomo status` e confirmar estado idle.
3. Rodar `pomo start --profile 25-5`.
4. Confirmar que `pomo status` mostra tempo restante.
5. Confirmar que `pomo status --waybar` retorna JSON válido.
6. Rodar `pomo pause` e confirmar estado paused.
7. Rodar `pomo resume` e confirmar contagem.
8. Abrir `pomo tui` e controlar o timer pela interface.
9. Fechar a TUI e confirmar que o timer continua rodando.
10. Aguardar uma sessão curta customizada terminar.
11. Confirmar notificação, som e entrada em `history.jsonl`.
12. Confirmar que `pomo history` mostra resumo do dia.
13. Configurar Waybar e Hyprland com os exemplos.
14. Clicar na Waybar e confirmar abertura da TUI flutuante.
```

### Critérios de aceitação

- O daemon é a única fonte de verdade do timer.
- Waybar nunca escreve estado diretamente.
- TUI nunca mantém o timer por conta própria.
- Fechar a TUI não interrompe a sessão.
- Sessões finalizadas são registradas apenas uma vez.
- Se o daemon estiver indisponível, CLI/Waybar retornam erro amigável.
- O MVP não inclui ciclos automáticos, long break inteligente, SQLite, calendário, tarefas, tray ou sincronização.

---

## Fora do escopo do MVP

Avaliar apenas após o MVP:

- systemd user service para iniciar daemon automaticamente;
- break automático após foco;
- ciclos completos de Pomodoro;
- long break após N sessões;
- presets configuráveis via TOML;
- temas da TUI;
- estatísticas semanais/mensais;
- streaks;
- integração com tarefas/calendário;
- pacote AUR ou script de instalação;
- app tray;
- múltiplos timers simultâneos.
