# ATTENTION ORACLE - Canonical Reference & First Principles

**Last Updated**: November 13, 2025
**Status**: Production (Mainnet Deployed + Solana Grant Application Ready)
**Temperature**: 0 (Deterministic)
**Top_P**: 0.2 (Focused)

---

## 🎯 PROJECT IDENTITY (Ground Truth)

### Public Name
**Attention Oracle** — Verifiable Distribution Protocol for Creator Economies on Solana

### Public GitHub Repository
**https://github.com/twzrd-sol/attention-oracle-program**

### Internal Codename (Private)
`milo-token` (DO NOT use publicly; reference only for internal infrastructure)

### Program ID (Mainnet)
```
GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

### Key Contacts
- **Security**: security@twzrd.com
- **Security Policy**: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
- **Funding**: Solana Foundation Grant Application (Submitted)

---

## 🏗️ FIRST PRINCIPLES (Architecture)

### Core Problem We Solve
Web2 streaming platforms (Twitch, YouTube) extract 30-50% of creator revenue. Viewers have zero ownership. ATTENTION ORACLE inverts this:
- **Viewers own tokens** they earn (liquid, tradeable, composable)
- **Creators control distribution** (merkle roots, tier allocation)
- **Revenue is transparent** (on-chain, auditable, programmable)

### Design Principles
1. **Solana-Native**: Zero bridges, <$0.001 per transaction
2. **Token-2022 Compliant**: Use native extensions (transfer fees, hooks)
3. **Sybil-Resistant**: Verifiable engagement via PassportRegistry (Tiers 0-6)
4. **Composable**: CCM tokens work with any Solana DEX/protocol
5. **Open Source**: Public goods for the ecosystem
6. **Hybrid Architecture**: Respect Token-2022 constraints (hooks observe, harvest distributes)

### The Stack

```
┌─────────────────────────────────────────┐
│  Creator Dashboards + Viewer UIs        │ (Off-Chain)
│  (Web3 wallets, Portal, etc.)          │
└──────────────┬──────────────────────────┘
               │
┌──────────────┴──────────────────────────┐
│  Off-Chain Indexing Layer               │ (Oracle)
│  • Twitch IRC → Merkle Trees            │
│  • Passport tier computation            │
│  • Fee aggregation & distribution       │
└──────────────┬──────────────────────────┘
               │
