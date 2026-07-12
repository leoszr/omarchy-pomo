# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

MVP concluído: TUI com tempo customizado, daemon via Unix Socket, CLI via IPC, JSON para Waybar, notificação/som opcional, exemplos Waybar/Hyprland e build release verificado.

## Instalação local

```bash
cargo build --release
install -Dm755 target/release/omarchy-pomo ~/.local/bin/omarchy-pomo
```

Garanta que `~/.local/bin` esteja no `PATH`.

## Uso

Em um terminal, inicie o daemon:

```bash
omarchy-pomo daemon
```

Em outro terminal, use a CLI:

```bash
omarchy-pomo start --profile 25-5
omarchy-pomo start --profile 30-10
omarchy-pomo start --break
omarchy-pomo start --custom 45 --type focus
omarchy-pomo start --custom 5 --type break
omarchy-pomo status
omarchy-pomo status --waybar
omarchy-pomo pause
omarchy-pomo resume
omarchy-pomo stop
omarchy-pomo history
omarchy-pomo tui
```

## Ciclo de vida do timer

O daemon finaliza sessões vencidas autonomamente em um tick interno de até
100 ms; Waybar, TUI e CLI não precisam ser consultadas para registrar o fim.
Leituras IPC ocorrem em no máximo 16 workers separados, com timeout de 250 ms;
um cliente que mantém a escrita aberta não bloqueia nem faz o daemon criar
threads indefinidamente. Erros transitórios do tick são registrados e tentados
novamente com intervalo limitado.
Ao executar `start` ou `stop`, uma sessão que venceu desde a última consulta é
reconciliada antes da nova operação, incluindo histórico e notificação.

O histórico usa o instante nominal do vencimento (`started_at + duração`) e a
data local desse instante. Sessões customizadas aceitam de 1 a 1.440 minutos;
o limite é aplicado tanto pela CLI quanto pelo domínio/IPC.

Se o daemon não estiver rodando, a CLI retorna erro amigável pedindo `omarchy-pomo daemon`. Para `status --waybar`, a saída continua sendo JSON válido com classe `error`.

## TUI

Com o daemon rodando:

```bash
omarchy-pomo tui
```

Atalhos:

```text
1 = foco 25 min
2 = foco 30 min
3 = break 5 min
4 = tempo customizado: digite minutos, f=focus ou b=break, Enter inicia, Esc cancela
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

## Hyprland / Omarchy

Use uma classe dedicada no terminal para a TUI flutuar:

```bash
kitty --class omarchy-pomo -e omarchy-pomo tui
```

Regras em [`examples/hyprland.conf`](examples/hyprland.conf): janela flutuante, centralizada e 700x420.

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
{"session_id":"session-...","date":"2026-05-29","type":"focus","label":"25/5 Focus","duration_secs":1500,"completed":true,"finished_at":"2026-05-29T10:00:00-03:00"}
```

`stop` manual não escreve histórico. Linhas inválidas em `history.jsonl` são ignoradas no resumo.

`state.json` é salvo com temporário no mesmo diretório, `fsync` e rename atômico. O
temporário usa permissão `0600`, e é removido quando uma etapa da gravação falha.
O estado contém `session_id`, `history_recorded` e `notification_sent` para
recuperar uma conclusão interrompida sem duplicar o histórico:

```json
{
  "status": "finished",
  "session_type": "focus",
  "label": "25/5 Focus",
  "duration_secs": 1500,
  "started_at": null,
  "paused_remaining_secs": 0,
  "session_id": "session-...",
  "history_recorded": true,
  "notification_sent": true
}
```

Ordem de commit: (1) `Finished` no estado, (2) linha de histórico sincronizada,
(3) marcador do histórico, (4) notificação e seu marcador. Em restart, estado
`Finished` repara as etapas ausentes; o `session_id` deduplica retries. Uma
falha de notificação mantém a sessão e o histórico persistidos e permite nova
tentativa (a notificação é at-least-once se houver crash após o comando e antes
do marcador).

Arquivos legados continuam válidos: campos novos ausentes recebem valores
seguros; uma sessão legada ativa/concluída recebe uma identidade antes de tocar
no histórico. Linhas antigas sem `session_id` continuam sendo lidas e só são
deduplicadas contra outra linha legada com a mesma chave completa (incluindo
`finished_at`); nunca são confundidas com uma sessão nova identificada.

## Troubleshooting

- `daemon indisponível`: inicie `omarchy-pomo daemon`.
- `daemon já parece estar rodando`: já existe daemon ativo; não inicie outro.
- Waybar mostra erro: confirme se `omarchy-pomo` está no `PATH` da sessão gráfica.
- Sem som: instale `paplay` ou `mpv` e crie `~/.config/omarchy-pomo/done.ogg`.
- Sem notificação: instale/configure `notify-send` e daemon de notificações.
- Socket antigo/stale: o daemon remove socket morto ao iniciar; socket ativo não é removido.

## Desenvolvimento

Backlog por sprints: [`docs/SPRINTS.md`](docs/SPRINTS.md).

Plano arquitetural: [`docs/PLAN.md`](docs/PLAN.md).

Cada sprint só termina quando:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

passam, os critérios de aceite são verificados e a documentação é atualizada.

## Verificação atual

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Passa com a suíte atual.
