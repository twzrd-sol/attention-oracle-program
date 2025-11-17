# Database Pool TLS Audit & Migration Plan

**Date**: 2025-11-17
**Status**: Option A (Quick Workaround) → Option C (Production CA Validation)
**CA Certificate**: `/home/twzrd/certs/do-managed-db-ca.crt`

---

## Executive Summary

All TWZRD services connect to DigitalOcean managed PostgreSQL with **Option A SSL workaround** (`rejectUnauthorized: false`). This is **functionally correct but not production-grade**. This document audits all database pools and provides migration path to **Option C** (proper CA certificate validation).

---

## Current State (Option A)

### Services Using Option A

| Service | File | Library | Lines | Status |
|---------|------|---------|-------|--------|
| **Aggregator** | `/apps/twzrd-aggregator/dist/db-pg.js` | `pg` (raw Pool) | 19, 27 | ⚠️ Option A |
| **Gateway** | `/gateway/src/db.ts` | `pg-promise` | N/A | ✅ Auto-handled |

### Option A Implementation

**File**: `/apps/twzrd-aggregator/dist/db-pg.js`

```javascript
// Line 14-28 (ingestPool)
this.ingestPool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_INGEST_MAX || 40),
  idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
  ssl: { rejectUnauthorized: false }, // ← Option A workaround
});

// Line 29-35 (maintenancePool)
this.maintenancePool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_MAINT_MAX || 8),
  idleTimeoutMillis: Number(process.env.DB_POOL_MAINT_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_MAINT_TIMEOUT || 10000),
  ssl: { rejectUnauthorized: false }, // ← Option A workaround
});
```

**Why This Works**:
- Node.js `pg` library connects to PostgreSQL over TLS
- DigitalOcean managed Postgres uses self-signed certificate
- `rejectUnauthorized: false` tells Node.js to accept any certificate
- Connection is still **encrypted**, but **not verified against CA**

**Why This Is Not Production-Grade**:
- Vulnerable to man-in-the-middle attacks (MITM)
- Certificate is not validated against trusted CA
- Acceptable for dev/staging, **not recommended for production**

---

## Migration to Option C (Production-Grade)

### CA Certificate Location

**Downloaded from DigitalOcean**:
```
/home/twzrd/certs/do-managed-db-ca.crt
```

**Verify Certificate**:
```bash
openssl x509 -in /home/twzrd/certs/do-managed-db-ca.crt -noout -text | head -20
```

**Certificate Details**:
- **Issuer**: DigitalOcean Project CA
- **Subject**: GEN 1 Project CA
- **Valid Until**: 2035-11-01
- **Key Size**: 384-bit RSA

---

## Option C Implementation

### 1. Aggregator (TypeScript Source)

**File**: `/apps/twzrd-aggregator/src/db-pg.ts`

**Current (Option A)**:
```typescript
this.ingestPool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_INGEST_MAX || 40),
  idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
  ssl: { rejectUnauthorized: false }, // Option A
});
```

**Upgrade to Option C**:
```typescript
import * as fs from 'fs';
import * as path from 'path';

// Load CA certificate
const caCertPath = process.env.DB_CA_CERT_PATH || '/home/twzrd/certs/do-managed-db-ca.crt';
const caCert = fs.readFileSync(caCertPath, 'utf8');

this.ingestPool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_INGEST_MAX || 40),
  idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
  ssl: {
    rejectUnauthorized: true,  // Option C: Verify certificate
    ca: caCert,                // Trusted CA certificate
  },
});

this.maintenancePool = new Pool({
  connectionString: connString,
  max: Number(process.env.DB_POOL_MAINT_MAX || 8),
  idleTimeoutMillis: Number(process.env.DB_POOL_MAINT_IDLE || 30000),
  connectionTimeoutMillis: Number(process.env.DB_POOL_MAINT_TIMEOUT || 10000),
  ssl: {
    rejectUnauthorized: true,  // Option C: Verify certificate
    ca: caCert,                // Trusted CA certificate
  },
});
```

**Environment Variable** (`.env`):
```bash
DB_CA_CERT_PATH=/home/twzrd/certs/do-managed-db-ca.crt
```

---

### 2. Gateway (Already Handled)

