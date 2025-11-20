# 🏗️ Attention Oracle Protocol Architecture

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`  
**Version:** 0.2.0 (Agave 3.0 Compatible)
**Note:** Public-facing overview; no secrets or private infra are documented here.

## 1. High-Level Overview

The Attention Oracle is a **Verifiable Distribution Protocol**. Off-chain aggregators measure attention (views, chats, interactions) and publish Merkle roots on-chain. Users then make verifiable on-chain claims against these published commitments; no trusted third party is required to validate claim inclusion or settlement.

### Canonical Flow (Production)

1. **Ingest** – A private aggregator validates events off-chain.
2. **Publish** – The aggregator calls `set_channel_merkle_root` to update the per-channel ring buffer.
3. **Claim** – Users call `claim_channel_open` (and optionally `claim_channel_open_with_receipt`).
4. **Reputation** – Users accumulate history via passport instructions.
5. **Fees** – `transfer_hook` reads Passport tier to shape fee splits; off-chain keepers harvest fees based on on-chain events.

Legacy epoch-state instructions are gated behind the `legacy` feature and are intended only for migrations and historical cleanup.

---

## 2. Core Data Structures

### 🟢 ChannelState (Ring Buffer)

- **Purpose:** Stores the last `CHANNEL_RING_SLOTS` epochs of Merkle roots for a given channel.
- **Type:** `#[account(zero_copy)]` with `AccountLoader` for safe zero-copy access.
- **Seeds:** `[
  b"channel_state",
  mint.key().as_ref(),
  streamer_key.as_ref(),
]`
- **Streamer Key:**
  - Derived from a human-readable channel identifier.
  - On-chain: `keccak256("channel:" || channel.to_ascii_lowercase())`.
- **Why:** Lets users claim from recent epochs even as new epochs are published, without a new account per epoch.

### 🛂 PassportRegistry (Identity)

- **Purpose:** Stores a user’s on-chain reputation and tier.
- **Seeds:** `[
  b"passport_owner",
  user_hash,
]`
- **Fields:** `owner`, `user_hash`, `tier`, `score`, `epoch_count`, `weighted_presence`, `badges`, `tree`, `leaf_hash`, `updated_at`, `bump`.
- **Usage:** Read by `transfer_hook` to apply creator-tier multipliers.

### ⚙️ ProtocolState

- **Purpose:** Global configuration for a given mint.
- **Seeds:**
  - Singleton: `[b"protocol"]` (original path).
  - Mint-keyed: `[b"protocol", mint.key().as_ref()]` (canonical open instance).
- **Fields:** admin, publisher, treasury, mint, paused, require_receipt, version, bump.

### 💸 FeeConfig

- **Purpose:** Controls transfer-fee behavior and tier multipliers.
- **Fields:** `basis_points`, `max_fee`, `drip_threshold`, `treasury_fee_bps`, `creator_fee_bps`, `tier_multipliers: [u32; 6]`, `bump`.
- **Tier Multipliers:** Fixed-point with denominator 10_000 (4 decimals), mapping passport tiers to fee multipliers.

---

## 3. Canonical Instructions (Frontend & Integrators)

### `set_channel_merkle_root`

- **Role:** Publish an epoch Merkle root for a given channel.
- **Accounts:**
  - `payer` – signs and pays rent.
  - `protocol_state` – mint-keyed config.
  - `channel_state` – zero-copy ring buffer PDA.
- **Inputs:** `channel: String`, `epoch: u64`, `root: [u8; 32]`.
- **Behavior:**
  - Derives `streamer_key` from `channel`.
  - Creates `ChannelState` account if missing.
  - Enforces monotonic epoch progression per slot.
  - Writes `root` and clears the bitmap for the slot.

### `claim_channel_open`

- **Role:** Canonical CCM claim path (used by user UI / wallets).
- **Accounts:**
  - `claimer`
  - `protocol_state` (mint-keyed)
  - `channel_state`
  - `mint`
  - `treasury_ata` (PDA-owned)
  - `claimer_ata` (init_if_needed)
