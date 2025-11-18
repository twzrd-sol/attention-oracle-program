# Changelog — Attention Oracle (Token-2022)

All notable changes to this project will be documented in this file. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased]

### Phase 1: Cost Optimization (WIP)

#### Changed
- **Cargo.toml**: Removed unused dependencies (`mpl-bubblegum`, `spl-noop`)
- **Default features**: Enabled `no-idl` by default (IDL not needed post-mainnet deployment; use Solscan for schema)
- **Release profile**: Added inline documentation explaining each optimization flag
- **Feature flags**: Commented out `passport` and `points` features (future use, currently unused)

#### Optimized
- **Binary size**: 571 KB → 521 KB (-50 KB, -8.8%)
  - Removed unnecessary transitive dependencies
  - IDL serialization overhead eliminated
  - Link-time optimization (LTO) coverage improved

#### Notes
- ✅ Build succeeds with no new errors (pre-existing stack offset warning in `ClaimOpen::try_accounts` unrelated to these changes)
- ✅ All existing instructions remain functional and backward-compatible
- ⏳ Next: Phase 1.2 devnet deployment + Phase 2 gateway optimization

---

## [v0.1.0] — 2025-11-07

### Added
- **Mainnet deployment**: Program ID `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Core instructions**:
  - `initialize_mint` + `initialize_mint_open` (protocol setup)
  - `claim_with_ring` + `set_merkle_root_ring` (ring buffer path, active)
  - `claim` + `set_merkle_root` (legacy EpochState path, deprecated)
  - Admin: `update_publisher`, `set_paused`, `set_policy`, `update_admin`, `close_channel_state`
  - Governance: `update_fee_config`

- **Ring buffer storage** (ChannelState):
  - 9 slots × 1.1 KB each = 9.5 KB total per channel
  - Supports 8,192 claims per epoch
  - ~0.07 SOL rent per channel (2-year cost)

- **Merkle claim verification**:
  - keccak256-based proof validation (O(log n))
  - Bitmap claim tracking (O(1) per-claim check)
  - Dynamic fee multipliers via off-chain tier system

- **Security**:
  - `security.txt` embedded in binary
  - Signer validation via Anchor macros
  - Protocol pause flag (circuit breaker)

### Technical Details
- **Binary size**: 571 KB (optimized SBF)
- **Dependencies**: Anchor 0.30.1, Solana 1.18
- **Solana compliance**: Token-2022 compatible, CPI-safe
- **Test coverage**: Full ring buffer path tested (ProgramTest)

---

## Project Milestones

| Date | Event | Status |
|------|-------|--------|
| Nov 7, 2025 | Mainnet deployment | ✅ Live |
| Nov 13, 2025 | Solana Foundation grant application ready | ⏳ Submitted |
| Nov 18, 2025 | Phase 1: Code optimization | 🔄 In Progress |
| TBD | Phase 2: Gateway optimization | ⏳ Planned |
| TBD | Phase 3: On-chain optimizations | ⏳ Planned |

---

## Future Work

### Phase 2: Gateway Optimization
- [ ] Batch claim requests (POST /api/batch-claims)
- [ ] Merkle tree caching (Redis TTL)
- [ ] Parallel proof verification (Node.js worker threads)
- [ ] Transaction building optimization (blockhash refresh strategy)

### Phase 3: On-chain Enhancements
- [ ] Tier-based claim multipliers (PassportRegistry integration, optional)
- [ ] Transfer hook harvest automation (periodic keeper job)
- [ ] Compressed proof storage (optional, for large Merkle trees)

---

## Breaking Changes

**None in v0.1.0+**. Legacy `claim` + `set_merkle_root` paths remain supported for backward compatibility. New deployments should use ring buffer path (`claim_with_ring` + `set_merkle_root_ring`).

---

## How to Deploy

### Mainnet
```bash
# Build
cargo build-sbf

# Deploy (authority required)
solana program deploy target/deploy/token_2022.so \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

### Devnet (Testing)
```bash
# Deploy to devnet with new program ID
solana program deploy target/deploy/token_2022.so \
  --url devnet
```

---

## Related Documents

- **README.md**: Architecture overview, deployment info
- **SECURITY.md**: Security policy and vulnerability disclosure (GitHub)
- **Cargo.toml**: Dependencies, features, release profile

---

**Last Updated**: November 18, 2025
**Maintainer**: Attention Oracle Dev Team
