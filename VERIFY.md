# Program Verification

This repo contains the Liquid Attention Protocol programs deployed on Solana mainnet.

## Current Status (Mainnet)

| Program | Program ID | Version | Framework | On-Chain Hash | Verification |
|---------|-----------|---------|-----------|---------------|-------------|
| ao-v2 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | v2 (Pinocchio) | `cargo build-sbf` | `e699f6b609d66befd2af09e63181bcf4d0d9f82b4be48141f11958a7e9f64bca` | Pending |
| channel_vault | `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ` | v1 (Anchor) | `anchor build` | `acc2f64f7c0ab2f21298717d64d427c163fd0ba74d7bb376fd1d680f69b8c732` | Verified (legacy) |

### Version History

The AO program (`GnGz...`) was upgraded from Anchor (v1, `token_2022` crate) to Pinocchio (v2, `ao-v2` crate) on 2026-03-14 via Squads V4 proposal #135. Binary size: 867KB -> 153KB. Same program ID, same account layouts, same discriminators.

Previous verification (Anchor v1):
- Commit: `430ccc60c2ee614b964e429aee9403cc95f45115`
- Tx: `KPmfuW67sKqKw9QPyfVDrsFw7i2okjXmFZwe6fY3DktLmFJs9cfyqz2pvCR9j45KSPByhKea33RGPzvEmrAbxnW`

## Verify ao-v2 (Pinocchio)

### 1) Fetch on-chain hash

```bash
solana-verify get-program-hash GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

### 2) Build locally with cargo build-sbf

```bash
cargo build-sbf --manifest-path programs/ao-v2/Cargo.toml
solana-verify get-executable-hash target/deploy/ao_v2.so
```

### 3) Verify from repo

```bash
solana-verify verify-from-repo \
  --url https://github.com/twzrd-sol/attention-oracle-program \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --library-name ao_v2 \
  --bpf-flag channel_staking
```

Note: `--bpf-flag channel_staking` enables the channel staking feature which is included in the deployed binary.

## Verify channel_vault (Anchor, legacy)

```bash
anchor build --verifiable --program-name channel_vault --no-idl
solana-verify get-executable-hash target/verifiable/channel_vault.so
```

## Upgrade Authority

Both programs are governed by Squads V4 multisig (3-of-5): `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`

## Build Configuration

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.91.1"

# Cargo.toml [profile.release]
overflow-checks = true
lto = "fat"
codegen-units = 1
opt-level = 3
strip = true
panic = "abort"
```

## Toolchain

- solana-cli: 2.2.x
- solana-verify: 0.4.11
- cargo-build-sbf (via Agave)
- anchor-cli: 0.32.1 (channel_vault only)
