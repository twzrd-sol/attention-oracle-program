# Expansion Strategy: 9.3K → 100K+ Eligible Users

## Current Baseline (2025-11-12)

| Metric | Count | Notes |
|--------|-------|-------|
| **Eligible Users** | 9,318 | Unique users in claimable_allocations |
| **Total Allocations** | 22,447 | All user-channel combinations |
| **CHAT Staged** | 2,246,910 | At launch |
| **Mappable Users** | 174,805 | Rows in sealed_participants with username |
| **Total Participants (Raw)** | 961,564 | All sealed_participants rows |
| **Unmapped Hashes** | 956,035 | Hashes without username |

## Data Sources (Unlocked)

### Source 1: Sealed Participants (Mapped)
- **174,805 mapped usernames** available in sealed_participants
- Distribution: multi-epoch (179 epochs total), some users in multiple epochs
- **Unique users**: ~20K-50K (estimate, post-dedup)
- **Signal proxy**: Participation count (occurrences in table)

### Source 2: User Mapping
- **9,618 hash → username mappings**
- Direct supply of verifiable identities

### Source 3: Live IRC Collection
- **290 events/15sec** (~19 events/sec) on current 4 channels
- Scales to ~1.6K events/min
- **Expected daily**: ~150K new events (~10-20K new usernames/day with dedup)

### Source 4: Historical Data (DigitalOcean)
- **2.7M channel_participation rows** (Oct 14-30)
- All hashes (no usernames)
- Unmappable unless we bulk-fetch from Twitch API

## Expansion Target: 100K Users

### Option A: Conservative (150K from Sealed + Live)
**Approach**: Dedupe all mapped users from sealed_participants, allocate base 100 CHAT + signal-weighted bonus

**Formula**:
```
allocation = MIN(100 + (participation_count * 10), 1000)
```

**Expected Result**:
- ~150K unique users eligible
- ~17M additional CHAT tokens
- Safe supply impact (CHAT mint uncapped)

**Timeline**: 2 minutes (single SQL batch insert)

### Option B: Aggressive (300K+ from Twitch API Bulk)
**Approach**: Fetch usernames for unmapped hashes via Twitch API bulk endpoint

**Requirements**:
- Twitch Client ID + OAuth (have it?)
- User IDs (need to extract from hashes—impossible, or store alongside)
- Rate limits (~100 users/min, so 1M users = 10K minutes = 7 days)

**Expected Result**:
- ~300K+ users (if we can resolve hashes)
- Supply impact: ~30M CHAT

**Timeline**: Days (rate-limited by Twitch)
**Feasibility**: LOW (unless we have stored user IDs)

### Option C: Hybrid (100K from Sealed + Live, Ramp to 300K)
**Phase 1** (Today): Run Option A → 150K users
**Phase 2** (Week 1): Scale IRC to 20 channels → 2K new daily users
**Phase 3** (Month 1): Add manual whitelist / community imports → 300K total

**Timeline**: 1 day + ongoing
**Feasibility**: HIGH

---

## Recommended: Option A → Option C

### Step 1: Immediate Backfill (Today)
**Goal**: 150K users from sealed_participants

```sql
-- Dedupe & allocate all mapped users
INSERT INTO claimable_allocations (epoch, channel, username, amount)
SELECT
  MAX(sp.epoch) as epoch,
  'base_allocation' as channel,
  sp.username,
  LEAST(
    100 + (COUNT(*) * 10),  -- Base 100 + signal weight
    1000  -- Cap
  ) as amount
FROM sealed_participants sp
WHERE sp.username IS NOT NULL
  -- Filter suppressed (if table exists)
  AND NOT EXISTS (
    SELECT 1 FROM (VALUES
      -- Placeholder: Add blocked usernames here
      ('spam_account'), ('banned_user')
    ) AS blocked(name) WHERE blocked.name = sp.username
  )
  -- Avoid duplicates
  AND NOT EXISTS (
    SELECT 1 FROM claimable_allocations ca
    WHERE ca.username = sp.username
  )
GROUP BY sp.username
ON CONFLICT (epoch, channel, username) DO NOTHING;
```

**Expected Impact**:
- +~140K rows in claimable_allocations
- +~17M CHAT tokens
- New total: 150K+ unique eligible users

