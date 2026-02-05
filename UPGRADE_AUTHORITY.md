# Upgrade Authority Change (Feb 5, 2026)

## What Changed
- **Before**: Both programs had upgrade authority set to Squads V4 vault PDA (`2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW`)
- **After**: Upgrade authority transferred to operational keypair (`2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`)

## Why

Solana's BPF Upgradeable Loader **rejects CPI calls** for security-sensitive operations including:
- `Upgrade` - Deploy new program code
- `Close` - Close buffer accounts and recover SOL
- `SetAuthority` - Only works because it's simpler than upgrade

This is a Solana security feature to prevent reentrancy attacks during upgrades.

**Consequence**: When upgrade authority is a PDA (like Squads vault), programs become effectively **unupgradeable** because:
1. PDAs can only sign via CPI
2. BPF Loader rejects CPI for upgrades
3. Result: No valid signature path exists

## Multisig Transactions
- **#43**: Transfer AO (token_2022) authority - Executed Feb 5, 2026
- **#44**: Transfer Channel Vault authority - Executed Feb 5, 2026
- **#45**: Transfer Vault buffer authority - Executed Feb 5, 2026
- **#46**: Transfer AO buffer authority - Executed Feb 5, 2026

## Security Fixes Deployed (Feb 5, 2026)

**AO Program (GnGzNds...)**
- Tx: `2PrL581ZNUVcA2zqnT8twgq29ytmVDi6eLEMHhUsjGvNayodnxTy1Lufp2gjhcWGzxw8HwtDMhU2Wxd1WNzoR5C2`
- Fix: Future-dated proof prevention (`snapshot_slot <= clock.slot`)
- Fix: Mint validation in close_stake_pool

**Channel Vault (5WH4UiS...)**
- Tx: `4i52Qbkm8HqiJeujjH2jbF7Tvyd7MUVzo54JREk7UnQwqt9Ypn6Su7MS9kSuhC3Xf12moAKKdm5uTaTkSLdWbfya`
- Fix: excess_rewards guard in compound (prevents phantom inflation)
- Fix: excess_rewards guard in emergency unstake

## Going Forward
- Program upgrades done via `solana program deploy` with operational keypair
- Security-critical changes still require code review + staged deployment
- Once protocol is stable, consider making programs immutable (`--final`)

## Trust Model
- Operational keypair is held by core team
- All upgrade transactions are public on-chain
- Code changes reviewed before deployment
- Future: Timelock + governance for major upgrades
