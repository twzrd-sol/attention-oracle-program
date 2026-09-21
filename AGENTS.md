# AGENTS.md

This repo is used by Codex and other coding agents.

For **new agent / evidence work**, follow `LEDGER.md`. Program source is
`programs/evidence-ledger` (excluded from the workspace). Do not edit the
wzrd-final copy of that program in parallel. Do not route new work into
`programs/wzrd-rails/` or Listen settlement.

`CLAUDE.md` remains the reference for the immutable Attention Oracle binary,
historical AO accounts, and build/safety constraints around those programs. It
is not the routing brief for new ledger work.

If multiple agents are active, use a local `AGENT_COORDINATION.md` scratchpad
when present. Treat that file as coordination state, not as the source of truth
for shipped code.

Key rules:

- Do not revert or overwrite another agent/user edit.
- Keep changes surgical and verify with the narrowest relevant build/test command.
- Treat the deployed AO v2/token_2022 program as immutable.
- No force push, hard reset, broad delete, production deploy, Doppler mutation,
  or on-chain transaction without explicit user approval at action time.
