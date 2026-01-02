# TWZRD Attention Oracle Program

This repository contains the on-chain Solana programs for TWZRD's attention oracle, including:

- Merkle-based reward claims for verified social engagement.
- Token-2022 transfer-hook logic and related configuration.
- On-chain state for channels, epochs, and protocol governance.

## Programs

- `token_2022` (main program): Merkle claims, channel state, fee and staking flows.
- `ccm_hook` (transfer-hook program): Token-2022 hook integration for extra accounts.

See `programs/` for sources and `Anchor.toml` for program IDs.

## Trust model and upgrades

These programs are upgradeable on-chain. If you integrate against them, you should:

- Verify the deployed program binary (see `VERIFY.md`).
- Check the current upgrade authority on-chain.
- Track published upgrade policy (see `DEPLOYMENTS.md`).

## Repository layout

- `programs/` - Anchor programs and instruction logic.
- `scripts/` - Operational scripts (require explicit CLUSTER + KEYPAIR).
- `docs/` - Public protocol specs.

## Root System

**V2 cumulative roots are the active system on mainnet.**

- `publish-root-v2.ts` - Current publisher script (calls `publish_cumulative_root`)

All V1 ring-buffer infrastructure has been removed. Legacy accounts have been closed and rent reclaimed.

## Build and verify

- Build: `anchor build`
- Verify: follow `VERIFY.md`

## Security

- Report vulnerabilities via `SECURITY.md`.

## Integration

- See `INTEGRATION.md` for Token-2022 transfer-hook guidance.
- See `DEPLOYMENTS.md` for program IDs and release policy.
- See `docs/LIVE_STATUS.md` for on-chain deployment facts + verification status.
