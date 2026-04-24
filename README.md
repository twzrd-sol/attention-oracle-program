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

The `token_2022` deployed binary is immutable, but the exact public source snapshot that reproduces the live mainnet hash is not yet identified.

As of `2026-04-24`, a clean verifiable build of the public `token_2022` source tree produced executable hash:

```text
15367a5ae5bd3fd4fcb5421b4c0380bbf1d116425738768e564530f0558c889e
```

That does not match the live mainnet hash:

```text
b5330fcca2c8dd807fb7d2609b74e72ae7d709c003d7697f275ff54dca7b53b1
```

Do not treat this public source tree as verified source for the live immutable `token_2022` program until that mismatch is resolved.

## What This Source Contains

The `programs/attention-oracle` crate is built with Anchor `0.32.1` and exposes the `token_2022` library name.

Source-level features include:

- Merkle-based cumulative reward claims
- Publisher and protocol state controls
- Token-2022 transfer-fee harvesting
- Market-vault deposit and settlement flows
- Optional `phase2` modules for channel staking, prediction markets, strategy vaults, and price feeds

The workspace also contains `programs/wzrd-rails`, an Anchor `0.32.1` program for CCM productivity rails.

## Token Mints

| Token | Mint | Standard |
|-------|------|----------|
| CCM | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` | Token-2022, 9 decimals, 50 bps transfer fee |
| vLOFI | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | Standard SPL Token, 9 decimals |

Verify with:

```bash
spl-token display Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM --url mainnet-beta
spl-token display E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS --url mainnet-beta
```

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
