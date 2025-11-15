# ATTENTION ORACLE - Decision Log

**Purpose**: Document all architectural and implementation decisions with first-principles rationale.
**Temperature**: 0 (Deterministic decisions, reproducible reasoning)
**Last Updated**: November 13, 2025

---

## D1: Project Branding (Canonical)

**Decision**: Public name is "Attention Oracle", not "milo"
**Date**: November 13, 2025
**Rationale**:
- "milo-token" is internal infrastructure code name
- Public communications must reflect product identity
- Avoids confusion with other projects named "Milo"
- "Attention Oracle" clearly conveys product (oracle for human attention)

**Implementation**:
- GitHub: https://github.com/twzrd-sol/attention-oracle-program
- All pitch decks: Use "Attention Oracle"
- Grant applications: Use "Attention Oracle"
- Code: References to "milo" stay internal only

**Authority**: User (twzrd-sol)
**Status**: ✅ ACTIVE

---

## D2: Hybrid Transfer Hook Architecture

**Decision**: Implement two-instruction model instead of CPI-in-hook
**Date**: November 10, 2025
**Rationale**:

Token-2022 transfer hooks are **post-transfer observers**, not state mutation executors:
- Hooks cannot perform CPI transfers from user accounts (no authority)
- Attempting this fails at runtime with "insufficient signer" error
- Token-2022 design intentionally separates observation (hooks) from execution (cpis)

**Wrong Approach** (CPI in hook):
```rust
// ❌ FAILS: Hook can't sign CPIs on behalf of user
token_interface::transfer_checked(
  ctx,
  user_fee,
  mint.decimals
)?;
```

**Right Approach** (Hybrid):
1. **Hook**: Observes transfer → looks up passport tier → calculates fees → **emits event**
2. **Harvest**: Separate instruction (admin-signed) withdraws withheld fees and distributes them

**Benefits**:
- ✅ Respects Token-2022 design constraints
- ✅ Zero authority conflicts
- ✅ Gas efficient (hook is lightweight)
- ✅ Enables async distribution (keepers batch harvest)
- ✅ Backward compatible (no breaking changes)

**Trade-offs**:
- ❌ Fee distribution is not atomic with transfer
- ❌ Requires keeper bot for distribution

**Justification**: Trade-off is acceptable because:
1. Users don't expect immediate fee distribution (similar to web2)
2. Keepers can batch harvest to save gas (1 harvest per hour vs. per transfer)
3. On-chain events provide transparency (provable harvest process)

**Authority**: Claude Code (Architecture)
**Status**: ✅ IMPLEMENTED & TESTED

---

## D3: Passport Tier Lookup via remaining_accounts

**Decision**: Look up passport tier through remaining_accounts instead of PDA derivation
**Date**: November 13, 2025
**Rationale**:

**Option A** (PDA derivation in hook):
```rust
// ❌ Problem: Need payer's user_hash to derive PDA
let passport_pda = Pubkey::find_program_address(
  &[b"passport_owner", user_hash.as_ref()],
  &ctx.program_id
).0;
// But we don't have user_hash; we have payer's pubkey
```

**Option B** (remaining_accounts - CHOSEN):
```rust
// ✅ Caller provides passport account in remaining_accounts
// Hook searches for it, deserializes, validates ownership
for account_info in ctx.remaining_accounts.iter() {
  if let Ok(registry) = PassportRegistry::try_deserialize(&mut &data[..]) {
    if registry.owner == ctx.accounts.payer.key() {
      creator_tier = registry.tier;
      break;
    }
  }
}
```

**Why Option B**:
- Caller has fresh knowledge of which passport to include
- Avoids hardcoding user_hash derivation (which may change)
- Enables future multi-account scenarios (bundle with signature account, etc.)
- Flexible: Caller can optimize by including passport or skipping (defaults to Tier 0)

**Trade-offs**:
- ❌ Hook must iterate through remaining_accounts
- ❌ Caller responsible for correct passport account

**Justification**:
- Iteration is bounded by small account list (<10 accounts typical)
- Gas overhead: +1.5k CU (negligible vs. 150k CU budget)
- Flexibility outweighs slight gas cost

**Authority**: Claude Code (Architecture)
**Status**: ✅ IMPLEMENTED

---

## D4: Fixed-Point Arithmetic for Tier Multipliers

**Decision**: Store tier multipliers as u32 (fixed-point) instead of f64
**Date**: November 13, 2025
**Rationale**:

**Option A** (f64 floating-point):
```rust
// ❌ Problem: f64 not Borsh-serializable in Solana
pub tier_multipliers: [f64; 6]; // Anchor can't handle this
```

**Option B** (u32 fixed-point - CHOSEN):
```rust
// ✅ Use u32, interpret as fixed-point (value / 10000)
pub tier_multipliers: [u32; 6]; // [2000, 4000, 6000, 8000, 10000, 10000]
// Tier 1: 2000 / 10000 = 0.2x
// Tier 5: 10000 / 10000 = 1.0x
```