**File**: `/gateway/src/db.ts`

**Current Implementation**:
```typescript
import pgPromise from 'pg-promise';

const pgp: IMain = pgPromise({
  capSQL: true,
});

const db: IDatabase<any> = pgp(process.env.DATABASE_URL || '');
```

**Status**: ✅ **No changes needed**

**Why**: `pg-promise` automatically handles SSL configuration:
- If `DATABASE_URL` contains `?sslmode=require`, it enables SSL
- If `DATABASE_URL` contains `?sslrootcert=<path>`, it uses that CA cert
- Falls back to Node.js default CA bundle if no cert specified

**To Upgrade to Option C** (optional):

Modify `DATABASE_URL` in `.env`:
```bash
# Current (works with pg-promise auto-SSL)
DATABASE_URL=postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool

# Upgrade to Option C (explicit CA validation)
DATABASE_URL=postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require&sslrootcert=/home/twzrd/certs/do-managed-db-ca.crt
```

**OR** configure programmatically:
```typescript
import pgPromise from 'pg-promise';
import * as fs from 'fs';

const caCert = fs.readFileSync('/home/twzrd/certs/do-managed-db-ca.crt', 'utf8');

const pgp: IMain = pgPromise({
  capSQL: true,
});

const db: IDatabase<any> = pgp({
  connectionString: process.env.DATABASE_URL,
  ssl: {
    rejectUnauthorized: true,
    ca: caCert,
  },
});
```

---

## Migration Checklist

### Pre-Migration

- [x] CA certificate downloaded from DigitalOcean
- [x] Certificate stored at `/home/twzrd/certs/do-managed-db-ca.crt`
- [x] Certificate verified with `openssl x509`
- [ ] Aggregator TypeScript source updated (Option C)
- [ ] Aggregator rebuilt: `cd apps/twzrd-aggregator && pnpm build`
- [ ] Gateway configuration reviewed (optional upgrade)

### Migration Steps

#### 1. Update Aggregator (Required)

```bash
# Edit TypeScript source
vim /home/twzrd/milo-token/apps/twzrd-aggregator/src/db-pg.ts

# Add CA cert loading logic (see above)

# Rebuild
cd /home/twzrd/milo-token/apps/twzrd-aggregator
pnpm build

# Restart
pm2 restart milo-aggregator

# Verify logs
pm2 logs milo-aggregator --lines 20 --nostream
```

**Expected Log**:
- No SSL errors
- Database connections succeed
- Aggregator processes participation events normally

#### 2. Update Gateway (Optional)

```bash
# Option A: Update DATABASE_URL in .env
vim /home/twzrd/milo-token/.env

# Add sslrootcert parameter
# DATABASE_URL=...?sslmode=require&sslrootcert=/home/twzrd/certs/do-managed-db-ca.crt

# Restart
pm2 restart gateway

# Verify logs
pm2 logs gateway --lines 20 --nostream
```

**Expected**: No changes needed if pg-promise is already handling SSL correctly.

---

## Rollback Plan

If Option C migration fails, revert to Option A:

### Aggregator

```bash
# Restore Option A code in dist/db-pg.js (already exists)
# OR rebuild from git

git checkout apps/twzrd-aggregator/src/db-pg.ts
cd apps/twzrd-aggregator && pnpm build
pm2 restart milo-aggregator
```

### Gateway

```bash
# Remove sslrootcert from DATABASE_URL if added
vim /home/twzrd/milo-token/.env

# Restart
pm2 restart gateway
```

---

## Testing Option C

### 1. Pre-Migration Baseline

```bash
# Check current connection works (Option A)
pm2 logs milo-aggregator --lines 50 --nostream | grep -E "(connected|error|ssl)"
pm2 logs gateway --lines 50 --nostream | grep -E "(connected|error|ssl)"
```

**Expected**: No SSL errors, connections working.

### 2. Post-Migration Validation

```bash
# After upgrading to Option C
pm2 restart milo-aggregator gateway

# Wait 10 seconds for connections to establish
sleep 10

# Check logs for SSL errors
pm2 logs milo-aggregator --lines 50 --err --nostream | grep -i "ssl\|certificate\|unauthorized"
pm2 logs gateway --lines 50 --err --nostream | grep -i "ssl\|certificate\|unauthorized"
```

