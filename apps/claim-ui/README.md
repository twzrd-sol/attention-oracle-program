# CLS Claim UI

**A minimal, trustless interface for claiming CLS tokens on Solana mainnet.**

## Quick Links

- **Full Guide**: See `CLS_CLAIM_UI.md`
- **Sample Proof**: See `sample-proof.json`
- **Program Source**: `../../programs/token-2022/src/instructions/merkle_ring.rs`

## What Is This?

A standalone React + Vite application for submitting token claims using Merkle tree proofs. No backend required—all verification happens on-chain.

## Features

- ✅ Upload or paste proof JSON
- ✅ Phantom wallet integration
- ✅ Live balance tracking (before/after)
- ✅ Direct on-chain verification (Merkle + bitmap guard)
- ✅ Clear error handling
- ✅ Transaction explorer link

## Get Started

### Run Locally

```bash
npm install
npm run dev
```

Then open `http://localhost:5173`

### Build for Production

```bash
npm install
npm run build
```

Output is in `dist/`.

## Usage

1. **Get Your Proof**: Contact the CLS team for your proof JSON
2. **Load Proof**: Upload the file or paste JSON
3. **Connect Wallet**: Click "Connect Wallet" → Phantom
4. **Submit**: Click "Submit Claim" and sign in Phantom
5. **Confirm**: Wait for transaction confirmation (~30s)

## Environment Variables

Optional `.env.local`:

```
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com
```

## Architecture

```
src/
├── App.tsx          (Entry point → ClaimCLS)
├── ClaimCLS.tsx     (Main claim component, ~400 lines)
├── App.css          (Styling)
└── main.tsx         (React init)
```

### Key Dependencies

- `@solana/web3.js` — Solana blockchain interaction
- `@solana/spl-token` — Token account operations
- `js-sha3` — keccak256 for streamer key derivation
- `react` + `vite` — UI framework & bundler

## How It Works

1. **Load Proof**: Parse and validate proof JSON
2. **Verify Wallet**: Ensure your wallet matches the `claimer` field
3. **Derive PDAs**: Compute protocol and channel state addresses
4. **Check Balance**: Fetch account balance before claim
5. **Build Instruction**: Manually construct `claim_with_ring` instruction
   - Discriminator: SHA256(`global:claim_with_ring`)
   - Args: epoch, index, amount, proof[], id, streamer_key
6. **Sign & Submit**: Send transaction via Phantom
7. **Confirm**: Wait for on-chain confirmation
8. **Display Result**: Show balance delta and explorer link

## Important Notes

- **Wallet Requirement**: Must have Phantom installed and connected
- **Mainnet Only**: Currently configured for Solana mainnet
- **Transfer Fee**: Token-2022 applies 1% transfer fee on token transfers
- **One Claim Per Epoch**: Double-claiming is rejected by the program

## Error Handling

| Error | Cause | Fix |
|-------|-------|-----|
| "Proof is for X, but you're using Y" | Wallet mismatch | Switch wallets in Phantom |
| "You already claimed this epoch" | Double-claim attempt | Wait for next epoch |
| "Invalid proof JSON: missing required fields" | Malformed JSON | Check all required fields are present |
| "AlreadyClaimed" | On-chain claim guard | Proof was already used |

## Development

### Adding Tests

Create `src/__tests__/ClaimCLS.test.tsx`:

```typescript
import { describe, it, expect } from 'vitest';
// Add tests here
```

### Modifying for Devnet

In `src/ClaimCLS.tsx`:

```typescript
const PROGRAM_ID = new PublicKey('YOUR_DEVNET_PROGRAM_ID');
const RPC_URL = 'https://api.devnet.solana.com';
```

Then test with devnet proofs.

## Deployment

### Vercel (Recommended)

```bash
npm run build
# Deploy dist/ folder to Vercel
```

### Self-Hosted

```bash
npm run build
npm run preview  # Test locally
# Serve dist/ folder with any static web server
```

## License

See `../../LICENSE`

## Resources

- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (mainnet)
- **RPC**: `https://api.mainnet-beta.solana.com`
- **Phantom Wallet**: `https://phantom.app`
- **Solana Explorer**: `https://explorer.solana.com`
