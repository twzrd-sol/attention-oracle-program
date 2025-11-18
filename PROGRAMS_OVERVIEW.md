# Programs Overview

Authoritative map of on-chain programs and how this public repository relates to the broader Twzrd stack.

This file is safe for GitHub. It references only public program IDs, code paths, and high-level private repo names. No keys, RPC URLs, or secrets belong here.

---

## 1. Active On-Chain Program (Public)

**Program ID (mainnet):** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

- **Purpose:** Token-2022-based distribution + transfer-hook program (claims, passports, points, and fee routing).
- **Runtime:** Solana mainnet, validator / CLI v2.3.x.
- **Source of truth (this repo):**
  - Crate: `programs/Cargo.toml` (name: `token-2022`, lib: `token_2022`)
  - Code: `programs/src/*`
  - Program ID binding: `programs/src/lib.rs` via `declare_id!("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");`

**Config references (this repo):**

- `Anchor.toml` — `[programs.mainnet] token_2022 = "GnGz..."`
- `Solana.toml` — `[programs.mainnet] GnGz... = "programs"`
- `.github/workflows/*` — `PROGRAM_ID` / `MAINNET_PROGRAM_ID` set to `GnGz...` for build/verify flows.
- `cli/src/cli.ts` — defaults to `AO_PROGRAM_ID` (env) or `GnGz...` for program ID.
- `sdk/*` — TypeScript and Rust SDKs default to `GnGz...`.

This is the only active on-chain program defined in this repository.

---

## 2. Legacy Program Artifacts (Historical)

The following directories exist only as legacy build artifacts; they do **not** contain active source code in this repo:

- `programs/attention-oracle/`
- `programs/milo-2022/`

Both contain only `target/` (compiled) directories in the current tree. The canonical source for the live program is `programs/src/*` as described above.

---

## 3. Public-Side Tooling in This Repo

These components are part of the open-core surface and interact with `GnGz...`, but do not hold secrets:

- **SDKs** (`sdk/`)
  - TypeScript and Rust SDKs for integrators.
  - Provide instruction builders, PDA helpers, and types.
  - Program ID taken from `AO_PROGRAM_ID` (env) or `GnGz...` by default.

- **CLI** (`cli/`)
  - Admin and diagnostic commands.
  - RPC URL default comes from `AO_RPC_URL` or `ANCHOR_PROVIDER_URL` (env), then mainnet.
  - Program ID from `AO_PROGRAM_ID` or `GnGz...`.

- **Oracle Example** (`oracles/x402-switchboard/`)
  - Minimal x402 + Switchboard integration example.
  - Uses `.env` for `PORT`, `SB_CLUSTER`, `SB_FEED` (validated at startup).
  - Demonstrates how off-chain services can read prices and handle HTTP 402 flows; it is not production infra.

- **Scripts** (`scripts/`)
  - `devnet-smoke.sh`: simple devnet deploy/test loop.
  - `upgrade-mainnet.sh`: mainnet upgrade + hash verification against `GnGz...`.
  - `bootstrap-env.sh`: creates `.env` from `.env.example` if missing.

All of these components are safe for GitHub and follow the secrets policy below.

---

## 4. Secrets and Keys Policy

- **Keys:**
  - Private keys are **never** stored in this repository.
  - Solana keypairs live in local paths (e.g., `~/.config/solana/id.json`, `~/.config/solana/admin-keypair.json`).
  - Code and config reference key *paths* only (e.g., `ANCHOR_WALLET=~/.config/solana/id.json`).

- **Environment variables:**
  - `.env.example` is tracked and contains placeholders only.
  - `.env` is local-only and gitignored (`.env`, `.env*`, `**/.env*`).
  - Core variables for this repo:
    - `ANCHOR_PROVIDER_URL` — RPC URL for Anchor / tests.
    - `AO_RPC_URL` — optional override for CLI/SDK RPC.
    - `ANCHOR_WALLET` — local keypair path.
    - `AO_PROGRAM_ID` — program ID (defaults to `GnGz...`).
    - `SB_CLUSTER`, `SB_FEED`, `PORT` — for the oracle example.

- **No secrets in Git:**
  - No `.env` with real values is committed.
  - No private key JSONs are committed.
  - No API keys, webhooks, or DB URLs are present in tracked files.

This repository is intentionally safe to make fully public.

---

## 5. Private Twzrd Infrastructure (Out of Scope Here)

The rest of the Twzrd stack lives in **private** repositories and codebases. They are not part of this public repo but are conceptually connected:

- **Aggregator / Oracle Services**
  - Collect off-chain engagement signals.
  - Produce Merkle roots and call the on-chain program.
  - Example private repos (names only, no code):
    - `twzrd-aggregator`
    - `attention-oracle-gateway`

- **Gateway / API Layer**
  - Exposes HTTP/JSON APIs to clients.
  - Handles authentication, rate limiting, and orchestration.
  - Example private repos:
    - `twzrd-backend`
    - `twzrd-gateway`

- **Database / Storage**
  - Stores off-chain state (engagement logs, creator configs, analytics).
  - Typically Postgres/Redis/OLAP systems.
  - Example private repos:
    - `twzrd-db`
    - `twzrd-analytics`

- **Infra / Observability**
  - Terraform/Ansible/Kubernetes, monitoring, alerting.
  - Example private repos:
    - `twzrd-infra`
    - `twzrd-observability`

These private repos should follow the same secrets policy as this one (no keys in Git; `.env` + secret stores only) but are intentionally kept off GitHub or in private mode.

---

## 6. Tagging and Versioning

The commit that corresponds to the first fully cleaned, secrets-free public state of this repo should be tagged:

- **Tag:** `v0.2.1-clean`
- **Scope:**
  - `programs/src` is the source of truth for `GnGz...`.
  - SDK/CLI/oracle example wired to env-based configuration.
  - All internal docs, keys, and envs removed from history.

Going forward:

- Use semantic version tags (`v0.2.x`, `v0.3.0`, etc.) on this repo when:
  - Program logic changes in `programs/src`.
  - SDKs or CLIs change in ways integrators care about.
- For each mainnet upgrade of `GnGz...`, consider tagging the exact source commit used to build the deployed binary.

