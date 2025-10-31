## Account Lifecycle & Rent Reclamation

State accounts created by this protocol (e.g., `EpochState`) are not intended to be permanent and can be closed to reclaim rent. The process is governed by the following strict, non‑emergency invariants:

### `close_epoch_state` / `close_epoch_state_open`

This is the sole mechanism for on‑chain state cleanup.

- Authorization: The instruction can only be successfully invoked by the `admin` key stored in the corresponding `ProtocolState` account.
- Target Validation: The instruction targets a specific `EpochState` PDA and will fail if the provided account does not match the seeds for the given `epoch` and `streamer_key` (and `mint` for the open variant).
- Time‑lock Invariant: A mandatory, non‑overridable time‑lock is enforced. An `EpochState` account can only be closed after a fixed grace period has elapsed since its creation (`timestamp`). This grace period is defined by `EPOCH_FORCE_CLOSE_GRACE_SECS` (currently 7 days), ensuring a fair and deterministic claim window for all users.

### Emergency Functions

Emergency or "force close" functions with hardcoded keys have been removed from the protocol. There are no backdoors or special administrative privileges that can bypass the time‑lock invariant.

