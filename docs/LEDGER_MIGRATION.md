# 🔐 Migrating to Ledger Cold Storage

## When to Switch to Ledger

### ✅ Keep Hot Wallet (Current) When:
- Active development/testing
- Frequent program upgrades needed
- Debugging issues
- Iterating on features
- **During hackathon and immediate post-launch period**

### ✅ Switch to Ledger When:
- Program is stable (no upgrades for 3+ months)
- Feature complete
- Security audit complete
- All tests passing consistently
- User base is stable
- **Ready for "set it and forget it" mode**

---

## Recommended Timeline

```
┌─────────────────────────────────────────────────────────┐
│ PHASE 1: Active Development (NOW - Week 4)             │
│ - Hot wallet as admin                                   │
│ - Quick iterations                                      │
│ - Frequent upgrades possible                            │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ PHASE 2: Stabilization (Week 4 - Week 8)              │
│ - Bug fixes only                                        │
│ - Monitor for issues                                    │
│ - Gather user feedback                                  │
│ - Security review                                       │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ PHASE 3: Production Hardening (Week 8 - Week 12)      │
│ - No major changes                                      │
│ - Performance monitoring                                │
│ - Prepare for Ledger migration                          │
└─────────────────────────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────┐
│ PHASE 4: Cold Storage (Week 12+)                      │
│ - Transfer admin to Ledger                             │
│ - Hot wallet becomes emergency backup only             │
│ - Multi-sig optional for extra security                │
└─────────────────────────────────────────────────────────┘
```

**Recommended switch date:** 2-3 months after mainnet launch

---

## Current Authority Setup

### Program: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

**Current admin (hot wallet):**
- Can upgrade program
- Can set publisher
- Can pause protocol
- Can update fee config

**Current publisher (hot wallet):**
- Can publish merkle roots
- Cannot upgrade program
- Cannot change admin

---

## Pre-Migration Checklist

Before switching to Ledger, ensure:

- [ ] No planned program upgrades for next 3+ months
- [ ] All features working as expected
- [ ] Security audit completed
- [ ] Bug-free for at least 4 weeks
- [ ] Monitoring in place (no surprises)
- [ ] Users have successfully claimed tokens
- [ ] Ledger device set up and tested
- [ ] Recovery phrase backed up (multiple locations)
- [ ] Test the full transfer process on devnet first

---

## Migration Process

### Option A: Single Authority (Simple)

**Step 1: Set up Ledger**
```bash
# Install Solana CLI with Ledger support
cargo install solana-cli --features=ledger

# Connect Ledger and verify
solana-keygen pubkey usb://ledger
# Example output: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

**Step 2: Transfer Admin Authority**
```bash
# Get current admin
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# Transfer to Ledger (YOU CANNOT UNDO THIS!)
solana program set-upgrade-authority GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --new-upgrade-authority usb://ledger

# Verify
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

**Step 3: Update Protocol Admin**
```bash
# Call update_admin instruction with Ledger signature
anchor run update-admin-ledger
```

---

### Option B: Multi-Sig (Recommended for Production)

**Why Multi-Sig?**
- Requires multiple signatures for critical operations
- Protects against single point of failure
- Better security for high-value programs

**Setup:**
```bash
# Install Squads multisig
# https://squads.so/

# Create 2-of-3 multisig
# - Your Ledger
# - Team member's Ledger
# - Emergency hot wallet

# Transfer authority to multisig address
solana program set-upgrade-authority GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --new-upgrade-authority <MULTISIG_ADDRESS>
```

---

## After Migration

### What Changes:

**Upgrades become harder** (intentionally!)
- Need physical Ledger access
- Slower process (security > speed)
- Can't do emergency fixes quickly

**What stays the same:**
- Publisher can still publish merkle roots (if publisher ≠ admin)
- Users can still claim
- Protocol keeps running

### Emergency Access

**Keep hot wallet as backup:**
1. Store in secure location
2. Only use if Ledger lost/damaged
3. Requires multisig approval (if using multisig)

