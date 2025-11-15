# Operations Cheatsheet (PostgreSQL-only)

This repo is now running fully on PostgreSQL. SQLite has been archived under `backups/sqlite/`.

## Quick Health

- Aggregator: `curl -s http://127.0.0.1:8080/health | jq`
- Gateway: `curl -s http://127.0.0.1:8082/health | jq`
- Last sealed epoch + top channels: `curl -s http://127.0.0.1:8080/stats | jq`
- Category root (crypto) for last sealed: `E=$(curl -s :8080/stats|jq -r .lastSealedEpoch); curl -s "http://127.0.0.1:8080/claim-root?channel=crypto&epoch=$E" | jq`

## PM2 Services

- `stream-listener` – Solana PDA subscriptions (env CHANNELS from repo `.env`)
- `milo-worker-v2` – Twurple IRC ingestion → `/ingest` (TWZRD + MILO)
- `cls-worker` – Auto-discovered Twitch Crypto channels (viewers only → TWZRD L1)
- `milo-aggregator` – API + merkle sealing + auto-publisher
- `gateway` – Fastify backend (PostgreSQL-backed proofs; port 8082)
- `publisher` – hourly CLS category root publisher (cron @ minute 5)
- `cls-discovery` – cron job (every 5m) refreshing CLS channel list

Restart a service: `pm2 restart <name>`

Persist process list across reboots: `pm2 save`

## nginx Routing (api.twzrd.xyz)

- `/api/*` → `127.0.0.1:8082` (gateway)
- Health check: `curl -k -H 'Host: api.twzrd.xyz' https://127.0.0.1/health`

## Monitoring & Alerts

- Cron (every 2 minutes): `*/2 * * * * bash /home/twzrd/milo-token/scripts/monitor/health-check.sh`
- Script: `scripts/monitor/health-check.sh` (MIT)
- Reconciliation: `20 * * * * cd /home/twzrd/milo-token && LIMIT=200 /usr/bin/npx tsx scripts/ops/reconcile-roots.ts >> /home/twzrd/milo-token/logs/ops/reconcile.log 2>&1`
  - Checks: gateway/aggregator/worker `/health`, Postgres `SELECT 1`
  - Auto‑restart after 3 consecutive failures per service
  - Log: `logs/monitor/health.log`
  - Slack (optional): set `SLACK_WEBHOOK` in `.env`

## Postgres Handy Queries

Recent activity per channel (last 10 min):

```sql
SELECT channel,
       COUNT(*) AS records_last_10min,
       to_timestamp(MAX(first_seen)) AS last_record
FROM channel_participation
WHERE first_seen >= EXTRACT(EPOCH FROM NOW()) - 600
GROUP BY channel
ORDER BY last_record DESC;
```

Sealed epochs waiting to publish:

```sql
SELECT epoch, channel, root
FROM sealed_epochs
WHERE published IS NULL OR published = 0
ORDER BY epoch ASC;
```

## Notes

- SQLite is sunset. Do not point any service at `data/twzrd.db`.
- Category trees are built on-demand by the aggregator and cached in `l2_tree_cache`.
- The `tree-builder` worker is only used for per-channel builds; category builds are served by the aggregator in PG mode.
- CLS allowlist: `config/cls-channels.json` (auto-managed). MILO channels (env `MILO_CHANNELS`) are excluded automatically.
