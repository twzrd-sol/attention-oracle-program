# Attention Oracle Program

Open-source Solana program source for the Liquid Attention Protocol.

## Current Mainnet State

| Program | Program ID | Status | Upgrade Authority |
|---------|------------|--------|-------------------|
| `token_2022` / Attention Oracle | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Immutable on mainnet | None |
| `wzrd_rails` | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | Upgradeable on mainnet | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` |

Mainnet `token_2022` details:

- ProgramData: `5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L`
- Last deployed slot: `411276636` (`2026-04-05T21:47:02Z`)
- On-chain executable hash: `b5330fcca2c8dd807fb7d2609b74e72ae7d709c003d7697f275ff54dca7b53b1`

The historical Squads V4 multisig `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ` is retired for `token_2022`; it is not the current upgrade authority.

Verify the live authority yourself:

```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

## Provenance Status

The `token_2022` deployed binary is immutable on mainnet (executable hash `b5330fcca2c8dd807fb7d2609b74e72ae7d709c003d7697f275ff54dca7b53b1`, authority `null` since 2026-04-05). Bit-for-bit reproduction of that hash from this public source tree is in progress. The most recent verifiable-build attempt (`2026-04-24`, public source, current toolchain) produced a non-matching hash, consistent with build-environment drift rather than source divergence. Until reproduction is confirmed, treat this source tree as audit and reference material, not as verified deployed source.

Verify mainnet identity yourself:

```bash
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
```

## What This Source Contains

The `programs/attention-oracle` crate is built with Anchor `0.32.1` and exposes the `token_2022` library name. Active source-level features are the core attention loop (vault deposit, settle, claim), Merkle-based cumulative claims, treasury routing, and Token-2022 transfer-fee harvesting.

The workspace also contains `programs/wzrd-rails`, an Anchor `0.32.1` upgradeable program for protocol productivity rails.

## Build

```bash
anchor build --verifiable --program-name token_2022
anchor build --verifiable --program-name wzrd_rails
```

To compare a local verifiable build with mainnet:

```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
solana-verify get-executable-hash target/verifiable/token_2022.so
```

## Test

The repo uses LiteSVM for local program tests.

```bash
cargo test -p attention-oracle-token-2022 --features localtest --tests
cargo test -p wzrd-rails --features localtest --test core_loop
```

## License

MIT. See [LICENSE](LICENSE).
