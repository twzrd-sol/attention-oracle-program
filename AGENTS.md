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
- Treat the deployed AO v2/token_2022 program as immutable; new economic work
  belongs in `programs/wzrd-rails/` unless the user says otherwise.
- No force push, hard reset, broad delete, production deploy, Doppler mutation,
  or on-chain transaction without explicit user approval at action time.
