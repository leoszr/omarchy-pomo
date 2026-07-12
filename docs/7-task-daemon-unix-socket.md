# Task 7 — Daemon e Unix Socket

## Protocolo

O transporte é Unix-only e usa uma conexão por operação. Cada mensagem é um
frame JSON UTF-8 delimitado por newline (`JSON + \n`):

```json
{"command":"status"}
```

O conteúdo JSON das requests/responses permanece o mesmo da Sprint 4. O
newline é o framing preferido; EOF após payload não vazio também é aceito para
compatibilidade com clientes antigos. O cliente atual envia newline e EOF,
permitindo upgrade cruzado com daemon antigo. A conexão é encerrada pelo
daemon depois da response.

Limites e timeouts explícitos:

- máximo de 65.536 bytes por request e por response, sem contar `\n`;
- timeout de leitura/escrita do daemon: 250 ms;
- timeout de leitura/escrita do cliente: 2 segundos.

O daemon responde `{"type":"error","message":"..."}` para payload vazio,
payload excessivo e JSON inválido. EOF vazio e timeout também incluem a
operação afetada na mensagem; EOF com payload não vazio é o frame legado.

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
