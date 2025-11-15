# ☀️ 9am CT Morning Verification Checklist
**Target Time:** 9:00 AM CT (15:00 UTC)
**Date:** Nov 5, 2025

---

## Expected State by 9am CT

### 📊 Expected Seals Since Last Night

**Last Verified Seal:** 1762308000 (Nov 5, 02:00 UTC)

**Expected New Epoch Timestamps:** 12 hourly boundaries
- 03:00, 04:00, 05:00, 06:00, 07:00, 08:00, 09:00, 10:00, 11:00, 12:00, 13:00, 14:00 UTC

**Expected Database Growth:**
- **12 unique epoch timestamps** (hourly boundaries from 03:00-14:00 UTC)
- **~1,300+ total sealed_epochs rows** (all active MILO + CLS channels across 12 epochs)
- **MILO:** ~9 channels × 12 epochs = ~108 new seals
- **CLS:** ~100 channels × 12 epochs = ~1,200 new seals

**Critical Verification:**
- Both MILO and CLS MAX(epoch) should be **IDENTICAL** at ~1762351200 (14:00 UTC)
- This proves the system is in sync

---

## ✅ 3-Step Quick Verification

### Step 1: Service Health (`pm2 status`)
**GO:** All critical services online (cls-discovery stopped = correct)
**NO-GO:** Any critical service errored/stopped

### Step 2: Run Monitor Script
```bash
/home/twzrd/milo-token/scripts/overnight-monitor.sh
```
**GO:** "Sealing frequency normal" + "All critical services online"
**NO-GO:** "No new epochs sealed in last 2 hours"

### Step 3: Gateway Logs
```bash
pm2 logs gateway --lines 50 --nostream
```
**GO:** Standard HTTP logs, no errors
**NO-GO:** Database connection errors or crashes

---

## ✅ FIXED: Gateway CLS Bug

**Discovered:** Nov 5, 04:00 UTC - 2 of 3 proof endpoints were broken for CLS claims
**Fixed:** Nov 5, 04:30 UTC - All endpoints now support token_group and category

**Status:**
- ✅ `/proof` endpoint → **WORKS** (accepts token_group + category parameters)
- ✅ `/proof-sealed` → **FIXED** (now accepts token_group + category)
- ✅ `/claim-proof` → **FIXED** (now accepts token_group + category)

**Impact on Today:**
- ✅ **MILO claims work** (backward compatible, defaults to MILO)
- ✅ **CLS claims now work** (can specify token_group=CLS + category)
- ✅ **Full two-tier testing possible** (test both MILO and CLS in one day!)

**Details:** See `GATEWAY_CLS_FIX.md` for complete fix documentation

**Deployment:** Gateway restarted at 04:28 UTC, currently online and healthy

---

## 📋 Verification Queries

### Query 1: MILO Latest Epochs
```bash
psql "postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require" -c "
SELECT channel, MAX(epoch) as latest, TO_TIMESTAMP(MAX(epoch)) as time
FROM sealed_epochs
WHERE token_group = 'MILO' AND epoch > 1762308000
GROUP BY channel
ORDER BY latest DESC LIMIT 5;
"
```
**Expected:** MAX(epoch) around 1762351200 (14:00 UTC)

### Query 2: CLS Latest Epochs
```bash
psql "postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require" -c "
SELECT MAX(epoch) as latest, TO_TIMESTAMP(MAX(epoch)) as time, COUNT(*) as seals
FROM sealed_epochs
WHERE token_group = 'CLS' AND epoch > 1762308000;
"
```
**Expected:** MAX(epoch) = 1762351200 (same as MILO), ~1,200 seals

### Query 3: Total New Seals
```bash
psql "postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool?sslmode=require" -c "
SELECT token_group, COUNT(*) as new_seals, COUNT(DISTINCT channel) as channels
FROM sealed_epochs
WHERE epoch > 1762308000
GROUP BY token_group;
"
```
**Expected:** MILO ~108, CLS ~1,200 (total ~1,300)

---

## ✅ All-Clear Criteria

**GREEN LIGHT:** Proceed to `ON_CHAIN_TEST_PLAN.md`
- All services online
- ~1,300 new seals
- MAX(epoch) identical for MILO/CLS
- No crashes or errors

**RED LIGHT:** Do not proceed
- Critical services offline
- No seals in 4+ hours
- Database errors in gateway

---

**Test Case Ready:** marlon (MILO) epoch 1762308000
**Merkle Root:** 6fce67da102af54283b0deb46e6d1880fb7670e6bbff240c149234f6333ee3b0 (✅ on-chain)
**Test Participant:** 012c318b0b549fef8d9c4b10258307b57fcb55949c39637919bf572e9b149338

**Status:** ✅ Ready for On-Chain Day
