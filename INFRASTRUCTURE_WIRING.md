# Infrastructure Wiring - Complete

**Status:** ✅ Ready for Aggregator Launch
**Date:** 2025-11-21
**Network:** Solana Mainnet-Beta

---

## 🎯 Summary

The infrastructure layer for the Attention Oracle economy has been fully configured and is ready for the aggregator daemon to start submitting events.

### Deployed Addresses

| Component | Address | Status |
|-----------|---------|--------|
| **Attention Token Mint** | `ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe` | ✅ Live |
| **Oracle Program** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | ✅ Deployed |
| **Transfer Hook Program** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | ✅ Enabled |
| **Protocol State PDA** | `FcyWuzYhxMnqPBvnMPXyyYPjpRvaweWku2qQo1a9HtuH` | ✅ Initialized |
| **Fee Config PDA** | `6hXmmQWQrygTVXK7ad4FFdinTcThQFVjTtnimfdpT4JC` | ✅ Configured |

---

## 📁 Files Created/Modified

### 1. Aggregator Configuration
**Location:** `~/private_twzrd/twzrd-aggregator-rs/.env`

```bash
# Oracle Program (The Brain)
AO_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Attention Token Mint (The Economy)
ATTENTION_MINT=ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe

# Publisher Keypair (MUST be in Oracle's allowlist)
PAYER_KEYPAIR=~/.config/solana/aggregator-us.json

# RPC
RPC_URL=https://api.mainnet-beta.solana.com
```

**Status:** ✅ Created

---

### 2. Passport Client Library
**Location:** `~/milo-token/apps/twzrd-interface/lib/passport-client.ts`

**Exports:**
- `ORACLE_PROGRAM_ID` - Oracle program address
- `ATTENTION_MINT` - Token mint address
- `PROTOCOL_STATE_PDA` - Protocol state PDA
- `getNodeScorePDA(user)` - Derive user's NodeScore PDA
- `getEpochRootPDA(channelHash, epoch)` - Derive EpochRoot PDA
- `fetchNodeScore(user)` - Query on-chain NodeScore
- `formatAttentionPoints(lamports)` - Display formatting

**Status:** ✅ Created

---

### 3. Smoke Test Script
**Location:** `~/milo-token/scripts/smoke_test_oracle.ts`

**Purpose:** Verify aggregator configuration before launching daemon

**Run:**
```bash
cd ~/milo-token
KEYPAIR_PATH=~/.config/solana/aggregator-us.json npx ts-node scripts/smoke_test_oracle.ts
```

**Checks:**
- ✓ Oracle program is deployed
- ✓ RPC connection is working
- ✓ Aggregator keypair has sufficient balance
- ✓ Configuration is consistent

**Status:** ✅ Created

---

### 4. Deployment Script (from previous step)
**Location:** `~/milo-token/scripts/init_economy.ts`

**Status:** ✅ Completed - Mint deployed

**Transaction:** `C9eG7SkqfXCgAqwiLwzr5qmtFxbanEFNQeEYv5MtZX8nbKVh84SoLoyaoLgo6iYgmNToZ4zBy34jUgxbgV3SsHQ`

---

## 🚀 Next Steps: Launch the Aggregator

### Step 1: Run Smoke Test

```bash
cd ~/milo-token
KEYPAIR_PATH=~/.config/solana/aggregator-us.json npx ts-node scripts/smoke_test_oracle.ts
```

**Expected Output:**
```
✅ SMOKE TEST PASSED (Configuration Verified)
   Oracle Program: GnGzNds... ✓
   RPC Connection: https://api.mainnet-beta.solana.com ✓
   Authority Keypair: <your-key> ✓
   Balance: X.XX SOL ✓
```

---

### Step 2: Launch Aggregator Daemon

```bash
cd ~/private_twzrd/twzrd-aggregator-rs
RUST_LOG=info,twzrd_aggregator_rs=debug cargo run --release
```

**What to Expect:**
1. `👁️ Connected to Lofi Girl (@lofiirl)`
2. `📥 Buffer: N events...`
3. `📦 Batching 60s window...`
4. `🚀 Submitting Batch to Oracle...`
5. `✅ Confirmed. Tx: <signature>`

**Monitor:**
- First submission will test the authority allowlist
- If authorized, you'll see: `✅ Confirmed`
- If NOT authorized, you'll see: `❌ Error: custom program error: 0xXXX`

---

### Step 3: Verify On-Chain Updates

After the aggregator has made its first submission, verify NodeScore PDAs are updating:

