# Task 7 — Daemon e Unix Socket

## Protocolo

O transporte é Unix-only e usa uma conexão por operação. Cada mensagem é um
frame JSON UTF-8 delimitado por newline (`JSON + \n`):

```json
{"command":"status"}
```

O conteúdo JSON das requests/responses permanece o mesmo da Sprint 4; apenas
o delimitador foi adicionado para que o daemon não dependa de EOF. O cliente
envia uma request, lê uma response e a conexão é encerrada pelo daemon.

Limites e timeouts explícitos:

- máximo de 65.536 bytes por request e por response, sem contar `\n`;
- timeout de leitura: 2 segundos;
- timeout de escrita: 2 segundos.

O daemon responde `{"type":"error","message":"..."}` para payload vazio,
EOF prematuro, frame sem newline, payload excessivo e JSON inválido. Timeout
ou falha de I/O também inclui a operação afetada na mensagem.

## Concorrência e recursos

O accept loop usa quatro workers fixos e uma fila de 16 conexões. Um worker
preso em um cliente lento não bloqueia os demais. A fila cheia é rejeitada com
erro explícito; não há criação ilimitada de threads. No encerramento controlado,
o sender é fechado e os workers são aguardados (reaping).

Comandos suportados:

- `STATUS`
- `START`
- `PAUSE`
- `RESUME`
- `STOP`
- `HISTORY`
