# Program Verification

This repo contains two upgradeable programs deployed on Solana mainnet:

- `token_2022` (oracle + claims + staking)
- `ccm_hook` (Token-2022 transfer-hook helper program)

Verification is intentionally treated as a first-class status item. When the repo commit that matches the deployed
bytecode is tagged (or mainnet is upgraded to a verifiable build of a tagged release), verification is **Verified**
(green).

## Current Status (Mainnet)

| Program | Program ID | Last Deployed Slot | On-Chain Executable Hash | Verification |
|--------|-----------|--------------------|--------------------------|-------------|
| token_2022 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `391176164` (`2026-01-04T03:00:04Z`) | `ca17ba5923d1867e1a66feb6aaa05e18b27ebeee0f98a04dcea8e5c6af6ab18d` | Verified |
| ccm_hook | `8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS` | `391176540` (`2026-01-04T03:02:31Z`) | `fae7cf0cd9fcd6d19d97fd45720727f85bfd4c2d31d102d7d8b7be9df4c06140` | Verified |

Repo `main` head matches the on-chain hashes above (commit `3215f7b`).

## Path 1: Solana Verify CLI (Recommended)

### 1) Fetch on-chain hash

```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com <PROGRAM_ID>
```

Note: `solana-verify` expects a full RPC URL for `-u`. If `-u mainnet-beta` fails, use the HTTPS URL above (or set your
default RPC via `solana config set --url https://api.mainnet-beta.solana.com`).

### 2) Build deterministic artifacts (Docker)

Anchor verifiable builds (recommended for this repo):

```bash
anchor build --verifiable --program-name token_2022 --no-idl
anchor build --verifiable --program-name ccm_hook --no-idl
```

Hash the local verifiable artifacts:

```bash
solana-verify get-executable-hash target/verifiable/token_2022.so
solana-verify get-executable-hash target/verifiable/ccm_hook.so
```

### 3) Verify a specific commit directly from GitHub

```bash
solana-verify verify-from-repo https://github.com/twzrd-sol/attention-oracle-program.git \
  --program-id <PROGRAM_ID> \
  --commit-hash <COMMIT> \
  --library-name <LIBRARY_NAME> \
  --mount-path .
```

## Path 2: Anchor Verify (Wrapper Around solana-verify)

From the repo root:

```bash
anchor verify --program-name token_2022 --current-dir GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop -- -u https://api.mainnet-beta.solana.com
anchor verify --program-name ccm_hook --current-dir 8VE2Wt5JNusGUCwkrpVmdwgdgyD6vYPEHop2g2CAArzS -- -u https://api.mainnet-beta.solana.com
```

## How We Keep Verified

1) **Tag the exact deployed commit**

- Identify the commit that reproduces the on-chain hash for each program.
- Tag it (e.g., `mainnet-token_2022-2025-12-31`, `mainnet-ccm_hook-2025-12-06`).
- Re-run `solana-verify verify-from-repo ... --commit-hash <TAG_COMMIT>` and record results here.

2) **Upgrade mainnet to a verifiable build from a tagged release**

- Produce a verifiable build (`anchor build --verifiable`).
- Deploy that exact artifact to mainnet.
- Immediately verify and record the on-chain hash + the release tag in `DEPLOYMENTS.md` and this file.

## Gotchas

- Avoid deploying a non-verifiable build after producing a verified artifact for the same release; hashes will not match.
- Verification is per-program; `token_2022` and `ccm_hook` may be on different deployed versions.

## Toolchain (Reference)

- solana-cli: 3.0.10
- solana-verify: 0.4.12
- anchor-cli: 0.32.1
- Docker image: `solanafoundation/anchor:v0.32.1`
