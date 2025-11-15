# V2 Migration Monitor - Live Status

**Last Updated**: 2025-11-06 00:50 UTC

---

## Deployment Summary

### V2 Program Deployment
- **Signature**: `2tNBGeEZEKmzREFMEAkKypew295B5ho3ko2fiV9YVzo2AokzwDYfFKvZLveXPraBWD5BmWHJZRUi7LP3WmtjTMYU`
- **Slot**: 378,187,962
- **Timestamp**: 2025-11-06 00:17:46 UTC
- **Deployed**: ~33 minutes ago

---

## Current Migration Status

### Account Distribution
| Account Type | Size (bytes) | Count | Status |
|--------------|-------------|-------|--------|
| **V1** | 1,782 | 401 | ✅ Active |
| **V2** | 10,742 | 0 | ⏳ Pending |

### Migration Progress
- **V1 → V2 Progress**: 401 → 0 (0%)
- **Ring Buffer Rotation**: 0/10 slots migrated
- **Estimated Full Migration**: ~10 hours from first v2 epoch

---

## Ghost Account Analysis

### Scan Results
- **Total ChannelState Accounts**: 401
- **Ghost Accounts Found**: 0 ✅
- **Conclusion**: All existing accounts are valid v1 accounts

**Why No Ghosts?**
- The "185 ghost accounts" mentioned in deployment docs don't exist on mainnet
- All 401 accounts have correct:
  - Discriminator: `[74, 132, 141, 196, 64, 52, 83, 136]`
  - Size: 1,782 bytes (valid v1 structure)
  - Mint: AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5 (MILO)
  - Valid streamer keys and epoch data

---

## Aggregator Status

### Configuration
- `CHANNEL_MAX_CLAIMS=8192` ✅ Applied
- `ENABLE_AUTO_PUBLISH=true` ✅
- `PUBLISH_ON_SEAL=true` ✅
- Services restarted with `--update-env` ✅

### Current Processing
- **Current Epoch**: 1762383600 (2025-11-05 23:00:00 UTC)
- **Tree Building**: Active (multiple channels finalizing trees)
- **Publishing**: Auto-publish enabled (50 epochs/minute)

---

## Next V2 Account Creation

### When Will It Happen?
V2 accounts will be created when:
1. Aggregator finishes current epoch processing
2. Publisher calls `set_merkle_root_ring` for new epoch
3. On-chain program allocates new ChannelState account with 10,742-byte size

### Expected Timeline
- **First v2 account**: Within next epoch boundary (1-60 minutes)
- **Account size**: 10,742 bytes (10 slots × 1,066 bytes/slot + 82-byte header)
- **Ring buffer migration**: Gradual over ~10 hours

---

## Account Size Verification

### V1 Structure (Current - 1,782 bytes)
```
Header:     82 bytes (disc + version + bump + mint + streamer + epoch)
Ring Slots: 10 × 170 bytes = 1,700 bytes
  Per Slot: 8 (epoch) + 32 (root) + 2 (count) + 128 (bitmap) = 170 bytes
  Bitmap:   128 bytes (CHANNEL_MAX_CLAIMS = 1,024)
Total:      1,782 bytes ✅
```

### V2 Structure (Expected - 10,742 bytes)
```
Header:     82 bytes (disc + version + bump + mint + streamer + epoch)
Ring Slots: 10 × 1,066 bytes = 10,660 bytes
  Per Slot: 8 (epoch) + 32 (root) + 2 (count) + 1,024 (bitmap) = 1,066 bytes
  Bitmap:   1,024 bytes (CHANNEL_MAX_CLAIMS = 8,192)
Total:      10,742 bytes
```

---

## Monitoring Commands

### Check for V2 Accounts
```bash
cd /home/twzrd/milo-token && node -e "
const { Connection, PublicKey } = require('@solana/web3.js');
const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const discriminator = [74, 132, 141, 196, 64, 52, 83, 136];

(async () => {
  const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
    filters: [{
      memcmp: {
        offset: 0,
        bytes: require('bs58').encode(Buffer.from(discriminator))
      }
    }]
  });

  const v1 = accounts.filter(a => a.account.data.length === 1782).length;
  const v2 = accounts.filter(a => a.account.data.length === 10742).length;

  console.log(\`V1: \${v1}, V2: \${v2}, Total: \${accounts.length}\`);
})();
"
```

### Check Aggregator Logs
```bash
pm2 logs milo-aggregator --lines 20 --nostream | grep -E "(epoch|publish|tree_finalized)"
```

### Check Recent Transactions
```bash
solana transaction-history GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --limit 5 --url mainnet-beta
```

---

## Post-Migration Tasks

### ✅ Completed
1. **Update aggregator config** - `CHANNEL_MAX_CLAIMS=8192` applied to `.env`
2. **Ghost account scan** - 0 ghosts found, all accounts valid
3. **Service restart** - Aggregator and tree-builder restarted with new config

### 🔄 In Progress
1. **Monitor first v2 epoch creation** - Waiting for epoch boundary
2. **Verify account size** - Will confirm 10,742 bytes when created

### ⏳ Pending
1. **Full ring buffer migration** - Monitor over ~10 hours
2. **High-traffic channel verification** - Confirm >1,024 claims accepted
3. **Update deployment docs** - Correct ghost account count (0, not 185)

---

## Success Criteria

### Immediate (Within 1 Hour)
- [ ] First v2 ChannelState account created (10,742 bytes)
- [ ] Account contains valid epoch and merkle root
- [ ] No program errors during initialization

### Short-Term (24 Hours)
- [ ] 10+ epochs rotated (full ring buffer migration)
- [ ] All active channels using v2 account structure
- [ ] Zero `InvalidIndex` errors for valid indices (0-8191)

### Long-Term (7 Days)
- [ ] High-traffic channels accepting >1,024 claims per epoch
- [ ] 99%+ claim success rate maintained
- [ ] No rollback or emergency pauses required

---

**Status**: ⏳ **Migration pending - Monitoring active**
**Next Check**: 2025-11-06 01:00 UTC (10 minutes)
**Auto-generated by**: Claude Code v4.5
