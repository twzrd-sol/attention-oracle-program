# Attention Oracle v1.1.0 - Deployment Guide

## Release Overview

**v1.1.0** represents the clean, generic infrastructure layer of the Attention Oracle protocol.

### Architecture Separation

```
┌─ PUBLIC REPOSITORY ─────────────────────────────────┐
│ Attention Oracle v1.1.0                             │
│ - EntityState (generic entities & signals)          │
│ - NodeScore (pure graph primitives)                 │
│ - Merkle tree claim system                          │
│ - Passport tier accumulation                        │
│ - Program ID: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop       │
│ (Fully verifiable, auditable, open-source)         │
└─────────────────────────────────────────────────────┘
                        ↓ CPI Interface
┌─ PRIVATE REPOSITORY ────────────────────────────────┐
│ Creator Bonds DeFi Engine (Separate Program ID)    │
│ - Bond issuance & purchase logic                    │
│ - Transfer hooks for reputation updates             │
│ - Fee distribution mechanics                        │
│ - Revenue sharing calculations                      │
│ (Deployed after OLUG integration testing)           │
└─────────────────────────────────────────────────────┘
```

## What's NOT in v1.1.0

- ❌ Transfer hooks (will live in Creator Bonds program)
- ❌ Revenue/fee distribution logic
- ❌ Bond issuance contracts
- ❌ Monetization mechanics
- ❌ Any creator-specific code

**Result**: Public oracle is truly generic. Can be used for any signal/entity system.

## What's in v1.1.0

- ✅ EntityState account (generic subject of observation)
- ✅ NodeScore account (inbound/outbound weights)
- ✅ Merkle-tree backed claims (provable signal accumulation)
- ✅ Passport tiers (reputation levels based on accumulated signals)
- ✅ CPI-exposed instruction set (integrable by other programs)

## Build & Deployment Sequence

### Step 1: Build Verifiable Binary

```bash
cd programs/token_2022
cargo build-sbf --release
# or for verifiable build:
anchor build --verifiable --arch sbf --program-name attention_oracle_program
```

**Output**: `target/deploy/token_2022.so` (or named binary)

### Step 2: Verify Locally (Optional)

```bash
# Verify hash matches on-chain deployment
sha256sum target/deploy/token_2022.so
# Compare against: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Or use solana-verify (if setting up fresh):
solana-verify verify-from-repo \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --commit v1.1.0 \
  twzrd-sol/attention-oracle-program
```

### Step 3: Deploy to Mainnet

**If upgrading existing program:**

```bash
solana program deploy \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --upgrade-authority ~/.config/solana/id.json \
  --keypair ~/.config/solana/id.json \
  target/deploy/token_2022.so
```

**Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` (same as before)

**Cost**: ~6-10 SOL + transaction fees

### Step 4: Devnet Testing (Recommended First)

```bash
# Deploy to devnet
solana program deploy \
  --url devnet \
  --keypair ~/.config/solana/id.json \
  target/deploy/token_2022.so

# Note the Program ID output

# Initialize protocol
solana program show <PROGRAM_ID> --url devnet
# Should show recent deployment slot

# Test claims, passport minting, etc. on devnet first
```

## Post-Deployment: Creator Bonds Integration

Once Attention Oracle v1.1.0 is stable on mainnet:

1. **Deploy Creator Bonds** (separate program, private repo)
   - New Program ID (different from Attention Oracle)
   - CPI calls to Attention Oracle for NodeScore updates

2. **Integration Flow**
   ```
   User buys bond → Creator Bonds contract
     ↓ (CPI call)
   Updates NodeScore in Attention Oracle
     ↓ (if hook enabled)
   Transfer hook records reputation update
   ```

3. **Public Announcement**
   > "Creator Bonds is a DeFi primitive that uses Attention Oracle to measure creator fan engagement.
   > Fan bonds are backed by creator revenue, distributed proportionally to bondholders.
   > Transparent on-chain: [Solscan link to program]"

## Transparency & Honesty

### What Users See (Post-Launch)
- **On-chain**: Creator Bonds contract (fully auditable)
- **Docs**: "Bonds measure creator engagement via Merkle proofs of platform activity"
- **Code**: All logic open-source (both Oracle + Bonds)
- **Solscan**: Full transaction history, no private components

### What This Is NOT
- ❌ No "plausible deniability"
- ❌ No hidden monetization layer
- ❌ No deceptive naming (Creator Bonds is called Creator Bonds)
- ❌ No transfer hooks disguised as "governance" features

### What This IS
- ✅ Honest separation of concerns (infrastructure + monetization)
- ✅ Pure infrastructure publicly available first
- ✅ DeFi engine built separately, tested thoroughly
- ✅ Full transparency when launched

## Environment Setup

Before deployment, ensure:

```bash
# Solana CLI installed and configured
solana --version
solana config get

# Set to mainnet-beta for production
solana config set --url https://api.mainnet-beta.solana.com

# Keypair configured
solana config get | grep "Keypair Path"

# Sufficient SOL for deployment
solana balance
```

## Rollback Plan

If issues arise post-deployment:

1. **Pause Protocol** (if admin authority allows)
   ```bash
   # Call pause_protocol instruction
   # Requires admin signer
   ```

2. **Upgrade to New Version**
   ```bash
   # Build fixed version
   cargo build-sbf --release

   # Deploy upgrade
   solana program deploy \
     --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
     --upgrade-authority ~/.config/solana/id.json \
     target/deploy/token_2022.so
   ```

3. **Notify Community**
   - Post upgrade announcement on Discord/Twitter
   - Link to verifiable commit (git tag v1.1.0)
   - Explain changes made

## Success Criteria

Post-deployment, verify:

- [ ] Program deployed to mainnet (check Solscan)
- [ ] Protocol initializable via `initialize_mint`
- [ ] Merkle claims processable (`claim_channel_open`)
- [ ] Passports mintable (`mint_passport_open`)
- [ ] NodeScore updates work
- [ ] Events emitted correctly (check Helius/Magic Eden indexers)

## Support & Verification

**If you need to verify this code:**

```bash
# Anyone can verify the deployed program matches the v1.1.0 tag:
git clone https://github.com/twzrd-sol/attention-oracle-program.git
git checkout v1.1.0
cargo build-sbf --release
sha256sum target/deploy/token_2022.so
# Compare against on-chain hash
```

**No third parties required. Pure mathematics.**

---

**Generated**: November 21, 2025
**Version**: v1.1.0
**Status**: Ready for Mainnet Deployment
**Author**: Built with Claude Code + Cypherpunk Principles
