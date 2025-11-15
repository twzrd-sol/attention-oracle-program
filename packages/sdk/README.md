# @twzrd/sdk

Official TypeScript SDK for **TWZRD Attention Oracle** on Solana.

> Open-core Solana primitive for tokenized attention. Presence → Proof → Tokens.

## Installation

```bash
npm install @twzrd/sdk
# or
yarn add @twzrd/sdk
```

## Quick Start

```typescript
import { Connection, PublicKey } from '@solana/web3.js';
import { TwzrdClient } from '@twzrd/sdk';

const connection = new Connection('https://api.mainnet-beta.solana.com');
const client = new TwzrdClient(connection);

// Get channel state
const channelState = await client.getChannelState(streamerPubkey);

// Claim tokens
const signature = await client.claimTokens(userPubkey, channelPubkey);
```

## Features

- 🎯 Type-safe Solana program interactions
- ⚡ Lightweight and dependency-minimal
- 🔐 Built on SPL Token 2022
- 📚 Comprehensive TypeScript definitions

## Documentation

- [Getting Started](https://docs.twzrd.xyz/getting-started)
- [API Reference](https://docs.twzrd.xyz/api)
- [Examples](https://github.com/twzrd-sol/attention-oracle-program/tree/main/examples)

## License

MIT © TWZRD Inc.

---

Built in Houston, TX · [Website](https://twzrd.xyz) · [GitHub](https://github.com/twzrd-sol/attention-oracle-program)
