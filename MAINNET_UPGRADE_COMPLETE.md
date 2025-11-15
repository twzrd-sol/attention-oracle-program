# Mainnet Upgrade Complete - Hybrid Dynamic Fee System

**Status**: ✅ **DEPLOYED & VERIFIED**
**Date**: November 14, 2025
**Program**: Attention Oracle (GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)

---

## Executive Summary

The Attention Oracle program has been successfully upgraded on Solana mainnet with the **Hybrid Dynamic Fee System** implementation. The upgrade includes:

- ✅ Transfer hook enhancements with dynamic fee calculation based on passport tier
- ✅ Harvest instruction for periodic fee distribution
- ✅ On-chain security.txt metadata with vulnerability disclosure
- ✅ Full Token-2022 compliance with tier multiplier architecture

---

## Upgrade Details

### On-Chain Identifiers

| Field | Value |
|-------|-------|
| **Program ID** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| **Upgrade Authority** | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |
| **Program Data Account** | `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L` |
| **Upgrade Signature** | `2mqkcFt1M3Sc9bXytRNecQkd42UAKBr2YRCodjnas2nQLkhLk1KRHdWX8i5JBN9hhaQX9xGFgsV3t53m3KApVjMf` |

### Source Code Verification

| Field | Value |
|-------|-------|
| **Git Tag** | `v1.0.0-hybrid-fees` |
| **Commit SHA** | `fc61cca4f33abe88e1cc3ff1e03130a6379d0cbc` |
| **Binary SHA256** | `36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0` |
| **GitHub Repository** | `https://github.com/twzrd-sol/attention-oracle-program` |
| **Reproducible** | ✅ Yes (Verified: local build matches on-chain deployment) |

---

## Embedded Security.txt

The following vulnerability disclosure metadata is embedded in the on-chain binary and verifiable via `solana program dump` or `strings`:

```
name: Attention Oracle - Verifiable Distribution Protocol (Token-2022)
project_url: https://github.com/twzrd-sol/attention-oracle-program
contacts: email:security@twzrd.xyz
policy: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
source_code: https://github.com/twzrd-sol/attention-oracle-program
expiry: 2026-06-30
```

**Verification Command**:
```bash
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop /tmp/program.so --url mainnet-beta
strings /tmp/program.so | grep -A 5 "Attention Oracle"
```

---

## Upgrade Timeline

### Phase 1: Recovery & Consolidation
- **Old Buffer 1** (`GBVduvu5ZsVBaEXNW67U9F4LnPuVZAUH7p5Un5edym8F`): Closed ✅
- **Old Buffer 2** (`4nHnJcUvMLL8hnowonwBFGHgDALox4GQrcpCgn2FBdmR`): Closed ✅
- **SOL Recovered**: ~9.64 SOL from stale buffers

### Phase 2: Fresh Buffer & Deployment
- **New Buffer** (`7E186N6YkRhGoERK4xpAR5QW6ZfBAg262MW6djrsiPmQ`): Created
- **Buffer Written**: Program binary with embedded security.txt
- **Upgrade Executed**: Signature recorded above
- **Buffer Consumed**: Normal behavior (no additional recovery needed)

### Phase 3: Verification
- **SHA256 Match**: ✅ Local binary matches on-chain deployment
- **Security.txt Visible**: ✅ On Solscan after cache refresh
- **Gas Costs**: Upgrade transaction consumed as expected
- **Final Fee Payer Balance**: 10.522 SOL

---

## Feature Implementation

### 1. Transfer Hook Enhancement
**File**: `programs/token-2022/src/instructions/hooks.rs`

```rust
// Dynamic fee calculation based on passport tier
fn transfer_hook() {
    // Lookup passport tier from remaining_accounts
    let passport_tier = get_passport_tier(user_pubkey);

    // Calculate fees
    let treasury_fee = transfer_amount * TREASURY_FEE_RATE; // 0.05%
    let creator_fee = transfer_amount * CREATOR_FEE_RATE * tier_multiplier(passport_tier);

    // Emit TransferFeeEvent for off-chain indexing
    emit_event(TransferFeeEvent { ... });
}
```

**Gas**: +1.5k CU per transfer

### 2. Harvest Instruction
**File**: `programs/token-2022/src/instructions/governance.rs`

```rust
// Periodic fee harvesting and distribution
fn harvest_fees() {
    // Query withheld_amount from Token-2022 mint extension
    let withheld = get_withheld_amount(mint_pubkey);

    // Distribute to treasury and creator pool
    transfer_from_mint(withheld_treasury_amount);
    transfer_from_mint(withheld_creator_amount);

    // Emit FeesHarvested event for keeper coordination
    emit_event(FeesHarvested { ... });
}
```

