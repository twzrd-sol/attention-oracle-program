# twzrd-cli

Command-line interface for **TWZRD Attention Oracle** on Solana.

> Open-core Solana primitive for tokenized attention. Presence → Proof → Tokens.

## Installation

```bash
cargo install twzrd-cli
```

Or build from source:

```bash
git clone https://github.com/twzrd-sol/attention-oracle-program
cd attention-oracle-program/rust-packages/twzrd-cli
cargo install --path .
```

## Usage

```bash
# Show program info
twzrd info

# Get channel state
twzrd channel <STREAMER_PUBKEY>

# Claim tokens
twzrd claim <USER_PUBKEY> <CHANNEL_PUBKEY>

# Use custom RPC
twzrd --rpc-url https://api.devnet.solana.com info
```

## Features

- 🚀 Fast native binary
- 📊 JSON output support
- 🔐 Wallet integration
- 🌐 Multi-network support (mainnet/devnet/testnet)

## Documentation

- [Getting Started](https://docs.twzrd.xyz/cli/getting-started)
- [Command Reference](https://docs.twzrd.xyz/cli/commands)
- [Examples](https://github.com/twzrd-sol/attention-oracle-program/tree/main/rust-packages/examples)

## License

MIT © TWZRD Inc.

---

Built in Houston, TX · [Website](https://twzrd.xyz) · [GitHub](https://github.com/twzrd-sol/attention-oracle-program)
