# TWZRD Publisher Deployment Guide

## 🎯 Mission
Get merkle roots from PostgreSQL → Solana mainnet for 114 unpublished sealed epochs.

---

## ✅ Pre-Flight Checklist (COMPLETED)

### Infrastructure
- ✅ **Redis 7.4.1** upgraded (required: 6.2+)
- ✅ **PostgreSQL** accessible and healthy
- ✅ **Database refactor** complete (all endpoints using factory pattern)
- ✅ **114 sealed epochs** ready to publish

### Endpoints Verified
- ✅ `/stats` - Returns correct sealed epoch data
- ✅ `/metrics` - Returns backlog count (114)
- ✅ `/claim-root` - **CRITICAL** - Now returning valid 32-byte hex roots
- ✅ `/proof` - Ready for user claims

### Example Working Response
```bash
$ curl -s "http://localhost:8080/claim-root?channel=adapt&epoch=1761825600" | jq .
{
  "root": "0xbd98574f9062c472e51511373669312f29040beb3bc18afb02004ca358f772c0",
  "participantCount": 45,
  "builtAt": 1761830774,
  "cached": true
}
```

---

## 🚀 Deployment Steps

### 1. Verify Publisher Script Exists
```bash
ls -lh scripts/publisher/publish-cls-category.ts
```

Expected: File exists (TypeScript publisher script)

### 2. Check Environment Variables
```bash
# Required environment variables for publisher:
# - DATABASE_URL (PostgreSQL connection)
# - SOLANA_RPC_URL (mainnet endpoint)
# - PUBLISHER_KEYPAIR_PATH (wallet with SOL for tx fees)

# Verify they're set:
printenv | grep -E "DATABASE_URL|SOLANA_RPC_URL|PUBLISHER_KEYPAIR"
```

### 3. Test Publisher (Dry Run)
Before deploying, test the publisher script manually:
```bash
cd /home/twzrd/milo-token
npx tsx scripts/publisher/publish-cls-category.ts
```

Expected output:
- Connects to PostgreSQL ✓
- Queries unpublished roots ✓
- Connects to Solana RPC ✓
- Publishes first root to mainnet ✓
- Marks root as published in DB ✓

### 4. Deploy Publisher with PM2
```bash
pm2 start scripts/publisher/publish-cls-category.ts \
  --name publisher \
  --interpreter npx \
  --interpreter-args "tsx" \
  --cron-restart="0 */4 * * *" \
  --no-autorestart

pm2 save
```

**Why `--no-autorestart`?** Publisher runs once per invocation (batch job), not a long-running daemon.

**Cron schedule:** Runs every 4 hours to catch up on backlog.

### 5. Monitor First Publish
```bash
pm2 logs publisher --lines 50
```

Watch for:
- `✓ Connected to PostgreSQL`
- `✓ Found X unpublished roots`
- `✓ Publishing root for epoch XXXXXXX, channel: adapt`
- `✓ Transaction confirmed: [signature]`
- `✓ Marked as published in database`

### 6. Verify On-Chain
Check that the root was published to Solana:
```bash
# Get transaction signature from logs, then:
solana confirm [SIGNATURE] --url mainnet-beta

# Or check program state directly (if exposed via PDA query)
```

### 7. Verify Database Updated
```bash
psql postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd \
  -c "SELECT COUNT(*) FROM sealed_epochs WHERE published = 1;"
```

Expected: Count should increment after each publish.

---

## 📊 Monitoring

### Check Backlog
```bash
curl -s http://localhost:8080/metrics | jq '.backlog_count'
```

Should decrease from 114 → 113 → 112... as publisher runs.

### Check Last Sealed Epoch
```bash
curl -s http://localhost:8080/metrics | jq '.last_sealed_epoch'
```

### PM2 Status
```bash
pm2 status publisher
```

