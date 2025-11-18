# @attention-oracle/sdk

TypeScript SDK for Attention Oracle - Verifiable token distribution on Solana.

## Installation

```bash
npm install @attention-oracle/sdk
# or
yarn add @attention-oracle/sdk
```

## Quick Start

```typescript
import {
  AttentionOracleClient,
  ClaimBuilder,
  MerkleProof,
} from '@attention-oracle/sdk';
import { Connection, Keypair } from '@solana/web3.js';

// Initialize client
const connection = new Connection('https://api.mainnet-beta.solana.com');
const client = new AttentionOracleClient(connection);

// Check passport tier
const passport = await client.getPassport(userPubkey);
console.log('Tier:', passport?.tier);

// Claim tokens
const proof: MerkleProof = {
  claimer: userPubkey,
  index: 42,
  amount: BigInt(1_000_000_000),
  id: 'claim_001',
  proof: [...proofHashes],
  epochIndex: 12345,
};

const claimTx = new ClaimBuilder()
  .addClaim(userPubkey, 'kaicenat', proof)
  .build();
```

## Features

- ✅ Type-safe instruction builders
- ✅ PDA derivation helpers
- ✅ Merkle proof verification
- ✅ Passport tier checking
- ✅ Token-2022 transfer hooks support

## PDA Derivation

```typescript
// Passport PDA
const [passportPda, bump] = AttentionOracleClient.derivePassportPda(user);

// Channel PDA
const [channelPda, bump] = AttentionOracleClient.deriveChannelPda('kaicenat');

// Treasury PDA
const [treasuryPda, bump] = AttentionOracleClient.deriveTreasuryPda();
```

## Examples

See [examples/](../examples/) for complete copy-paste ready code:

- `01-claim-tokens.ts` - Claim tokens using merkle proof
- `02-check-passport.ts` - Check user's passport tier
- `03-transfer-with-hooks.ts` - Transfer with dynamic fees

## API Reference

### AttentionOracleClient

**Methods:**
- `getPassport(user: PublicKey)` - Fetch passport account
- `getChannel(channelId: string)` - Fetch channel info
- `hasUserClaimed(user, channelId, epochIndex)` - Check claim status

**Static Methods:**
- `derivePassportPda(user)` - Derive passport PDA
- `deriveChannelPda(channelId)` - Derive channel PDA
- `deriveEpochPda(channel, epochIndex)` - Derive epoch PDA
- `deriveTreasuryPda()` - Derive treasury PDA
- `deriveCreatorPoolPda()` - Derive creator pool PDA

### ClaimBuilder

Build claim transactions:

```typescript
const claimTx = new ClaimBuilder()
  .addClaim(user, channelId, proof)
  .build();
```

### Types

- `PassportTier` - Enum of tier levels (0-6)
- `MerkleProof` - Proof structure for claims
- `ChannelConfig` - Channel configuration
- `PassportState` - Passport account data

## Development

```bash
# Generate types from IDL
npm run generate

# Build
npm run build

# Test
npm test

# Publish
npm publish --access public
```

## License

Dual MIT/Apache-2.0

## Links

- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Repository**: https://github.com/twzrd-sol/attention-oracle-program
- **Documentation**: https://github.com/twzrd-sol/attention-oracle-program/tree/main/sdk
