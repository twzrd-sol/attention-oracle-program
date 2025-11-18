# x402 + Switchboard Demo

Minimal, builder-neutral example server.

- GET `/price` – returns the latest Switchboard feed (cluster, feed, price, slot).
- GET `/protected` – returns HTTP 402 unless `x-402-payment: true` header is present.

## Config

`.env` keys:

```env
PORT=3000
SB_CLUSTER=devnet
SB_FEED=<switchboard-aggregator-pubkey>
```

## Run

```bash
cd oracles/x402-switchboard
npm install
npm run dev
curl http://localhost:3000/price
```

This demo is stateless; it persists no data and performs no auth. Use it as a reference implementation only.
