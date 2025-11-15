# 🤖 AI Review Kit v1.0

**Purpose**: Comprehensive system context for AI-assisted review of your Attention Oracle scaling plan

**Status**: Launch-ready (9,318 users staged, IRC live, workers healthy)
**Goal**: Validate path to 100K+ eligible users
**Time to Review**: 45-60 minutes

---

## 📂 Files in This Kit

### 1. Diagnostics (Read First)
- **00-README.md** ← You are here
- **01-db-inventory.txt** - Database table sizes & structure
- **02-row-counts.txt** - Current user counts (sealed_participants, claimable_allocations, etc.)
- **07-pm2-status.txt** - Running processes & memory usage

### 2. Strategy Documents (Read Second)
- **11-expansion-strategy.md** - Scaling plan (9K → 100K) with SQL & formulas

### 3. Source Code (Read With Strategy)
- **03-server.ts** - Claim API (Express.js)
- **04-claim.ts** - Solana transaction builder
- **05-eligibility.ts** - Eligibility check logic
- **06-irc-collector.ts** - Live username ingestion

### 4. Logs (Diagnostic Reference)
- **08-aggregator-logs.txt** - cls-aggregator recent activity
- **09-api-logs.txt** - API server health
- **10-irc-logs.txt** - IRC collector activity

---

## 🎯 Key Questions for AI Reviewer

### Tier 1: Can We Scale to 100K?
1. **Database Performance**: Will inserting 150K rows into claimable_allocations cause latency spike?
2. **Query Optimization**: Any missing indexes on (username, channel, amount)?
3. **Deduplication**: Is the ON CONFLICT logic correct? Any risk of orphaned rows?

### Tier 2: Is Expansion Logic Correct?
4. **Signal Formula**: Is `100 + (count * 10), cap 1000` optimal or does it favor high-volume channels unfairly?
5. **Suppression Enforcement**: Any edge cases (case sensitivity, special chars, duplicates)?
6. **Supply Impact**: We're adding ~17M CHAT. Is this acceptable? Any tokenomics risk?

### Tier 3: Can We Handle Live Inflow?
7. **IRC Throughput**: Current rate is ~1.6K events/min on 4 channels. Can we scale to 20 channels (8K events/min)?
8. **Username Mapping**: Is keccak256(lowercase(username)) collision-safe? Any birthday paradox risk at 1M users?
9. **Crash Recovery**: If IRC collector dies, do we lose data? Should we add write-through cache?

### Tier 4: Security & Integrity
10. **Rate Limiting**: Is /api/claim endpoint protected? (Prevent spam claiming)
11. **Double-Claim Prevention**: Is redeemed_at tracking enforced in DB? Any race conditions?
12. **Key Management**: Mint authority key location secure? Suggestions for HSM/Ledger?

### Tier 5: Post-Launch Scaling
13. **Leaderboard Performance**: Can we query "top 100 claimers by CHAT" in <500ms at 1M users?
14. **Database Replication**: Current split (DO vs local) is OK for launch but fragile. Recommended strategy?
15. **User Retention**: What metrics should we track? (claim frequency, wallet activity, repeat claims)

---

## 📊 Current System State (TL;DR)

### Data
```
Eligible Users (staged):       9,318
Allocations Ready:             22,447 rows
CHAT Tokens:                   2.24M
Mapped Usernames (Available):  174K (in sealed_participants)
New Users/Day (IRC):           ~1.6K
```

### Infrastructure
```
Compute:     DigitalOcean 8vCPU / 32GB RAM / 640GB SSD
Database:    PostgreSQL 14 (local) + PostgreSQL 16 (DO) [split, post-launch unify]
Redis:       Valkey 8 (DO)
On-Chain:    Solana Mainnet (CLS program GnGzNds... / CHAT mint AAHd7u22...)
Network:     NYC3 (all components)
```

### Services
```
✅ IRC Collector:    RUNNING (290 events/15sec on 4 channels)
✅ Claim API:        RUNNING on :3000 (health check OK)
✅ Aggregator:       ONLINE (19 restarts, processing)
✅ Tree-Builder:     ONLINE (sealing epochs hourly)
✅ Workers:          ONLINE (cls-worker-s0/s1, BullMQ)
⚠️  Gateway:         ONLINE (567 restarts in 4 days—RPC flakiness)
```

---

## 🚀 Expansion Plan (TL;DR)