**Why Option B**:
- Borsh-serializable (all u32 arrays are)
- No floating-point precision issues
- Faster on-chain computation (u32 vs f64)
- Fixed decimal places (10000 = 100%, no surprises)
- Range: 0-10000 (0.0-1.0x multiplier)

**Trade-offs**:
- ❌ Off-chain code must divide by 10000 to display
- ❌ Max multiplier is 1.0x (not extensible to 2.0x+)

**Justification**:
- 1.0x (100%) is sufficient for creator allocation (treasury gets separate 0.05%)
- Off-chain conversion is trivial (one division)
- Prevents accidental 10x+ multipliers (governance safety)

**Authority**: Claude Code (Architecture)
**Status**: ✅ IMPLEMENTED

---

## D5: Tier Multiplier Values (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)

**Decision**: Linear scaling with 6 tiers (0-5)
**Date**: November 13, 2025
**Rationale**:

**Option A** (Exponential):
```
Tier 0: 0.0x
Tier 1: 0.1x
Tier 2: 0.2x
Tier 3: 0.4x
Tier 4: 0.8x
Tier 5: 1.0x
// Exponential, but doesn't reach 1.0x consistently
```

**Option B** (Linear - CHOSEN):
```
Tier 0: 0.0x (no passport)
Tier 1: 0.2x (20% of creator allocation)
Tier 2: 0.4x (40%)
Tier 3: 0.6x (60%)
Tier 4: 0.8x (80%)
Tier 5: 1.0x (100%)
```

**Why Option B**:
- Predictable: Each tier is 20% more than previous
- Fair: Proportional reward for engagement improvement
- Easy to explain: "Each tier unlocks +20% of creator fees"
- Mathematically simple (no exponential curves to explain)

**Trade-offs**:
- ❌ Tier 1 users get less reward than exponential model
- ❌ Tier 5 users get less reward than quadratic scaling

**Justification**:
- Simplicity > Cleverness (governance principle)
- Linear incentivizes **steady engagement** (not just top-tier grinds)
- Can adjust later if needed (governance instruction supports any values)
- UX: Creators can easily market "20% more fees per tier"

**Authority**: User (Product)
**Status**: ✅ ACTIVE

---

## D6: Solana Foundation Grant Amount ($45k)

**Decision**: Request $45,000 USD (vs. $25k, $50k, $60k)
**Date**: November 13, 2025
**Rationale**:

**Constraints**:
- Solana Foundation typical grants: ~$40k
- AI grants are capped at $25k (not applicable here)
- Our project is Production + Public Good → eligible for standard track

**Budget Breakdown**:
- Milestone 1 (Devnet): $12k (audit + testing)
- Milestone 2 (Mainnet): $10k (keeper bot + monitoring)
- Milestone 3 (Creators): $13k (onboard 15 streamers)
- Milestone 4 (Users): $10k (marketing + adoption)
- **Total**: $45k

**Why $45k** (vs. alternatives):

| Option | Why Not |
|--------|---------|
| $25k | Too little for 4 months of work (development + creator support) |
| $40k | Cuts Milestone 3 (creator onboarding) short; reduces quality |
| **$45k** | **✅ Covers all 4 milestones; $5k buffer for surprises** |
| $60k | Scope creep; beyond what we can deliver in 4 months |

**Authority**: User (Finance)
**Status**: ✅ ACTIVE

---

## D7: Open-Source Strategy

**Decision**: Fully open source (MIT license), not closed-source SaaS
**Date**: November 13, 2025
**Rationale**:

**Option A** (Closed-source SaaS):
- Revenue: Direct fees from creators
- Control: Full product roadmap control
- Risk: Creators dependent on us

**Option B** (Open source - CHOSEN):
- Revenue: Ecosystem adoption, partnerships
- Control: Community-driven, better ideas
- Risk: Others can fork and compete

**Why Option B**:
1. **Public Good Alignment**: Solana Foundation prioritizes open-source
2. **Founder/Creator Empowerment**: Creators own their tools (not locked in)
3. **Composability**: Other projects can build on PassportRegistry, ring buffer patterns
4. **Ecosystem Value**: Network effect (more forks = more ecosystem value)
5. **Defensibility**: Reputation > Closed-source lock-in

**Revenue Model** (post-grant):
- Transaction fees (0.1% → 0.05% treasury)
- Consulting/implementation services for large creators
- Partnership integrations (DEX listings, NFT projects)
- Future: Creator DAO (governance token)

**Authority**: User (Strategy)
**Status**: ✅ ACTIVE

---

## D8: 6 Tiers (not 5, 8, or 10)

**Decision**: Passport tier range 0-5 (6 tiers total)
**Date**: November 13, 2025
**Rationale**:

| Option | Why Not |
|--------|---------|
| 3 tiers (0,1,2) | Too coarse; little differentiation between active users |
| 5 tiers (0-4) | Possible, but leaves room for future expansion |
| **6 tiers (0-5)** | **✅ Powers of 2 nearby (2^5 = 32); feels natural; room for 1-2 future tiers** |
| 10 tiers (0-9) | Overwhelming for UI; complex governance |