- **Inputs:** `channel`, `epoch`, `index`, `amount`, `id`, `proof`.
- **Guarantees:**
  - Protocol not paused.
  - `ChannelState` PDA matches `[CHANNEL_STATE_SEED, mint, streamer_key]`.
  - Epoch slot matches `epoch`.
  - `index` within bounds and bitmap bit clear (no double-claim).
  - Merkle proof verified using sorted Keccak pairs.
  - CCM transferred from treasury to claimer using the program PDA as authority.

### `claim_channel_open_with_receipt`

- Same as `claim_channel_open`, with an optional cNFT receipt mint via Bubblegum when `mint_receipt = true`. Useful for high-value attention events where an on-chain receipt NFT adds value.

### `transfer_hook` (Token-2022)

- **Role:** Dynamic fee allocation based on passport tier. Registered as a Token-2022 transfer hook.
- **Behavior:**
  - Computes base treasury + creator BPS for a given transfer amount.
  - Scans `remaining_accounts` for a `PassportRegistry` matching the transfer owner.
  - Applies tier multiplier (0.0–1.0 in fixed point) to compute creator share.
  - Emits `TransferFeeEvent` (no direct token movement; Token-2022 handles withheld fees).

### Passports (`mint_passport_open`, `upgrade_passport_open`, etc.)

- Mint, upgrade, reissue, and revoke on-chain reputation tied to `user_hash`.
- Used by transfer hooks and off-chain systems as a durable measure of fandom/engagement.

---

## 4. Legacy vs Canonical

The program deliberately separates canonical production paths from historical/experimental ones.

- **Canonical (default build):**
  - `initialize_mint`
  - `set_channel_merkle_root`
  - `claim_channel_open`
  - `claim_channel_open_with_receipt`
  - `transfer_hook`
  - `update_fee_config[_open]`, `update_tier_multipliers`, `harvest_fees`
  - Admin and passport entrypoints

- **Legacy (feature = `legacy`):**
  - EpochState-based instructions (`set_merkle_root`, `set_merkle_root_open`, `claim`, `claim_open`, `claim_points_open`, epoch close / force-close).
  - Intended for data migration and cleanup only.

- **Demo (feature = `demo`):**
  - `initialize_channel`, `set_merkle_root_ring`, `claim_with_ring`, `close_old_epoch_state`.
  - Provided as a reference; not used in production flows.

By default, `legacy` and `demo` are **off**, keeping the IDL and binary focused on the canonical ring-buffer + passport architecture.

---

## 5. Solana Kit / Frontend Integration

### Streamer Key Derivation (TypeScript)

```ts
import { keccak_256 } from "@noble/hashes/sha3";
import { PublicKey } from "@solana/web3.js";

export const PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
);

export const getStreamerKey = (channel: string): PublicKey => {
  const lower = channel.toLowerCase();
  const preimage = Buffer.from(`channel:${lower}`);
  const hash = keccak_256(preimage); // 32 bytes
  return new PublicKey(hash);
};

export const getChannelStatePda = (
  mint: PublicKey,
  channel: string
): PublicKey => {
  const streamerKey = getStreamerKey(channel);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), mint.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  )[0];
};
```

### Project Layout (example)

```text
my-solana-dapp/
├── web/
│   ├── components/
│   └── utils/
│       └── attention-oracle/
│           ├── idl.json     # Copy from target/idl/token_2022.json
│           ├── types.ts     # Copy from target/types/token_2022.ts
│           └── client.ts    # Program wrapper + PDA helpers
```

Copy artifacts from this repo after `anchor build`:

```bash
anchor build
cp target/idl/token_2022.json   <frontend>/src/idl/
cp target/types/token_2022.ts   <frontend>/src/types/
```

---

## 6. Binary Verification ("Green Checkmark")

To prove that the on-chain binary matches this source tree:

```bash
# Local reproducible build
anchor clean && anchor build

# Verifier environment (e.g., GitHub Actions runner):
# Compare on-chain program with this source using Anchor's verifier
anchor verify GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --current-dir .
```

