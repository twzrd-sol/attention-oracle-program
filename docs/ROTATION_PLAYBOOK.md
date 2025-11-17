# Secrets Rotation Playbook (DO Managed PG + Valkey)

Date: 2025-11-17
Window: 5 minutes

Scope:
- Rotate Postgres `doadmin` password on cluster `twzrd-prod-postgres`.
- Rotate Valkey (Redis) password on cluster `twzrd-bullmq-redis`.
- Update local `.env` files and `ecosystem.config.js`.
- Restart aggregator and verify CA‑TLS connectivity + metrics.

Pre‑checks
- doctl installed and authed: `doctl auth list`
- CA cert at `/home/twzrd/certs/do-managed-db-ca.crt` (aggregator uses CA TLS).
- PM2 processes stable: `pm2 ls`.

Runbook
1) Prepare backups (auto):
   - Script backs up `.env`, aggregator `.env`, and `ecosystem.config.js` to `/home/twzrd/backups/env_YYYYmmdd_HHMMSS/`.

2) Rotate Postgres password (CLI):
   - Command executed by script (masked here):
     - `doctl databases user reset <PG_ID> doadmin -o json > /home/twzrd/backups/pg_reset_TIMESTAMP.json`
   - Script extracts the new password and updates:
     - `DATABASE_URL` in `/home/twzrd/milo-token/.env`
     - `DATABASE_URL` in `/home/twzrd/milo-token/apps/twzrd-aggregator/.env`
     - Postgres URL inside `ecosystem.config.js`
   - It also flips `NODE_TLS_REJECT_UNAUTHORIZED` from `0` to `1` in `ecosystem.config.js`.

3) Rotate Valkey password (UI):
   - DO Control Panel → Databases → `twzrd-bullmq-redis` → Reset Password (copy new value).
   - Export for script: `export NEW_VALKEY_PASSWORD='<new-valkey-password>'`.
   - Script updates `REDIS_URL` in `ecosystem.config.js` and in any `.env` lines that contain `REDIS_URL=`.

4) Restart and verify:
   - `pm2 restart milo-aggregator --update-env`
   - `curl http://localhost:8080/health` → `{"ok":true,...}`
   - `curl http://localhost:8080/metrics | head` (Prometheus output)

One‑liner (dry‑run):
```
CONFIRM=0 bash scripts/ops/rotate-pg-valkey.sh
```

Execute during window (PG only):
```
CONFIRM=1 bash scripts/ops/rotate-pg-valkey.sh
```

Execute with Valkey password (after UI reset):
```
export NEW_VALKEY_PASSWORD='REDACTED'
CONFIRM=1 bash scripts/ops/rotate-pg-valkey.sh
```

Rollback (if needed within the window)
- Restore backups from `/home/twzrd/backups/env_*/` and restart pm2.
- Postgres: use prior password snapshot (if still valid) or reset again via doctl.

Notes
- The aggregator uses CA‑validated TLS now; no `sslmode=require` needed in URL.
- Secrets should never be committed; `ecosystem.config.js` is updated in place by the script.