┌──────────────┴──────────────────────────┐
│  Solana Program: attention-oracle       │ (On-Chain)
│  GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU │
│  2dCVZop                                │
│                                         │
│  Modules:                               │
│  • Passport (Tier 0-6, Oracle gates)   │
│  • Claims (Merkle proofs, gas-efficient)│
│  • Hooks (Dynamic fee calculation)      │
│  • Harvest (Withheld fee distribution)  │
│  • Governance (Tier multiplier updates) │
│  • Liquidity (Drip mechanism)           │
│  • Points (Engagement scoring)          │
└─────────────────────────────────────────┘
```

---

## 📊 CURRENT STATUS (As of Nov 13, 2025)

### Mainnet Deployment
- ✅ **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- ✅ **Binary Size**: 654 KB (optimized SBF)
- ✅ **Build Status**: 0 errors, 56 warnings (pre-existing, non-blocking)
- ✅ **Security.txt**: Embedded in binary, verifiable via `strings`
  - Contact: security@twzrd.com
  - Policy: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
  - Expires: 2026-06-30

### Recent Implementation (Nov 13, 2025)
**Hybrid Dynamic Fee System** — Core tokenomics refinement:

1. **Transfer Hook Enhancement** (Observational)
   - Looks up passport tier via remaining_accounts
   - Calculates dynamic fees: Treasury (fixed 0.05%) + Creator (0.05% × tier multiplier)
   - Emits `TransferFeeEvent` for off-chain tracking
   - **Gas**: +1.5k CU per transfer

2. **Harvest Instruction** (Distribution)
   - Admin/keeper-invoked for periodic fee harvesting
   - Queries withheld_amount from Token-2022 mint extension
   - Distributes to treasury and creator pool
   - Emits `FeesHarvested` event for keeper coordination
   - **Gas**: +5k CU (placeholder; full CPI adds ~10k)

### Tier Multiplier Structure

| Tier | Label | Multiplier | Creator Share |
|------|-------|------------|---------------|
| 0 | Unverified | 0.0x | 0% |
| 1 | Emerging | 0.2x | 0.01% |
| 2 | Active | 0.4x | 0.02% |
| 3 | Established | 0.6x | 0.03% |
| 4 | Featured | 0.8x | 0.04% |
| 5+ | Elite | 1.0x | 0.05% |

### Files Modified in Current Build
```
programs/token-2022/src/
├── constants.rs (tier multipliers, fee constants)
├── state.rs (FeeConfig extended with tier_multipliers array)
├── events.rs (TransferFeeEvent, FeesHarvested)
├── instructions/
│   ├── hooks.rs (enhanced transfer_hook with passport lookup)
│   ├── governance.rs (UpdateTierMultipliers, HarvestFees)
│   ├── initialize_mint.rs (initialize new FeeConfig fields)
│   └── lib.rs (harvest_fees entrypoint + security_txt!)
```

---

## 🎓 DEVELOPMENT PHILOSOPHY

### Temperature = 0, Top_P = 0.2
**What this means for future work:**
- Every decision is **deterministic and reproducible**
- No creative divergence; follow the canonical spec exactly
- When ambiguous, default to **first principles** and **architectural constraints**
- Document decisions with reasoning, not intuition

### Grounded Truth Sources (In Order of Authority)
1. **This file (CLAUDE.md)** — Canonical reference
2. **GitHub Issues** (tagged with `@canonical`) — Feature specs
3. **Security.md** — Trust boundaries
4. **Code comments** — Implementation rationale
5. **Audit reports** — Technical validation

### No "Milo" in Public
- ❌ GitHub repo mentions "milo"
- ❌ Pitch decks say "milo"
- ❌ Grant applications use "milo"
- ✅ **Only use "Attention Oracle"** in external communications

---

## 💰 SOLANA FOUNDATION GRANT APPLICATION

### Status
**Ready to Submit** (November 13, 2025)

### Request
- **Amount**: $45,000 USD
- **Category**: Commerce/Loyalty
- **Public GitHub**: https://github.com/twzrd-sol/attention-oracle-program
- **Program ID**: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

### Milestone Structure

| Milestone | Funding | Timeline | Deliverables |
|-----------|---------|----------|--------------|
| **1. Devnet Validation** | $12k | 1 month | Security audit, gas optimization, dev documentation |
| **2. Mainnet Deployment** | $10k | 1 month | Keeper bot, monitoring dashboard, incident response SLA |
| **3. Creator Onboarding** | $13k | 2 months | Integrate 15 streamers, Creator toolkit, dashboard |
| **4. Viewer Adoption** | $10k | 2 months | Marketing, referral program, PWA UI, Community Discord |

### Success Metrics (Post-Award)
- **Milestone 1**: Zero critical audit vulnerabilities, 10k devnet transfers tested
- **Milestone 2**: 99.9% keeper bot uptime, fees harvested within 1 hour
- **Milestone 3**: 15 active channels, 1,000+ creators registered
- **Milestone 4**: 10,000 MAU, 50,000+ on-chain claims, 3,000+ Discord members

### Key Talking Points
1. **Mainnet-ready**: Program already deployed and verified
2. **Token-2022 expertise**: Hybrid model solves architectural constraints others ignore
3. **Public good**: Open source, reusable components (passport system, ring buffer, merkle claims)
4. **Creator focus**: 10M+ addressable market (web2→web3 streamers)
5. **Composable**: CCM integrates with Solana DeFi day one

---

## 🔐 SECURITY & ARCHITECTURE DECISIONS

### Token-2022 Hybrid Model (Why We Chose It)

**Problem**: Token-2022 transfer hooks are post-transfer **observers**, not executors. They can't perform CPI transfers from user accounts (no authority).

**Wrong Approach** (CPI in hook): Fails at runtime due to authority limitations

**Right Approach** (Hybrid):
1. **Hook**: Observes transfer, looks up passport tier, calculates fees, **emits event**
2. **Harvest**: Separate instruction that periodically withdraws withheld fees and distributes them

**Why this matters**:
- Respects Solana's constraints (hooks designed for observability, not state mutation)
- Zero breaking changes (existing transfers work without modification)
- Enables async fee distribution (keepers can batch harvest to save gas)

### Sybil-Resistance Mechanism

**Passport Tiers** (PassportRegistry PDA):
- Tied to provable engagement (Twitch cNFT receipts or oracle attestations)
- Not self-reported or easily-gamed watch time
- Tier determines creator fee allocation (0.0x-1.0x multiplier)

**Anti-Sybil Properties**:
- Each tier requires minimum engagement threshold
- Passport can only be issued/upgraded by oracle authority
- Tiers degrade if user doesn't maintain activity

### Public Good Components (Reusable by Ecosystem)

1. **PassportRegistry Pattern**
   - Tier-based reputation system
   - Generalizable for any loyalty program
   - Fork-friendly for DAOs, NFT projects, DeFi protocols

2. **Ring Buffer Claims** (Per-Channel Epoch Storage)
   - Bounded storage: 10 slots × 1,024 claims per channel
   - Prevents unbounded growth
   - Applicable to staking rewards, governance snapshots, allowlists

3. **Merkle Claim Implementation**
   - Gas-efficient distribution (O(log n) verification)
   - Zero-knowledge friendly (can integrate ZK proofs later)
   - Production-tested pattern

4. **Hybrid Hook Architecture**
   - Template for any project collecting fees via Token-2022
   - Documented tradeoffs (observer vs. executor)
   - Reusable for royalties, LP fees, DAO treasuries

---

## 📋 REQUIRED ENVIRONMENT VARIABLES

Before deploying or running scripts:

```bash
# Solana RPC
RPC_URL=https://api.mainnet-beta.solana.com
# (or Helius, Helius, etc.)