### Phase 1: Today (Immediate Backfill)
- Insert all 174K mapped users from sealed_participants
- Formula: `100 + (participation_count * 10), cap 1000`
- Expected: ~150K eligible users, ~17M CHAT
- Duration: <5 minutes (SQL batch)
- Risk: LOW (ON CONFLICT, dedup via NOT EXISTS)

### Phase 2: Week 1 (Scale IRC)
- Expand IRC collector: 4 → 20 channels
- Expected: +2K new users/day
- Cumulative by day 8: 164K users

### Phase 3: Month 1 (Viral Loop)
- Leaderboard, community imports, streamer sponsorships
- Expected: +150K organically
- Target: 300K+ users

---

## 📋 What This Kit Contains

| File | Size | Type | Read Time |
|------|------|------|-----------|
| 00-README.md | 3 KB | Guide | 5 min |
| 01-db-inventory.txt | 2 KB | Diagnostic | 2 min |
| 02-row-counts.txt | <1 KB | Diagnostic | 1 min |
| 03-server.ts | 24 KB | Source | 10 min |
| 04-claim.ts | 6 KB | Source | 3 min |
| 05-eligibility.ts | 4 KB | Source | 2 min |
| 06-irc-collector.ts | 13 KB | Source | 5 min |
| 07-pm2-status.txt | 4 KB | Diagnostic | 2 min |
| 08-aggregator-logs.txt | 26 KB | Logs | Scan |
| 09-api-logs.txt | <1 KB | Logs | <1 min |
| 10-irc-logs.txt | <1 KB | Logs | <1 min |
| 11-expansion-strategy.md | 8 KB | Strategy | 15 min |
| **TOTAL** | **~95 KB** | **Mixed** | **~45-60 min** |

---

## ✅ How to Use This Kit

### Option 1: Direct Paste to Claude
```bash
# Copy all files to clipboard (macOS)
cd /home/twzrd/milo-token/ai-review
cat *.md *.ts *.txt | pbcopy

# Then paste into Claude with prompt:
# "Review this system architecture for scaling from 9K to 100K users.
#  Answer the 15 key questions in 00-README.md.
#  Flag any critical blockers or security issues.
#  Provide 5 optimization recommendations."
```

### Option 2: Upload to GitHub Discussions
```bash
# Create a GitHub issue with kit contents
gh issue create --title "AI System Review: 9K → 100K Scaling Plan" \
  --body "$(cat 00-README.md; echo '---'; cat 11-expansion-strategy.md)"
```

### Option 3: Create a Gist
```bash
# Share as private Gist
gh gist create --private *.md *.ts *.txt
```

---

## 🎯 Expected AI Review Output

**Good outcome** (30 min review):
- 3-5 critical issues identified (if any)
- 10-15 optimization suggestions
- Confidence score for 100K launch (expected: 8.5/10)
- 2-3 post-launch priorities

**Excellent outcome** (60 min deep dive):
- Above + detailed SQL optimizations
- Architecture diagram for 1M users
- Security hardening checklist
- Go-live playbook (monitoring, rollback, scaling triggers)

---

## 🔐 Security Note

**Redacted**:
- Solana keypair locations (reference only)
- DigitalOcean credentials (in .env, not shared here)
- Twitch OAuth tokens (in .env, not shared here)

**Included as-is** (non-sensitive):
- Program IDs, mint addresses (public on Solana)
- Database structure (not security-sensitive)
- Source code (public MIT repo)

---

## 📞 Next Steps

1. **Share this kit** with your AI reviewer (Claude, ChatGPT, or your security team)
2. **Provide context**: "We're launching in 24 hours. Validate scaling plan + spot blockers."
3. **Collect feedback**: Use reviewer's 15 answers to prioritize fixes
4. **Execute backfill**: Run Phase 1 SQL (if AI gives thumbs up)
5. **Go live**: Announce 150K users immediately, ramp over month

---

## 📈 Metrics to Watch Post-Launch

```sql
-- Daily dashboard
SELECT
  DATE(NOW()) as date,
  COUNT(DISTINCT username) as eligible_users,
  SUM(amount) as total_chat_staged,
  (SELECT COUNT(*) FROM twitch_events_raw WHERE ts > NOW() - INTERVAL '1 day') as new_events,
  (SELECT COUNT(DISTINCT username) FROM claimable_allocations WHERE created_at > NOW() - INTERVAL '1 day') as new_users_today;
```

---

**Generated**: 2025-11-12
**System**: Launch-Ready ✅
**Reviewer**: Human or AI
**Time Investment**: 45-60 min
**Expected ROI**: 3-5 blockers caught, 100K users unlocked 🚀
