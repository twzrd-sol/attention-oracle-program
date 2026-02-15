# Attention Oracle Protocol

Open-source Solana infrastructure for verifiable, engagement-based reward distribution.

This protocol enables:
- **Merkle-based Distribution:** Efficient verification of off-chain engagement data for on-chain claiming.
- **Transfer Fee Harvesting:** Automated management of Token-2022 transfer fees into protocol treasuries.
- **Publisher Controls:** On-chain state management for data publishers and channel configurations.

## Architecture

The protocol consists of Anchor-based programs designed for the **Solana Token-2022** standard.

### Core Components
* **`token_2022` (Attention Oracle):** Merkle-based cumulative reward claims (V2), channel configuration, publisher controls, and transfer fee harvesting.
* **`channel_vault` (Staking Vault):** Liquid staking wrapper — deposits, withdrawals, auto-compound of transfer fees, and an on-chain ExchangeRateOracle.
* **Data Oracles:** Off-chain publishers submit Merkle roots representing verified user engagement.
* **Treasury Management:** Permissionless sweeping of withheld Token-2022 transfer fees to the protocol treasury.

## Trust Model & Upgrades

These programs are **upgradeable**. Integrators and users should verify the security posture before interacting:

1.  **Verify Binaries:** See `VERIFY.md` to reproduce the on-chain build from source.
2.  **Check Authority:** Monitor the on-chain upgrade authority.
3.  **Review Policy:** See `DEPLOYMENTS.md` for active program IDs and upgrade policies.

## Repository Layout

* `programs/` - Smart contracts (Anchor/Rust).
* `scripts/` - Operational scripts for root publishing and maintenance.
* `docs/` - Protocol specifications and integration guides.

## Build and Verify

Ensure you have the [Solana Tool Suite](https://docs.solanalabs.com/cli/install) and [Anchor](https://www.anchor-lang.com/) installed.

```bash
# Build the programs
anchor build

# Verifiable build (required for mainnet deployment — uses Docker)
anchor build --verifiable --program-name token_2022
anchor build --verifiable --program-name channel_vault

# Verify against mainnet
./verify.sh
```

### Testing

The repo uses [LiteSVM](https://github.com/LiteSVM/litesvm) for fast, deterministic program tests — no validator required.

```bash
# Rust unit + integration tests (LiteSVM)
cargo test -p channel-vault --lib              # 57 vault tests
cargo test -p attention-oracle-token-2022      # Cumulative claim tests

# TypeScript integration tests
./scripts/anchor-test-safe.sh
```

### Testing Safety (Important)

`Anchor.toml` targets **mainnet**. Running `anchor test` directly will attempt to deploy.
Always use one of the following:

```bash
# Guarded runner (blocks mainnet unless explicitly allowed)
./scripts/anchor-test-safe.sh

# Safe: run tests without deploying
anchor test --skip-deploy

# Safe: run against localnet
ANCHOR_PROVIDER_URL=http://127.0.0.1:8899 anchor test
```

If you must run against mainnet (rare), set `ALLOW_MAINNET_ANCHOR_TEST=1` explicitly.

## Integration

Developers integrating with the Attention Oracle (e.g., wallets, analytics dashboards) should refer to:

* `INTEGRATION.md`: Technical guide for generating proofs and claiming rewards.
* `DEPLOYMENTS.md`: List of active deployments and Program IDs.

## Security

This project takes security seriously.

* **Audits:** See `docs/SECURITY_AUDIT.md`.
* **Reporting:** Please report vulnerabilities responsibly via the process outlined in `SECURITY.md`.

## License

Licensed under Apache 2.0 / MIT. See `LICENSE` for details.
