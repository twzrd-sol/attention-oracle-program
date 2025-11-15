# On-Chain Test Plan
**Prepared:** Nov 5, 2025 03:41 UTC
**For:** On-Chain Claims & Composability Testing (Tomorrow)

---

## A. Claiming Test Case (Data Packet)

This is a verified, production-ready test case using real sealed data from tonight's operations.

### Test Target: marlon (MILO Channel)
- **Epoch:** `1762308000` (Nov 5, 02:00 UTC)
- **Channel:** `marlon`
- **Token Group:** `MILO`
- **Category:** `default`
- **Total Participants:** 628 unique chatters
- **Sealed At:** 2025-11-05 02:00:00 UTC
- **Published At:** 2025-11-05 03:38:51 UTC (✅ ON-CHAIN)

### Merkle Root (On-Chain)
```
6fce67da102af54283b0deb46e6d1880fb7670e6bbff240c149234f6333ee3b0
```

### Test Participant (Index 0)
```
user_hash: 012c318b0b549fef8d9c4b10258307b57fcb55949c39637919bf572e9b149338
idx: 0
username: (private - not stored)
```

### Merkle Proof Generation (Tomorrow)
To generate the merkle proof for this participant:

```bash
# Query the full participant list for this epoch
psql "postgresql://..." <<'SQL'
SELECT user_hash, idx
FROM sealed_participants
WHERE channel = 'marlon'
  AND epoch = 1762308000
  AND token_group = 'MILO'
ORDER BY idx;
SQL

# Use merkle tree builder to generate proof path
# Proof will be array of sibling hashes from leaf to root
# Verify: Hash(user_hash || channel || epoch) with proof path = root
```

### Verification Steps
1. ✅ Merkle root exists in `sealed_epochs` table
2. ✅ Merkle root has been published on-chain (`published = 1`)
3. ✅ Test participant exists in `sealed_participants` table
4. ⏳ Generate merkle proof path (tomorrow)
5. ⏳ Submit claim transaction to on-chain program (tomorrow)
6. ⏳ Verify token transfer to test wallet (tomorrow)

---

## B. On-Chain Program Verification

### Critical Addresses (Solana Mainnet)

**Attention Oracle Program (Token-2022):**
```
GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```
- Deployed: Oct 18, 2025
- Slot: 376962961
- Status: ✅ Live on mainnet
- Verification: In progress (reproducible build)

**MILO Token (SPL Token-2022):**
```
AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5
```
- Standard: Token-2022 (with transfer hooks)
- Token Group: MILO (premium partners)
- Supply: TBD (check on-chain)

**CLS Token (SPL Token-2022):**
```
FZnUPK6eRWSQFEini3Go11JmVEqRNAQZgDP7q1DhyaKo
```
- Standard: Token-2022 (with transfer hooks)
- Token Group: CLS (general ledger)
- Supply: TBD (check on-chain)

**Legacy TWZRD Token (Deprecated):**
```
FHFCPLierqNwqMkATmnCbT2ZPnnQ9j1AWWydKAUEB6Cj
```
- Status: ❌ Program closed, no claims possible
- Note: Historical reference only

### On-Chain State Accounts
These PDAs (Program Derived Addresses) will be needed for claims:

```
# Derive program state account
[b"state", program_id] -> State PDA

# Derive epoch claim account
[b"epoch", channel.as_bytes(), epoch.to_le_bytes(), program_id] -> Epoch PDA

# Derive user claim account
[b"claim", user_pubkey, channel.as_bytes(), epoch.to_le_bytes(), program_id] -> Claim PDA
```

---

## C. Launch Readiness Checklist

### Pre-Flight (Tonight/Tomorrow Morning)

- [ ] **Verify On-Chain Program Deployment**
  - [ ] Confirm program ID matches: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
  - [ ] Query program account data via Solana CLI
  - [ ] Verify program authority/admin keys are secure
  - [ ] Check program upgrade authority (if upgradeable)

- [ ] **Prepare Test Wallets**
  - [ ] Create 3 test keypairs (or use existing devnet wallets)
  - [ ] Airdrop sufficient SOL for gas (min 1 SOL each on devnet, or use mainnet test accounts with small amounts)
  - [ ] Verify all 3 wallets have 0 MILO tokens
  - [ ] Verify all 3 wallets have 0 CLS tokens
  - [ ] Document wallet addresses for tracking

- [ ] **Database Verification**
  - [ ] Confirm test participant `012c318b...` exists in sealed_participants
  - [ ] Confirm merkle root `6fce67da...` exists in sealed_epochs
  - [ ] Confirm merkle root has been published on-chain
  - [ ] Generate and save merkle proof for test participant

### Claim Testing (Tomorrow - Phase 1)

- [ ] **Generate Merkle Proof**
  - [ ] Build merkle tree from all 628 participants in epoch 1762308000
  - [ ] Generate proof path for test participant at idx 0
  - [ ] Verify proof locally (hash to root)
  - [ ] Save proof as JSON for transaction

- [ ] **Submit Test Claim (Wallet 1)**
  - [ ] Construct claim instruction with:
    - Epoch: 1762308000
    - Channel: marlon
    - User hash: 012c318b0b549fef8d9c4b10258307b57fcb55949c39637919bf572e9b149338
    - Merkle proof: [proof path]
  - [ ] Sign and submit transaction
  - [ ] Verify transaction confirmed
  - [ ] Check wallet 1 MILO token balance (should increase)

