# Attention Oracle

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Solana](https://img.shields.io/badge/Solana-Mainnet-blue)](https://explorer.solana.com/address/4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5)
[![Anchor](https://img.shields.io/badge/Anchor-0.30.1-purple)](https://www.anchor-lang.com/)

> On-chain merkle proof validation for decentralized attention rewards

The Attention Oracle is the public, verifiable core of TWZRD's attention rewards protocol. It enables **anyone** to prove their engagement on social platforms and claim Token-2022 rewards via cryptographic merkle proofs—without revealing private data or relying on centralized APIs.

---

## 🎯 What Problem Does This Solve?

**Today's Problem:** Social platforms control all attention data. Creators can't reward their audience directly, and users can't prove their engagement without platform APIs.

**Our Solution:**
1. **Off-chain:** TWZRD aggregators collect engagement signals (Twitch views, etc.) and build merkle trees
2. **On-chain:** This program validates proofs and distributes Token-2022 rewards
3. **Result:** Decentralized, verifiable, and permissionless attention rewards

**Why It Matters:**
- ✅ Creators reward their most engaged fans directly
- ✅ Users own proof of their attention history
- ✅ No platform intermediaries or API access required
- ✅ Fully auditable via deterministic builds

---

## 🏗️ Architecture

```
┌─────────────────────┐
│   Twitch Viewers    │
│   (Off-chain)       │
└──────────┬──────────┘
           │ Engagement Signals
           ▼
┌─────────────────────┐
│   TWZRD Aggregator  │    [Private: Sybil Detection]
│   (Off-chain)       │
└──────────┬──────────┘
           │ Sealed Epoch Data
           ▼
┌─────────────────────┐
│   Merkle Tree       │
│   (Computed)        │
└──────────┬──────────┘
           │ Root Hash + Proofs
           ▼
┌─────────────────────┐
│  Attention Oracle   │    [Public: This Repo]
│  (On-chain Program) │
└──────────┬──────────┘
           │ Proof Validation ✅
           ▼
┌─────────────────────┐
│  CCM Token Rewards  │
│  (Token-2022)       │
└─────────────────────┘
```

**Key Innovation:** We separate **signal collection** (private, anti-sybil) from **reward distribution** (public, verifiable). This gives users privacy while maintaining protocol transparency.

---

## 🚀 Quick Start

### For Integrators: Claim Rewards in 5 Minutes

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Milo2022 } from "./target/types/milo_2022";

// 1. Connect to the program
const programId = new anchor.web3.PublicKey("4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5");
const program = new Program<Milo2022>(IDL, programId, provider);

// 2. Fetch your merkle proof from TWZRD API
const proof = await fetch(`https://api.twzrd.com/proof/${epoch}/${user}`).then(r => r.json());

// 3. Derive your claim PDA
const [claimPda] = anchor.web3.PublicKey.findProgramAddressSync(
  [Buffer.from("user-claim"), userPubkey.toBuffer(), channelPda.toBuffer()],
  programId
);

// 4. Execute the claim transaction
await program.methods
  .claimOpen(proof.amount, proof.proof)
  .accounts({
    protocolState,
    channelState,
    userClaim: claimPda,
    mint,
    user: userPubkey,
  })
  .rpc();

console.log("✅ Rewards claimed!");
```

### For Auditors: Verify the Deployed Program

```bash
# Clone and verify the program matches on-chain bytecode
git clone https://github.com/twzrd-sol/attention-oracle
cd attention-oracle
export PROGRAM_ID=4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5
export SOLANA_RPC_URL=https://api.mainnet-beta.solana.com

# Run deterministic build verification
./scripts/verify-build.sh

# Expected output:
# ✅ Local build hash matches on-chain program
# ✅ Verification complete - program is authentic
```

### For Builders: Fork and Customize

```bash
# Install dependencies
npm install -g @coral-xyz/anchor-cli
cargo install --git https://github.com/coral-xyz/anchor anchor-cli --locked

# Clone repository
git clone https://github.com/twzrd-sol/attention-oracle
cd attention-oracle

# Build the program
anchor build

# Run tests
anchor test

# Deploy your own instance
anchor deploy --provider.cluster mainnet
```

---

## 📖 Documentation

Comprehensive guides for all experience levels:

- **[Integration Guide](docs/INTEGRATION.md)** - Step-by-step integration with code examples
- **[Architecture Deep Dive](docs/ARCHITECTURE.md)** - Protocol design, PDAs, and data flow
- **[Security Model](docs/SECURITY.md)** - Threat model, authorization, and audits
- **[API Reference](docs/API.md)** - Complete instruction documentation

---

## 🔐 Security

### Current Status
- **Program ID:** `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5`
- **Upgrade Authority:** Secured via hardware wallet (post-hackathon migration planned)
- **Admin Separation:** Admin and publisher roles are isolated for defense-in-depth
- **Emergency Controls:** Pause functionality available if critical issues discovered

### Security Features
- ✅ **Merkle Proof Verification** - Cryptographic validation of all claims
- ✅ **Double-Claim Prevention** - Per-user, per-epoch claim tracking
- ✅ **Token-2022 Integration** - Transfer fee support built-in
- ✅ **Circuit Breaker** - Emergency pause capability
- ✅ **Access Control** - Multi-tier authorization (admin/publisher/user)

### Responsible Disclosure
Found a vulnerability? Please see [SECURITY.md](SECURITY.md) for our responsible disclosure policy.

**Bug Bounty:** Coming soon (post-hackathon launch)

---

## 🎓 Key Features

### 1. Epoch-Based Claiming System
Claims are organized into discrete epochs (e.g., daily, weekly) with sealed merkle roots. This prevents retroactive manipulation and enables atomic reward distributions.

### 2. Ring Buffer Architecture
Channels use ring buffers to limit state growth while maintaining recent claim history. This enables efficient verification without unbounded account sizes.

```rust
pub struct ChannelState {
    pub ring_buffer: [u8; 32], // Compact circular buffer
    pub current_epoch: u64,
    pub total_claims: u64,
    // ... more fields
}
```

### 3. Token-2022 Advanced Features
- **Transfer Fees:** Protocol-level fee collection for sustainability
- **Transfer Hooks:** Extensible token behavior for future features
- **Metadata Extensions:** Rich token information on-chain

### 4. Passport Integration (Coming Soon)
Identity oracle integration for verified users, enabling tiered rewards and sybil resistance.

### 5. cNFT Receipt System (Experimental)
Compressed NFT receipts for on-chain proof of participation—reducing state costs by 1000x.

---

## 🛠️ Technical Specifications

| Spec | Value |
|------|-------|
| **Framework** | Anchor v0.30.1 |
| **Language** | Rust (Edition 2021) |
| **Program Size** | 636 KB (deployed) |
| **Total LOC** | 3,384 lines across 19 files |
| **Token Standard** | Token-2022 (SPL Token Extensions) |
| **Merkle Proof** | Keccak256 hashing |
| **Max Proof Depth** | 32 levels (4.2B potential claimants) |

---

## 📊 Program Instructions

### Admin Operations
- `initialize_protocol` - Bootstrap protocol state
- `update_admin_open` - Transfer admin authority (planned: 2-step transfer)
- `update_publisher_open` - Change merkle root publisher
- `set_paused_open` - Emergency circuit breaker
- `set_policy_open` - Update receipt requirements

### Channel Management
- `initialize_channel` - Create new claim channel
- `seal_epoch` - Finalize epoch and publish merkle root
- `update_ring_buffer` - Maintain circular claim history

### User Claims
- `claim_open` - Claim rewards with merkle proof
- `validate_passport` - Verify identity oracle credentials
- `verify_receipt_cnft` - Validate compressed NFT receipts

**Full Documentation:** See [docs/API.md](docs/API.md)

---

## 🧪 Testing

The program includes comprehensive test coverage (not public during beta):

- ✅ **Merkle Proof Validation** - Valid/invalid proof handling
- ✅ **Double-Claim Prevention** - Per-user epoch tracking
- ✅ **Epoch Lifecycle** - Sealing, unsealing, overwrites
- ✅ **Admin Authorization** - Role-based access checks
- ✅ **Token-2022 Fees** - Fee calculation accuracy
- ✅ **Emergency Pause** - Circuit breaker functionality
- ✅ **Ring Buffer Rotation** - Circular buffer edge cases

Tests run in CI/CD pipeline before each mainnet deployment.

---

## 🌟 Why Open-Core?

**Open (This Repo):**
- ✅ Core protocol logic (verifiable by anyone)
- ✅ Token distribution mechanisms
- ✅ Security-critical validations
- ✅ Deterministic build verification

**Private (TWZRD Backend):**
- ✅ Signal aggregation heuristics (anti-gaming)
- ✅ Sybil detection algorithms (proprietary)
- ✅ Data pipelines and infrastructure
- ✅ Operational tooling and monitoring

**Rationale:** Users can verify **what they receive** (on-chain), while TWZRD protects **how signals are collected** (competitive advantage, anti-gaming).

This mirrors industry standards:
- **Stripe:** Payment APIs (public) + fraud detection (private)
- **Chainlink:** Oracle contracts (public) + data sourcing (private)
- **Helium:** Network protocol (public) + coverage algorithms (private)

---

## 📜 License

MIT License - See [LICENSE](LICENSE) file for details.

**Open-source and free to:**
- Audit the code
- Fork for your own use case
- Integrate into your application
- Submit issues and feature requests

---

## 🤝 Contributing

We're currently in **private beta** and not accepting external contributions, but we welcome:

- **Bug Reports:** See [SECURITY.md](SECURITY.md) for responsible disclosure
- **Feature Suggestions:** Open an issue with your use case
- **Integration Questions:** Join our [Discord](https://discord.gg/twzrd) (coming soon)

### Building Locally

```bash
# Prerequisites
rustc 1.75+ (stable)
solana-cli 1.17+
anchor-cli 0.30+

# Clone and build
git clone https://github.com/twzrd-sol/attention-oracle
cd attention-oracle
anchor build

# Run verification
cargo test-sbf
```

### Submitting Issues

Please include:
1. Program ID and RPC endpoint
2. Transaction signature (if applicable)
3. Expected vs actual behavior
4. Minimal reproduction steps

See [.github/ISSUE_TEMPLATE/bug_report.md](.github/ISSUE_TEMPLATE/bug_report.md)

---

## 🚀 Roadmap

### Completed ✅
- [x] Core merkle proof claim system
- [x] Token-2022 integration with transfer fees
- [x] Multi-channel architecture with ring buffers
- [x] Admin/publisher role separation
- [x] Emergency pause functionality
- [x] Deterministic build verification
- [x] Mainnet deployment

### In Progress 🚧
- [ ] Passport identity oracle integration (90% complete)
- [ ] cNFT receipt verification (experimental)
- [ ] Fee distribution mechanisms (planned)

### Planned 📅
- [ ] Multi-signature admin support (post-hackathon)
- [ ] Two-step admin transfer safety (Q1 2025)
- [ ] Audit by external security firm (funding dependent)
- [ ] Points system for non-transferable rewards (Q1 2025)
- [ ] Cross-chain bridge support (Q2 2025)

---

## 🏆 Built For

**Hackathon:** Solana Radar Hackathon 2024
**Category:** DeFi Infrastructure / Creator Economy
**Team:** TWZRD Labs

**Why This Matters:**
- First decentralized attention rewards protocol on Solana
- Novel application of merkle proofs for social engagement
- Production-ready code quality with deterministic verification
- Open-core model balances transparency and sustainability

---

## 📞 Contact

- **Website:** https://twzrd.com
- **GitHub:** https://github.com/twzrd-sol
- **Twitter:** [@twzrd_sol](https://twitter.com/twzrd_sol)
- **Discord:** Coming soon post-hackathon

---

## 🙏 Acknowledgments

Built with:
- [Anchor Framework](https://www.anchor-lang.com/) - Solana program development
- [SPL Token-2022](https://spl.solana.com/token-2022) - Advanced token standard
- [solana-verify](https://github.com/Ellipsis-Labs/solana-verifiable-build) - Deterministic build verification

Special thanks to the Solana and Anchor communities for their excellent documentation and tooling.

---

**🔍 Verify this program on-chain:** [Solana Explorer](https://explorer.solana.com/address/4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5)

**⭐ Star this repo** if you find it useful for your project!

---

*Last Updated: October 30, 2025*
*Program Version: 1.0.0 (Mainnet)*
