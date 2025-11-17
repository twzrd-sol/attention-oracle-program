# Attention Oracle Claim Portal

**A trustless, Anchor-powered interface for claiming creator tokens from Twitch channel rewards.**

**Status**: ✅ Phase 2 Complete - 4-component claim flow with Anchor integration

## Quick Links

- **Architecture**: See `ANCHOR_HOOKS_STRUCTURE.md`
- **Program Source**: `../../programs/token-2022/src/instructions/merkle_ring.rs`
- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (mainnet)

## What Is This?

A React + TypeScript + Anchor application implementing a 4-step claim flow for Attention Oracle tokens:

1. **ProofUpload** - Load merkle proof from file or JSON
2. **WalletConnect** - Connect Phantom/Solflare/Torus wallet
3. **ClaimReview** - Review claim details and fee breakdown
4. **ClaimExecution** - Sign and submit on-chain claim transaction

All verification happens on-chain via Anchor IDL-driven program calls. No backend required.

## Features

- ✅ **Anchor Integration**: IDL-driven instruction calls
- ✅ **4-Step Flow**: Intuitive multi-step wizard with progress stepper
- ✅ **Multi-Wallet Support**: Phantom, Solflare, Torus
- ✅ **Comprehensive Validation**: Proof format, address matching, fee calculations
- ✅ **Real-time Fee Breakdown**: Treasury (0.05%) + Creator (0.05% × tier) display
- ✅ **Transaction Explorer Link**: View claim on Solscan
- ✅ **Address Verification**: Prevents claiming with wrong wallet
- ✅ **Error Handling**: Detailed messages for all failure modes

## Quick Start

### Run Locally

```bash
npm install
npm run dev
```

Opens at `http://localhost:3000`

### Build for Production

```bash
npm install
npm run build
npm run preview
```

Output is in `dist/`

## Usage

### Step 1: Load Proof
- Upload a `.json` file **or** paste JSON directly
- Proof is validated for required fields
- See "Proof JSON Format" below

### Step 2: Connect Wallet
- Click "Connect Wallet"
- Select Phantom, Solflare, or Torus
- Wallet address must match proof's `claimer` field

### Step 3: Review Claim
- Verify claim details (channel, epoch, amount)
- See fee breakdown:
  - **Gross**: Your claim amount
  - **Treasury Fee**: 0.05% (fixed)
  - **Creator Fee**: 0.05% × tier multiplier
  - **Net**: Amount after fees
- Address must match to proceed

### Step 4: Execute
- Click "Execute Claim"
- Sign transaction in your wallet
- Watch for on-chain confirmation
- View transaction on Solscan
- Tokens arrive in your wallet

## Environment Variables

Optional `.env`:

```bash
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com
VITE_SOLANA_NETWORK=mainnet-beta
VITE_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

## Project Structure

```
src/
├── hooks/                    # Anchor hooks
│   ├── useAnchorProgram.ts  # Program initialization
│   ├── useWallet.ts         # Wallet adapter
│   └── useMerkleProof.ts    # Proof validation
│
├── components/              # 4-step flow components
│   ├── ProofUpload.tsx      # Step 1
│   ├── WalletConnect.tsx    # Step 2
│   ├── ClaimReview.tsx      # Step 3
│   └── ClaimExecution.tsx   # Step 4
│
├── lib/                      # Utilities
│   ├── constants.ts         # Program IDs, tiers
│   └── instructions.ts      # Instruction builders
│
├── context/
│   └── WalletProvider.tsx   # Solana wallet adapter setup
│
├── App.tsx                  # 4-step orchestrator + stepper UI
├── main.tsx                 # Vite entry point
└── index.css                # Global styles

idl/
└── token-2022.json          # Anchor IDL

index.html                   # HTML entry point
package.json
tsconfig.json
vite.config.ts
```

### Key Dependencies

- `@coral-xyz/anchor` — Anchor framework
- `@solana/web3.js` — Solana blockchain
- `@solana/spl-token` — Token operations
- `@solana/wallet-adapter-react` — Wallet integration
- `react` + `vite` — UI framework

## Proof JSON Format

Required fields in proof JSON:

```json
{
  "claimer": "9B5X...",          // Your Solana wallet address (base58)
  "mint": "GngzN...",            // Token mint address (base58)
  "channel": "twitch_channel",   // Twitch channel name
  "epoch": 42,                   // Epoch number (integer)
  "index": 100,                  // Claim index (integer)
  "amount": "1000000000",        // Tokens (as string, >0)
  "root": "aabbccdd...",         // 32-byte hex string
  "proof": ["0x1234...", "..."], // Array of 32-byte hex strings
  "id": "claim_id_hex"           // 32-byte hex string
}
```

### Field Validation

- `claimer`: Valid Solana PublicKey (base58 format)
- `mint`: Valid Solana PublicKey
- `channel`: Non-empty string
- `epoch`: Positive integer
- `index`: Non-negative integer
- `amount`: Valid BigInt string (>0)
- `root`: 64-character hex string (32 bytes)
- `proof`: Array of 64-character hex strings
- `id`: 64-character hex string (32 bytes)

## Important Notes

- **Wallet Requirement**: Phantom, Solflare, or Torus must be installed
- **Mainnet Only**: Currently configured for Solana mainnet
- **Address Matching**: Wallet address must match proof's `claimer` field
- **One Claim Per Epoch**: Attempting to claim twice is rejected
- **Fee Breakdown**: Displayed before execution (0.05% treasury + creator)

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| "Wallet address does not match proof" | Connected wallet ≠ claimer | Switch to correct wallet |
| "Missing required program, provider, wallet, or proof" | Incomplete setup | Wait for all to load |
| "Failed to build claim instruction" | Invalid proof data | Verify proof JSON format |
| "Failed to submit transaction" | Wallet rejection or network issue | Check wallet & retry |
| "AlreadyClaimed" | Proof already used | Get a new proof |

## Hooks Reference

### `useAnchorProgram()`
Initializes Anchor program with wallet and RPC connection.

```typescript
const { program, provider, isReady, error } = useAnchorProgram();
```

### `useWallet()`
Wraps wallet adapter, provides connection state.

```typescript
const { address, connected, connect, disconnect } = useWallet();
```

### `useMerkleProof()`
Loads, validates, and parses merkle proof JSON.

```typescript
const { proof, loading, error, loadProofFromFile, getSummary } = useMerkleProof();
```

See `ANCHOR_HOOKS_STRUCTURE.md` for detailed hook documentation.

## Development

### Modifying for Devnet

In `src/lib/constants.ts`:

```typescript
export const RPC_URL = 'https://api.devnet.solana.com';
export const NETWORK = 'devnet';
export const PROGRAM_ID = new PublicKey('YOUR_DEVNET_PROGRAM_ID');
```

Then rebuild and test with devnet proofs.

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
# Serve dist/ with any static web server (nginx, Apache, etc.)
```

## License

See `../../LICENSE`

## Resources

- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (mainnet)
- **RPC**: `https://api.mainnet-beta.solana.com`
- **Phantom Wallet**: `https://phantom.app`
- **Solana Explorer**: `https://explorer.solana.com`
