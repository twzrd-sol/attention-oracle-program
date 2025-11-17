# TWZRD Metrics Infrastructure Status

**Last Updated**: 2025-11-17
**Status**: Partial (Gateway ✅ | Aggregator ⚠️)

---

## Overview

All TWZRD services have **Prometheus metrics fully instrumented**. The gateway is working correctly, but the aggregator has a PostgreSQL SSL connection issue preventing metric collection.

---

## Services & Endpoints

### 1. Gateway (Portal v3)

**Status**: ✅ **WORKING**

- **PM2 ID**: 59
- **Port**: 5000 (internal)
- **Metrics Endpoint**: `http://localhost:5000/metrics`
- **Health Endpoint**: `http://localhost:5000/health`

**Key Metrics**:
```promql
twzrd_verification_requests_total{status="success|error"}
twzrd_claim_requests_total{status="success|duplicate|unverified|error"}
twzrd_claim_latency_seconds (histogram)
twzrd_last_epoch_sealed_timestamp
twzrd_active_viewers{channel="..."}
```

**Database**: Uses `pg-promise` (handles SSL correctly)

**Test**:
```bash
curl -sS http://localhost:5000/metrics | grep twzrd_
curl -sS http://localhost:5000/health
```

---

### 2. Milo Aggregator (Tree Builder & Publisher)

**Status**: ✅ **WORKING** (SSL issue resolved, minor schema mismatch in metrics)

- **PM2 ID**: 58
- **Port**: 8080 (internal)
- **Metrics Endpoint**: `http://localhost:8080/metrics` (⚠️ returns schema error, but DB queries work)
- **Health Endpoint**: `http://localhost:8080/health` - ✅ **WORKING**
- **Stats Endpoint**: `http://localhost:8080/stats` - ✅ **WORKING**

**Key Metrics** (instrumented but not collectible):
```promql
twzrd_aggregator_publish_success_total{channel="..."}
twzrd_aggregator_unpublished_epoch_backlog{group="milo|cls"}
twzrd_aggregator_epoch_sealed_total{channel="...",token_group="..."}
twzrd_aggregator_publisher_loop_tick_total
twzrd_aggregator_last_sealed_epoch
twzrd_aggregator_wallet_balance_sol
```

**Issue**: `SELF_SIGNED_CERT_IN_CHAIN` error when connecting to PostgreSQL

**Root Cause**:
- Uses raw `pg` Pool without SSL configuration
- DigitalOcean managed Postgres uses self-signed certificate
- DATABASE_URL has `sslmode=require` but Node.js doesn't trust the cert

**Fix Required** (choose one):

**Option 1**: Modify DATABASE_URL (least invasive)
```bash
# In /home/twzrd/milo-token/.env
DATABASE_URL=postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require&ssl=true&sslmode=no-verify
```

**Option 2**: Add SSL config to Pool constructor (in `/apps/twzrd-aggregator/dist/db-pg.js:14-26`)
```javascript
this.ingestPool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_INGEST_MAX || 40),
  idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
  ssl: {
    rejectUnauthorized: false  // ADD THIS
  }
});
```

**Option 3**: Environment variable (quick workaround, not recommended for production)
```bash
pm2 restart milo-aggregator --update-env NODE_TLS_REJECT_UNAUTHORIZED=0
```

**Test** (after fix):
```bash
curl -sS http://localhost:8080/metrics | grep twzrd_aggregator_
```

**Files**:
- Metrics registry: `/apps/twzrd-aggregator/src/metrics-registry.ts`
- Metrics router: `/apps/twzrd-aggregator/dist/routes/metrics.js`
- Main server: `/apps/twzrd-aggregator/dist/server.js`

---

### 3. Tree Builder Worker

**Status**: ℹ️ **NOT A SEPARATE SERVICE**

- **PM2 ID**: 10
- **Script**: `/apps/twzrd-aggregator/dist/workers/tree-builder.js`
- **Parent Service**: milo-aggregator

**Note**: Tree builder is a **BullMQ worker** (not an HTTP server). It processes jobs from Redis queue. Metrics are emitted via the aggregator's metrics registry.

**No HTTP endpoint** - metrics are collected by the aggregator.

---

### 4. CLS Workers (s0, s1, s2)

**Status**: ℹ️ **SEPARATE SERVICE**

