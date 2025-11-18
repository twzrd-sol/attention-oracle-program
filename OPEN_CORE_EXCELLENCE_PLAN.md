# Open‑Core Excellence Plan (Trimmed)

Goals
- High‑quality, well‑documented on‑chain code
- Deterministic builds and verifiable deployments
- Minimal public surface area; clear interfaces

Engineering Standards
- Rust + Anchor 0.30+, Solana 1.18+
- `anchor build` and `anchor test` must pass in CI
- No secrets in repo; enforce with pre‑commit and CI checks

Release Discipline
- Semantic versioning for the on‑chain program
- Changelogs limited to technical changes (no hype language)

Security
- Coordinated disclosure via SECURITY.md
- Reproducible builds preferred for verification
