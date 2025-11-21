# v1.1.0 Deployment Checklist

## Pre-Deployment (Complete ✓)
- [x] Code sanitized (subject terminology applied end-to-end)
- [x] Build verified (cargo build-sbf passes)
- [x] Binary ready: target/deploy/token_2022.so
- [x] Hash verified: 97f9880ddf21ba9d1b50c45ed7717e7bf646f23a203bf10392329ca8e416f1cf
- [x] VERIFY.md updated with v1.1.0 hash & instructions
- [x] Creator Bonds isolated in private repo
- [x] Deployment docs written
- [x] Commit tagged as v1.1.0

## GitHub Release Management
**ACTION NEEDED:** Clean up old releases before pushing v1.1.0 tag
- Review: https://github.com/twzrd-sol/attention-oracle-program/releases
- Remove any releases with exposed source code links
- Rationale: We want v1.1.0 to be the first "clean" generic version with transparent verification

**Commands to execute:**
```bash
# From /home/twzrd/milo-token/attention-oracle-program
git checkout main
git pull origin main

# Push the v1.1.0 branch and tag
git push origin v1.1-entity-refactor
git push origin v1.1.0

# Draft new release on GitHub with:
# - Title: "v1.1.0 - Entity/Signal Infrastructure Release"
# - Description: See DEPLOYMENT_v1.1.0.md & STATUS_v1.1.0.md
# - Hash: 97f9880ddf21ba9d1b50c45ed7717e7bf646f23a203bf10392329ca8e416f1cf
# - Attach binary: target/deploy/token_2022.so
```

## Mainnet Deployment
**ACTION REQUIRED: User confirmation before executing**

```bash
# STEP 1: Verify wallet is set up
solana config get
# Should show: 
#   - RPC URL: https://api.mainnet-beta.solana.com
#   - Keypair Path: /path/to/upgrade-authority.json

# STEP 2: Check balance (need ~10 SOL for deployment)
solana balance

# STEP 3: Deploy upgrade to existing Program ID
solana program deploy \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --upgrade-authority /path/to/upgrade-authority.json \
  target/deploy/token_2022.so

# STEP 4: Wait ~30 seconds for finalization
# STEP 5: Verify on Solscan
#   https://solscan.io/account/GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop

# STEP 6: Run verification
solana-verify verify -u m \
  --program-id GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop \
  --commit v1.1.0 \
  twzrd-sol/attention-oracle-program
```

## Post-Deployment (Creator Bonds Integration)
- [ ] Deploy Creator Bonds to devnet (separate program ID)
- [ ] Test CPI integration with v1.1.0
- [ ] Verify NodeScore updates propagate correctly
- [ ] Deploy Creator Bonds to mainnet when devnet testing passes
- [ ] Public announcement with transparent explanation

## Rollback Plan (If Issues Arise)
If mainnet deployment causes issues:
1. Pause protocol (if admin permits)
2. Build new version with fixes
3. Deploy upgrade
4. Update VERIFY.md with new hash
5. Announce fix with commit reference

---

**Status**: Ready for GitHub release + mainnet deployment
**Next Step**: User confirmation on mainnet deployment (requires real SOL)
