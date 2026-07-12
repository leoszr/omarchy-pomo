# Task 10 — Notificação e som

- Implementar `notify.rs`
- Tolerar ausência de `notify-send`, `paplay`, `mpv` ou arquivo de som
- Executar notificação e áudio em workers não bloqueantes
- Aguardar cada processo filho no worker e fazer fallback `paplay` -> `mpv` apenas após falha real
- Registrar falhas no `stderr` sem tornar o daemon fatal
