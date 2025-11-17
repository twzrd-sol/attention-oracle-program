# Anchor Hooks Architecture - claim-ui

**Date**: 2025-11-15
**Status**: ✅ Phase 2 Complete - All 4 claim flow components scaffolded with Anchor integration
**Target**: Replace web3.js manual transactions with @coral-xyz/anchor IDL-driven calls

---

## 📁 Directory Structure

```
apps/claim-ui/
├── src/
│   ├── hooks/                      ← React hooks (NEW)
│   │   ├── useAnchorProgram.ts     ← Initialize Anchor program + provider
│   │   ├── useWallet.ts            ← Wallet adapter integration
│   │   ├── useMerkleProof.ts       ← Proof validation + parsing
│   │   └── index.ts                ← Barrel exports
│   │
│   ├── context/                    ← Context providers (NEW)
│   │   └── WalletProvider.tsx      ← Solana wallet adapter setup
│   │
│   ├── components/                 ← React components (NEW, Phase 2)
│   │   ├── ProofUpload.tsx         ← Step 1: Load proof file/JSON
│   │   ├── WalletConnect.tsx       ← Step 2: Connect wallet
│   │   ├── ClaimReview.tsx         ← Step 3: Review claim details
│   │   ├── ClaimExecution.tsx      ← Step 4: Execute & sign transaction
│   │   └── index.ts                ← Barrel exports
│   │
│   ├── lib/                        ← Utilities (NEW)
│   │   ├── constants.ts            ← Program IDs, network config
│   │   ├── instructions.ts         ← Anchor instruction builders
│   │   └── index.ts                ← Barrel exports
│   │
│   ├── App.tsx                     ← Main app orchestrator (NEW)
│   ├── main.tsx                    ← Vite entry point (NEW)
│   ├── index.css                   ← Global styles (NEW)
│   └── ClaimCLS.tsx                ← Legacy (deprecated)
│
├── idl/                            ← Program IDL (NEW)
│   └── token-2022.json             ← Extracted from mainnet program
│
├── index.html                      ← HTML entry point (NEW)
├── package.json                    ← Updated with Anchor + wallet-adapter
├── tsconfig.json                   ← TypeScript config (NEW)
├── vite.config.ts                  ← Vite build config (NEW)
└── ANCHOR_HOOKS_STRUCTURE.md       ← This document

```

---

## 🪝 Hook Reference

### 1. **useAnchorProgram**
Initializes `AnchorProvider` with wallet + connection, loads IDL, creates `Program` instance.

```typescript
const { program, provider, loading, error, isReady } = useAnchorProgram();

// When ready:
if (isReady) {
  // program is Program<Token2022IDL>
  // Can call: program.methods.claimWithRing(...)
}
```

**Returns**:
- `program`: Anchor Program instance (methods available)
- `provider`: AnchorProvider (for RPC calls)
- `isReady`: Boolean (program + provider initialized)
- `loading`: Boolean (initialization in progress)
- `error`: String | null (error message)
- `refresh()`: Reinitialize (e.g., after wallet change)

---

### 2. **useWallet**
Wraps Solana wallet-adapter, provides connection state + methods.

```typescript
const { address, shortAddress, connected, connect, disconnect, isReady } = useWallet();

if (!connected) {
  <button onClick={() => connect('Phantom')}>Connect Wallet</button>
}
```

**Returns**:
- `address`: PublicKey | null (connected wallet)
- `shortAddress`: String | null (truncated for UI: "Abc...XYZ")
- `connected`: Boolean (wallet connected)
- `connecting`: Boolean (connection in progress)
- `disconnecting`: Boolean (disconnection in progress)
- `isReady`: Boolean (ready to use)
- `connect(walletName?)`: Promise<void> (connect wallet)
- `disconnect()`: Promise<void> (disconnect wallet)
- `error`: String | null (error message)

---

### 3. **useMerkleProof**
Loads, parses, validates merkle proof JSON. Converts to on-chain format.

```typescript
const { proof, loading, error, loadProofFromFile, getProofBytes, isLoaded } = useMerkleProof();

// Load from file
await loadProofFromFile(file);

// Or from JSON string
loadProofFromJSON(jsonString);

// Get proof bytes for instruction
const proofBytes = getProofBytes();

// Get summary
const summary = getSummary();
// { channel, epoch, claimer, amount, proofDepth }
```

**Returns**:
- `proof`: MerkleProof | null (parsed proof data)
- `loading`: Boolean (parsing in progress)
- `error`: String | null (validation error)
- `isLoaded`: Boolean (proof ready)
- `loadProofFromFile(file)`: Promise<void>
- `loadProofFromJSON(string)`: void
- `getProofBytes()`: Uint8Array[] (for Anchor instruction)
- `getSummary()`: ProofSummary | null
- `clearProof()`: void

---

## 📦 Utility Functions

### **constants.ts**
```typescript
PROGRAM_ID                  // GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
TOKEN_2022_PROGRAM_ID      // TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBP4nEde2Kyn
RPC_URL                    // https://api.mainnet-beta.solana.com
NETWORK                    // 'mainnet-beta' | 'devnet' | 'testnet'
TIER_MULTIPLIERS           // { 0: 0, 1: 20, ... 5: 100 }
TRANSFER_FEE_BPS           // 100 (1%)
```

