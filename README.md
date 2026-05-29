# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

Sprint 7 concluída: daemon via Unix Socket, CLI via IPC, JSON para Waybar, notificação/som opcional e TUI Ratatui mínima.

Comandos atuais:

Em um terminal, inicie o daemon:

```bash
cargo run -- daemon
```

Em outro terminal, use a CLI:

```bash
cargo run -- start --profile 25-5
cargo run -- start --profile 30-10
cargo run -- start --break
cargo run -- start --custom 45 --type focus
cargo run -- start --custom 5 --type break
cargo run -- status
cargo run -- status --waybar
cargo run -- pause
cargo run -- resume
cargo run -- stop
cargo run -- history
cargo run -- tui
```

Se o daemon não estiver rodando, a CLI retorna erro amigável pedindo `omarchy-pomo daemon`. Para `status --waybar`, a saída continua sendo JSON válido com classe `error`.

## TUI

Com o daemon rodando:

```bash
cargo run -- tui
```

Atalhos:

```text
1 = foco 25 min
2 = foco 30 min
3 = break 5 min
p = pause/resume
s = stop
q = sair sem parar timer
```

A TUI consulta/controla o daemon via IPC. Fechar com `q` não para o timer.

## Notificação e som

Ao finalizar uma sessão, o daemon tenta executar:

```bash
notify-send "Pomodoro finalizado" "Sessão concluída: <label>"
```

Som opcional: crie o arquivo abaixo para tocar ao concluir:

```text
~/.config/omarchy-pomo/done.ogg
```

O daemon tenta `paplay` e depois `mpv --no-video`. Ausência de `notify-send`, `paplay`, `mpv` ou do arquivo de som não derruba o daemon.

## Waybar

Exemplo em [`examples/waybar.jsonc`](examples/waybar.jsonc):

```jsonc
{
  "custom/pomodoro": {
    "exec": "omarchy-pomo status --waybar",
    "return-type": "json",
    "interval": 1,
    "on-click": "kitty --class omarchy-pomo -e omarchy-pomo tui",
    "tooltip": true
  }
}
```

Classes emitidas: `idle`, `running`, `paused`, `break`, `finished`, `error`.

## Arquivos locais

O estado local fica em:

```text
~/.local/state/omarchy-pomo/state.json
~/.local/state/omarchy-pomo/history.jsonl
~/.local/state/omarchy-pomo/pomo.sock
```

Quando `state.json` não existe, o estado inicial é `Idle`.

`history.jsonl` é append-only. Cada linha registra uma sessão concluída:

```json
{"date":"2026-05-29","type":"focus","label":"25/5 Focus","duration_secs":1500,"completed":true,"finished_at":"2026-05-29T10:00:00-03:00"}
```

`stop` manual não escreve histórico. Linhas inválidas em `history.jsonl` são ignoradas no resumo.

## Desenvolvimento

Backlog por sprints: [`docs/SPRINTS.md`](docs/SPRINTS.md).

Plano arquitetural: [`docs/PLAN.md`](docs/PLAN.md).

Cada sprint só termina quando:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

passam, os critérios de aceite são verificados e a documentação é atualizada.

## Verificação atual

```bash
cargo test
```

Passa com a suíte atual.