```bash
# Check a test user's NodeScore PDA
solana account <PDA_ADDRESS> --url https://api.mainnet-beta.solana.com
```

Or use the passport client:

```typescript
import { fetchNodeScore } from './lib/passport-client';
import { PublicKey } from '@solana/web3.js';

const user = new PublicKey('<user-wallet>');
const score = await fetchNodeScore(user);
console.log('NodeScore:', score);
```

---

## 🔧 Configuration Reference

### Environment Variables

| Variable | Value | Location |
|----------|-------|----------|
| `AO_PROGRAM_ID` | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Aggregator `.env` |
| `ATTENTION_MINT` | `ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe` | Aggregator `.env` |
| `PAYER_KEYPAIR` | `~/.config/solana/aggregator-us.json` | Aggregator `.env` |
| `RPC_URL` | `https://api.mainnet-beta.solana.com` | Aggregator `.env` |
| `NEXT_PUBLIC_RPC_URL` | `https://api.mainnet-beta.solana.com` | UI `.env.local` |

### PDA Seeds Reference

```typescript
// NodeScore PDA
seeds: ["node_score", user_pubkey]
program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

// EpochRoot PDA
seeds: ["epoch_root", keccak256(channel), epoch_u64]
program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

// Protocol State PDA (singleton)
seeds: ["protocol"]
program: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

---

## 📊 System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    SOLANA MAINNET-BETA                       │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  Token-2022 Mint                                     │   │
│  │  ESpcP35Waf5xuniehGopLULkhwNgCgDUGbd4EHrR8cWe        │   │
│  │  ├─ Transfer Hook: Enabled                           │   │
│  │  └─ Hook Program: GnGzNds...                         │   │
│  └──────────────────────────────────────────────────────┘   │
│                          ▲                                   │
│                          │                                   │
│  ┌──────────────────────┴───────────────────────────────┐   │
│  │  Oracle Program (GnGzNds...)                         │   │
│  │  ├─ update_root(channel, epoch, root, total_amount)  │   │
│  │  ├─ claim(channel, epoch, index, amount, proof)      │   │
│  │  └─ NodeScore PDAs (per-user)                        │   │
│  └──────────────────────────────────────────────────────┘   │
│                          ▲                                   │
└──────────────────────────┼───────────────────────────────────┘
                           │
               ┌───────────┴───────────┐
               │                       │
    ┌──────────▼──────────┐  ┌────────▼─────────┐
    │  Aggregator Daemon  │  │  Passport UI     │
    │  (twzrd-aggreg...  │  │  (twzrd-inter... │
    │  ├─ YouTube Sidecar │  │  ├─ NodeScore    │
    │  ├─ 60s Submissions │  │  │   Display      │
    │  └─ Merkle Trees    │  │  └─ Claim UI     │
    └─────────────────────┘  └──────────────────┘
```

---

## ✅ Pre-Flight Checklist

Before starting the aggregator:

- [x] Token-2022 mint deployed
- [x] Transfer hook configured
- [x] Oracle program verified on-chain
- [x] Aggregator `.env` configured
- [x] Passport client library created
- [x] Smoke test script ready
- [x] Aggregator keypair funded (>0.01 SOL)
- [ ] **Run smoke test**
- [ ] **Launch aggregator daemon**
- [ ] **Verify first submission**

---

## 🆘 Troubleshooting

### Aggregator fails with "custom program error: 0xXXX"

**Cause:** Aggregator keypair not in Oracle's allowlist

**Solution:**
1. Verify the keypair at `PAYER_KEYPAIR` is correct
2. Check with Oracle deployer that this pubkey is authorized
3. If using multi-sig, ensure all signers are available

### NodeScore PDAs not updating

**Cause:** Aggregator not submitting, or submissions failing silently

**Solution:**
1. Check aggregator logs: `RUST_LOG=debug cargo run`
2. Verify RPC connectivity
3. Check wallet balance for gas fees
4. Inspect transaction signatures on Solscan

### Passport UI shows "No score found"

**Cause:** User hasn't participated in any scored events yet

**Expected Behavior:** NodeScore PDAs are only created after a user's first scored event

---

## 📝 Notes

- The protocol state is a **singleton** (not keyed by mint)
- Extra account metas are initialized on **first transfer** (standard Token-2022 pattern)
- The aggregator uses **Keccak256** for channel hashing (not SHA256)
- Epoch numbers are **u64** (8 bytes, little-endian)

---

**Ready to launch!** 🚀

Once the smoke test passes, start the aggregator and watch the on-chain economy breathe.