- [ ] **Duplicate Claim Prevention (Wallet 1)**
  - [ ] Attempt to claim same epoch again with same proof
  - [ ] Verify transaction FAILS (duplicate claim protection)
  - [ ] Check on-chain claim account marked as claimed

- [ ] **Multi-Wallet Claims (Wallets 2-3)**
  - [ ] Generate proofs for different participants
  - [ ] Submit claims from wallets 2 and 3
  - [ ] Verify all claims succeed
  - [ ] Verify token balances increase correctly

### Composability Testing (Tomorrow - Phase 2)

- [ ] **Jupiter Swap Integration**
  - [ ] Check if MILO/USDC pool exists on Jupiter (mainnet or devnet)
  - [ ] If pool exists, prepare test swap script
  - [ ] If pool doesn't exist, create liquidity pool (devnet first)
  - [ ] Execute test swap: 1 USDC → MILO
  - [ ] Verify swap completes and tokens received

- [ ] **Transfer Hook Verification**
  - [ ] Transfer MILO tokens between wallets 1 and 2
  - [ ] Verify transfer hook executes correctly
  - [ ] Check for any on-chain events emitted
  - [ ] Transfer CLS tokens between wallets 2 and 3
  - [ ] Verify CLS transfer hook executes correctly

- [ ] **Cross-Program Invocation (CPI) Test**
  - [ ] If applicable, test CPI from another program
  - [ ] Verify MILO/CLS tokens can be used in composable protocols
  - [ ] Document any restrictions or special handling needed

### Monitoring & Verification (Ongoing)

- [ ] **On-Chain State Verification**
  - [ ] Query all epoch accounts created
  - [ ] Query all claim accounts created
  - [ ] Verify merkle roots match database
  - [ ] Verify claim statuses are correct

- [ ] **System Health During Claims**
  - [ ] Monitor aggregator continues sealing new epochs
  - [ ] Monitor database growth remains stable
  - [ ] Monitor service memory/CPU during claim activity
  - [ ] Check no service restarts needed

- [ ] **User Experience Testing**
  - [ ] Time claim transaction submission to confirmation
  - [ ] Calculate gas costs (SOL) per claim
  - [ ] Document any error messages or UX issues
  - [ ] Test claim UI/frontend (if ready)

---

## D. Known Issues & Edge Cases

### Current System State
- **MILO Channels:** 12 cemented (9 actively streaming, 3 offline)
- **CLS Channels:** 160 discovered (98 active in last 24h)
- **Database Size:** ~2.2 GB (healthy, no runaway growth)
- **Epochs Sealed:** 119 MILO, 79 CLS (total 198 unique epochs)

### Edge Cases to Test
1. **Historical Artifact Claims:** 19 channels have 1 MILO epoch from Nov 4 (epoch 1762290000) - verify these can claim
2. **Category-Aware Claims:** CLS has 42 categories - verify category-specific merkle roots work
3. **Dual Classification:** 10 channels have both MILO+CLS epochs - verify users can claim from both
4. **Offline Channel Claims:** threadguy, thesketchreal, orangieyt sealed epochs but offline - verify claims still work

### Potential Blockers
- **Merkle Proof Generation:** Need to implement/verify proof builder works correctly
- **Gas Costs:** If mainnet, ensure test wallets have sufficient SOL
- **Program Authority:** Verify we have correct signing keys for admin operations
- **Token Supply:** Verify sufficient MILO/CLS tokens exist in distribution account

---

## E. Success Criteria

### Minimum Viable Test (Must Pass)
1. ✅ Generate valid merkle proof for test participant
2. ✅ Submit claim transaction successfully
3. ✅ Verify tokens transferred to claiming wallet
4. ✅ Verify duplicate claim prevention works
5. ✅ System continues sealing new epochs during claims

### Stretch Goals (Nice to Have)
1. ⏳ Multi-wallet claims (3+ successful claims)
2. ⏳ Category-aware CLS claims
3. ⏳ Jupiter swap integration
4. ⏳ Transfer hook verification
5. ⏳ Frontend claim UI test

---

## F. Rollback Plan

If critical issues arise during testing:

1. **Stop New Claims:** Pause claim submissions (program upgrade or pause instruction if available)
2. **Preserve Data:** Database snapshot before testing begins
3. **Service Continuity:** Aggregator/workers keep sealing epochs regardless of claim status
4. **Communication:** Document issues, prepare user communication if needed

**Note:** On-chain state is immutable. Any incorrect claims cannot be rolled back, only future claims can be prevented.

---

## G. Next Steps (Tomorrow)

**Morning (Before Claims):**
1. Review this test plan
2. Generate merkle proof for test participant
3. Prepare test wallets with SOL
4. Verify program deployment and authority

**Afternoon (Claim Testing):**
1. Submit first test claim (single wallet)
2. Verify success and token transfer
3. Test duplicate claim prevention
4. Submit multi-wallet claims (if phase 1 succeeds)

**Evening (Composability):**
1. Test Jupiter swap (if pool exists)
2. Test transfer hooks
3. Document findings and prepare launch summary

---

**Prepared by:** Claude Code (Attention Oracle Monitoring System)
**Contact:** System logs at `/home/twzrd/milo-token/logs/`
**Status:** ✅ Ready for tomorrow's on-chain testing
