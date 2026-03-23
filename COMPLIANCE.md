# Compliance and Controls

Operational and security controls for the on-chain programs in this repository. Provided for transparency; does not constitute legal or regulatory advice.

## Scope

- **In scope:** On-chain programs in `programs/`, build and verification process, deployment metadata.
- **Out of scope:** Off-chain services, third-party infrastructure, partner integrations.

## Security

- **Vulnerability reporting:** [SECURITY.md](SECURITY.md)
- **On-chain security.txt:** Embedded in program binary (Neodyme standard)
- **Build verification:** [VERIFY.md](VERIFY.md) (deterministic Docker builds)

## Upgradeability and Governance

- Programs are **upgradeable** via Squads V4 multisig (3-of-5 threshold).
- Upgrade authority: `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW` (Squads vault PDA)
- No timelock. Upgrades take effect immediately.
- Full proposal history in [UPGRADE_AUTHORITY.md](UPGRADE_AUTHORITY.md).

## Treasury Controls

- Reward outflows via cumulative merkle claims (`claim_global_v2`, `claim_global_sponsored_v2`).
- Token-2022 transfer fees harvested via `harvest_fees` (permissionless, batched).
- Admin can pause claims when needed.

## Data Handling

- On-chain state is **public** by design.
- Programs store only public keys and protocol state. No personal identifiers.
- See [PRIVACY.md](PRIVACY.md).

## Regulatory Notes

- This repository provides open-source on-chain programs.
- It does not perform KYC/AML, custody, or user identity verification.
- Integrators are responsible for compliance within their jurisdictions.

## Contact

For security issues, see [SECURITY.md](SECURITY.md).
