# Attention Oracle Protocol

Open-source Solana infrastructure for verifiable, engagement-based reward distribution.

This protocol enables:
- **Merkle-based Distribution:** Efficient verification of off-chain engagement data for on-chain claiming.
- **Transfer Fee Harvesting:** Automated management of Token-2022 transfer fees into protocol treasuries.
- **Publisher Controls:** On-chain state management for data publishers and channel configurations.

## Architecture

The protocol consists of Anchor-based programs designed for the **Solana Token-2022** standard.

### Core Components
* **`token_2022` (Main Program):** Handles cumulative Merkle claims, channel configuration, and fee harvesting logic.
* **Data Oracles:** Off-chain publishers submit Merkle roots representing verified user engagement (e.g., "Attention").
* **Treasury Management:** Logic to sweep withheld transfer fees to a designated destination.

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
# Build the program
anchor build

# Verify against mainnet (Docker required)
./verify.sh
```

### Testing Safety (Important)

`Anchor.toml` currently targets **mainnet**. Running `anchor test` will deploy by default.
Use one of the following to avoid accidental mainnet deployments:

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
