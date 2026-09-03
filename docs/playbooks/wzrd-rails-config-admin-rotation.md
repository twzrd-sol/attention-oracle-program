# wzrd-rails Config admin rotation

**Status**: not executed. Everything below is verified against mainnet; the
broadcast is the only step outstanding.

## Why this exists

The `wzrd_rails` BPF upgrade authority rotated to
`8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8` on 2026-08-24. The program's own
Config PDA admin **was not rotated with it**. Reading mainnet on 2026-09-03 the
Config admin is still:

```
2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

That key was retired from its authority roles. It still administers this
program, so `set_admin`, `set_reward_rate`, `compensate_external_stakers` and
`initialize_pool` all still answer to a key the operator has stood down.

> Note for whoever reads `docs/playbooks/rails-canary-launch.md`: the
> `Config admin` row there is **current on-chain state, not stale copy**. Do not
> "correct" it to the new authority. It becomes stale only once this rotation
> lands, and then both should move together.

## The instruction

`set_admin` is **single step**. There is no `pending_admin` on `Config` and no
accept leg. The two-step `set_payout_admin` / `accept_payout_admin` pair belongs
to `PayoutAuthorityConfig`, a different account, and does not apply here.

Once this lands, only the new admin can administer the config. There is no
recovery from a wrong `NEW_ADMIN` except a program upgrade.

| Field | Value | How verified |
|---|---|---|
| Program | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | `declare_id!` in `programs/wzrd-rails/src/lib.rs` |
| Config PDA | `7pwUU1hv3hCNNTAPmDyMRCeKoMPEz3k5cH1PTbWDNQR6` | derived from seed `b"config"`, bump 255; matches the live account |
| Accounts | `config` writable, `admin` signer | `AdminOnly` context, `lib.rs` |
| Discriminator | `fba300345bc2bb5c` | `sha256("global:set_admin")[0..8]` |
| Data | discriminator + 32-byte `new_admin` | `set_admin(ctx, new_admin: Pubkey)` |
| Current admin | `2pHj...` | bytes 8..40 of the Config account |

On-chain guard: `set_admin` rejects `Pubkey::default()`. Per audit M-3 / EZ-7 an
all-zeros admin permanently retires the role and is recoverable only via program
upgrade, so the check is in the program, not just in tooling.

## Preconditions

1. **Confirm you control `NEW_ADMIN`.** Single step, no accept leg. For
   `8di6...`, that key is active and funded - 5 successful mainnet transactions
   on 2026-08-24 and 0.178 SOL at the time of writing - but "active" is not
   "you can sign with it right now". Check before broadcasting.
2. The signer must be the **current** admin. The keypair for `2pHj...` is the
   default Solana CLI identity on battleship. Any tool that reads the default
   keypair path signs as the live config admin of a mainnet program; treat that
   as a finding in its own right, separate from this rotation.
3. Use the dedicated RPC, not a public endpoint.

## Procedure

Follow `scripts/set-reward-rate.ts`, the existing admin-instruction script in
this repo. Its contract is the one to copy: dry-run by default, derive accounts,
verify the signer really is the on-chain admin, sign, simulate, and only then
broadcast behind `BROADCAST=1` plus a typed confirmation phrase, with
`I_UNDERSTAND_MAINNET=1` additionally required on mainnet.

A `scripts/set-admin.ts` following that shape does not exist yet. Write it, or
perform the rotation with equivalent tooling, but keep these properties:

- refuse if the loaded keypair is not the current on-chain admin
- refuse `Pubkey::default()` as `NEW_ADMIN`, mirroring the on-chain guard
- refuse a no-op where `NEW_ADMIN` already equals the current admin
- **simulate before sending, and abort on any simulation error** - this is what
  proves the deployed (pre-H-01) binary accepts the instruction, rather than
  assuming it from current source
- after confirmation, re-read the Config account and assert the admin actually
  changed, rather than trusting a successful signature

## After it lands

- Update the `Config admin` row in `docs/playbooks/rails-canary-launch.md` and
  remove the note explaining why it differs from the upgrade authority.
- Record the signature in `docs/wzrd-rails-mainnet-verification.md`.
