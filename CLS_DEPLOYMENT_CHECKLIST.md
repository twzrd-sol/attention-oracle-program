# CLS Launch Checklist – Proof of Builder Distribution

## Pre-Flight Checks

- [ ] Program deployed to devnet (record program ID)
- [ ] Program deployed to mainnet (record program ID)
- [ ] E2E test passing on localhost (run `scripts/e2e-direct-manual.ts`)
- [ ] Presentation deck reviewed and finalized
- [ ] Demo script tested locally (`scripts/claim-demo.ts`)
- [ ] Proof JSON generated from aggregator for target audience

---

## DEVNET DEPLOYMENT (Testnet Proof)

Target Audience: ZoWzrd, Justin, + optional community builders

### Step 1: Create CLS Mint

```bash
# Generate new keypair for CLS mint
solana-keygen new -o cls-mint.json

# Get mint pubkey
solana-keygen pubkey cls-mint.json

# Create Token-2022 mint with transfer fee extension
# (Use e2e script as reference for mint creation)
# Mint decimals: 9
# Transfer fee: 1% (100 basis points) or customizable
```

**Devnet Mint ID:** `___________` (to be filled)

### Step 2: Initialize Protocol

```bash
export CLAIM_JSON=../path/to/cls-claim-export-devnet.json
export RPC_URL=https://api.devnet.solana.com
export ANCHOR_WALLET=~/.config/solana/id.json

# Run initialization (part of e2e script)
# Steps:
# 1. Create mint with transfer fee
# 2. Initialize protocol state (permissionless)
# 3. Set publisher authority
# 4. Initialize channel
```

**Protocol PDA:** `___________` (to be filled)
**Channel PDA:** `___________` (to be filled)
**Treasury ATA:** `___________` (to be filled)

### Step 3: Publish Merkle Root

```bash
# Generate micro-epoch root from aggregator
# Include: ZoWzrd (100 CLS), Justin (100 CLS), optional others
# Epoch: Unix timestamp or sequential (e.g., 1)

# Publish root via set_merkle_root_ring instruction
# Root will be stored in channel state
```

**Epoch:** `___________` (to be filled)
**Root:** `___________` (to be filled)
**Claim Count:** `___________` (to be filled, e.g., 3)

### Step 4: Fund Treasury

```bash
# Calculate total CLS needed
total = sum(all_claims) = ZoWzrd(100) + Justin(100) + others(X)

# Mint to treasury ATA
# Amount: total * 1.01 (buffer for transfer fee)
```

**Treasury Funding TX:** `___________` (to be filled)
**Total Minted:** `___________` (to be filled)

### Step 5: Live Demo on Devnet

```bash
# For each claimer (ZoWzrd, Justin):
export CLAIM_JSON=../path/to/individual-claim.json
export RPC_URL=https://api.devnet.solana.com

tsx scripts/claim-demo.ts

# Expected output:
# ✅ Claim successful!
# Received: <amount * 0.99> tokens (after 1% fee)
```

**Demo TX Signatures:**
- ZoWzrd: `___________`
- Justin: `___________`

### Step 6: Verify on Devnet

```bash
# Check treasury balance decrease
solana account <TREASURY_ATA> --url devnet

# Check claimer balance increase
solana account <CLAIMER_ATA> --url devnet

# Try double-claim (should fail)
tsx scripts/claim-demo.ts
# Expected: AlreadyClaimed error
```

**Treasury Balance After:** `___________`
**ZoWzrd Balance:** `___________`
**Justin Balance:** `___________`

---

## MAINNET DEPLOYMENT (Production Launch)

### Step 1: Create CLS Mint on Mainnet

```bash
# Use same process as devnet
# Consider whether to reuse same mint or create new
# (Recommended: same mint for consistency, different treasury)
```

**Mainnet Mint ID:** `___________` (to be filled)

### Step 2: Initialize Protocol on Mainnet

```bash
export RPC_URL=https://api.mainnet-beta.solana.com
export ANCHOR_WALLET=~/.config/solana/id.json

# Repeat devnet steps 2-4 on mainnet
```

**Mainnet Protocol PDA:** `___________`
**Mainnet Channel PDA:** `___________`
**Mainnet Treasury ATA:** `___________`

### Step 3: Publish Root & Fund Treasury on Mainnet

```bash
# Use same epoch and root as devnet (for consistency)
# Or create new epoch if distribution has changed
```

**Mainnet Epoch:** `___________`
**Mainnet Root:** `___________`
**Mainnet Treasury TX:** `___________`

### Step 4: Announcement & Go-Live

```bash
# Publish claim instructions to: ZoWzrd, Justin, + community
# Include:
# - Claim JSON (proof)
# - Mint address (CLS token)
# - Channel address
# - RPC endpoint (https://api.mainnet-beta.solana.com)
# - Claim script: scripts/claim-demo.ts

# Example message:
# "CLS token now claimable! Your proof is ready at [url].
#  Run: CLAIM_JSON=[proof-url] RPC_URL=[mainnet] tsx claim-demo.ts
#  More info: [docs-link]"
```

**Announcement Date/Time:** `___________`
**Social Media Posts:** `___________`

---

## ONGOING OPS

### Weekly/Monthly Epochs

- [ ] Collect new builder metrics (streams, contributions, etc.)
- [ ] Generate aggregator input (CSV or JSON)
- [ ] Build new Merkle tree
- [ ] Publish new root on-chain (same channel, new epoch)
- [ ] Fund treasury for new epoch
- [ ] Announce distribution and share proofs
- [ ] Monitor claim submissions and resolve issues

### Monitoring

```bash
# Watch for claim submissions
solana logs <PROGRAM_ID> --url <RPC>

# Monitor treasury balance
solana account <TREASURY_ATA> --url <RPC>

# Query epoch state
solana account <CHANNEL_PDA> --url <RPC>
```

### Escalations

- Double-claim attempt → Expected behavior (AlreadyClaimed)
- Proof verification fail → Regenerate proof from aggregator
- Balance mismatch → Check transfer fee calculation
- Missing ATA → Script auto-creates (costs small SOL)

---

## DOCUMENTATION & COMMS

- [ ] CLS token page (mint address, distribution schedule)
- [ ] Claim instructions (step-by-step guide)
- [ ] FAQ (What is CLS? How do I claim? Transfer fees?)
- [ ] Developer docs (Merkle tree structure, proof format)
- [ ] Changelog (new epochs, amounts, dates)

---

## Notes

- **Test Thoroughly on Devnet First**: Demo live claiming before mainnet launch
- **Communicate Clearly**: Builders need to know amount, deadline, and claim method
- **Monitor Gas Costs**: ATA creation (~5K SOL), claim submission (~10K SOL)
- **Plan for Batch Claims**: If multiple epochs, coordinate timing to avoid congestion
- **Archive Proofs**: Keep all epoch proofs for audit trail

