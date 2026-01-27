# Compliance and Controls

This document summarizes operational and security controls for the on-chain programs in this repository. It is provided for transparency and does not constitute legal or regulatory advice.

## Scope

- **In scope:** On-chain programs in `programs/`, build/verification process, and deployment metadata in this repo.
- **Out of scope:** Off-chain services (indexers, APIs, wallets, frontends), third-party infrastructure, and partner integrations.

## Security Program

- **Vulnerability reporting:** See `SECURITY.md`.
- **Internal review:** See `docs/SECURITY_AUDIT.md`.
- **CI testing:** Automated checks run on each change (see `.github/workflows`).

## Build and Deployment Integrity

- **Verifiable builds:** See `VERIFY.md` for reproducible build steps.
- **On-chain verification:** See `DEPLOYMENTS.md` for program IDs, upgrade authority, and on-chain hashes.
- **Release process:** Documented in `DEPLOYMENTS.md`.

## Upgradeability and Governance

- Programs are currently **upgradeable**.
- Upgrade authority is documented in `DEPLOYMENTS.md`.
- Roadmap includes transition to multisig and governance with timelock.

## Treasury Controls

- Treasury outflows are via cumulative claims (`claim_cumulative`, `claim_cumulative_sponsored`).
- Native Token-2022 transfer fees are harvested via `harvest_fees` (see `docs/specs/transfer-fee-capture.md`).
- Treasury behavior is documented in `docs/TREASURY.md`.

## Incident Response

- **Pause mechanism:** Admin can pause claims when needed.
- **Publisher rotation:** Admin can rotate publisher keys.
- **Emergency response:** Vulnerability response timelines in `SECURITY.md`.

## Data Handling and Privacy

- On-chain state is **public** by design.
- Programs do **not** store personal identifiers; they store public keys and program state.
- See `PRIVACY.md` for on-chain specifics.

## Regulatory and Compliance Notes

- This repository provides open-source on-chain programs.
- It does not perform KYC/AML, custody, or user identity verification.
- Integrators are responsible for compliance within their jurisdictions.

## Contact

For security issues, see `SECURITY.md`.
