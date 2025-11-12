# Attention Oracle — Release Notes

## Summary
- Open-source, production-ready proof of x402 payment-gated data access.
- On-chain verification (Merkle + ring buffer) with Token‑2022.
- Switchboard integration for external price context.
- Minimal dev tool and trustless verification helpers.

## Components
- programs/attention-oracle: Anchor program (mainnet ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
- x402-api-server: Next.js API/UI demo
  - GET /api/get-attention-score
  - GET /api/switchboard/price
  - GET /api/verify-payment?tx=<SIG>
- clients/js/x402-client.ts: Minimal 402 handshake client

## Quick Start
```bash
cd x402-api-server
npm install
SB_CLUSTER=devnet npm run dev
# curl http://localhost:3000/api/switchboard/price
# curl "http://localhost:3000/api/get-attention-score?creator=example_user"
```

## Config (optional)
- SB_CLUSTER=devnet|mainnet-beta
- SB_FEED=Switchboard aggregator pubkey (defaults to devnet SOL/USD)
- X402_RECIPIENT=wallet pubkey for payment verification
- X402_MIN_LAMPORTS=minimum lamports for verification

## License
Apache-2.0 OR MIT (dual-license)

