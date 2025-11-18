# Attention Oracle SDKs

Official SDKs and tooling for Attention Oracle on Solana.

## 📦 Packages

### TypeScript SDK

```bash
npm install @attention-oracle/sdk
```

Full-featured TypeScript SDK for web and Node.js applications.

- ✅ Type-safe instruction builders
- ✅ Merkle proof verification
- ✅ Passport tier checking
- ✅ Token-2022 hooks support
- 📚 [Documentation](./typescript/README.md)

### Rust SDK

```toml
[dependencies]
attention-oracle-sdk = "0.2"
```

Native Rust SDK for on-chain programs and validators.

- ✅ Zero-copy deserialization
- ✅ PDA derivation helpers
- ✅ Merkle proof utils
- ✅ `no_std` compatible
- 📚 [Documentation](./rust/)

### CLI

```bash
npm install -g @attention-oracle/cli
```

Command-line tools for admin operations.

- ✅ Passport management
- ✅ Fee harvesting
- ✅ PDA derivation
- ✅ Receipt export
- 📚 [Documentation](../cli/README.md)

## 🚀 Quick Start

### TypeScript Example

```typescript
import { AttentionOracleClient } from '@attention-oracle/sdk';
import { Connection } from '@solana/web3.js';

const connection = new Connection('https://api.mainnet-beta.solana.com');
const client = new AttentionOracleClient(connection);

// Check passport tier
const passport = await client.getPassport(userPubkey);
console.log('Tier:', passport?.tier); // 0-6

// Claim tokens
const claimTx = new ClaimBuilder()
  .addClaim(user, 'kaicenat', proof)
  .build();
```

### Rust Example

```rust
use attention_oracle_sdk::{AttentionOracleClient, ID};
use solana_sdk::pubkey::Pubkey;

// Derive passport PDA
let (passport_pda, bump) = AttentionOracleClient::derive_passport_pda(&user, &ID);

// Compute merkle leaf
let leaf = AttentionOracleClient::compute_leaf(&user, 0, 1000, "claim_001");

// Verify proof
let valid = AttentionOracleClient::verify_proof(leaf, &proof, root);
```

### CLI Example

```bash
# Check passport
ao passport 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD

# Derive treasury PDA
ao pda --type treasury

# Harvest fees
ao harvest
```

## 📚 Examples

Copy-paste ready examples for common operations:

- [Claim Tokens](./examples/01-claim-tokens.ts)
- [Check Passport](./examples/02-check-passport.ts)
- [Transfer with Hooks](./examples/03-transfer-with-hooks.ts)

## 🏗️ Architecture

```
sdk/
├── typescript/          # TypeScript SDK (npm)
│   ├── src/
│   │   ├── client.ts    # Main client
│   │   ├── types.ts     # Type definitions
│   │   └── utils.ts     # Merkle utils
│   └── package.json
│
├── rust/                # Rust SDK (crates.io)
│   ├── src/
│   │   ├── lib.rs       # Main library
│   │   └── utils.rs     # Utilities
│   └── Cargo.toml
│
├── examples/            # Copy-paste examples
│   ├── 01-claim-tokens.ts
│   ├── 02-check-passport.ts
│   └── 03-transfer-with-hooks.ts
│
└── cli/                 # Admin CLI tool
    ├── src/cli.ts
    └── package.json
```

## 🔑 Key Concepts

### PDA Derivation

All PDAs follow consistent patterns:

```typescript
// Passport: ["passport", user_pubkey]
const [passportPda] = AttentionOracleClient.derivePassportPda(user);

// Channel: ["channel", channel_id]
const [channelPda] = AttentionOracleClient.deriveChannelPda("kaicenat");

// Epoch: ["epoch", channel_pda, epoch_index]
const [epochPda] = AttentionOracleClient.deriveEpochPda(channel, 12345);

// Treasury: ["treasury"]
const [treasuryPda] = AttentionOracleClient.deriveTreasuryPda();

// Creator Pool: ["creator_pool"]
const [creatorPoolPda] = AttentionOracleClient.deriveCreatorPoolPda();
```

### Merkle Proofs

Leaf computation:

```typescript
const leaf = keccak256(
  claimer_pubkey +
  index (u32, LE) +
  amount (u64, LE) +
  id (UTF-8 string)
);
```

Verification:

```typescript
computedHash = leaf;
for (const proofElement of proof) {
  computedHash = keccak256(
    min(computedHash, proofElement) +
    max(computedHash, proofElement)
  );
}
return computedHash === merkleRoot;
```

### Passport Tiers

| Tier | Label | Creator Fee Multiplier |
|------|-------|------------------------|
| 0 | Unverified | 0.0x (no fees) |
| 1 | Emerging | 0.2x (0.01%) |
| 2 | Active | 0.4x (0.02%) |
| 3 | Established | 0.6x (0.03%) |
| 4 | Featured | 0.8x (0.04%) |
| 5+ | Elite/Legendary | 1.0x (0.05%) |

## 🧪 Testing

```bash
# TypeScript SDK
cd sdk/typescript
npm test

# Rust SDK
cd sdk/rust
cargo test

# CLI
cd cli
npm test
```

## 📦 Publishing

### TypeScript SDK

```bash
cd sdk/typescript
npm run build
npm publish --access public
```

### Rust SDK

```bash
cd sdk/rust
cargo publish
```

### CLI

```bash
cd cli
npm run build
npm publish --access public
```

## 🔗 Links

- **Program**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Repository**: https://github.com/twzrd-sol/attention-oracle-program
- **Documentation**: https://github.com/twzrd-sol/attention-oracle-program/tree/main/sdk
- **NPM**: https://www.npmjs.com/package/@attention-oracle/sdk
- **Crates.io**: https://crates.io/crates/attention-oracle-sdk

## 📄 License

Dual MIT/Apache-2.0
