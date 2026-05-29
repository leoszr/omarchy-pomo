# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

Sprint 2 concluída: timer local por timestamp e CLI local via `state.json`.

Comandos atuais:

```bash
cargo run -- start --profile 25-5
cargo run -- start --profile 30-10
cargo run -- start --break
cargo run -- start --custom 45 --type focus
cargo run -- start --custom 5 --type break
cargo run -- status
cargo run -- pause
cargo run -- resume
cargo run -- stop
```

Ainda não há daemon, histórico, Waybar ou TUI implementados.

## Arquivos locais

O estado local fica em:

```text
~/.local/state/omarchy-pomo/state.json
~/.local/state/omarchy-pomo/history.jsonl  # reservado para sprint futura
~/.local/state/omarchy-pomo/pomo.sock      # reservado para daemon futuro
```

Quando `state.json` não existe, o estado inicial é `Idle`.

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

Passa com 0 testes no protótipo atual.
