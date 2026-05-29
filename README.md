# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

Sprint 5 concluída: daemon via Unix Socket, CLI via IPC e JSON para Waybar.

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
```

Se o daemon não estiver rodando, a CLI retorna erro amigável pedindo `omarchy-pomo daemon`. Para `status --waybar`, a saída continua sendo JSON válido com classe `error`.

Ainda não há TUI implementada.

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