**Gas**: +5k-10k CU (depends on number of creator accounts)

### 3. Tier Multiplier Structure
**File**: `programs/token-2022/src/constants.rs`

| Tier | Label | Multiplier | Creator Fee |
|------|-------|------------|-------------|
| 0 | Unverified | 0.0x | 0.000% |
| 1 | Emerging | 0.2x | 0.010% |
| 2 | Active | 0.4x | 0.020% |
| 3 | Established | 0.6x | 0.030% |
| 4 | Featured | 0.8x | 0.040% |
| 5+ | Elite | 1.0x | 0.050% |

---

## Deployment Commands

### To Verify Locally
```bash
cd /home/twzrd/milo-token/clean-hackathon

# Checkout verified tag
git checkout v1.0.0-hybrid-fees

# Build (requires Solana CLI 1.18.26 & Rust 1.51+)
cargo build-sbf --release

# Compare SHA256
sha256sum target/deploy/token_2022.so
# Expected: 36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0
```

### To Verify On-Chain
```bash
# Download current program
solana program dump GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop program.so --url mainnet-beta

# Check SHA256
sha256sum program.so
# Expected: 36da3c130d95556d096a96549cd9029086e8367a91e47dd9c5b02992e2a46de0

# Extract security.txt
strings program.so | grep -B2 -A10 "Attention Oracle"
```

---

## Migration Notes

### For Existing Integrations
- **No breaking changes** to existing user transactions
- Transfer hooks are **post-transfer observers** (don't modify balances)
- Harvest instruction is **admin-only** (controlled fee distribution)
- Tier multipliers can be updated via governance instruction

### For Token Holders
- Fee collection is transparent and auditable on-chain
- Withheld amounts visible via Token-2022 mint extensions
- Distribution events logged for off-chain indexing
- No action required for token holders

### For Creators
- Fee allocation based on passport tier (determined by oracle)
- Harvest events coordinated via keeper bot
- Creator pool accumulates distributed fees
- Eligible for governance participation after tier threshold

---

## Audit & Security

### Security Review Status
- ✅ Code review completed (internal)
- ⏳ Third-party audit: Pending (Solana Grant Milestone 1)
- ✅ On-chain security.txt embedded and verifiable
- ✅ Vulnerability disclosure policy published

### Known Limitations (By Design)
1. **Hook Observability**: Hooks observe transfers but cannot execute CPI transfers
   - **Reason**: Solana's constraint on post-transfer hooks (no authority)
   - **Solution**: Async harvest instruction for fee distribution

2. **Tier Assignment**: Passport tiers assigned by oracle only
   - **Reason**: Prevents self-reporting and sybil attacks
   - **Solution**: Oracle authority can issue/upgrade tiers based on engagement

3. **Treasury Address**: Fixed treasury address (no dynamic routing)
   - **Reason**: Simplifies on-chain logic and reduces attack surface
   - **Solution**: Governance can update via treasury reallocation instruction

---

## Performance Metrics

| Metric | Value | Status |
|--------|-------|--------|
| Binary Size | 654 KB | ✅ Optimized |
| Transfer Hook Cost | +1.5k CU | ✅ <10% overhead |
| Harvest Instruction | +5-10k CU | ✅ Batched execution |
| Program Data Account | 692.2 KB | ✅ Within limits |
| Mint Extensions | 2.5 KB | ✅ Minimal storage |

---

## Next Steps (Post-Award)

### Week 1: Monitoring & Observability
- Deploy keeper bot to monitor harvest events
- Configure real-time metrics dashboard
- Set up alerting for unusual fee patterns

### Week 2-4: Security Audit
- Engage third-party auditor (Halborn, OtterSec, etc.)
- Focus areas: Passport tier lookups, harvest logic, fee calculations
- Target: Zero critical findings

### Week 5-8: Creator Onboarding
- Integrate with 5 initial creator channels
- Set up tier allocation merkle roots
- Document creator toolkit and workflows

---

## Contact & Support

**Security Vulnerabilities**: [security@twzrd.xyz](mailto:security@twzrd.xyz)
**Security Policy**: https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md
**GitHub Issues**: https://github.com/twzrd-sol/attention-oracle-program/issues
**Public Program**: https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

---

## Verification Signatures

```
Upgrade Tx: 2mqkcFt1M3Sc9bXytRNecQkd42UAKBr2YRCodjnas2nQLkhLk1KRHdWX8i5JBN9hhaQX9xGFgsV3t53m3KApVjMf
Block Slot: [Check on Solscan or Explorer]
Status: ✅ Confirmed
```

---

**Document Updated**: November 14, 2025
**Last Verified**: On-chain (Mainnet)
**Owner**: Attention Oracle Team
