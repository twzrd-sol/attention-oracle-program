# HTTP API Reference - Aggregator Service

**Base URL:** `https://api.twzrd.xyz` (Production) | `http://localhost:8080` (Development)
**Last Updated:** October 31, 2025

---

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Rate Limiting](#rate-limiting)
4. [Ingestion Endpoints](#ingestion-endpoints)
5. [Proof Endpoints](#proof-endpoints)
6. [Epoch Management](#epoch-management)
7. [Opt-Out Endpoints](#opt-out-endpoints)
8. [Metrics & Health](#metrics--health)
9. [Error Responses](#error-responses)

---

## Overview

The Aggregator Service provides HTTP endpoints for:
- **Data Ingestion**: Workers submit participation events
- **Proof Generation**: Users/gateway request merkle proofs for claims
- **Epoch Finalization**: Seal epochs and build merkle trees
- **Opt-Out Management**: Privacy/compliance requests
- **Metrics**: Real-time protocol health

---

## Authentication

### Internal Endpoints

Some endpoints accept an `x-internal: gateway` header to bypass rate limits:
- `/ingest`
- `/claim-root`
- `/proof`

**Example:**
```bash
curl -H "x-internal: gateway" https://api.twzrd.xyz/claim-root?channel=lacy&epoch=1761865200
```

### Public Endpoints

All other endpoints are public with rate limiting.

---

## Rate Limiting

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/ingest` | 300 requests | 60 seconds |
| `/claim-root`, `/proof`, `/receipt-proof` | 100 requests | 60 seconds |
| All others | No limit | - |

**Rate Limit Headers:**
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1761870000
```

---

## Ingestion Endpoints

### POST /ingest

Submit participation events from Twitch chat workers.

**Request:**
```json
{
  "epoch": 1761865200,
  "events": [
    {
      "channel": "lacy",
      "user": "viewer_username",
      "signals": {
        "presence": 1,
        "sub": false,
        "resub": false,
        "gift": 0,
        "bits": 0,
        "raid": false
      }
    }
  ]
}
```

**Response:**
```json
{
  "ok": true
}
```

**Errors:**
- `400 invalid_payload` - Missing or malformed epoch/events
- `500 ingest_error` - Database write failure

**Suppression:** Users on the opt-out list are automatically filtered and not recorded.

---

## Proof Endpoints

### GET /claim-root

Get merkle root and metadata for a channel epoch (lightweight, no proof generation).

**Query Parameters:**
- `channel` (required): Twitch channel name or "crypto" for CLS category
- `epoch` (required): Unix timestamp (must be on epoch boundary)

**Request:**
```bash
curl "https://api.twzrd.xyz/claim-root?channel=lacy&epoch=1761865200"
```

**Response:**
```json
{
  "root": "0x2f913100f6364fffe7f22c75d75ad5052780fd64ede20251b0a666a9d0d413fe",
  "participantCount": 970,
  "builtAt": 1761868982,
  "cached": true
}
```

**Category Mode (CLS):**
```bash
curl "https://api.twzrd.xyz/claim-root?channel=crypto&epoch=1761865200"
```

Returns aggregated root across all CLS-eligible channels.

---

### GET /receipt-proof

Generate merkle proof for a specific user's participation.

**Query Parameters:**
- `channel` (required): Twitch channel name
- `epoch` (required): Unix timestamp
- `user` (required): Twitch username

**Request:**
```bash
curl "https://api.twzrd.xyz/receipt-proof?channel=lacy&epoch=1761865200&user=viewer_name"
```

**Response:**
```json
{
  "channel": "lacy",
  "epoch": 1761865200,
  "user_hash": "0x3a7bd3e2f8e40c8d...",
  "username": "viewer_name",
  "index": 42,
  "weight": 1.5,
  "signals": {
    "presence": 5,
    "sub": 1,
    "resub": 0,
    "gift": 0,
    "bits": 0,
    "raid": 0
  },
  "proof": [
    "0x9f86d081884c7d65...",
    "0x6e340b9cffb37a98..."
  ],
  "root": "0x2f913100f6364fff...",
  "total_participants": 970,
  "version": "v0.1"
}
```

**Errors:**
- `404 no_participants` - Epoch not sealed or channel inactive
- `404 user_not_found` - User did not participate in this epoch
- `500 proof_failed` - Merkle tree generation error

---

### GET /proof-by-index

Generate merkle proof by participant index (used internally by gateway).

**Query Parameters:**
- `channel` (required)
- `epoch` (required)
- `index` (required): Zero-indexed participant position

**Response:** Same structure as `/receipt-proof`

---

## Epoch Management

### POST /finalize

Seal all active channels for a given epoch and generate merkle roots.

**Request:**
```json
{
  "epoch": 1761865200
}
```

**Response:**
```json
{
  "epoch": 1761865200,
  "channels": 28,
  "roots": {
    "lacy": {
      "root": "2f913100f6364fffe7f22c75d75ad5052780fd64ede20251b0a666a9d0d413fe",
      "participants": 970,
      "sealed": true
    },
    "jasontheween": {
      "root": "e202a4d08a9c8ebb4448ad9a44295766ddb639c8b70f287c40b2f680a2e106bf",
      "participants": 3190,
      "sealed": true
    }
  }
}
```

**Behavior:**
- Computes merkle root from sealed participants
- Records weighted signals and payout snapshots
- Triggers tree builder queue for L2 proof generation

---

### POST /build-category

Queue CLS category tree build (aggregates all CLS-eligible channels).

**Request:**
```json
{
  "epoch": 1761865200
}
```

**Response:**
```json
{
  "epoch": 1761865200,
  "jobId": "123",
  "status": "queued",
  "message": "Category tree build queued. Check /category/status for progress."
}
```

---

### GET /category/status

Check CLS category tree build status.

**Query Parameters:**
- `epoch` (required)

**Response (Building):**
```json
{
  "epoch": 1761865200,
  "status": "building",
  "message": "Category tree is being built. Check again in 60–90 seconds."
}
```

**Response (Ready):**
```json
{
  "epoch": 1761865200,
  "status": "ready",
  "root": "0xfb8b...",
  "participantCount": 5500,
  "builtAt": 1761869000
}
```

---

## Opt-Out Endpoints

### POST /opt-out

Submit opt-out request to suppress user data from future collection.

**Request:**
```json
{
  "username": "twitch_username",
  "reason": "Optional reason for opting out"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Opt-out request recorded. Your data will not be collected going forward.",
  "username": "twitch_username",
  "effective_immediately": true
}
```

**Errors:**
- `400 invalid_username` - Missing or invalid username
- `500 opt_out_failed` - Database write failure

**Privacy:**
- IP address is hashed (first 16 chars) for audit trail
- Username is stored for lookup, but all participation uses hashed identities
- Suppression is enforced at ingestion time (data never enters database)

---

### GET /opt-out/status

Check if a username is suppressed.

**Query Parameters:**
- `username` (required): Twitch username

**Request:**
```bash
curl "https://api.twzrd.xyz/opt-out/status?username=viewer_name"
```

**Response (Suppressed):**
```json
{
  "suppressed": true,
  "requested_at": 1761870003
}
```

**Response (Not Suppressed):**
```json
{
  "suppressed": false
}
```

---

## Metrics & Health

### GET /health

Health check endpoint.

**Response:**
```json
{
  "ok": true,
  "service": "twzrd-aggregator"
}
```

---

### GET /lap

Get Live Attention Price (reward rate per weighted minute).

**Query Parameters:**
- `channel` (optional): Channel-specific LAP (future feature)

**Response:**
```json
{
  "lap": 1.333333,
  "lapPerHour": 80,
  "lapPerEpoch": 80,
  "unit": "MILO per weighted minute",
  "epochSeconds": 3600,
  "basePerWeight": 80,
  "decimals": 9,
  "channel": "global",
  "note": "LAP is currently fixed across all channels. Weight=1.0 for presence, +10 for subs, etc."
}
```

**LAP Calculation:**
- Base reward: 80 MILO per weighted unit per epoch
- Weight formula: `presence + 10*sub + 10*resub + 5*gift + 0.01*bits + 0.1*raid`
- Example: 1 hour of presence + 1 sub = weight 11.0 = 880 MILO

---

## Error Responses

### Standard Error Format

```json
{
  "error": "error_code",
  "message": "Human-readable description"
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `invalid_payload` | 400 | Missing or malformed request body |
| `invalid_epoch` | 400 | Epoch must be positive integer |
| `missing_params` | 400 | Required query parameter missing |
| `not_found` | 404 | Resource does not exist |
| `no_participants` | 404 | Epoch not sealed or no data |
| `user_not_found` | 404 | User did not participate |
| `ingest_error` | 500 | Database write failure |
| `finalize_error` | 500 | Epoch sealing failure |
| `proof_failed` | 500 | Merkle proof generation error |
| `opt_out_failed` | 500 | Suppression request failure |

---

## Examples

### Complete Claim Flow

```bash
# 1. Worker ingests participation
curl -X POST https://api.twzrd.xyz/ingest \
  -H "Content-Type: application/json" \
  -d '{
    "epoch": 1761865200,
    "events": [{"channel": "lacy", "user": "viewer1", "signals": {"presence": 1}}]
  }'

# 2. Finalize epoch (usually automated)
curl -X POST https://api.twzrd.xyz/finalize \
  -H "Content-Type: application/json" \
  -d '{"epoch": 1761865200}'

# 3. User requests proof
curl "https://api.twzrd.xyz/receipt-proof?channel=lacy&epoch=1761865200&user=viewer1"

# 4. User submits claim transaction to Solana program using proof
# (See on-chain API docs for claim_channel_open instruction)
```

### Opt-Out Flow

```bash
# 1. User opts out
curl -X POST https://api.twzrd.xyz/opt-out \
  -H "Content-Type: application/json" \
  -d '{"username": "viewer1", "reason": "Privacy request"}'

# 2. Verify suppression
curl "https://api.twzrd.xyz/opt-out/status?username=viewer1"
# Returns: {"suppressed": true, "requested_at": 1761870003}

# 3. Future ingestion ignores this user
# Worker continues sending events, but aggregator drops them at ingestion
```

---

## Notes

- **Epochs:** 1-hour boundaries (3600 seconds). Epoch `1761865200` spans 22:00:00 to 22:59:59 UTC.
- **Hashing:** User identities are SHA3-256 hashed (Keccak256 with "twitch:" prefix).
- **Merkle Trees:** Built using Keccak256, sorted by participant order (not hash).
- **Suppression:** Checked at ingestion AND claim time (double-layered enforcement).
- **Rate Limits:** Apply per IP address (X-Forwarded-For header respected).

---

## Contact

- **API Issues:** https://github.com/twzrd/milo-token/issues
- **Support:** support@twzrd.xyz
- **Discord:** https://discord.gg/twzrd (coming soon)
