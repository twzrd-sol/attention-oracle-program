# wzrd-rails mainnet verification

Status of record for the live `wzrd-rails` program binary on Solana mainnet.
Locked 2026-08-09 (hash match + deploy chronology). All times UTC.

## Identity

| Field | Value |
|-------|-------|
| Program ID | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` |
| ProgramData | `ftWybjbYPRamJFCZQ14wndSPYRbbHqgmGoCHaZtxEaU` |
| Upgrade authority | `8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8` (upgradeable) |
| Last deployed slot | `428118420` (2026-06-22 08:03:22 UTC) |
| Executable hash (solana-verify) | `3128b6448cfa18c15d543bd755935c4fb01eca382bd4e5a20d41edddbd882006` |

## Claim levels - keep the evidence streams separate

**Mechanical (proved):** the mainnet program binary is byte-identical (mod
loader padding) to the build artifact `target/deploy/wzrd_rails.so`
(mtime 2026-06-22 07:46:56 UTC). Both sides hash to `3128b644...` via
`solana-verify` (`get-program-hash` on mainnet, `get-executable-hash` on the
artifact). Match confirmed 2026-08-08.

**Inferential (strong chronology, not a build proof):** artifact -> commit
`d441724` (fix(wzrd-rails): M-03 emission remainder-carry + StakePool realloc
migration, 2026-06-22 07:46:21 UTC). The artifact mtime is 35 seconds after
the commit; the deploy landed ~17 minutes after that. Between `d441724` and
`425e115` (main tip at lock time), the only change under `programs/wzrd-rails/`
is `AUDIT_REPORT.md` - program source is untouched. A verifiable-build
reproduction was deliberately not attempted: the deploy came from a host
`anchor build`, so a docker verifiable rebuild could mismatch for
uninformative reasons, and a host rebuild in place would overwrite the proof
artifact.

**Net:** the live instruction surface matches the rails surface as of
`d441724`, including the June audit-fix wave (H-01..H-03, M-01..M-03, L-01,
I-04), under the mechanical binary-artifact proof plus the chronological
source inference above.

## Re-check commands

```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com \
  BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9

solana-verify get-executable-hash target/deploy/wzrd_rails.so

solana program show BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9 \
  -u mainnet-beta -k <any-keypair>
```

Expected hash: `3128b6448cfa18c15d543bd755935c4fb01eca382bd4e5a20d41edddbd882006`.

## Artifact preservation

The proof artifact lives in gitignored `target/` - one `cargo clean` destroys
it, and `solana program dump` recovers only the CURRENT binary (useless for
this deploy once an upgrade lands). Rules:

1. Off-repo preserved copies exist (operator box, hash re-verified identical):
   `~/_preserve-aop-rails-artifact-20260622/wzrd_rails.so` and
   `~/_preserve-aop-markets-artifact-20260707/wzrd_markets.so`.
2. Any future deploy must append a new verification record here (slot, time,
   hash, artifact path) BEFORE the old binary is replaced.

## Sibling programs - evidence levels at lock time

| Program | ID | Evidence level |
|---------|----|----------------|
| wzrd-markets | `7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy` | **Reproduced from committed source** - strongest. Deploy slot `431429172` (2026-07-07 18:27:13 UTC). PR #124 (`425e115`) verified byte-for-byte: deployed programdata body sha256 `4f7b14d65629cbcb298d97cba0cc7add236b5619a345b912648c15b5bba0fbcb` == original deploy artifact == fresh rebuild from committed source; markets src byte-identical between audited `cb9a1fe` and main. solana-verify executable hash: `d61bd46d6f21a195393f4db4ec00e12cc84ebdffd1d1a7baa456abe34cbc2ab1` (re-confirmed against mainnet 2026-08-09). |
| wzrd-rails | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | **Artifact-hash-verified** (this document) - binary matches repo artifact mechanically; source tie to `d441724` is chronological inference, not a rebuild proof. |
| AO v2 | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | **Unreproduced** - immutable; on-chain hash `b5330fcca2c8dd807fb7d2609b74e72ae7d709c003d7697f275ff54dca7b53b1` has not been reproduced from public source. Source tree is reference material only. |

Do not conflate these three levels. Both upgradeable programs share the same
single-key upgrade authority (`2pHjZL...` at lock time; rotated to
`8di6hHF8...` on 2026-08-24, RPC re-verified 2026-09-02).
