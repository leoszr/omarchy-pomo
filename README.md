# Omarchy Pomo

Pomodoro em Rust para Omarchy/Hyprland. Objetivo do MVP: daemon como fonte de verdade, Waybar exibindo status e TUI controlando sessões.

## Estado atual

MVP concluído: TUI com tempo customizado, daemon via Unix Socket, CLI via IPC, JSON para Waybar, notificação/som opcional, exemplos Waybar/Hyprland e build release verificado. A API pública não inclui tarefas; esse recurso está fora do MVP.

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

## Protocolo IPC e disponibilidade

O socket Unix usa um frame por conexão: um objeto JSON UTF-8 seguido por `\n`.
O cliente atual também sinaliza EOF depois do frame para interoperar com
daemons antigos; payload não vazio terminado por EOF é aceito como frame
legado. Payload vazio/EOF sem conteúdo continua sendo erro.

- request e response têm limite de 64 KiB, sem contar o `\n`;
- o cliente tem timeout de leitura/escrita de 2 segundos; o daemon usa 250 ms
  para leitura e escrita, evitando que workers presos consumam toda a margem;
- payload vazio, EOF prematuro, JSON inválido, excesso de tamanho e timeout
  retornam `type: "error"` com mensagem específica;
- o daemon limita a 16 workers IPC simultâneos. Ao atingir o limite, novas
  conexões recebem erro de sobrecarga em vez de criar threads sem controle.

Um cliente Unix Socket que conecta e não envia request não impede outros
clientes: o worker dedicado expira após o timeout.

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

## Performance de persistência e TUI

O daemon compara o estado antes de persistir. Um `status` que não provoca
transição não regrava `state.json`; comandos que não mudam o estado também
evitam a escrita.

A TUI redesenha o tempo localmente a cada 100 ms usando o timestamp recebido,
mas consulta o daemon apenas uma vez por segundo. O resumo de histórico tem
frequência separada de 5 segundos. Depois de detectar `Finished`, a TUI busca o
resumo imediatamente. Nenhum desses relógios locais substitui o daemon como
fonte de verdade.

O daemon mantém o histórico em cache. O JSONL só é relido e parseado quando
tamanho ou data de modificação do arquivo mudam; uma nova conclusão invalida o
cache naturalmente pelo append. A troca de data recalcula o resumo usando as
entradas já cacheadas, sem reler o arquivo.

## Notificação e som

Ao finalizar uma sessão, o daemon tenta executar:

```bash
notify-send "Pomodoro finalizado" "Sessão concluída: <label>"
```

Som opcional: crie o arquivo abaixo para tocar ao concluir:

```text
~/.config/omarchy-pomo/done.ogg
```

Notificação e áudio são disparados em workers de background: a resposta IPC não espera
`notify-send`, `paplay` ou `mpv`. O worker de áudio espera o resultado de `paplay` antes
de tentar `mpv --no-video`; portanto os dois players nunca são iniciados juntos.

Tudo é best-effort. Ausência de `notify-send`, `paplay`, `mpv` ou do arquivo de som não
derruba o daemon. Falhas de spawn, status de saída diferente de zero e falhas de espera
são registradas no `stderr` do daemon. No encerramento normal do loop, o daemon espera
os workers terminarem; cada processo filho é aguardado para evitar zombies.

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

Exemplo de sessão customizada persistida (a categoria não depende do label):

```json
{"status":"running","session_type":"custom","category":"break","label":"Descanso","duration_secs":300,"started_at":"2026-05-29T10:00:00-03:00","paused_remaining_secs":null}
```

`history.jsonl` é append-only. Cada linha registra uma sessão concluída:

```json
{"session_id":"session-...","date":"2026-05-29","type":"focus","label":"25/5 Focus","duration_secs":1500,"completed":true,"finished_at":"2026-05-29T10:00:00-03:00"}
```

`stop` manual não escreve histórico. Linhas inválidas não entram no resumo, mas
geram diagnóstico observável no stderr (com número da linha).

O estado do timer também possui `category` (`focus` ou `break`), que é a fonte
tipada para histórico, Waybar, CLI e TUI. O campo `label` é apenas texto de
exibição e pode ser traduzido ou arbitrário. Estados antigos sem `category`
continuam legíveis: o formato antigo gerado para `Custom Break (N min)` é
migrado para `break`; outros customizados legados assumem `focus` por falta de
informação semântica. O próximo `write` persiste o campo novo.

Ao ler o histórico, entradas válidas são preservadas. Para cada linha inválida,
o daemon emite diagnóstico no stderr com o número da linha e o erro de parsing;
a corrupção não é apagada silenciosamente.

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
- `timeout lendo request IPC`: o cliente não enviou um frame completo em 250 ms; envie JSON seguido de `\n`.
- `request IPC vazio`: o cliente fechou sem enviar um frame; EOF com JSON não vazio é aceito para compatibilidade.
- `excede o limite de 65536 bytes`: reduza o payload; o limite vale para request e response.
- `daemon ocupado`: o limite de workers simultâneos foi atingido; tente novamente.
- Waybar mostra erro: confirme se `omarchy-pomo` está no `PATH` da sessão gráfica.
- Sem som: instale `paplay` ou `mpv` e crie `~/.config/omarchy-pomo/done.ogg`.
- Sem notificação: instale/configure `notify-send` e daemon de notificações.
- Diagnóstico: execute `omarchy-pomo daemon` em um terminal e observe o `stderr` para
  falhas dos comandos externos.
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