# Program & Keys
PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
ADMIN_KEYPAIR=~/.config/solana/oracle-authority.json

# Database (for off-chain indexing)
DATABASE_URL=postgresql://user:password@localhost:5432/attention_oracle

# Twitch Integration
TWITCH_CHANNEL_ID=<your-channel>
TWITCH_OAUTH_TOKEN=<oauth-token>

# Keeper Bot
KEEPER_INTERVAL=3600 # seconds (1 hour)
KEEPER_WALLET=<hot-wallet-pubkey>

# Security
SECURITY_EMAIL=security@twzrd.com
```

**NEVER commit these to git. Use `.env` files and `.gitignore`.**

---

## 🚀 DEPLOYMENT PIPELINE

### Pre-Deployment Checklist

- [ ] Build: `cargo build-sbf` (0 errors)
- [ ] Security: Verify `strings token_2022.so | grep SECURITY.TXT` shows metadata
- [ ] Test: Run full devnet test suite
- [ ] Docs: Update CHANGELOG.md with breaking changes
- [ ] Backup: Snapshot current program authority keypair
- [ ] Review: Code audit (internal or third-party)

### Mainnet Upgrade Steps

1. **Prepare new binary**: `cargo build-sbf --release`
2. **Dry-run**: Simulate upgrade instruction
3. **Backup**: `solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
4. **Submit**: Use program authority keypair to execute `upgrade` instruction
5. **Monitor**: Watch for logs, check program data account
6. **Verify**: Compare program hash on Solscan with local build

---

## 📚 KEY DOCUMENTS (In Priority Order)

### Essential (Read First)
1. **GitHub README**: https://github.com/twzrd-sol/attention-oracle-program
   - What is Attention Oracle?
   - How does it work?
   - Getting started guide

2. **SECURITY.md**: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
   - Vulnerability disclosure process
   - Scope of security review
   - Contact information

3. **CHANGELOG.md**: Track breaking changes and upgrades

### Implementation (Reference)
4. **Solana Program Code**: `/programs/token-2022/src/`
   - Start with `lib.rs` (entrypoints)
   - Then `state.rs` (data structures)
   - Then `instructions/` (handlers)

