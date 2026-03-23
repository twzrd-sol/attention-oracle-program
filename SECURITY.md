# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in the Liquid Attention Protocol (on-chain program, server, SDK, or agent infrastructure), please report it responsibly.

**Email**: security@twzrd.xyz

**What to include**:
- Description of the vulnerability
- Steps to reproduce
- Affected component (on-chain program, server API, SDK, agent runner)
- Impact assessment (fund loss, data exposure, denial of service, etc.)

**Response timeline**:
- Acknowledgment within 48 hours
- Initial assessment within 7 days
- Fix timeline communicated within 14 days

## Scope

| Component | Address / URL | In Scope |
|-----------|--------------|----------|
| AO Program (mainnet) | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` | Yes |
| Server API | `api.twzrd.xyz` | Yes |
| Frontend | `twzrd.xyz` | Yes |
| SDK | `@wzrd_sol/sdk`, `wzrd-client` (PyPI) | Yes |
| Agent runner | `agents/swarm-runner/` | Yes |
| Relay | `/v1/relay/*` endpoints | Yes |

## Program Verification

The on-chain program is built with Pinocchio (raw Solana BPF) with deterministic build settings:

```toml
[profile.release]
overflow-checks = true
lto = "fat"
codegen-units = 1
opt-level = 3
strip = true
panic = "abort"
```

Program upgrades are governed by a Squads V4 multisig (3-of-5): `BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ`.

To verify the deployed binary matches this source:
```bash
solana-verify verify-from-repo \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --url https://github.com/twzrd-sol/wzrd-final \
  --library-name ao_v2 \
  --bpf-flag channel_staking
```

## Bug Bounty

No formal bug bounty program at this time. Significant findings will be acknowledged and credited.
