# Stranded CCM — Buffer Authority Analysis

Date: 2026-08-01

This document records why 7,661,500 CCM held in the vault CCM buffer is
permanently unreachable, with the full derivation, so the conclusion is not
re-litigated. It is a closed historical record, not an open action item. No
recovery path exists; nothing here needs execution.

## Summary

| Item | Value |
|------|-------|
| Stranded balance | 7,661,500 CCM (`7661500000000000` raw, 9 decimals) |
| Holding account | `61jgyDAvbPEaX4hmaQguxuYNxx33hGnyP459zaoMD2N9` (Token-2022) |
| Mint | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` (CCM) |
| Token authority | `3QVLLPD6NfdsSCeaLvbZwavxrmGB8NECqXBBJUUD9bYA` |
| Share of total supply | 0.3831% of 1,999,999,989.3 CCM |
| Status | Permanently unreachable |

Recorded for supply accounting as **burned / non-circulating**.

## Root Cause

The tokens are not stranded because the legacy channel-vault program was closed.
They are stranded because the buffer was migrated to an `attention-oracle`-owned
PDA, and `attention-oracle` was made immutable while the only instruction that
can sign for that PDA was non-functional.

The two facts compound: either alone would have been recoverable.

## Derivation

### 1. The token authority holds no key

The buffer's token authority is `3QVLLPD6…D9bYA`. That address has no account on
mainnet, which leaves two possibilities: an unfunded keypair, or a PDA. The two
are distinguishable without knowing any seeds, because PDAs are constructed to
lie off the ed25519 curve.

```python
from solders.pubkey import Pubkey
Pubkey.from_string("3QVLLPD6NfdsSCeaLvbZwavxrmGB8NECqXBBJUUD9bYA").is_on_curve()
# False
```

Off-curve. No private key can exist for it, now or ever. Only a program can
authorize transfers out, via `invoke_signed`.

### 2. The owning program is `attention-oracle`

The signer seeds are `["vault", channel_config]`. Source lives in the
**`wzrd-final`** repository, not this one:
`programs/ao-v2/src/instructions/compound.rs`, `validate_buffer_authority`. Its
comment states the migration outright:

> `buffer_authority` is now the AO vault PDA `["vault", channel_config]` — a bare
> PDA with no stored data (unlike the old channel-vault vault PDA which had 291
> bytes).

and it derives the bump via `find_program_address` against AO's own program ID.
With `channel_config = J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW`:

| Candidate program | Derived PDA | Match |
|---|---|---|
| `attention-oracle` `GnGz…VZop` | `3QVLLPD6NfdsSCeaLvbZwavxrmGB8NECqXBBJUUD9bYA` (bump 254) | **yes** |
| channel-vault `5WH4…CXmQ` (closed) | `7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw` | no |
| `wzrd-markets` `7YZZ…93sy` | `7sBersS5mwcQuT61k6YYG4da14F2XZbBBEtvYUCaeZyA` | no |
| `wzrd-rails` `BdSv…SZy9` | `CSioEyA5kPB27msgD71NeLFXaXtpvp9t5mFgAFx83B5G` | no |

The derivation is self-consistent: the same seeds against the closed
channel-vault program produce `7tjCgZcs…`, which is exactly the legacy vault
account that no longer exists on-chain.

### 3. The only signer is unreachable

`compound` in `ao-v2` (in `wzrd-final`) is the sole instruction that signs for
this PDA. The
channel staking instruction family returns `Custom(101)` on the deployed program
(`docs/playbooks/rails-canary-launch.md`, 2026-05-05). `attention-oracle` has
`Authority: none` — revoked 2026-04-05 — so the instruction cannot be fixed,
replaced, or added.

There is no path by which any key or program can move these tokens.

## Verification

```bash
# Balance and authority
solana account 61jgyDAvbPEaX4hmaQguxuYNxx33hGnyP459zaoMD2N9

# Immutability of the only possible signer
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop | grep Authority
# Authority: none

# Legacy vault program is closed
solana program show 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ
# Error: Program ... has been closed
```

## Note for Supply Accounting

Treat the 7,661,500 CCM as burned rather than circulating. It cannot re-enter
supply under any future program deployment, because the authorizing program is
immutable and its signing path is broken.

A USD figure is deliberately omitted; the share of total supply (0.3831%) is the
durable measure.

## Related

The governance lesson — that burning upgrade authority before code paths are
exercised in production turns latent bugs into permanent ones — is recorded in
[`UPGRADE_AUTHORITY.md`](./UPGRADE_AUTHORITY.md), where it is the primary
argument for routing `wzrd-markets` and `wzrd-rails` through a multisig rather
than revoking their authority now.
