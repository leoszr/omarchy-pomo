# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

Sprint 1 concluída: base de domínio, resolução de caminhos e persistência local do estado.

Comandos atuais:

```bash
cargo run -- start   # inicia sessão foco 25/5 local e grava state.json
cargo run -- status  # lê state.json ou mostra Idle se não existir
cargo run -- task    # comando legado do protótipo
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