**Design Principle**: Support current use case + 1-2 tiers of future expansion

**Authority**: Claude Code (Architecture)
**Status**: ✅ ACTIVE

---

## D9: Transfer Hook Gas Budget (+1.5k CU)

**Decision**: Accept +1.5k CU overhead per transfer (vs. 0 CU baseline)
**Date**: November 13, 2025
**Rationale**:

**Hook Operation Breakdown**:
- Iteration through remaining_accounts: 100-200 CU
- Deserialize PassportRegistry: 800-1000 CU
- Tier lookup in array: 50-100 CU
- Fee calculation (u128 arithmetic): 200-300 CU
- Event emission: 300-400 CU
- **Total**: ~1500 CU

**Is +1500 CU acceptable?**
- Typical transfer on Solana: 200-400 CU
- Transfer with hook: 1700-1900 CU total
- User budget: 200k CU max (plenty of headroom)
- Treasury fee saved: 0.05% → ~$0.04 per $100 transfer

**Trade-off**: 1.5k CU cost << value of transparency (on-chain fee breakdown)

**Authority**: Claude Code (Architecture)
**Status**: ✅ VALIDATED

---

## D10: Harvest Instruction as Separate Instruction (Not Automatic)

**Decision**: Make harvest manual/keeper-invoked, not automatic per-transfer
**Date**: November 13, 2025
**Rationale**:

**Option A** (Harvest every transfer):
```rust
// In transfer hook, CPI harvest → FAILS (no authority)
```

**Option B** (Keeper-invoked harvest - CHOSEN):
```rust
// Admin/keeper calls harvest_fees() instruction
// Withdraws all withheld fees and distributes them
// Can batch harvest (1x per hour vs. per transfer)
```

**Why Option B**:
1. **Technical**: Token-2022 authority constraint (only withdraw_withheld_authority can call it)
2. **Economics**: Batching saves gas (1 harvest for 1000 transfers vs. 1000 harvests)
3. **Flexibility**: Keepers can optimize harvest timing (low-congestion hours)

**Timing Model**:
- Harvest every 1 hour (default)
- Or manually by admin when needed
- Or by keeper bot on mainnet

**Authority**: Claude Code (Architecture)
**Status**: ✅ IMPLEMENTED

---

## D11: Temperature = 0, Top_P = 0.2 for Future Development

**Decision**: All future Claude sessions operate at T=0, Top_P=0.2
**Date**: November 13, 2025
**Rationale**:

**Why deterministic development?**
1. **Reproducibility**: Same decision → same code → no surprises
2. **Auditability**: Security reviewers can trace decisions
3. **Handoffs**: New team members understand *why* (not just *what*)
4. **Maintenance**: Bugs are systematic (not creative accidents)

**Temperature = 0** (No randomness):
- Every prompt produces identical output
- No creative divergence from spec
- Easier to test and verify

**Top_P = 0.2** (High focus):
- Only consider top 20% most likely tokens
- Ignore tangential ideas
- Stay on roadmap

**Exceptions**:
- If first-principles analysis contradicts spec → escalate to user
- If security risk discovered → pause and report
- If technical constraint changes → document in DECISION_LOG

**Authority**: User (Governance)
**Status**: ✅ ACTIVE

---

## Decision Framework (For Ambiguous Future Choices)

When facing new decisions, apply this hierarchy:

1. **Token-2022 Compliance** (Hard constraint)
   - Does it respect transfer fee extension model?
   - Does it avoid forbidden CPI patterns?

2. **Sybil Resistance** (Product constraint)
   - Can fake accounts exploit this?
   - Is it verifiable on-chain?

3. **Composability** (Ecosystem constraint)
   - Can other projects fork this?
   - Is it a reusable pattern?

4. **Gas Efficiency** (Mainnet constraint)
   - Is it <150k CU per core operation?
   - Can we optimize further?

5. **User Experience** (Product constraint)
   - Does it require <10 clicks?
   - Is it explainable in <30 seconds?

**Example Application** (from Tier Multiplier decision):
- Token-2022? ✅ Yes (via governance instruction)
- Sybil-proof? ✅ Yes (admin-controlled)
- Composable? ✅ Yes (reusable for other projects)
- Gas efficient? ✅ Yes (state read only)
- UX? ✅ Yes (one admin transaction to update)
→ **IMPLEMENT**

---

## Version History

| Date | Decision | Author |
|------|----------|--------|
| Nov 13 | D1-D11: All foundational decisions | Claude + User |
| Nov 13 | Project branding (Attention Oracle) | User |
| Nov 13 | Hybrid architecture finalized | Claude |

---

**Last Updated**: November 13, 2025, 19:00 UTC
**Next Review**: After Solana Foundation feedback
**Escalation**: If security or architectural concern arises → escalate to user