Expected:
- **Status:** stopped (if running as cron job)
- **Restarts:** 0 (publisher shouldn't crash)
- **Uptime:** Recent if just ran

### View Recent Publishes
```bash
psql postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd \
  -c "SELECT epoch, channel, LEFT(root, 16) || '...' as root, published
      FROM sealed_epochs
      WHERE published = 1
      ORDER BY epoch DESC
      LIMIT 10;"
```

---

## 🔧 Troubleshooting

### Error: "invalid root length: 0"
**Cause:** `/claim-root` endpoint returning `0xundefined`
**Status:** ✅ FIXED (refactor complete)
**Verify:** `curl http://localhost:8080/claim-root?channel=adapt&epoch=1761825600`

### Error: "Connection refused (PostgreSQL)"
**Cause:** DATABASE_URL not set or PostgreSQL down
**Fix:**
```bash
export DATABASE_URL="postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd"
sudo systemctl status postgresql
```

### Error: "Insufficient SOL for transaction"
**Cause:** Publisher wallet empty
**Fix:**
```bash
solana balance --url mainnet-beta --keypair ~/.config/solana/publisher.json
# Top up wallet with SOL
```

### Error: "No unpublished roots found"
**Cause:** All epochs already published (or none sealed)
**Status:** This is success state! Check with:
```bash
psql $DATABASE_URL -c "SELECT COUNT(*) FROM sealed_epochs WHERE published = 0 OR published IS NULL;"
```

---

## 🎯 Success Metrics

### Short-Term (Next 24 Hours)
- ✅ Publisher successfully publishes 1-10 roots to mainnet
- ✅ Backlog count decreases from 114 → <100
- ✅ No publisher crashes or errors in PM2 logs

### Medium-Term (Next Week)
- ✅ Backlog fully cleared (0 unpublished roots)
- ✅ Tree-builder + publisher running in sync (new epochs published within 1 hour of sealing)
- ✅ Users can successfully claim tokens via `/proof` endpoint

### Long-Term (Production)
- ✅ 100% of sealed epochs published to mainnet
- ✅ Average publish latency: <5 minutes after epoch sealed
- ✅ Zero manual intervention required

---

## 📝 Next Steps After Publisher Deploys

1. **Test User Claims Flow**
   - Generate proof for test wallet: `curl /proof?wallet=...&epoch=...`
   - Submit claim transaction to Solana
   - Verify tokens transferred to user wallet

2. **Deploy Frontend Claim UI**
   - Privy-based Twitch auth
   - Gasless transactions via relayer
   - Show claimable epochs per user

3. **Hackathon Demo Recording**
   - Show real-time data: 2.7M+ events processed
   - Demonstrate claim flow end-to-end
   - Highlight 99% cost reduction vs NFTs

---

## 🔐 Security Considerations

### Publisher Wallet Management
- **DO NOT** commit publisher keypair to git
- Store in secure location: `~/.config/solana/publisher.json`
- Fund with minimum SOL needed (0.1-0.5 SOL)
- Monitor balance via `/metrics` endpoint

### Rate Limiting
Publisher should respect:
- Solana RPC rate limits (use paid tier if needed)
- PostgreSQL connection pool limits (max 20 concurrent)
- Self-throttle: 1 publish per 10 seconds to avoid spam

### Failure Handling
- **Partial publish failure:** Don't mark as published in DB if Solana tx fails
- **RPC outage:** Retry with exponential backoff (max 3 retries)
- **Database disconnect:** Publisher should crash gracefully (PM2 will restart on next cron)

---

## 📊 Current State Summary

| Metric | Value | Status |
|--------|-------|--------|
| Redis Version | 7.4.1 | ✅ Ready |
| PostgreSQL | Accessible | ✅ Ready |
| Unpublished Epochs | 114 | ✅ Ready |
| `/claim-root` Endpoint | Valid roots | ✅ Fixed |
| Tree-Builder | Running (PM2 ID: 7) | ✅ Online |
| Publisher | Stopped (awaiting deploy) | 🟡 Deploy Now |

---

**Generated:** 2025-10-30 13:30 UTC
**Status:** ✅ ALL SYSTEMS GO - READY FOR PUBLISHER DEPLOYMENT
