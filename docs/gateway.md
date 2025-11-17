# TWZRD Gateway (Portal v3)

**Canonical HTTP gateway for TWZRD / CLS claims + Portal v3 SPA.**
Serves the frontend, exposes claim/verification APIs, and publishes Prometheus metrics.

---

## 1. Location & Process

- Repo path: `/home/twzrd/milo-token/gateway`
- Entrypoint: `dist/index.js`
- Runtime: Node 20.x (ESM)
- Process manager: **PM2**
  - App name: `gateway`
  - PM2 ID: 59
  - Port: `5000` (internal)

### PM2 commands

From `/home/twzrd/milo-token/gateway`:

```bash
# Start (from ecosystem config)
pm2 start ecosystem.config.cjs

# Show status
pm2 ls
pm2 logs gateway --lines 50

# Restart after deploy/build
pm2 restart gateway

# Persist process list (after any changes)
pm2 save
```

PM2 startup (one-time on new machine):

```bash
pm2 startup systemd -u twzrd --hp /home/twzrd
# then run the command it prints (sudo systemctl enable ...)
```

---

## 2. Environment Variables

Gateway expects its `.env` file at:

```bash
/home/twzrd/milo-token/gateway/.env
```

Minimum required:

```env
# Solana
SOLANA_RPC=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
MINT_PUBKEY=AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5

# Database
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/twzrd

# Server
PORT=5000
NODE_ENV=production
```

The gateway also reads from `../.env` via `dotenv` (for shared secrets).
Priority: local `.env` in `/gateway` → parent `.env` → process env.

---

## 3. Build & Deploy Flow

From `/home/twzrd/milo-token/gateway`:

```bash
# Install deps
npm install

# TypeScript build
npm run build

# Start/Restart via PM2
pm2 restart gateway   # or pm2 start ecosystem.config.cjs on first run
pm2 save
```

Typical deploy:

1. `git pull`
2. `npm install` (if deps changed)
3. `npm run build`
4. `pm2 restart gateway && pm2 save`
5. Sanity-check:
   - `curl -sS http://localhost:5000/health`
   - `curl -sS http://localhost:5000/metrics | head`

---

## 4. HTTP Endpoints

Base URL (internal): `http://localhost:5000`
Base URL (external): `https://<your-domain>` via reverse proxy.

### 4.1 Health

```http
GET /health
```

Returns HTTP 200 if the process is up (and optionally DB/RPC reachable).

Sample response:

```json
{
  "status": "ok",
  "uptimeSeconds": 1234,
  "timestamp": "2025-11-17T04:12:30.000Z",
  "version": "1.0.0"
}
```

Used by external uptime monitors.

### 4.2 Verification Status

```http
GET /api/verification-status?wallet=<pubkey>
```

Response (example):

```json
{
  "twitterFollowed": true,
  "discordJoined": true,
  "passportTier": 3,
  "lastVerified": "2025-11-17T04:33:36.988Z"
}
```

This is a pure off-chain check (Twitter, Discord, DB).

### 4.3 Claim CLS

```http
POST /api/claim-cls
Content-Type: application/json
```

Body:

```json
{
  "wallet": "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD",
  "epochId": 424245,
  "index": 0,
  "amount": "100000000",
  "id": "test-claim-424245",
  "proof": []
}
```

Response (success):

```json
{
  "transaction": "<base64-encoded-solana-transaction>",
  "signature": null
}
```

Response (error):

```json
{
  "error": "Already claimed for this epoch"
}
```

**Merkle Proof Format:**
- Leaf = `keccak_256(wallet || index(u32) || amount(u64) || id(string))`
- Empty proof array `[]` for single-entry trees
- Proof elements: array of 64-char hex strings (32 bytes each)

All claim logic is **idempotent** and enforced by DB unique constraint `(wallet, epoch_id)`.

### 4.4 Metrics

```http
GET /metrics
```

Prometheus-formatted metrics including:

* Process metrics (CPU, memory, event loop lag)
* Custom counters:
  - `twzrd_verification_requests_total{status="success|error"}`
  - `twzrd_claim_requests_total{status="success|duplicate|unverified|error"}`
  - `twzrd_claim_latency_seconds` (histogram)
* Gauges:
  - `twzrd_last_epoch_sealed_timestamp`
  - `twzrd_active_viewers{channel="..."}`

Use this as a Prometheus scrape target and as a quick local diagnostics endpoint.

---

## 5. Reverse Proxy (Nginx / Cloudflare)

Upstream should point at the internal gateway:

```nginx
location / {
    proxy_pass http://127.0.0.1:5000;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

External routes:

- `GET https://<domain>/` → Portal v3
- `GET https://<domain>/api/verification-status`
- `POST https://<domain>/api/claim-cls`
- `GET https://<domain>/metrics`
- `GET https://<domain>/health`

---

## 6. Troubleshooting

**Gateway not responding / 502 from proxy**

```bash
pm2 ls
pm2 logs gateway --lines 100
curl -sS http://localhost:5000/health
```

**Metrics 500 or missing fields**

- Check `gateway/dist/metrics.js` logic.
- Verify DB connectivity (`DATABASE_URL`) and that tables exist.

**Claim errors**

- Check `pm2 logs gateway` for stack traces in `/api/claim-cls`.
- Confirm Solana RPC, `PROGRAM_ID`, and `MINT_PUBKEY` are set correctly.
- Validate epoch data in Postgres (sealed epochs, participants).

---

## 7. Deprecated Gateway (apps/gateway)

- Old path: `/home/twzrd/milo-token/apps/_deprecated-gateway-2025-11-17`
- Status: **deprecated / incomplete**
  - Only partial TypeScript sources.
  - Compiled against older `@noble/hashes` version.
- Do **not** deploy or modify.
- Safe to delete after confirming no references remain.

---

## 8. Devnet Testing

**Test wallet**: `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` (14.72 SOL)

Verified flows:
- ✅ Verification status lookup
- ✅ Merkle proof validation (keccak_256)
- ✅ Claim transaction building
- ✅ Duplicate prevention
- ✅ Metrics collection

**Test commands:**

```bash
# Health check
curl -sS http://localhost:5000/health | jq

# Metrics
curl -sS http://localhost:5000/metrics | grep twzrd

# Verification status
curl -sS "http://localhost:5000/api/verification-status?wallet=2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD" | jq

# Claim (requires setup: epoch 424245, merkle root, social verification)
curl -sS -X POST http://localhost:5000/api/claim-cls \
  -H "Content-Type: application/json" \
  -d '{"wallet":"2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD","epochId":424245,"index":0,"amount":"100000000","id":"test","proof":[]}' | jq
```

---

This document is the single source of truth for the TWZRD HTTP gateway.

**Last Updated**: 2025-11-17
**Maintainer**: twzrd
