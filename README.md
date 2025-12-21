# TWZRD Attention Oracle Program

This repository contains the on-chain Solana programs for TWZRD's attention oracle, including:

- Merkle-based reward claims for watch-time participation.
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

## Build and verify

- Build: `anchor build`
- Verify: follow `VERIFY.md`

## Security

- Report vulnerabilities via `SECURITY.md`.

## Integration

- See `INTEGRATION.md` for Token-2022 transfer-hook guidance.
- See `DEPLOYMENTS.md` for program IDs and release policy.

## Feature logic (no program upgrade required)

### Gasless claims (sponsored)

The program already supports a gasless claim path via `claim_channel_sponsored`:

- **Payer signs, claimer does not.** Authorization comes from the Merkle proof, which encodes the
  claimer pubkey in the leaf. Funds can only flow to the wallet in the published tree.
- **Relayer flow (off-chain):**
  1. Backend fetches proof + leaf data for an eligible wallet/epoch.
  2. Backend builds a transaction calling `claim_channel_sponsored`, with the relayer as payer.
  3. Backend submits, records the signature, and rate-limits per wallet/channel/epoch.
- **Security model:** the relayer cannot redirect funds; proof verification + bitmap replay
  protection enforce correctness.

This enables trustless, gasless claims for non-crypto-native users without upgrading the program.

### Streamer graph (off-chain)

The streamer graph can be derived off-chain from watch-time/chat events and mapped to on-chain
channels:

- **Nodes:** streamers (channel subjects) and viewers (wallets or linked identities).
- **Edges:** viewer → streamer with weights (watch seconds, chat count, streaks, unique days).
- **Rollups:** per-epoch aggregates that can be used for reputation, boosts, or recommendations.

This is an off-chain indexing task and does not require an on-chain change.

## Planned (requires on-chain upgrade)

- Auto-split CCM between viewers and creators.
- Staking on streamers with earned CCM (multiplier + reputation).
