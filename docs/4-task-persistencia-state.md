# Task 4 — Persistência de estado

- Persistir `state.json`
- Ler estado inicial `Idle` quando o arquivo não existir

## Formato atual e migração

Além dos campos originais, o estado atual contém:

- `session_id`: identidade estável da sessão, presente em toda sessão não-`Idle`;
- `history_recorded`: indica que a entrada de histórico foi confirmada;
- `notification_sent`: indica que a notificação foi concluída.

Os três campos têm defaults compatíveis com arquivos antigos. Ao ler um estado
legado não-`Idle`, o daemon gera uma identidade e a persiste antes de qualquer
append no histórico.

## Escrita crash-safe

`state.json` nunca é truncado diretamente. A gravação cria um temporário no
mesmo diretório com modo `0600`, escreve e faz `sync_all`, troca o arquivo por
`rename` atômico e sincroniza o diretório. Falhas removem o temporário criado.
Assim, um crash deixa o estado anterior ou o novo, nunca JSON parcialmente
escrito.
