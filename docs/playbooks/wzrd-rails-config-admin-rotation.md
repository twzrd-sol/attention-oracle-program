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

## Blocker: the current admin cannot pay its own fee

**`2pHj...` holds 0 SOL** (checked 2026-09-03). It has to sign this instruction,
and with a zero balance it cannot be the fee payer: simulation fails before the
program is ever reached, with

```
Simulation failed: "AccountNotFound"
```

That is the account-not-funded error, not a missing Config account - the Config
PDA is present and owned by the program with 1,872,240 lamports. So this is
almost certainly why the rotation was never completed on 2026-08-24: the key was
drained as part of standing it down, which also removed its ability to hand off
its own role.

Two ways through, both needing an operator decision:

1. **Fund `2pHj...` with a few thousand lamports** (~0.001 SOL covers a 5,000
   lamport fee many times over), rotate, and let the remainder sit. Simplest,
   and the funding is trivially small - but it briefly re-funds a key that was
   deliberately drained.
2. **Use a separate fee payer.** A Solana transaction's fee payer need not be
   the instruction's signer: `2pHj...` signs as admin while a funded key pays.
   This needs a `FEE_PAYER` keypair option added to `scripts/set-admin.ts`
   (set `payerKey` to the fee payer and pass both keypairs to `tx.sign`), plus a
   funded key the operator controls. `8di6...` has 0.178 SOL but its keypair is
   not on the ops box, so it cannot co-sign from there.

Option 2 is cleaner if a funded operator key is available to sign locally;
option 1 is faster.

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

`scripts/set-admin.ts` implements this, modeled on `scripts/set-reward-rate.ts`:
dry-run by default, derive accounts, verify the signer really is the on-chain
admin, sign, simulate, and only then broadcast behind `BROADCAST=1` plus a typed
confirmation phrase, with `I_UNDERSTAND_MAINNET=1` additionally required on
mainnet. It refuses an all-zeros `NEW_ADMIN` (mirroring the on-chain guard), a
no-op where `NEW_ADMIN` already equals the current admin, and any keypair that
is not the current on-chain admin. After a confirmed send it re-reads the Config
account and asserts the admin actually changed rather than trusting the
signature.

```bash
CLUSTER=mainnet-beta RPC_URL="<dedicated rpc>" \
  KEYPAIR=~/.config/solana/id.json \
  NEW_ADMIN=8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8 \
  npx tsx scripts/set-admin.ts                       # dry run, simulate only

# then, once the fee-payer blocker above is resolved:
... BROADCAST=1 I_UNDERSTAND_MAINNET=1 npx tsx scripts/set-admin.ts
```

Dry run on 2026-09-03 got as far as confirming the derivation and authority:

```
Config    : 7pwUU1hv3hCNNTAPmDyMRCeKoMPEz3k5cH1PTbWDNQR6
Signer    : 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
Current on-chain admin: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

so the signer check passes. Simulation then failed on the zero balance, which
means **the deployed binary has not yet been proven to accept `set_admin`** -
the transaction never reached the program. Treat a clean simulation as the real
go/no-go once the fee payer is sorted; do not skip it.

## After it lands

- Update the `Config admin` row in `docs/playbooks/rails-canary-launch.md` and
  remove the note explaining why it differs from the upgrade authority.
- Record the signature in `docs/wzrd-rails-mainnet-verification.md`.
