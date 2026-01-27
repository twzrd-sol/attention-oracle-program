# Program Verification

This repo contains the `token_2022` upgradeable program deployed on Solana mainnet.

Verification is intentionally treated as a first-class status item. When the repo commit that matches the deployed
bytecode is tagged (or mainnet is upgraded to a verifiable build of a tagged release), verification is **Verified**
(green).

## Current Status (Mainnet)

| Program | Program ID | Last Deployed Slot | On-Chain Executable Hash | Verification |
|--------|-----------|--------------------|--------------------------|-------------|
| token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `395779276` (`2026-01-25T06:14:40Z`) | `34d097682eadf09986cebdc25579c9c3aa30125f605cb7e0aa5751787bb5d4db` | Verified |

## Path 1: Solana Verify CLI (Recommended)

### 1) Fetch on-chain hash

```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

Note: `solana-verify` expects a full RPC URL for `-u`. If `-u mainnet-beta` fails, use the HTTPS URL above (or set your
default RPC via `solana config set --url https://api.mainnet-beta.solana.com`).

### 2) Build deterministic artifacts (Docker)

Anchor verifiable builds (recommended for this repo):

```bash
anchor build --verifiable --program-name token_2022 --no-idl
```

Hash the local verifiable artifacts:

```bash
solana-verify get-executable-hash target/verifiable/token_2022.so
```

### 3) Verify a specific commit directly from GitHub

```bash
solana-verify verify-from-repo https://github.com/twzrd-sol/attention-oracle-program.git \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --commit-hash <COMMIT> \
  --library-name token_2022 \
  --mount-path .
```

## Path 2: Anchor Verify (Wrapper Around solana-verify)

From the repo root:

```bash
anchor verify --program-name token_2022 --current-dir GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop -- -u https://api.mainnet-beta.solana.com
```

## How We Keep Verified

1) **Tag the exact deployed commit**

- Identify the commit that reproduces the on-chain hash.
- Tag it (e.g., `mainnet-token_2022-2025-12-31`).
- Re-run `solana-verify verify-from-repo ... --commit-hash <TAG_COMMIT>` and record results here.

2) **Upgrade mainnet to a verifiable build from a tagged release**

- Produce a verifiable build (`anchor build --verifiable`).
- Deploy that exact artifact to mainnet.
- Immediately verify and record the on-chain hash + the release tag in `DEPLOYMENTS.md` and this file.

## Gotchas

- Avoid deploying a non-verifiable build after producing a verified artifact for the same release; hashes will not match.

## Toolchain (Reference)

- solana-cli: 3.0.10
- solana-verify: 0.4.12
- anchor-cli: 0.32.1
- Docker image: `solanafoundation/anchor:v0.32.1`