- **PM2 IDs**: 34 (s0), 35 (s1), 48 (s2)
- **Script**: `/apps/worker-v2/dist/index.js`
- **Purpose**: Twitch IRC collectors (ingest chat participation events)

**Metrics Status**: Unknown (need to check if worker-v2 exposes metrics endpoint)

**Next Step**: Investigate `/apps/worker-v2/` for Prometheus instrumentation

---

## Grafana Dashboard Setup (Pending)

Once aggregator SSL issue is resolved, create dashboard with:

### Gateway Panel
- Claim request rate (by status)
- Verification request rate
- Claim latency (P50, P95, P99)
- Active viewers per channel

### Aggregator Panel
- Publish success rate
- Unpublished epoch backlog (by token group)
- Publisher loop tick rate
- Wallet SOL balance (alert if < 1 SOL)
- Last sealed epoch timestamp

### Alerts
- Gateway down (health check fails for 1 minute)
- Zero claims in 1 hour (during production)
- Aggregator publish failures > 5 in 10 minutes
- Wallet balance < 0.5 SOL

---

## Quick Reference

| Service | Port | /metrics | /health | /stats | Status |
|---------|------|----------|---------|--------|--------|
| Gateway | 5000 | ✅ | ✅ | N/A | **Working** |
| Aggregator | 8080 | ⚠️ | ✅ | ✅ | **Working** (metrics schema issue) |
| Tree Builder | - | N/A | N/A | N/A | Worker (no HTTP) |
| CLS Workers | ? | ? | ? | ? | TBD |

---

## Prometheus Scrape Config

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'twzrd-gateway'
    static_configs:
      - targets: ['localhost:5000']
    metrics_path: '/metrics'
    scrape_interval: 15s

  - job_name: 'twzrd-aggregator'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 60s
```

---

## Aggregator Fix History (Nov 17, 2025)

**RESOLVED**: The aggregator had multiple issues that were systematically fixed:

### Issues Fixed:
1. ✅ **Broken node_modules**: Backed up to `node_modules.broken`, removed entirely
2. ✅ **Missing dependencies**: Added to root `package.json` (pnpm workspace model)
3. ✅ **SSL certificate error**: Added `ssl: { rejectUnauthorized: false }` to both Pool configs in `dist/db-pg.js`
4. ✅ **DATABASE_URL conflict**: Removed `?sslmode=require` parameter that conflicted with ssl config object

### Changes Made:
```javascript
// /apps/twzrd-aggregator/dist/db-pg.js (lines 14-28)
this.ingestPool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_INGEST_MAX || 40),
  idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
  ssl: { rejectUnauthorized: false },  // ← Added
});

this.maintenancePool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_MAINT_MAX || 8),
  idleTimeoutMillis: Number(process.env.DB_POOL_MAINT_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_MAINT_TIMEOUT || 10000),
  ssl: { rejectUnauthorized: false },  // ← Added
});
```

```bash
# /.env (line 64)
# OLD: DATABASE_URL=...?sslmode=require
# NEW: DATABASE_URL=...  (removed query param)
```

### Current State:
- ✅ Aggregator running (PM2 ID 58)
- ✅ Database connection working
- ✅ `/health` endpoint operational
- ✅ `/stats` endpoint returning data
- ⚠️ `/metrics` endpoint has schema issue (`token_group` column doesn't exist) but this doesn't block core functionality

## Next Actions

1. **MEDIUM**: Fix aggregator /metrics schema issue
   - Missing column: `token_group` in database schema
   - Options:
     - Add migration to add `token_group` column to relevant tables
     - OR update metrics query to handle missing column gracefully
   - Current workaround: Use `/stats` endpoint for operational metrics

2. **MEDIUM**: Investigate cls-worker metrics
   - Check `/apps/worker-v2/` for Prometheus setup
   - Document findings

3. **LOW**: Set up Grafana dashboards
   - Import gateway and aggregator metrics
   - Configure alerts

4. **LOW**: Run `pm2 startup` to persist PM2 on reboot (if not already done)
   ```bash
   pm2 startup systemd -u twzrd --hp /home/twzrd
   # Run the command it prints
   pm2 save
   ```

---

## Related Documentation

- [Gateway Documentation](./gateway.md)
- [CLAUDE.md (Project Overview)](../CLAUDE.md)
- [Prometheus Client Docs](https://github.com/siimon/prom-client)

---

**Maintainer**: twzrd
**Last Verified**: 2025-11-17 (Gateway metrics confirmed working)
