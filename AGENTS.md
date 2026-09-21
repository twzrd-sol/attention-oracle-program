# AGENTS.md

This repo is used by Codex and other coding agents. Follow `CLAUDE.md` for
project truth, immutable-program warnings, build/test commands, and safety
constraints.

If multiple agents are active, use a local `AGENT_COORDINATION.md` scratchpad
when present. Treat that file as coordination state, not as the source of truth
for shipped code.

Key rules:

- Do not revert or overwrite another agent/user edit.
- Keep changes surgical and verify with the narrowest relevant build/test command.
- Treat the deployed AO v2/token_2022 program as immutable. Do not route new
  work into `programs/wzrd-rails/` or Listen settlement. The continuation is
  the agent ledger in `LEDGER.md`: evidence roots over signed leaves.
  Program source is `programs/evidence-ledger` (excluded from the workspace).
  Do not edit the wzrd-final copy in parallel.
- No force push, hard reset, broad delete, production deploy, Doppler mutation,
  or on-chain transaction without explicit user approval at action time.
