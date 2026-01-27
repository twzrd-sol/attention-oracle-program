# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in this project, please report it privately.

**Email:** security@twzrd.xyz

Please do not open public issues for security vulnerabilities.

## Scope

- On-chain program logic
- Merkle proof verification
- Access control and PDA derivation
- Token handling

## Out of scope

- Frontend/UI issues
- Third-party dependencies (report to upstream)
- Issues requiring social engineering

## Program Verification

The deployed program can be verified against this source code:

```bash
# Install solana-verify
cargo install solana-verify

# Get on-chain hash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com <PROGRAM_ID>

# Build and compare
anchor build --verifiable
solana-verify get-executable-hash target/verifiable/token_2022.so
```

## Treasury Controls

Treasury outflows are limited to cumulative claims:

- `claim_cumulative`
- `claim_cumulative_sponsored`

There is **no `admin_withdraw` instruction** in the current program interface.

The program is upgradeable (see `DEPLOYMENTS.md`).