**Performance**: ~30 seconds (batch insert on 174K rows)
**Risk**: LOW (ON CONFLICT, dedup via NOT EXISTS)

---

### Step 2: Scale IRC (Week 1)
**Goal**: 2K new users/day from live chat

**Action**:
```bash
# Expand IRC collector to 20 channels (script tweak)
CHANNELS=xqc,n3on,yourragegaming,jasontheween,hasan_piker,pokimane,\
sykkuno,valkyrae,summit1g,ludwig,tfue,shroud,5uppp,fuslie,tenz,\
nadeshot,timthetatman,sodapoppin,sypherpk,yassuo
npx tsx scripts/twitch-irc-collector.ts
```

**Expected Daily Inflow**:
- ~2K new usernames/day (from 20 channels @ 1.6K events/min aggregate)
- Auto-inserted into user_mapping + claimed via promo 100 CHAT
- No schema changes needed

**Cumulative**: 150K (day 1) + 14K (week 1) = 164K by day 8

---

### Step 3: Leaderboard & Viral Loop (Month 1)
**Goal**: 300K users via word-of-mouth + community imports

**Actions**:
- Publish "Top 100 Claimers" leaderboard (FOMO driver)
- Add "Invite" link for communities (fork repo for YouTube/Discord)
- Sponsor 10 streamers to host claim events

**Expected**: +150K users organically

---

## Supply & Economics

### CHAT Token Impact
| Phase | Users | Avg Allocation | Total CHAT | Cumulative |
|-------|-------|-----------------|-----------|-----------|
| Today (Base) | 9,318 | 100-1000 | 2.2M | 2.2M |
| Phase 1 (Sealed) | 150,000 | 100-1000 | 17M | 19.2M |
| Phase 2 (IRC week) | 14,000 | 100 | 1.4M | 20.6M |
| Phase 3 (Month) | 150,000 | 100 | 15M | 35.6M |

**Total Supply Check**: 35.6M CHAT staged (your mint authority)
**Current mint supply**: Check on-chain
**Risk**: LOW (uncapped mint, but track inflation)

---

## Risk Assessment

### Critical Issues (Block Launch?)
- [ ] Claim endpoint rate-limited? (Prevent spam claiming)
- [ ] Double-claim prevention working? (redeemed_at tracking)
- [ ] Suppression list ready? (Who's blocked?)

### High Priority (Fix Before 100K)
- [ ] Database indexes optimized for claimable_allocations queries?
- [ ] IRC collector crash recovery (systemd/PM2 auto-restart)?
- [ ] Mint authority key secure? (HSM/Ledger?)

### Medium Priority (Fix Before 1M)
- [ ] Leaderboard query performance (Index on (amount, claimed_at))
- [ ] Replication lag (DO → local sync delay)
- [ ] Governance: Who authorizes new users/suppression?

---

## Validation Queries

### Before Expansion
```sql
-- Check current state
SELECT COUNT(DISTINCT username) as eligible_users FROM claimable_allocations;
SELECT SUM(amount) as total_chat FROM claimable_allocations;
```

### After Expansion
```sql
-- Verify success
SELECT COUNT(DISTINCT username) as new_eligible_users FROM claimable_allocations;
-- Expected: ~150K
SELECT SUM(amount) as new_total_chat FROM claimable_allocations;
-- Expected: ~19M total
```

### Monitor Live Inflow (IRC)
```sql
-- New users added per hour
SELECT
  date_trunc('hour', MAX(ts)) as hour,
  COUNT(DISTINCT channel || ':' || user) as new_users
FROM twitch_events_raw
WHERE type = 'chat'
GROUP BY date_trunc('hour', MAX(ts))
ORDER BY hour DESC
LIMIT 24;
```

---

## AI Review Checklist

Ask your AI reviewer to validate:

1. **Signal-weighted formula**: Is `100 + (count * 10)` optimal or does it bias high-volume channels?
2. **Cap at 1000**: Should we tier allocations (100 = base, 500 = active, 1000 = super-user)?
3. **Suppression list**: Any edge cases (case sensitivity, emoji usernames)?
4. **Supply economics**: Is 35M CHAT sustainable? Any tokenomics implications?
5. **Scalability**: Can we hit 1M users with current infrastructure?

---

**Next**: Run Option A backfill, then monitor Phase 2 & 3.