**Emergency Procedure:**
1. Detect issue requiring immediate upgrade
2. Gather multisig signers (if applicable)
3. Retrieve Ledger from secure storage
4. Sign upgrade transaction
5. Deploy fix
6. Verify on-chain

---

## Testing the Migration

### Test on Devnet First!

```bash
# Deploy test program to devnet
anchor build
anchor deploy --provider.cluster devnet

# Transfer to test Ledger account
solana program set-upgrade-authority <DEVNET_PROGRAM> \
  --new-upgrade-authority <TEST_LEDGER_PUBKEY> \
  --provider.cluster devnet

# Try to upgrade (should require Ledger signature)
anchor upgrade <DEVNET_PROGRAM> \
  --program-id <NEW_PROGRAM_PATH> \
  --provider.cluster devnet
```

**Verify:**
- ✅ Upgrade requires Ledger signature
- ✅ Ledger device prompts for confirmation
- ✅ Upgrade succeeds after signing
- ✅ Cannot upgrade without Ledger

---

## Comparison: Hot Wallet vs Ledger

| Feature | Hot Wallet | Ledger |
|---------|-----------|--------|
| **Speed** | Instant | Manual approval |
| **Security** | Medium (online) | High (offline) |
| **Convenience** | High | Medium |
| **Recovery** | Via private key | Via recovery phrase |
| **Best for** | Development | Production |
| **Cost** | Free | ~$100-200 |
| **Hack risk** | Higher | Very low |

---

## Security Best Practices

### For Hot Wallet (Current):
- ✅ Keep private key encrypted
- ✅ Use different wallet for each environment (dev/test/prod)
- ✅ Never commit private keys to git
- ✅ Rotate keys if compromised
- ✅ Monitor transactions

### For Ledger (Future):
- ✅ Buy from official manufacturer only
- ✅ Backup recovery phrase in 2-3 secure locations
- ✅ Never store recovery phrase digitally
- ✅ Use PIN code
- ✅ Test recovery process
- ✅ Consider multisig for extra protection

---

## Cost Analysis

### Keeping Hot Wallet:
- **Pros:** Free, fast, convenient
- **Cons:** Higher risk, harder to insure, trust issues

### Switching to Ledger:
- **Pros:** Maximum security, user trust, insurance-friendly
- **Cons:** $100-200 hardware cost, slower operations

**For a production protocol with real user funds:**
→ Ledger cost is negligible compared to security benefits

---

## FAQ

**Q: Can I switch back to hot wallet after moving to Ledger?**
A: Yes, but it requires the Ledger signature to transfer authority back.

**Q: What if I lose my Ledger?**
A: Use the 24-word recovery phrase to restore to a new Ledger device.

**Q: What if I lose both Ledger and recovery phrase?**
A: Authority is permanently lost. This is why multisig is recommended.

**Q: Can I upgrade the program after switching to Ledger?**
A: Yes, but you need the physical Ledger device and must sign the upgrade.

**Q: Should I use multisig?**
A: Recommended for programs with >$100k value or >10k users.

**Q: How long does the migration take?**
A: 15-30 minutes for single authority, 1-2 hours for multisig setup.

---

## Summary

```
Current State (Phase 1):
├─ Hot wallet admin: FAST but LESS SECURE
├─ Good for: Active development
└─ Switch when: Stable + Feature complete

Future State (Phase 4):
├─ Ledger admin: SLOWER but VERY SECURE
├─ Good for: Production/Long-term
└─ Timeline: 2-3 months after launch
```

**Recommended action now:** Keep hot wallet, plan for Ledger in Q1 2025

---

## Resources

- [Solana Ledger Guide](https://docs.solana.com/wallet-guide/ledger-live)
- [Squads Multisig](https://squads.so/)
- [Program Upgrade Authority](https://docs.solana.com/cli/deploy-a-program#upgrading-a-program)
- [Ledger Security Best Practices](https://www.ledger.com/academy/security)

---

**When in doubt: Stay on hot wallet until you're 100% ready.**

Better to wait an extra month than rush into cold storage.
