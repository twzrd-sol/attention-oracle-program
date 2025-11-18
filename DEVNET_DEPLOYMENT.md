# Devnet Deployment Record

This document tracks the devnet deployment of the Attention Oracle program for testing and verification purposes.

## Current Deployment (v0.2.0 - Anchor 0.32.1)

### Program Details
- **Program ID**: `J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn`
- **Network**: Devnet
- **Deploy Date**: 2025-11-18 10:25:28 UTC
- **Deploy Slot**: 422375724
- **Authority**: `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`

### Build Information
- **Commit**: `240a008`
- **Branch**: `chore/anchor-0.32-upgrade`
- **Binary Size**: 732,608 bytes (715.4 KB)
- **SHA256**: `6dedc0ab78f3e3b8ea5533500b83c006a9542d893fb5547a0899bcbc4982593f`

### Toolchain
- **Solana**: 2.3.0 (Agave)
- **Anchor**: 0.32.1
- **Rust**: 1.89.0 (stable)

### Verification

The devnet binary is **verified** to match the local build:

```bash
# Local build
sha256sum target/deploy/token_2022.so
6dedc0ab78f3e3b8ea5533500b83c006a9542d893fb5547a0899bcbc4982593f

# On-chain dump
solana program dump J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn devnet.so --url devnet
sha256sum devnet.so
6dedc0ab78f3e3b8ea5533500b83c006a9542d893fb5547a0899bcbc4982593f
```

✅ **Hashes match perfectly** - Devnet deployment is verified reproducible.

### Deployment Transaction
- **Signature**: `5BpQEVZAbPKq424ZkXBQH1goY2cZxiGMfb1ry3wmCr7pqTLbK1qeekNYRx3werYRsMn6ZXMkJ14NFMi17MgkRk6B`
- **Explorer**: https://explorer.solana.com/tx/5BpQEVZAbPKq424ZkXBQH1goY2cZxiGMfb1ry3wmCr7pqTLbK1qeekNYRx3werYRsMn6ZXMkJ14NFMi17MgkRk6B?cluster=devnet

### Testing Checklist

Before mainnet upgrade, verify on devnet:

- [ ] Program deploys successfully
- [ ] Binary hash matches local build
- [ ] IDL can be extracted
- [ ] Program authority is correct
- [ ] Initialize instruction works
- [ ] Claim instruction works
- [ ] Transfer hook executes
- [ ] No runtime errors in logs

### Differences from Mainnet

| Property | Devnet | Mainnet |
|----------|---------|---------|
| Program ID | `J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn` | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| Binary Hash | `6dedc0ab...593f` | `[to be updated after upgrade]` |
| Toolchain | Anchor 0.32.1, Solana 2.3.0 | Anchor 0.30.1, Solana 1.18 *(old)* |
| Deploy Date | 2025-11-18 | 2025-11-13 |

### Next Steps

1. **Test on Devnet**: Run integration tests against `J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn`
2. **Verify Reproducibility**: Ensure CI builds match this hash
3. **Mainnet Upgrade**: Use `scripts/upgrade-mainnet.sh` to deploy same binary
4. **Update Verification**: Submit commit `240a008` to OtterSec/Ellipsis Labs

### Commands Used

```bash
# Switch to devnet
solana config set --url devnet

# Deploy
solana program deploy target/deploy/token_2022.so

# Verify
solana program show J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn
solana program dump J42avxcb6MFavCA5Snaw4u24QLznBdbLvuowxPYNdeAn devnet.so
sha256sum devnet.so
```

---

**Status**: ✅ Devnet deployment verified and ready for testing
**Next**: Run integration tests, then proceed to mainnet upgrade