**Expected**:
- ✅ No "self-signed certificate in certificate chain" errors
- ✅ No "DEPTH_ZERO_SELF_SIGNED_CERT" errors
- ✅ Connections succeed
- ✅ Aggregator processes events
- ✅ Gateway serves requests

### 3. Functional Tests

```bash
# Test aggregator /health endpoint
curl http://localhost:8080/health

# Test gateway /health endpoint
curl http://localhost:5000/health

# Test claim transaction build
curl -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{"wallet": "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD", "epochId": 1}'
```

**Expected**: All endpoints return successfully.

---

## Security Benefits (Option A → Option C)

| Aspect | Option A | Option C |
|--------|----------|----------|
| **Encryption** | ✅ Yes (TLS) | ✅ Yes (TLS) |
| **Certificate Verification** | ❌ No | ✅ Yes |
| **MITM Protection** | ❌ Vulnerable | ✅ Protected |
| **Production Ready** | ⚠️ No | ✅ Yes |
| **Compliance** | ⚠️ Risky | ✅ Auditable |

**MITM Attack Scenario (Option A)**:
1. Attacker intercepts connection between app and database
2. Attacker presents fake certificate
3. App accepts fake certificate (rejectUnauthorized: false)
4. Attacker can read/modify database traffic

**Protection with Option C**:
1. Attacker intercepts connection
2. Attacker presents fake certificate
3. App rejects certificate (not signed by trusted CA)
4. Connection fails, attack prevented

---

## Other Services to Audit

### 1. CLS Workers (worker-v2)

**Location**: `/apps/worker-v2/dist/index.js`

**Database Access**: Indirect (via aggregator `/ingest` endpoint)

**Status**: ✅ No direct DB connection, uses HTTP API

### 2. Tree Builder Worker

**Location**: `/apps/twzrd-aggregator/dist/workers/tree-builder.js`

**Database Access**: Via aggregator's DB factory

**Status**: ✅ Uses same DB pools as aggregator (Option A → C migration applies)

### 3. Scripts (Ad-Hoc)

**Location**: `/scripts/*.ts`

**Database Access**: Varies per script

**Status**: ⚠️ Audit case-by-case

**Recommendation**: Add helper function for scripts:

```typescript
// /scripts/lib/db-connection.ts
import { Pool } from 'pg';
import * as fs from 'fs';

export function createSecurePool(): Pool {
  const caCert = fs.readFileSync(
    process.env.DB_CA_CERT_PATH || '/home/twzrd/certs/do-managed-db-ca.crt',
    'utf8'
  );

  return new Pool({
    connectionString: process.env.DATABASE_URL,
    ssl: {
      rejectUnauthorized: true,
      ca: caCert,
    },
  });
}
```

---

## Timeline & Priority

### Immediate (Next Deployment)

- [ ] Update aggregator TypeScript source (Option C)
- [ ] Rebuild and test in staging
- [ ] Deploy to production

**Priority**: **MEDIUM** (Option A works but not production-grade)

**Effort**: 1 hour (code change + testing)

**Risk**: LOW (easy rollback to Option A if issues)

### Next Sprint

- [ ] Audit all scripts for direct DB connections
- [ ] Create helper library for secure DB connections
- [ ] Update documentation for new scripts

**Priority**: LOW

**Effort**: 2-3 hours

---

## Related Documentation

- **SSL Fix History**: `/docs/METRICS_INFRASTRUCTURE.md` (lines 194-237)
- **Incident Report**: `/docs/incidents/INCIDENT_RESPONSE_2025-11-17.md`
- **CA Certificate Setup**: Documented in METRICS_INFRASTRUCTURE.md

---

## References

- **DigitalOcean Managed Postgres SSL**: https://docs.digitalocean.com/products/databases/postgresql/how-to/secure/
- **Node.js pg SSL Options**: https://node-postgres.com/features/ssl
- **OpenSSL Certificate Verification**: https://www.openssl.org/docs/man1.1.1/man1/verify.html

---

**Maintainer**: twzrd
**Last Updated**: 2025-11-17
**Next Review**: After Option C migration (production deployment)
