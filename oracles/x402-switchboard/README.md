Minimal, builder‑neutral x402 + Switchboard integration example.

Endpoints
- GET /price — returns latest Switchboard feed (cluster, feed, price, slot)
- GET /protected — requires `x-402-payment: true` header; otherwise 402

Config
- `SB_CLUSTER` (default: devnet)
- `SB_FEED` (required) — aggregator public key
- `PORT` (default: 3000)

Run
```bash
cd oracles/x402-switchboard
npm install
npm run dev
```

Note: This is an example only. It persists no data and performs no auth.
