# Task 8 — CLI via IPC

- Fazer os subcomandos conversarem com o daemon via `ipc.rs`

O cliente usa o framing documentado em [Task 7](7-task-daemon-unix-socket.md),
envia newline e EOF para compatibilidade cruzada, com timeouts de 2 segundos e
limite de 64 KiB. O erro de socket inexistente
continua orientando o usuário a iniciar `omarchy-pomo daemon`.
