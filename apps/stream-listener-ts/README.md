# Stream Listener (Token-2022)

Lightweight indexer for Attention Oracle events on Solana.

- Subscribes to program logs (`GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`)
- Parses typed events via Anchor IDL
- Writes NDJSON to `../logs/events.ndjson`
- Optional POST to `GATEWAY_URL/internal/event`

## Setup

```bash
cd apps/stream-listener-ts
npm install
cp .env.example .env
```

`.env` (required):
```env
RPC_URL=https://api.mainnet-beta.solana.com
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
# Optional
RPC_URL_WS=wss://api.mainnet-beta.solana.com
GATEWAY_URL=https://your-gateway/internal
INTERNAL_EVENT_TOKEN=secret
LOG_DIR=../logs
STREAM_COMMITMENT=confirmed
```

## IDL (one-time)

```bash
cd ../../programs/token_2022
anchor idl parse --file src/lib.rs --out ../../apps/stream-listener-ts/idl/token_2022.json
```

## Run

Dev:
```bash
npm run dev
```

Prod (PM2):
```bash
npm run build
pm2 start ecosystem.config.js
```

## Output example
```json
{"ts":"2025-11-19T08:14:22.337Z","signature":"5f9eX...","slot":380987123,"name":"PassportMinted","data":{"userHash":"...","owner":"...","tier":3,"score":1543200}}
```

Safe to fork/modify. Part of open-core tooling. Use it.