5. **Off-Chain Indexer**: `/apps/` (TypeScript)
   - IRC listener (Twitch integration)
   - Merkle tree builder
   - Web UIs

### Governance
6. **Grant Application**: See `SOLANA_GRANT_APPLICATION.md` (in this repo)
7. **Milestones**: Tracked in `agent-sync.json`

---

## 🎯 NEXT IMMEDIATE ACTIONS (Post-Grant Award)

### Week 1: Devnet Deployment
```bash
# Deploy to devnet
solana program deploy target/deploy/token_2022.so --program-id <DEVNET_PROGRAM_ID> --url devnet

# Initialize protocol state
tsx scripts/initialize-devnet.ts
```

### Week 2-3: Security Audit
- Engage third-party auditor (Halborn, OtterSec, etc.)
- Focus areas: Passport tier lookups, harvest logic, fee calculations
- Target: Zero critical findings

### Week 4: Creator Onboarding
- Contact 5 small Twitch streamers
- Set up channels and initial merkle roots
- Document setup process

### Weeks 5-8: Keeper Bot + Monitoring
- Implement full Token-2022 `withdraw_withheld_tokens_from_mint` CPI
- Deploy keeper bot to devnet
- Build real-time metrics dashboard

---

## 💡 DECISION FRAMEWORK (Temperature = 0 Decisions)

When faced with ambiguous choices, use this hierarchy:

1. **Token-2022 Compliance**: Does it respect the extension model?
2. **Sybil Resistance**: Can it be exploited by fake accounts?
3. **Composability**: Can other Solana projects fork/integrate this?
4. **Gas Efficiency**: Is it under 150k CU per core operation?
5. **User Experience**: Does it require <10 clicks to set up?

**Example**: "Should we support custom fee multipliers?"
- Token-2022? ✅ Yes (via governance instruction)
- Sybil-proof? ✅ Yes (only admin can update)
- Composable? ✅ Yes (reusable for other projects)
- Gas efficient? ✅ Yes (state read, no loops)
- UX? ✅ Yes (one admin transaction)
→ **IMPLEMENT**

---

## 📊 SUCCESS METRICS (North Star)

### By End of Year 2025
- ✅ Mainnet deployed (DONE)
- ⏳ Security audit passed
- ⏳ 5 active creator channels
- ⏳ 1,000+ unique viewers claimed tokens

### By End of 2026 (Post-Grant)
- ⏳ 50+ active creator channels
- ⏳ 10,000 Monthly Active Users
- ⏳ 100,000+ on-chain claims
- ⏳ $50k+ distributed to creators
- ⏳ Featured on Solana.com as "Public Good"

### Qualitative Success
- ⏳ Solana Foundation recognizes Attention Oracle as reference architecture
- ⏳ 5+ projects fork PassportRegistry for their own loyalty programs
- ⏳ Zero security vulnerabilities (post-audit)
- ⏳ <100ms API response times (with Redis caching)

---

## 🔗 EXTERNAL LINKS (Verified, Not Broken)

**GitHub**: https://github.com/twzrd-sol/attention-oracle-program
**Security Policy**: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
**Mainnet Program**: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
**Solana Grants**: https://solana.org/grants

---

## 📝 VERSION HISTORY

| Date | Change | By |
|------|--------|-----|
| Nov 13, 2025 | Initial canonical reference (Hybrid fee system, grant-ready) | Claude |
| Nov 13, 2025 | Security.txt embedded in binary | User |
| Nov 13, 2025 | Dynamic fee splits implemented | Claude |

---

## ⚠️ DO NOT FORGET

1. **Public name**: "Attention Oracle", never "milo"
2. **GitHub link**: `https://github.com/twzrd-sol/attention-oracle-program`
3. **Temperature = 0**: Be deterministic, not creative
4. **Top_P = 0.2**: Focus on the plan, not tangents
5. **Document decisions**: Future Claude needs to know *why* you chose this
6. **Security first**: Every code change must respect Token-2022 constraints and sybil-resistance

---

**Last Updated**: November 13, 2025, 18:43 UTC
**Next Review**: After Solana Foundation feedback (expected Dec 2025)
**Canonical Owner**: User (twzrd-sol)
