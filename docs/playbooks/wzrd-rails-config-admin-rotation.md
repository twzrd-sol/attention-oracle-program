# wzrd-rails Config admin rotation

**Status**: not executed. Everything below is verified against mainnet. Tooling
is ready and the deployed binary is proven to accept the instruction; what is
still outstanding is settling custody of `NEW_ADMIN` (precondition 1) and then
the broadcast.

## Why this exists

The `wzrd_rails` BPF upgrade authority rotated to
`8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8` on 2026-08-24. The program's own
Config PDA admin **was not rotated with it**. Reading mainnet on 2026-09-03 the
Config admin is still:

```
2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

That key was retired from its authority roles. It still administers this
program. In the **deployed** binary (built from `d441724`) **nine** instructions
answer to `Config.admin`, not four:

| Instruction | Accounts context | Gate |
|---|---|---|
| `set_admin` | `AdminOnly` | `has_one = admin` |
| `set_reward_rate` | `SetRewardRate` | `has_one = admin` |
| `initialize_pool` | `InitializePool` | `has_one = admin` |
| `compensate_external_stakers` | `CompensateExternalStakers` | `has_one = admin` |
| `realloc_stake_pool` | `ReallocStakePool` | `has_one = admin` |
| `register_verified_moment` | `RegisterVerifiedMoment` | `constraint = config.admin == authority.key()` |
| `init_payout_authority_config` | `InitPayoutAuthorityConfig` | `has_one = admin` |
| `init_payout_cap_config` | `InitPayoutCapConfig` | `has_one = admin` |
| `init_payout_vault_config` | `InitPayoutVaultConfig` | `has_one = admin` |

The last three carry more weight than the count suggests. All three
listen-payout config PDAs are **absent on mainnet** - `getAccountInfo` returns
null for each on 2026-09-03:

```
authority_config  D6vVhAmEGBpNA9NhAfTMgQAraeG1as2SbvCLNmesoNgP
cap_config        Ebxykho1xsxio3eQr8v4yViukTUrrVAJxYesAGsoHjCz
vault_config      B4mUpjzUDY82TG4mNxECEWewmbkPcLpqgATnvP3ma2rk
```

The Listen payout rail has therefore **never been bootstrapped on mainnet**, and
the only key that can bootstrap it is `Config.admin`. This rotation is not just a
staking-admin handoff; it also transfers the right to stand up the entire payout
rail.

All three `init_payout_*` contexts declare `payer = admin`, so the admin account
itself funds the rent for the PDA it creates. A separate `FEE_PAYER` does not
help there - it covers the transaction fee, not an account allocation bound to
`admin` in the context. Whoever holds `Config.admin` has to be a funded key in
its own right, which is an argument **for** rotating to one rather than
re-funding a key that was stood down.

> Note for whoever reads `docs/playbooks/rails-canary-launch.md`: the
> `Config admin` row there is **current on-chain state, not stale copy**. Do not
> "correct" it to the new authority. It becomes stale only once this rotation
> lands, and then both should move together.

## The instruction

`set_admin` is **single step**. There is no `pending_admin` on `Config` and no
accept leg - and there is no accept leg anywhere else in the deployed binary
either. It contains no `accept_payout_admin` at all, and its `set_payout_admin`
(which rotates `PayoutAuthorityConfig` and fans the same key out to the cap and
vault configs in one call) is single step too. A two-step propose/accept pattern
exists only in undeployed source, so do not go into this expecting a
second-chance leg to exist on the live program.

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
its own role. (`scripts/set-admin.ts` now catches the zero balance before
simulating, so in practice you get the clearer message quoted under Procedure
rather than this raw `AccountNotFound`.)

Two ways through, both needing an operator decision:

1. **Fund `2pHj...`** - and not with a few thousand lamports. The account does
   not merely have a low balance, it **does not exist**: 0 lamports, no account
   on chain. It has to be created at the rent-exempt minimum for a zero-data
   system account, **~890,880 lamports (~0.00089 SOL)**, before it can pay a
   5,000 lamport fee at all. That is still small in absolute terms but roughly
   two orders of magnitude more than a fee-sized top-up, and it re-funds a key
   that was deliberately drained.
2. **Use a separate fee payer.** A Solana transaction's fee payer need not be
   the instruction's signer: `2pHj...` signs as admin while a funded key pays.
   This **shipped in `f82e22e`** and is in `scripts/set-admin.ts` today - a
   `FEE_PAYER` keypair sets `payerKey` and both keypairs are passed to
   `tx.sign`, defaulting to the admin when unset. What it still needs is a
   funded key the operator can sign with locally. `8di6...` has 0.178 SOL but
   its keypair is not on the ops box, so it cannot co-sign from there; see
   precondition 1, which is the same problem viewed from the other end.

Option 2 is the implemented path and never re-funds the retired key, so prefer
it. Option 1 is the fallback if no funded operator key can sign locally.

## Preconditions

1. **Settle custody of `NEW_ADMIN` before broadcasting. This is currently
   unresolved and it is the real blocker.** The rotation target is `8di6...`,
   and option 2 above records that `8di6...`'s keypair **is not on the ops
   box**. If that is still true at broadcast time, this rotation trades one
   unsignable admin for another: `2pHj...` loses the role and `8di6...` cannot
   sign, so every instruction in the nine-row table above - including
   bootstrapping the listen-payout rail, which has never been done - is blocked
   permanently, with no accept leg to back out through and no recovery short of
   a program upgrade.

   `8di6...` is active and funded (5 successful mainnet transactions on
   2026-08-24, 0.178 SOL at the time of writing), but "active" is not "you can
   sign with it right now". Resolve it one of two ways: locate the keypair, or
   pick a `NEW_ADMIN` you can sign with and that is funded enough to pay
   `init_payout_*` rent. `scripts/set-admin.ts` now demands proof of custody
   rather than trusting the operator's memory: with `NEW_ADMIN_KEYPAIR` set,
   `NEW_ADMIN` co-signs the promoting transaction as its fee payer, so the key
   provably exists and is controlled. Without it, broadcast is refused unless
   `I_ACCEPT_UNPROVEN_NEW_ADMIN=1` is set explicitly, and the script then only
   warns (on-curve status, whether the key has ever been seen on the cluster).
   Take the default. If you use the override, record why.
2. The signer must be the **current** admin. On the ops box the keypair for
   `2pHj...` is the default Solana CLI identity, so any tool that reads the
   default keypair path signs as the live config admin of a mainnet program.
   Treat that as a finding in its own right, separate from this rotation. (Host
   deliberately not named here — this is a public repo, and until the rotation
   lands that pairing is an attacker roadmap. See the gitignore policy in
   `CLAUDE.md`.)
3. Use the dedicated RPC, not a public endpoint.

## Procedure

`scripts/set-admin.ts` implements this, modeled on `scripts/set-reward-rate.ts`:
dry-run by default, derive accounts, verify the signer really is the on-chain
admin, sign, simulate, and only then broadcast behind `BROADCAST=1` plus either
`CONFIRM_BROADCAST=mainnet-beta:<NEW_ADMIN>` (sufficient on its own - it binds
the cluster and the exact target key) or the interactive typed phrase, which on
mainnet additionally needs `I_UNDERSTAND_MAINNET=1`. It refuses an all-zeros
`NEW_ADMIN` (mirroring the on-chain guard), a no-op where `NEW_ADMIN` already
equals the current admin, and any keypair that is not the current on-chain
admin. After a confirmed send it re-reads the Config account at `confirmed` -
the same commitment the send was confirmed at, with a short bounded poll - and
asserts the admin actually changed rather than trusting the signature.

Someone other than the drained admin has to pay the fee. Preferred: `NEW_ADMIN`
itself pays via `NEW_ADMIN_KEYPAIR`, which is also the proof of custody
(precondition 1); `FEE_PAYER` is not needed on that path and the script rejects
setting both. Fallback, only when custody is established some other way: a
separate `FEE_PAYER` plus the explicit `I_ACCEPT_UNPROVEN_NEW_ADMIN=1` override.
With neither set the fee payer defaults to the admin, which has no balance, and
the script stops before simulating with

```
Fee payer 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD has 0 lamports and cannot
pay. Set FEE_PAYER to a funded keypair, or fund this one.
```

```bash
# Preferred - NEW_ADMIN co-signs as fee payer, proving custody:
CLUSTER=mainnet-beta RPC_URL="<dedicated rpc>" \
  KEYPAIR=~/.config/solana/id.json \
  NEW_ADMIN=8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8 \
  NEW_ADMIN_KEYPAIR="<path to NEW_ADMIN's keypair - do not commit this path>" \
  npx tsx scripts/set-admin.ts                       # dry run, simulate only

