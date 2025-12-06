# Program Verification

## Deployed Program

| Network | Program ID | Status |
|---------|------------|--------|
| Mainnet | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Active |

## Current On-Chain Hash

```
98d11157c302a71b294056ceb7854c8cf70c5fddd60dbf0b5b00a5d990c94b8b
```

Last verified: 2025-12-06

## Verification Steps

```bash
# 1. Install solana-verify
cargo install solana-verify

# 2. Get on-chain program hash
solana-verify get-program-hash -u mainnet-beta GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# 3. Build deterministic binary (requires Docker)
anchor build --verifiable

# 4. Get local build hash
solana-verify get-executable-hash target/verifiable/token_2022.so

# 5. Compare hashes
```

## Build Environment

- Anchor: 0.32.1
- Solana: 3.0.10
- Rust: 1.91.1 (workspace), 1.84.x (SBF target)

## Release History

| Version | Commit | Hash | Date |
|---------|--------|------|------|
| v1.0.0-mainnet | - | `4923cd27ee5a87ec1a3470efa5ced0e88ff10ece4dc278d1f998f58904607fe2` | 2025-11-23 |
| current | main HEAD | `98d11157c302a71b294056ceb7854c8cf70c5fddd60dbf0b5b00a5d990c94b8b` | 2025-12-06 |

## Notes

The on-chain program was upgraded after v1.0.0-mainnet with bug fixes:
- `fix: drop lamports borrow before CPI`
- `fix: assign via system CPI`
- `fix: release channel_state borrow`

A new tagged release should be created to match the current on-chain deployment.
