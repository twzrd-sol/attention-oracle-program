# Upgrade Authority Governance

Date: 2026-08-01

This document records the upgrade-authority posture of the deployed programs and
the runbook for migrating `wzrd-markets` and `wzrd-rails` off a single keypair
onto a Squads multisig. It is a decision record and a procedure — it is not
approval to execute the migration. The transfer step is irreversible and is
performed by a human holding the current authority key.

## Current State

Verified by direct RPC read on 2026-08-01 (`solana program show`, mainnet):

| Program | Address | Upgrade authority | Status |
|---------|---------|-------------------|--------|
| `attention-oracle` | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | `none` | Immutable — authority revoked 2026-04-05 |
| `wzrd-markets` | `7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy` | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` | Upgradeable |
| `wzrd-rails` | `BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9` | `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` | Upgradeable |

`2pHjZLqs…ZZaD` is a plain keypair, not a multisig. Its account is owned by the
System Program, is non-executable, carries zero data, and holds 0.000995 SOL. A
Squads vault would instead be owned by the Squads program and carry account
data. The earlier Squads arrangement is retired; one key currently controls both
upgradeable programs.

## Why This Matters

Upgrade authority on an upgradeable BPF program permits replacing the executable
wholesale. For `wzrd-markets`, which holds collateral, that means an attacker
holding the key can deploy a build whose withdraw path pays them, with no
timelock and a single signature. `wzrd-rails` is idle today but is an escrow
vehicle, so the same exposure applies the moment it carries value.

The low SOL balance is **not** a control. Funding the account costs roughly
0.002 SOL from any source and takes one transaction. It equally does not impede
legitimate use: in `solana program deploy` the upgrade authority only signs, and
the fee payer may be a separate keypair. Treat the balance as a footnote, not a
mitigation.

The unresolved variable is **custody** — where the private key physically lives,
how many copies exist, and whether it has ever been present on an
internet-connected machine. That determines whether this is a live exposure or a
dormant one, and it should be answered before scheduling the migration.

## Options Considered

| Option | Effect | Trade-off |
|--------|--------|-----------|
| **A. Squads multisig** | Upgrades require M-of-N approval | Preserves the ability to fix bugs; adds an approval step per deploy |
| **B. Burn authority (`none`)** | Nobody can ever upgrade | Maximum assurance against key compromise; makes every latent bug permanent |
| **C. Cold-key hardening** | Single key, moved to hardware | Cheapest; blast radius unchanged, one lost device from irreversible |

### Decision: A now, B later

Option B is the correct *end* state but is premature here, and this repository
contains the evidence for why.

`attention-oracle` took the burn route on 2026-04-05. The `compound` instruction
in `ao-v2` (source in the `wzrd-final` repository,
`programs/ao-v2/src/instructions/compound.rs`) is the only code that can
`invoke_signed` for the CCM buffer authority PDA, and the channel staking
instruction family returns `Custom(101)`
on the deployed program (see `docs/playbooks/rails-canary-launch.md`). Because
the program is immutable, that path can never be repaired, and 7,661,500 CCM is
permanently unreachable as a direct result. See
[`STRANDED_CCM_ANALYSIS.md`](./STRANDED_CCM_ANALYSIS.md).

Burning authority converts every unexercised code path into a permanent one. It
is appropriate only once the paths that matter have real production time.
`wzrd-markets` holds collateral and `wzrd-rails` has not yet carried meaningful
escrow volume, so neither is at that bar today.

Choose C only if Squads setup is blocked and same-day risk reduction is needed.

## Migration Runbook

Perform per program, signed by the current authority.

### Step 1 — Create and verify the destination

Create the Squads multisig and note the **vault** PDA. The vault PDA and the
multisig account are different addresses; transferring to the wrong one is not
recoverable. Confirm the destination exists on-chain before going further:

```bash
solana account <SQUADS_VAULT_PDA>
```

### Step 2 — Transfer, idle program first

```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --upgrade-authority <path/to/current-authority-keypair.json> \
  --new-upgrade-authority <SQUADS_VAULT_PDA> \
  --skip-new-upgrade-authority-signer-check
```

`--skip-new-upgrade-authority-signer-check` is **required** when the new
authority is a PDA, because a PDA cannot sign. That same flag disables the check
that would otherwise catch a mistyped destination. A typo here is
indistinguishable from handing the program to a stranger, and cannot be undone.

Run `wzrd-rails` (`BdSv824h…`) **first**. It is idle, so an error there is
survivable. Confirm before continuing:

```bash
solana program show BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9
```

The `Authority` line must read the Squads vault PDA.

### Step 3 — Prove the path works

Execute one trivial upgrade through Squads before trusting the arrangement. An
untested governance path is not a governance path, and the moment you need it
will not be the moment to discover a misconfigured threshold or a missing
signer.

### Step 4 — Migrate `wzrd-markets`

Only after steps 2 and 3 succeed, repeat for
`7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy` and verify with
`solana program show`.

## Verification

To re-check posture at any time:

```bash
for P in GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
         7YZZxQC9JrWwoo1W1fYgxZs6rnbW17iF72mi65vU93sy \
         BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9; do
  echo "== $P"
  solana program show "$P" | grep -E 'Program Id|Authority|Last Deployed'
done
```

`attention-oracle` must report `Authority: none`. Any other value means the
immutability assumption recorded here no longer holds and this document is
stale.
