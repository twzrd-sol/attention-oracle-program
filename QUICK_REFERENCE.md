# ATTENTION ORACLE - Quick Reference (Cheat Sheet)

**Read this first. Then read CLAUDE.md for details.**

---

## 🎯 TL;DR

| What | Answer |
|------|--------|
| **Project Name** | Attention Oracle (never "milo" publicly) |
| **What is it?** | Solana oracle that distributes creator tokens based on verifiable engagement |
| **Program ID** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| **GitHub** | https://github.com/twzrd-sol/attention-oracle-program |
| **Status** | Mainnet live, grant application ready, hybrid fees implemented |
| **Tech Stack** | Anchor (Rust), Token-2022, Solana |
| **Temperature** | 0 (deterministic), Top_P 0.2 (focused) |

---

## 🏗️ Architecture (One Sentence Each)

| Component | What It Does |
|-----------|-------------|
| **PassportRegistry** | Tiers (0-6) track user engagement; tied to Twitch verification |
| **Transfer Hook** | Observes transfers, looks up passport tier, calculates fees, emits event |
| **Harvest Instruction** | Keeper-invoked; withdraws withheld fees and distributes to treasury/creators |
| **Ring Buffer** | Stores 10 epochs of merkle roots per channel (1024 claims each) |
| **Merkle Claims** | Viewers claim tokens with zero-knowledge proofs (gas-efficient) |

---

## 💰 Fee Structure

```
Total Fee: 0.1% (10 basis points)
├── Treasury: 0.05% (fixed)
└── Creator: 0.05% × Tier Multiplier
    ├── Tier 0: 0.0x (no passport) → 0%
    ├── Tier 1: 0.2x (emerging) → 0.01%
    ├── Tier 2: 0.4x (active) → 0.02%
    ├── Tier 3: 0.6x (established) → 0.03%
    ├── Tier 4: 0.8x (featured) → 0.04%
    └── Tier 5: 1.0x (elite) → 0.05%
```

---

## 🚀 Solana Grant ($45k, 4 Milestones)

| Milestone | Funding | Deliverable |
|-----------|---------|------------|
| 1. Devnet | $12k | Security audit, gas optimization, docs |
| 2. Mainnet | $10k | Keeper bot, monitoring, SLA |
| 3. Creators | $13k | Onboard 15 streamers, tools, dashboard |
| 4. Users | $10k | Marketing, 10K MAU, 50K claims |

**Status**: Ready to submit (all code done, security.txt embedded)

---

## ⚡ Critical Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Fee Distribution | Hook observes + Harvest distributes | Token-2022 respects authority constraints |
| Tier Lookup | remaining_accounts | Flexible, caller provides context |
| Multiplier Storage | u32 fixed-point | Borsh-serializable, no float precision issues |
| Gas Budget | +1.5k CU per transfer | Acceptable vs. transparency value |
| Harvest | Keeper-invoked (not automatic) | Allows batching, respects Token-2022 authority |
| Open Source | MIT license | Public good, ecosystem > lock-in |

---

## 🔒 Security & Constraints

### Token-2022 Rules
- ❌ DON'T: CPI transfers from hook (no authority)
- ❌ DON'T: Mutate program state in hook
- ✅ DO: Emit events from hook
- ✅ DO: Harvest in separate instruction (admin-signed)

### Sybil Resistance
- Passport tier = verifiable engagement (not self-reported)
- Tier 0 = no fees (prevents bots from claiming)
- Admin controls passport issuance

---

## 📝 Key Files (Read Order)

1. **CLAUDE.md** — Canonical reference (this session + future)
2. **DECISION_LOG.md** — Why each choice was made
3. **GitHub README** — User-facing explanation
4. **programs/token-2022/src/lib.rs** — Program entrypoints
5. **SECURITY.md** — Vulnerability disclosure

---

## 🎓 "First Principles" Checklist

Before any code change, verify:

- [ ] Does it respect Token-2022 (no forbidden CPI patterns)?
- [ ] Is it sybil-resistant (can't be exploited by fakes)?
- [ ] Is it composable (can other projects fork this)?
- [ ] Is it gas-efficient (<150k CU)?
- [ ] Is it user-friendly (<10 clicks, explainable)?

If all ✅, implement. If any ❌, escalate to user.

---

## 🛠️ Common Tasks

### Check Build Status
```bash
cargo build-sbf 2>&1 | tail -5
```

### Verify Security.txt in Binary
```bash
strings target/deploy/token_2022.so | sed -n '/=======BEGIN SECURITY.TXT/,/=======END SECURITY.TXT/p'
```

### Check Git Status
```bash
git status --short | grep -E "\.rs|\.toml"
```

### Initialize Devnet (Post-Grant)
```bash
tsx scripts/initialize-devnet.ts
# Then test with Tier 0, 1, 6 passports
```

---

## 🚨 DO NOT EVER

- [ ] Use "milo" in public communications
- [ ] Hardcode credentials (use env vars)
- [ ] Break sybil-resistance (gates must be real)
- [ ] Ignore Token-2022 authority constraints
- [ ] Implement features not in DECISION_LOG

---

## 📊 Success Metrics (North Star)

**By End 2025**:
- ✅ Mainnet deployed (DONE)
- ⏳ Grant awarded
- ⏳ Security audit passed

**By End 2026**:
- ⏳ 50+ creator channels
- ⏳ 10K MAU (Monthly Active Users)
- ⏳ 100K+ claims executed
- ⏳ $50K distributed to creators

---

## 🔗 Quick Links

| What | Link |
|------|------|
| GitHub | https://github.com/twzrd-sol/attention-oracle-program |
| Program | https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop |
| Security | https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md |
| Solana Grants | https://solana.org/grants |
| This Repo (Internal) | /home/twzrd/milo-token/ |

---

## 🎭 Role Clarity

| Role | Person | Responsibility |
|------|--------|-----------------|
| **Product** | User (twzrd-sol) | Vision, milestone selection, go/no-go decisions |
| **Architecture** | Claude Code | First-principles decisions, code review, documentation |
| **Implementation** | Both | Code + testing |
| **Verification** | Claude Code | Build validation, security checks, git history |

---

## ⏭️ Next Immediate Actions

1. **Submit Solana Grant** (use template provided)
2. **Post-Award** (if funded):
   - Week 1: Devnet deployment
   - Week 2-3: Security audit
   - Week 4-8: Keeper bot + onboarding
   - Month 2: Creator & user adoption

---

**Version**: 1.0
**Last Updated**: November 13, 2025, 19:05 UTC
**Next Review**: After Solana Foundation feedback (expected Dec 2025)

*This is the quick reference. Details are in CLAUDE.md.*
