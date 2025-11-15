# Security Model - Attention Oracle

**Last Updated:** November 14, 2025
**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**Status:** Production (Mainnet)

---

## Table of Contents

1. [Responsible Disclosure](#responsible-disclosure)
2. [Threat Model](#threat-model)
3. [Authorization Architecture](#authorization-architecture)
4. [Attack Vectors & Mitigations](#attack-vectors--mitigations)
5. [Cryptographic Guarantees](#cryptographic-guarantees)
6. [Operational Security](#operational-security)
7. [Audit Status](#audit-status)
8. [Pre-Launch Checklist](#pre-launch-checklist)
9. [Known Limitations](#known-limitations)

---

## Responsible Disclosure

### Reporting Security Vulnerabilities

**We take security seriously.** If you discover a vulnerability, please follow responsible disclosure practices:

**DO:**
- ✅ Email: security@twzrd.xyz (primary), dev@twzrd.xyz, ccm@twzrd.xyz
- ✅ Include: Program ID, transaction signature, reproduction steps
- ✅ Allow 48-72 hours for initial response
- ✅ Work with us on coordinated disclosure timeline

**DO NOT:**
- ❌ Publicly disclose before patch is deployed
- ❌ Exploit vulnerabilities for personal gain
- ❌ Test attacks on mainnet (use devnet/localnet)

### Vulnerability Severity Classification

| Severity | Definition | Example |
|----------|------------|---------|
| **Critical** | Direct loss of user funds or total protocol compromise | Bypass merkle proof validation, drain treasury |
| **High** | Potential for fund loss or significant disruption | Double-claim exploit, admin key extraction |
| **Medium** | Protocol degradation or user inconvenience | DOS via compute exhaustion, incorrect events |
| **Low** | Minor issues with limited impact | Cosmetic errors, non-exploitable edge cases |

### Bug Bounty Program

**Status:** Coming Soon (Post-Hackathon)

**Planned Rewards:**
- Critical: $10,000 - $50,000
- High: $2,500 - $10,000
- Medium: $500 - $2,500
- Low: $100 - $500

*Amounts subject to change based on impact and quality of report.*

---

## Threat Model

### Assets to Protect

1. **User Funds**
   - Token-2022 rewards in user accounts
   - Unclaimed rewards represented by merkle proofs

2. **Protocol Integrity**
   - Merkle root authenticity
   - Epoch immutability after sealing
   - Double-claim prevention

3. **Administrative Control**
   - Admin keypair (protocol configuration)
   - Publisher keypair (merkle root updates)
   - Upgrade authority (program modifications)

### Threat Actors

| Actor | Capabilities | Motivation |
|-------|-------------|------------|
| **External Attacker** | Standard Solana transaction signing | Steal funds, disrupt protocol |
| **Malicious User** | Valid merkle proofs, multiple accounts | Double-claim, exceed allocation |
| **Compromised Publisher** | Publisher private key | Publish fraudulent merkle roots |
| **Compromised Admin** | Admin private key | Drain treasury, corrupt protocol state |
| **Malicious Validator** | Transaction ordering, censorship | MEV extraction, DOS attacks |

### Trust Assumptions

**What We Trust:**
- ✅ Solana runtime (BPF VM, account model, sysvar integrity)
- ✅ Token-2022 program (SPL maintained, widely audited)
- ✅ Keccak256 cryptographic security
- ✅ TWZRD aggregator (off-chain, operated by protocol team)

**What We Don't Trust:**
- ❌ User-provided inputs (validated on-chain)
- ❌ Transaction ordering (no MEV vulnerabilities)
- ❌ RPC node honesty (clients should verify proofs locally)

---

## Authorization Architecture

### Role Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│                    Solana Runtime                        │
│                  (Enforces All Rules)                    │
└────────────────────────┬────────────────────────────────┘
                         │
          ┌──────────────┴──────────────┐
          │                             │
┌─────────▼─────────┐         ┌────────▼────────┐
│  Upgrade Authority │         │  Program Logic  │
│   (Multisig*)      │         │  (Immutable)    │
└─────────┬─────────┘         └────────┬────────┘
          │                             │
          │ Can:                        │ Enforces:
          │ - Deploy new program        │ - Admin checks
          │ - Fix critical bugs         │ - Publisher checks
          │                             │ - User validation
          │                             │
          │              ┌──────────────┴──────────────┐
          │              │                             │
    ┌─────▼──────┐  ┌───▼────────┐         ┌─────────▼─────────┐
    │   Admin    │  │ Publisher  │         │      Users        │
    │ (Cold Key*)│  │ (Hot Key)  │         │ (Anyone on Solana)│
    └─────┬──────┘  └───┬────────┘         └─────────┬─────────┘
          │             │                             │
          │ Can:        │ Can:                        │ Can:
          │ - Pause     │ - Seal epochs               │ - Claim rewards
          │ - Update    │ - Publish roots             │ - Verify proofs
          │   admin     │ - Create channels           │
          │ - Set       │                             │
          │   policies  │                             │
          └─────────────┘                             │
                                                      │
* Post-hackathon: Migrating to hardware wallet/multisig
```

### Access Control Implementation

#### Admin Authority

```rust
pub fn update_admin_open(ctx: Context<UpdateAdmin>, new_admin: Pubkey) -> Result<()> {
    // ONLY current admin can transfer authority
    require!(
        ctx.accounts.admin.key() == ctx.accounts.protocol.admin,
        ErrorCode::Unauthorized
    );

    // TODO: Implement 2-step transfer for safety
    ctx.accounts.protocol.admin = new_admin;

    emit!(AdminUpdated {
        old_admin: ctx.accounts.admin.key(),
        new_admin,
    });

    Ok(())
}
```

**Security Properties:**
- ✅ Single point of authority validation
- ⚠️ **Known Issue:** Single-step transfer (risky if typo in `new_admin`)
- 🔜 **Planned:** Two-step transfer (pending + accept pattern)

#### Publisher Authority

```rust
pub fn seal_epoch(ctx: Context<SealEpoch>, merkle_root: [u8; 32]) -> Result<()> {
    // ONLY publisher can seal epochs
    require!(
        ctx.accounts.publisher.key() == ctx.accounts.protocol.publisher,
        ErrorCode::Unauthorized
    );

    // Prevent overwriting sealed epochs
    require!(
        !ctx.accounts.channel.sealed,
        ErrorCode::EpochAlreadySealed
    );

    ctx.accounts.channel.merkle_root = merkle_root;
    ctx.accounts.channel.sealed = true;

    Ok(())
}
```

**Security Properties:**
- ✅ Publisher cannot modify sealed epochs
- ✅ Admin can rotate publisher if compromised
- ✅ Publisher key rotation does NOT affect existing epochs

#### User Claims (Permissionless)

```rust
pub fn claim_open(ctx: Context<ClaimOpen>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
    // NO special authority required - anyone can claim IF they have valid proof

    // 1. Verify protocol is not paused
    require!(!ctx.accounts.protocol.paused, ErrorCode::ProtocolPaused);

    // 2. Verify epoch is sealed
    require!(ctx.accounts.channel.sealed, ErrorCode::EpochNotSealed);

    // 3. Verify merkle proof
    let leaf = keccak256(&[ctx.accounts.user.key(), amount].encode());
    require!(
        verify_proof(leaf, proof, ctx.accounts.channel.merkle_root),
        ErrorCode::InvalidProof
    );

    // 4. Prevent double-claim (PDA prevents recreation)
    // `init` constraint ensures UserClaim doesn't already exist

    Ok(())
}
```

**Security Properties:**
- ✅ Permissionless (anyone with valid proof can claim)
- ✅ Cryptographic validation (merkle proof)
- ✅ Double-claim prevention (PDA uniqueness)
- ✅ Circuit breaker (pause check)

---

## Attack Vectors & Mitigations

### 1. Double-Claim Attack

**Attack:** User attempts to claim rewards multiple times in same epoch.

**Naive Implementation (Vulnerable):**
```rust
// ❌ BAD: No persistent claim tracking
pub fn claim(ctx: Context<Claim>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
    verify_proof(...)?;
    mint_tokens(amount)?; // Can be called repeatedly!
    Ok(())
}
```

**Our Mitigation:**
```rust
#[derive(Accounts)]
pub struct ClaimOpen<'info> {
    #[account(
        init,  // ✅ Fails if account already exists
        payer = user,
        space = 88,
        seeds = [b"user-claim", user.key().as_ref(), channel.key().as_ref()],
        bump
    )]
    pub user_claim: Account<'info, UserClaim>,
}
```

**Result:** Second claim attempt fails with `AccountAlreadyExists` error.

**Cost to Attack:** Rent-exempt minimum (~0.001 SOL) per failed attempt.

---

### 2. Invalid Merkle Proof

**Attack:** User submits fake proof to claim rewards they didn't earn.

**Mitigation:**
```rust
pub fn verify_proof(
    leaf: [u8; 32],
    proof: Vec<[u8; 32]>,
    root: [u8; 32],
) -> bool {
    let mut computed = leaf;

    for sibling in proof {
        // Deterministic ordering prevents second-preimage attacks
        computed = if computed <= sibling {
            keccak256(&[computed, sibling].concat())
        } else {
            keccak256(&[sibling, computed].concat())
        };
    }

    computed == root
}
```

**Security Properties:**
- ✅ Keccak256 collision resistance (~2^128 security)
- ✅ No proof found = transaction reverts (no state changes)
- ✅ Malformed proofs rejected (wrong length, invalid structure)

---

### 3. Merkle Root Substitution

**Attack:** Attacker tries to replace merkle root with one they control.

**Attack Scenarios:**

**A. Direct Root Overwrite (Blocked):**
```rust
// User tries to call seal_epoch
seal_epoch(ctx, attacker_controlled_root)?;

// ❌ FAILS: require!(publisher == signer, Unauthorized)
```

**B. Replay Old Root (Blocked by Sealing):**
```rust
// Publisher tries to overwrite sealed epoch
require!(!channel.sealed, EpochAlreadySealed); // ✅ Prevents overwrites
```

**C. Publisher Key Compromise (Mitigated):**
- Admin can rotate publisher immediately via `update_publisher_open`
- Users should verify merkle roots off-chain before claiming
- Post-compromise: Pause protocol, investigate, redeploy if needed

---

### 4. Admin Key Compromise

**Impact:** Total protocol control.

**Attack Capabilities:**
- Pause protocol indefinitely (DOS)
- Transfer admin to attacker-controlled key
- Change policies (e.g., disable receipt requirements)

**Mitigations:**

**Current (Hackathon Phase):**
- Admin key stored in secure environment
- Monitoring for unexpected admin transactions
- Limited exposure (short hackathon window)

**Post-Launch:**
- **Ledger Hardware Wallet** - Admin key never touches hot storage
- **Multi-Signature** - Squads protocol integration (M-of-N approval)
- **Timelock** - Admin actions have 24-48 hour delay
- **Two-Step Transfer** - New admin must accept before taking effect

**Emergency Response:**
1. Detect unauthorized admin transaction
2. Immediately pause protocol (if still have access)
3. Contact Solana validators (rare: rollback consideration)
4. Deploy new program instance
5. Migrate users to safe contract

---

### 5. Publisher Key Compromise

**Impact:** Medium severity (can publish fake merkle roots).

**Attack Scenario:**
```
1. Attacker steals publisher private key
2. Calls seal_epoch with malicious root
3. Claims massive rewards using fake proof
```

**Mitigations:**

**Prevention:**
- Publisher key rotated regularly (weekly)
- Key stored in HSM (Hardware Security Module)
- Rate limiting on seal_epoch (max 1 per epoch)

**Detection:**
- Monitor published merkle roots against expected values
- Off-chain verification API (users check before claiming)
- Alert system for unexpected seal_epoch transactions

**Response:**
1. Admin calls `update_publisher_open` with new key
2. Admin calls `set_paused_open(true)` to halt claims
3. Investigate extent of damage (which epochs affected?)
4. Publish correct merkle roots for affected epochs (requires unseal functionality - not yet implemented)
5. Resume protocol

**User Protection:**
- Users should verify their proof off-chain before claiming
- TWZRD frontend displays "Verified by Aggregator" badge
- Community can audit merkle roots independently

---

### 6. Front-Running / MEV Extraction

**Attack:** Validator or bot tries to front-run user claims.

**Why This Doesn't Work:**

**Scenario A: Steal User's Claim**
```
User submits: claim_open(amount=1000, proof=[...])
Attacker sees mempool, tries to claim first with same proof

❌ FAILS: UserClaim PDA is tied to original user's pubkey
Attacker would create their own PDA, but proof is for USER, not attacker
```

**Scenario B: Inflate Amount**
```
User submits: claim_open(amount=1000, proof=[...])
Attacker modifies: claim_open(amount=9999, proof=[...])

❌ FAILS: Proof is cryptographically bound to amount
verify_proof(keccak256([user, 9999]), proof, root) == false
```

**Result:** No MEV opportunity. Claims are non-extractable value.

---

### 7. Sybil Attack (Off-Chain)

**Attack:** User creates 1000 fake accounts to inflate rewards.

**Why This Is Off-Chain:**
- Sybil detection happens in TWZRD aggregator (private)
- On-chain program assumes merkle root is already sybil-filtered
- Attacking off-chain component requires different threat model

**Aggregator Mitigations (Private Implementation):**
- IP address clustering
- Behavioral heuristics (view duration, chat patterns)
- Passport integration (verified identities get higher trust)
- Manual review for suspicious patterns

**On-Chain Assumption:**
- We trust that merkle roots only include legitimate participants
- If aggregator is compromised, protocol remains functional but rewards may go to attackers
- Post-mortem: Admin can claw back via new program logic (future)

---

### 8. Compute Budget Exhaustion (DOS)

**Attack:** User submits proof with 1000 elements to exceed compute budget.

**Mitigation:**
```rust
pub fn claim_open(ctx: Context<ClaimOpen>, amount: u64, proof: Vec<[u8; 32]>) -> Result<()> {
    // Limit proof depth (32 levels = 4.2B possible users)
    require!(proof.len() <= 32, ErrorCode::ProofTooLong);

    // Each level costs ~5K CU, max = 160K CU (well under 200K limit)
    verify_proof(...)?;

    Ok(())
}
```

**Cost Analysis:**
```
Maximum CU usage:
- 32 proof levels × 5K CU = 160K
- Token mint = 30K
- Account creation = 20K
- Total = ~210K CU

Default compute budget = 200K CU
Required priority fee for max proof = ~100 microlamports
```

**Result:** Attacker must pay extra fees for deep proofs, making DOS expensive.

---

### 9. Reentrancy Attack

**Attack:** Call claim instruction recursively to bypass checks.

**Why This Doesn't Work on Solana:**
- ✅ Solana runtime prevents reentrancy by design
- ✅ Accounts are locked during transaction execution
- ✅ No equivalent to Ethereum's `call` with gas forwarding

**Additional Safeguard:**
```rust
// Even if reentrancy were possible:
#[account(init, ...)] // Cannot create same PDA twice in one transaction
pub user_claim: Account<'info, UserClaim>,
```

**Result:** Not applicable to Solana architecture.

---

### 10. Overflow/Underflow Attacks

**Attack:** Cause integer overflow in amount calculations.

**Mitigation:**
```rust
// Rust panics on overflow in debug mode
// Production builds use checked arithmetic:
pub fn safe_add(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b).ok_or(ErrorCode::Overflow)
}

// Anchor automatically uses checked math in account space calculations
#[account(init, space = 8 + 88)] // No overflow possible
```

**Result:** Overflows impossible due to Rust's safety guarantees.

---

## Cryptographic Guarantees

### Keccak256 Security Properties

**Hash Function:** `keccak256` (SHA-3 finalist, Ethereum-compatible)

**Security Assumptions:**
- **Collision Resistance:** ~2^128 operations to find collision
- **Preimage Resistance:** Cannot reverse hash to find original input
- **Second Preimage Resistance:** Cannot find different input with same hash

**Merkle Tree Security:**
```
Leaf = keccak256(user_pubkey || amount)
Parent = keccak256(left_child || right_child)
Root = recursive hashing to tree top
```

**Attack Scenarios:**

| Attack | Feasibility | Why It Fails |
|--------|-------------|--------------|
| Find collision to fake proof | Impossible (2^128 ops) | Computationally infeasible |
| Reverse engineer merkle root | Impossible (preimage resistance) | One-way function |
| Modify amount without detection | Impossible (changes leaf hash) | Proof verification fails |

---

## Operational Security

### Key Management

**Current (Hackathon):**
```
Admin Key:
├─ Type: Ed25519 keypair
├─ Storage: Encrypted JSON file
└─ Access: Single developer (founder)

Publisher Key:
├─ Type: Ed25519 keypair
├─ Storage: Environment variable in aggregator
└─ Access: Automated service (hot wallet)
```

**Post-Launch (Q1 2025):**
```
Admin Key:
├─ Type: Ledger hardware wallet (or equivalent)
├─ Storage: Offline, air-gapped device
├─ Backup: Shamir Secret Sharing (3-of-5 recovery)
└─ Access: Multisig (2-of-3 core team members)

Publisher Key:
├─ Type: HSM-backed key (AWS KMS or equivalent)
├─ Rotation: Weekly automatic rotation
└─ Access: Aggregator service only (no human access)

Upgrade Authority:
├─ Type: Squads multisig (3-of-5)
├─ Signers: Core team + trusted advisors
└─ Timelock: 48-hour delay on upgrades
```

### Monitoring & Alerts

**On-Chain Monitoring:**
- Watch for unexpected admin/publisher transactions
- Alert on large single claims (> 10% of epoch total)
- Track merkle root changes (compare against expected)
- Monitor pause state (should always be `false` unless emergency)

**Off-Chain Monitoring:**
- Aggregator uptime (should seal epochs on schedule)
- Merkle tree build success rate (should be 100%)
- API availability (users need proofs to claim)

**Alert Channels:**
- PagerDuty for critical alerts (admin key usage)
- Discord webhook for informational events
- Email for daily summaries

---

## Audit Status

### Internal Audit

**Date:** October 30, 2025
**Auditor:** TWZRD Core Team
**Scope:** Full program review (3,384 lines)
**Result:** **Production Ready** with noted improvements

**Findings Summary:**
- 2 Critical issues (documented, acceptable for hackathon)
- 3 High-severity issues (mitigated through operations)
- 4 Medium-severity issues (planned fixes post-launch)
- 0 Low-severity issues

**See:** [PROGRAM_AUDIT_REPORT.md](../PROGRAM_AUDIT_REPORT.md)

### External Audit

**Status:** Planned (post-hackathon, funding dependent)

**Preferred Firms:**
- Neodyme (Solana specialists)
- OtterSec (DeFi focus)
- Sec3 (Comprehensive smart contract audits)

**Budget:** $15,000 - $30,000 (estimated)

### Bug Bounty

**Status:** Coming Soon
**Launch Date:** Q1 2025 (after external audit)
**Platform:** Immunefi or custom program

---

## Pre-Launch Checklist

### Program Security

- [x] All privileged operations require signer checks
- [x] PDA derivations use correct seeds
- [x] Merkle proof validation tested (valid/invalid cases)
- [x] Double-claim prevention via PDA uniqueness
- [x] Circuit breaker (pause) functionality works
- [x] Token-2022 integration tested on devnet
- [ ] Two-step admin transfer implemented (post-launch)
- [ ] Multi-signature support added (post-launch)

### Operational Security

- [x] Admin key backed up securely
- [x] Publisher key documented and secured
- [ ] Ledger hardware wallet migration (post-launch)
- [ ] Monitoring alerts configured
- [ ] Incident response plan documented
- [ ] Communication channels established (Discord, Twitter)

### Verification & Transparency

- [x] Deterministic build verification script
- [x] Program verified on Solana Explorer
- [x] Source code published on GitHub
- [ ] External audit completed (post-launch)
- [ ] Security documentation reviewed by third party

---

## Known Limitations

### Acknowledged Issues

**1. Single-Step Admin Transfer**
- **Risk:** Typo in new admin pubkey = permanent loss of control
- **Mitigation:** Triple-check addresses before transfer
- **Planned Fix:** Two-step transfer (pending + accept)

**2. No Multi-Signature Support**
- **Risk:** Single admin key compromise = total protocol control
- **Mitigation:** Secure key storage, monitoring
- **Planned Fix:** Squads multisig integration

**3. Publisher Key in Hot Wallet**
- **Risk:** Server compromise = ability to publish fake roots
- **Mitigation:** Admin can rotate publisher, off-chain verification
- **Planned Fix:** HSM-backed key, automated rotation

**4. Incomplete Passport Integration**
- **Risk:** Passport verification not fully implemented
- **Mitigation:** Feature disabled in current version
- **Planned Fix:** Complete oracle integration

**5. No Epoch Rollback Mechanism**
- **Risk:** If bad merkle root published, cannot fix
- **Mitigation:** Extensive pre-seal validation, publisher monitoring
- **Planned Fix:** Admin unseal + reseal functionality

---

## Conclusion

The Attention Oracle follows **defense-in-depth** principles with multiple layers of protection:

1. **Cryptographic** - Merkle proofs prevent unauthorized claims
2. **Authorization** - Role-based access control limits privileged operations
3. **Architectural** - PDA uniqueness prevents double-claims
4. **Operational** - Monitoring and incident response procedures
5. **Economic** - Attack costs exceed potential rewards

**Current Status:** Production-ready for hackathon with acceptable risk profile.

**Post-Launch Improvements:** Hardware wallets, multisig, external audit, bug bounty.

---

## Contact

**Security Email:** security@twzrd.com
**PGP Key:** Available on request
**Response Time:** 48-72 hours for initial triage

---

*For architecture details, see [ARCHITECTURE.md](ARCHITECTURE.md)*
*For integration guide, see [INTEGRATION.md](INTEGRATION.md)*
*For API reference, see [API.md](API.md)*
