# Task 9 — Finalização idempotente

- Detectar sessão finalizada no daemon
- Transição para `Finished` apenas uma vez

## Protocolo de conclusão

Uma sessão recebe `session_id` no `start`. Ao expirar, o daemon confirma nesta
ordem:

1. grava `Finished` duravelmente em `state.json`;
2. faz append sincronizado em `history.jsonl`, ignorando uma entrada já
   existente com a mesma identidade;
3. grava `history_recorded`;
4. tenta notificar e grava `notification_sent`.

Após um crash, qualquer chamada seguinte retoma a partir do primeiro marcador
ausente. O histórico é deduplicado tanto no append quanto na leitura do JSONL,
portanto retry não aumenta o resumo. Uma falha de notificação não desfaz
`Finished` nem o histórico; retorna erro claro ao chamador e deixa a etapa
pendente para retry. Como efeitos externos não oferecem transação, um crash
entre a notificação e seu marcador pode repetir a notificação, mas nunca perde
a sessão nem duplica sua entrada de histórico.

`session_id` em `HistoryEntry` também é opcional na desserialização: linhas
legadas continuam válidas. Para arquivos legados sem os novos campos, a leitura
gera a identidade e a persistência do estado acontece antes do histórico.
