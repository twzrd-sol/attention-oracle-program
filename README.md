# Attention Oracle — Verifiable Token-2022 Program

Minimal public repo for the deployed mainnet programs. Only the exact on-chain code is published for deterministic verification.

- **Attention Oracle Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **x402 Payment Rail**: `7buCx5353jtT5rsmNr8U6Xe7a41sL1pmkZwzYCCSAmKF`

## Architecture
```
programs/token_2022/              # Attention Oracle (GnGz…)
programs/x402-on-demand/          # x402 payment rail
sdk/                              # Lean IDL/clients
cli/                              # Admin CLI
docs/                             # Public docs
```

## Demo: Pay-per-Content Unlock with Oracle-Priced Rewards

Fully on-chain, Token-2022 compatible flow using the deployed programs (mainnet-beta):

1. User pays 0.01 USDC via x402 rail  
2. Switchboard SOL/USD feed pulled in same tx  
3. Attention Oracle mints oracle-priced engagement reward to payer  
4. Optional: same CPI mints access-pass NFT  
5. Atomic, verifiable, no off-chain trust.

**Live example tx** (0.01 USDC → reward at current SOL price): _insert after run_

**TypeScript (works today):**

```ts
import { Connection, PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import { BN } from "bn.js";
import { x402Program } from "./idl/x402_on_demand"; // or generated IDL

const connection = new Connection("https://api.mainnet-beta.solana.com");
const payer = /* your Keypair */;

const tx = new Transaction();

tx.add(
  await x402Program.methods
    .settleX402Payment(new BN(10_000)) // 0.01 USDC
    .accounts({
      paymentSession: /* [ "session", payer.key() ] PDA */,
      from: /* payer USDC ATA */,
      mint: USDC_MINT,
      to: /* protocol USDC ATA */,
      authority: payer.publicKey,
      switchboardFeed: new PublicKey("GvDMxPzN1sCj7KqQp6AJj3C52e2iWp6VATnCjud7i3Ha"), // SOL/USD
      tokenProgram: TOKEN_2022_PROGRAM_ID or TOKEN_PROGRAM_ID,
      rewardMint: /* reward mint */,
      rewardAta: /* payer reward ATA */,
      protocolAuthority: /* [ "global" ] PDA */,
      systemProgram: SystemProgram.programId,
    })
    .instruction()
);

const sig = await connection.sendTransaction(tx, [payer]);
console.log(`https://solscan.io/tx/${sig}`);
```

## Verification (deterministic)

```bash
solana-verify build --library-name attention_oracle_token_2022
solana-verify verify-from-repo https://github.com/twzrd-sol/attention-oracle-program \
  --library-name attention_oracle_token_2022 \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --commit-hash $(git rev-parse HEAD) \
  --remote
```

Or legacy:

```bash
anchor verify -p token_2022 GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

## Security

security@twzrd.xyz

MIT OR Apache-2.0
