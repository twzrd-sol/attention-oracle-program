# wzrd-rails H-01 deploy runbook (operator)

**Target**: mainnet `wzrd-rails` `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`
**Payload**: H-01 listen-payout governance fix (`03acd2c`, merged via #129; program
source unchanged through current `main` tip `7108e1f`)
**Requires**: upgrade-authority signer `8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8`
+ a docker-capable machine. **Never deploy from a host build.**

## Why this deploy

The live binary predates H-01: `claim_listen_payout` still reverts while
`paused == true`, payout-admin rotation is 1-step, and there is no
`Config.admin` emergency unpause. The trust-rail seam contract
(`docs/trust-rail-aop-seam.md` §3.1) assumes post–H-01 semantics for hard
binds, so the seam's pause-tolerant hard-bind rule is invalid against the
current on-chain pin until this deploy lands and pins refresh.

## Preflight state of record (RPC-verified 2026-08-13)

| Field | Value |
|-------|-------|
| ProgramData | `ftWybjbYPRamJFCZQ14wndSPYRbbHqgmGoCHaZtxEaU` |
| Live executable hash | `3128b6448cfa18c15d543bd755935c4fb01eca382bd4e5a20d41edddbd882006` (pre–H-01, = #126 pin) |
| Last deploy slot | `428118420` (2026-06-22) |
| Upgrade authority | `8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8` |

Layout safety: `PayoutAuthorityConfig.pending_admin` is carved from the former
`_reserved[32]`; body stays 334 bytes and live accounts deserialize with
`pending_admin = Pubkey::default()`. **No account migration IX is required**
before or after this deploy (see layout note in
`programs/wzrd-rails/src/state.rs`).

## Steps

1. **Pin the source.** `git fetch origin main && git checkout <main tip>`;
   confirm a clean tree and that `programs/wzrd-rails/` is unchanged since
   `03acd2c` (`git log --oneline 03acd2c..HEAD -- programs/wzrd-rails/` must
   show nothing, or docs/tests only).
2. **Tests.** `cargo test -p wzrd-rails --features localtest --tests` — all
   green before building.
3. **Verifiable build (docker).**
   `solana-verify build --library-name wzrd_rails`
   (or `anchor build --verifiable --program-name wzrd_rails`). Building via
   solana-verify from committed source is what qualifies the new binary for
   the `rebuild` evidence level — do not skip it for a host build.
4. **Record the artifact hash.**
   `solana-verify get-executable-hash target/deploy/wzrd_rails.so` → `NEW_HASH`.
5. **Confirm the chain is still pre-deploy.**
   `solana-verify get-program-hash -u https://api.mainnet-beta.solana.com BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9`
   must still print `3128b644…`. If it doesn't, stop and reconcile.
6. **Deploy via buffer** (atomic, retry-safe), signing with the upgrade
   authority keypair (path intentionally not recorded here):
   ```bash
   solana program write-buffer target/deploy/wzrd_rails.so \
     -u mainnet-beta -k <authority-keypair> --with-compute-unit-price <µlam>
   solana program deploy --buffer <BUFFER> \
     --program-id BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
     -u mainnet-beta -k <authority-keypair>
   ```
7. **Post-deploy verification.**
   - `solana-verify get-program-hash …` == `NEW_HASH`.
   - `solana-verify verify-from-repo -u https://api.mainnet-beta.solana.com \
     --program-id BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
     https://github.com/twzrd-sol/attention-oracle-program \
     --library-name wzrd_rails --commit-hash <pinned commit>`
     — a pass upgrades rails from `artifact_hash` to `rebuild`.
8. **On-chain sanity.** Fetch the listen-payout authority config PDA and
   confirm it deserializes under the new layout with
   `pending_admin == Pubkey::default()`; optionally smoke `set_paused`
   round-trip on the new auth rules before announcing.
9. **Pin + doc refresh (seam §5.1 — mandatory).** Prior hard binds are
   invalid until this completes:
   - `docs/wzrd-rails-mainnet-verification.md`: new hash, deploy slot,
     evidence level (`rebuild` if step 7 passed).
   - `docs/trust-rail-aop-seam.md` + `docs/agent-trust-integration-checklist.md`:
     refresh the rails pin; the post–H-01 pause-tolerant hard-bind rules
     become valid only for the new hash.
   - Trust-rail consumer config: update the pinned rails hash.
   - `GOAL.md` item 3: mark H-01 as deployed (not just merged).

## Failure handling

- Deploy lands but `verify-from-repo` fails: the binary is still your built
  artifact — keep the evidence level at `artifact_hash` (record the artifact
  proof) and investigate the rebuild mismatch before advertising `rebuild`.
- Any hash mismatch at step 5 or 7: halt, do not refresh pins, treat hard
  binds as suspended until reconciled.