A successful verification indicates the bytecode on-chain is compiled from this exact source, which is what explorers and auditors rely on when marking a program as "Verified".

---

## 7. Verified Toolchain

- Anchor: 0.32.1 (via `avm`)
- Rust: 1.91.x (CI uses 1.91.1)
- Solana/Agave: 3.0.x (CI installs 3.0.10)

The CI pipeline builds and verifies with the versions above; use these to reproduce the green check locally.

---

## 8. Instruction Matrix (Auth, Accounts, Constraints)

- set_channel_merkle_root
  - Signers: `payer` (must equal `protocol_state.admin` or `protocol_state.publisher`).
  - Accounts: `protocol_state` (mint‑keyed PDA), `channel_state` (ring buffer PDA), `system_program`.
  - Constraints: epoch monotonic per slot; `streamer_key` derived from `channel`; creates `channel_state` if missing.

- claim_channel_open / claim_channel_open_with_receipt
  - Signers: `claimer`.
  - Accounts: `protocol_state`, `channel_state`, `mint`, `treasury_ata` (PDA), `claimer_ata` (init_if_needed). Optional Bubblegum accounts when `mint_receipt = true`.
  - Constraints: protocol not paused; slot = `epoch % CHANNEL_RING_SLOTS`; `index` in range; bitmap bit must be clear; Merkle proof over sorted Keccak pairs.

- transfer_hook (Token‑2022)
  - Signers: payer (required by account struct, unused in hook logic).
  - Accounts: Token‑2022 program + mint with TransferHook and TransferFeeConfig; remaining accounts may include `PassportRegistry` for fee multipliers.
  - Behavior: computes treasury/creator fee shares; emits `TransferFeeEvent`; fee withholding handled by the mint’s extension.

- Admin updates (e.g., `update_fee_config[_open]`, `update_tier_multipliers`, `harvest_fees`)
  - Signers: admin (and/or designated authority as defined on `ProtocolState`).
  - Constraints: updates validated to remain within configured caps.

---

## 9. Threat Model & Assumptions

- Trust boundary: trustless with respect to inclusion/claim correctness once a Merkle root is published; trust‑minimized with respect to how off‑chain events are measured and aggregated.
- Off‑chain aggregator is trusted to compute correct Merkle roots; on‑chain verification prevents out‑of‑set claims but cannot validate off‑chain event semantics.
- Replay resistance via per‑slot bitmaps; double‑claims are rejected when the bit is already set.
- Channel identifiers are normalized to ASCII lowercase before deriving `streamer_key`.
- Program can be paused via `ProtocolState.paused` to stop claims during incidents.

IP disclosure note: This document intentionally omits off‑chain scoring heuristics, data sources, thresholds, and infrastructure topology. Only the on‑chain interfaces and guarantees are described.

Hardening options (future work): multiple authorized publishers, quorum commitments, and challenge windows to further reduce reliance on any single aggregator.

---

## 10. Constants & Limits (Reference)

- Ring buffer slots: `CHANNEL_RING_SLOTS = 10`.
- Max claims per epoch: `CHANNEL_MAX_CLAIMS = 4096` (bitmap size `CHANNEL_BITMAP_BYTES`).
- Max epoch claims (protocol‑level cap): `MAX_EPOCH_CLAIMS = 1_000_000`.
- ID length: `MAX_ID_BYTES = 64`.
- Token‑2022 decimals: `CCM_DECIMALS = 9`.
- Default transfer fee: `DEFAULT_TRANSFER_FEE_BASIS_POINTS = 10` (0.10%), capped by `MAX_FEE_BASIS_POINTS = 1000` (10%).
- Tier multipliers (fixed‑point denominator 10_000): `[0, 2000, 4000, 6000, 8000, 10000]`. Tier 0 default = 0% (no verified passport).

Note: CI/artifact paths in examples assume repo root. Workflows may copy artifacts to `dist/` for releases; adjust paths accordingly.
