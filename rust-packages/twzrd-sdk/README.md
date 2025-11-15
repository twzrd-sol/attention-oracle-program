# twzrd-sdk

Rust SDK for **TWZRD Attention Oracle** on Solana.

> Open-core Solana primitive for tokenized attention. Presence → Proof → Tokens.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
twzrd-sdk = "0.1"
```

## Quick Start

```rust
use twzrd_sdk::{TwzrdClient, PROGRAM_ID};
use solana_sdk::pubkey::Pubkey;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TwzrdClient::new("https://api.mainnet-beta.solana.com");

    // Get channel state
    let channel_state = client.get_channel_state(&streamer_pubkey).await?;
    println!("Channel epoch: {}", channel_state.current_epoch);

    Ok(())
}
```

## Features

- 🦀 Idiomatic Rust API
- ⚡ Async/await with Tokio
- 🔐 Type-safe program interactions
- 📚 Comprehensive documentation

## Documentation

- [Getting Started](https://docs.twzrd.xyz/rust/getting-started)
- [API Docs](https://docs.rs/twzrd-sdk)
- [Examples](https://github.com/twzrd-sol/attention-oracle-program/tree/main/rust-packages/examples)

## License

MIT © TWZRD Inc.

---

Built in Houston, TX · [Website](https://twzrd.xyz) · [GitHub](https://github.com/twzrd-sol/attention-oracle-program)