# Fallback - custody settled another way; loud, and recorded why:
CLUSTER=mainnet-beta RPC_URL="<dedicated rpc>" \
  KEYPAIR=~/.config/solana/id.json \
  NEW_ADMIN=8di6hHF8GhEgeCVzmjDeKQYcR51SMLdzDPR5ESf55gC8 \
  FEE_PAYER="<path to a funded keypair - do not commit this path>" \
  I_ACCEPT_UNPROVEN_NEW_ADMIN=1 \
  npx tsx scripts/set-admin.ts                       # dry run, simulate only

# then, once NEW_ADMIN custody is settled (precondition 1), the same
# invocation with:
... BROADCAST=1 I_UNDERSTAND_MAINNET=1 npx tsx scripts/set-admin.ts
```

An early dry run on 2026-09-03, before `FEE_PAYER` existed, confirmed the
derivation and authority:

```
Config    : 7pwUU1hv3hCNNTAPmDyMRCeKoMPEz3k5cH1PTbWDNQR6
Signer    : 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
Current on-chain admin: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD
```

so the signer check passes, but simulation then died on the zero balance without
reaching the program. **That is no longer where this stands.** With a separate
fee payer, `f82e22e` recorded a clean mainnet simulation against the deployed
pre-H-01 binary:

```
Program log: Instruction: SetAdmin
Program BdSv824h... consumed 3805 of 200000 compute units
Program BdSv824h... success
```

That is positive proof the **deployed** binary accepts `set_admin` - observed
on chain, not inferred from current source, which is ahead of what is deployed.
The fee-payer blocker is solved and the open item is custody of `NEW_ADMIN`, not
tooling. Still re-run the simulation as the go/no-go immediately before
broadcasting: it is cheap and it re-reads live state. Do not skip it.

## After it lands

Four documents state the pre-rotation position and go stale the moment it lands.
Update all of them in the same pass:

- `docs/playbooks/rails-canary-launch.md`: the `Config admin` row, the note
  explaining why it differs from the upgrade authority, and the Launch Blockers
  entry that records Config admin and upgrade authority as different keys.
- `docs/trust-rail-aop-seam.md` §3.1: the bootstrap gate names `Config.admin` as
  the retired `2pHj...`; the listen-payout rail stays un-bootstrapped until the
  `init_payout_*` calls actually run, so update the key but **do not** clear the
  gate on the rotation alone.
- `docs/agent-trust-integration-checklist.md` §2 and §5: same two facts — the
  Rails `Config.admin` role row, and the listen-payout blocking precondition.
- `docs/wzrd-rails-mainnet-verification.md`: record the signature.

Then re-read `Config.admin` from chain and paste the observed value, rather than
asserting the rotation succeeded from the transaction signature alone.