### **instructions.ts**
```typescript
deriveProtocolPDA(mint)           // → [PublicKey, bump]
deriveChannelStatePDA(mint, key)  // → [PublicKey, bump]
deriveStreamerKey(channel)        // → PublicKey (keccak256 hash)

buildClaimWithRingInstruction(program, proof, claimer, streamer)
  // → Transaction (not submitted, ready to sign)

submitClaimTransaction(provider, tx)
  // → Promise<string> (transaction signature)

calculateFees(amount, treasuryBps, creatorBps, multiplier)
  // → { gross, treasuryFee, creatorFee, totalFee, net }
```

---

## 🔗 Integration Example

```typescript
import { useAnchorProgram, useWallet, useMerkleProof } from '@hooks';
import { buildClaimWithRingInstruction, submitClaimTransaction } from '@lib';

export function ClaimFlow() {
  const { program, isReady } = useAnchorProgram();
  const { address, connected, connect } = useWallet();
  const { proof, isLoaded, loadProofFromFile } = useMerkleProof();

  const handleClaim = async () => {
    if (!program || !address || !proof) return;

    // Build instruction
    const tx = await buildClaimWithRingInstruction(
      program,
      proof,
      address,
      deriveStreamerKey(proof.channel)
    );

    // Submit
    const sig = await submitClaimTransaction(provider, tx);
    console.log('Claimed!', sig);
  };

  return (
    <>
      {!connected && <button onClick={() => connect()}>Connect</button>}
      {connected && <input type="file" onChange={e => loadProofFromFile(e.target.files![0])} />}
      {isLoaded && <button onClick={handleClaim} disabled={!isReady}>Claim</button>}
    </>
  );
}
```

---

## 🎯 Implementation Priority

### Phase 1: Complete (✅)
- [x] Hook structure (useAnchorProgram, useWallet, useMerkleProof)
- [x] IDL extraction (token-2022.json)
- [x] Utilities (constants, instructions)
- [x] WalletProvider context

### Phase 2: Complete (✅)
- [x] Component breakdown (ProofUpload → WalletConnect → Review → Execution)
- [x] App.tsx orchestrator with stepper UI
- [x] Global styles & HTML entry point
- [ ] Refactor ClaimCLS.tsx to use hooks (optional, deprecated)
- [ ] Discord verification integration (Phase 3)
- [ ] PDA bonus mint instruction (Phase 3)

### Phase 3: Testing (⏳)
- [ ] Devnet deployment
- [ ] E2E claim flow testing
- [ ] Error handling & recovery
- [ ] Live transaction signing

---

## 🔧 Environment Variables

```bash
# .env
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com
VITE_SOLANA_NETWORK=mainnet-beta
VITE_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

---

## ✅ Verification Checklist

### Phase 1: Hooks & Infrastructure
- [x] useAnchorProgram hook created + tested
- [x] useWallet hook created + tested
- [x] useMerkleProof hook created + tested (comprehensive validation)
- [x] WalletProvider context created
- [x] Constants defined
- [x] Instruction builders defined
- [x] Package.json updated with all dependencies
- [x] TypeScript config created
- [x] Vite config created

### Phase 2: Components & App
- [x] ProofUpload component (file upload, JSON paste, validation)
- [x] WalletConnect component (wallet selection, address verification)
- [x] ClaimReview component (claim details, fee breakdown)
- [x] ClaimExecution component (transaction signing, confirmation)
- [x] App.tsx orchestrator (4-step flow, stepper UI)
- [x] Components index exports
- [x] main.tsx entry point
- [x] index.html entry point
- [x] index.css global styles

### Phase 3: Discord Integration & Testing
- [ ] Discord OAuth login flow
- [ ] Passport tier verification
- [ ] PDA bonus mint instruction
- [ ] Devnet deployment & testing
- [ ] E2E claim flow testing
- [ ] Live transaction signing on devnet

---

## 🎯 4-Step Claim Flow

1. **ProofUpload** (Step 1)
   - Load merkle proof from file or JSON
   - Validate proof structure
   - Parse and display summary
   - Callback: `onProofLoaded()`

2. **WalletConnect** (Step 2)
   - Show wallet selection modal
   - Verify wallet address matches proof claimer
   - Display address mismatch warning if needed
   - Callback: `onConnected()` → auto-advance to Step 3

3. **ClaimReview** (Step 3)
   - Display claim details (channel, epoch, amount, claimer)
   - Show amount breakdown (gross → treasury fee → creator fee → net)
   - Verify address match
   - Callback: `onProceed()` → advance to Step 4

4. **ClaimExecution** (Step 4)
   - Build `claimWithRing` instruction
   - Get wallet signature
   - Submit transaction
   - Poll for confirmation
   - Display transaction signature link
   - Callback: `onSuccess(signature)` → completion screen

---

**Next**: Discord verification integration & devnet testing
