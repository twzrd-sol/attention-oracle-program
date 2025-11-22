# Attention Oracle — Verifiable Token‑2022 Program

This repository contains the minimal, verifiable on‑chain program that is deployed to Solana mainnet. All off‑chain components and any non‑critical code live in private repos. The public tree is kept intentionally small to guarantee reproducibility and trustless verification.

- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Program: `programs/token_2022/`
- Toolchain: Anchor 0.32.1 + Agave 3.0.10 + Rust 1.91.1

## What’s Included (and why)

- Only `programs/token_2022/` is published. This is the exact code used to produce the deployed binary. Keeping the public tree to this program ensures anyone can rebuild the same bytes and compare them to what’s on‑chain.

## Architecture (public tree)

```
programs/token_2022/      # Active Token-2022 program (GnGz...)
programs/x402-on-demand/  # x402 payment rail (Switchboard On-Demand + Token-2022)
sdk/                      # Shared IDL/clients (kept lean)
cli/                      # Admin CLI wired to AO_PROGRAM_ID and AO_RPC_URL
docs/                     # Public docs (open-core scope)
```

## Demo: Pay-per-Content Unlock with Oracle-Priced Rewards

Deployed mainnet programs:
- Attention Oracle: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- x402 payment rail: `7buCx5353jtT5rsmNr8U6Xe7a41sL1pmkZwzYCCSAmKF`

Flow (all on-chain, Token-2022 safe):
1) User pays 0.01 USDC via x402.
2) Switchboard SOL/USD feed is pulled in the same transaction.
3) Attention Oracle mints an oracle-priced engagement reward to the payer.
4) Optional: same CPI can mint an access-pass NFT to the payer.
5) Atomic, verifiable distribution; no off-chain trust.

TypeScript sketch (works with deployed IDs):
```ts
import { Connection, PublicKey, Transaction } from "@solana/web3.js";
import { BN } from "bn.js";
// program clients generated from the published IDLs
import { x402Program } from "./idl/x402_on_demand";

const connection = new Connection("https://api.mainnet-beta.solana.com");
const payer = /* your Keypair */;

const tx = new Transaction();
tx.add(
  await x402Program.methods
    .settleX402Payment(new BN(10_000)) // 0.01 USDC (6 decimals)
    .accounts({
      paymentSession: /* PDA: ["session", payer] */,
      from: /* payer USDC ATA */,
      mint: new PublicKey("Es9vMFrzaCERvZ3Z..."), // USDC Token-2022 or classic
      to: /* protocol USDC ATA */,
      authority: payer.publicKey,
      switchboardFeed: new PublicKey("GvDMxPzN1sCj7KqQp6AJj3C52e2iWp6VATnCjud7i3Ha"), // SOL/USD
      tokenProgram: /* Token-2022 program id */,
      rewardMint: /* attention reward mint */,
      rewardAta: /* payer reward ATA */,
      protocolAuthority: /* PDA seeds ["global"] */,
      systemProgram: PublicKey.default,
    })
    .instruction()
);

const sig = await connection.sendTransaction(tx, [payer]);
console.log("https://solscan.io/tx/" + sig);
```

Live example tx: _(insert solscan link after run)_.

## One‑Command Verification

Use Anchor’s native verifiable pipeline (Dockerized, deterministic):

```bash
# From repo root
anchor verify -p token_2022 GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

This builds in the pinned container and compares the trimmed executable section to mainnet. Our CI runs the same check on release tags.

## Expected Verifiable Build (v1.2.1)

- Size: `534,224` bytes
- SHA256: `8e60919edb1792fa496c20c871c10f9295334bf2b3762d482fd09078c67a0281`

If your local build or environment differs, re‑run in Docker via `anchor build --verifiable` or use the CI workflow on the `v1.2.1` tag.

## CI: What We Publish

Our GitHub Actions workflow (Verify Program) does the following on tags:
- Runs `anchor verify` against mainnet for the Program ID above
- Builds a verifiable artifact and uploads:
  - `local` verifiable `.so`
  - `on-chain.so` dump
  - A summary with local size/hash, on‑chain trimmed hash, and tool versions

## Security

Report vulnerabilities → security@twzrd.xyz

## License

MIT OR Apache‑2.0
