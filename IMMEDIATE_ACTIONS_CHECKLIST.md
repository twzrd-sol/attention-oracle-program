# Immediate Actions Checklist

**Read this first.** Everything else can wait.

---

## ✅ What I Did For You (Already Complete)

- [x] Backed up PostgreSQL and SQLite
- [x] Disabled auto-publish on legacy aggregator
- [x] Pushed main + v0.2.1-clean to GitHub (locked public repo)
- [x] Created SERVICES.md (target architecture)
- [x] Scaffolded Stream Listener from clean SDK
- [x] Created all documentation

**Status:** Your public repo is locked. Legacy system is pinned. Ready to rebuild.

---

## 🔴 Your Jobs (Do These Today)

### Job 1: Assign Owners (1 hour)

**Assign people to these roles:**

```
Stream Listener:     [ ] _________________
Aggregator:          [ ] _________________
Settlement:          [ ] _________________
Gateway:             [ ] _________________
Database/Schema:     [ ] _________________
Monitoring:          [ ] _________________
DRI (Final Decision):[ ] _________________
```

**Owner responsibilities:**
- Stream Listener: Build listener that connects to SDK, outputs to Redis queue + NDJSON
- Aggregator: Build Merkle tree processor, validate against legacy (byte-for-byte)
- Settlement: Build on-chain settlement via SDK
- Gateway: Rebuild API layer (replace broken 176-restart version)
- Database: Manage schema, backups, recovery plans
- Monitoring: Set up dashboards, alerts, metrics
- DRI: Greenlight cutover when all tests pass

### Job 2: Create Private Repos (30 minutes)

Create these GitHub repos (private):

```
1. twzrd-stream-listener
   - Copy these files:
     - twzrd-stream-listener-package.json → package.json
     - twzrd-stream-listener-tsconfig.json → tsconfig.json
     - twzrd-stream-listener-.env.example → .env.example
     - twzrd-stream-listener-README.md → README.md
     - twzrd-stream-listener-src-index.ts → src/index.ts
     - twzrd-stream-listener-src-listener.ts → src/listener.ts
   - Add: .gitignore, .github/workflows/build.yml
   - Give owners read/write access

2. twzrd-aggregator-new
   - Use SERVICES.md as spec
   - Start from scratch OR fork existing /apps/twzrd-aggregator/src
   - Ensure: PostgreSQL schema matches, BullMQ consumer, Merkle tree builder
   - Give owners read/write access

3. twzrd-gateway-new
   - API layer (Express/Fastify)
   - Spec: GET /claims, POST /verify, GET /epochs, GET /health
   - Give owners read/write access
```

### Job 3: Schedule Team Kickoff (15 minutes)

**Kickoff meeting:** Tomorrow (Nov 19) @ [TIME]

**Attendees:** All owners + you + CTO/Tech Lead

**Agenda (30 minutes):**
1. Review SERVICES.md together (5 min)
2. Assign specific subtasks to each owner (10 min)
3. Define success metrics (5 min)
4. Establish daily standups (2 min)
5. Q&A (8 min)

**Prep:** Have each owner read `SERVICES.md` before kickoff

### Job 4: Verify Backups (15 minutes)

Run this to verify we have safety net:

```bash
# Check backup files exist
ls -lh /home/twzrd/milo-token/backups/

# Expected output:
# -rw------- postgres_backup.sql (should be >1MB)
# -rw------- legacy_data_archive_*.tar.gz (should be >3GB)

# Verify postgres backup is valid
wc -l /home/twzrd/milo-token/backups/postgres_backup.sql
# Should be >1000 lines

# Verify you can restore (optional, don't actually do it):
# head -20 /home/twzrd/milo-token/backups/postgres_backup.sql
# Should show CREATE TABLE statements
```

---

## 📋 This Week (Nov 18-24)

### Mon-Tue: Build Phase 1
- [ ] Stream Listener: npm install, npm run build, npm run dev
- [ ] Aggregator: Start implementation (use SERVICES.md)
- [ ] Daily validation: Compare events with legacy stream-listener

### Wed-Thu: Build Phase 2
- [ ] Aggregator: Merkle root generation complete
- [ ] Aggregator: PostgreSQL writes working
- [ ] Settlement: Start implementation
- [ ] Daily validation: Merkle roots match legacy (byte-for-byte)

### Fri: Integration Testing
- [ ] All services building and running
- [ ] Queue events flowing end-to-end
- [ ] PostgreSQL consistency checks passing
- [ ] Prepare for Week 2 load testing

---

## 📄 Documentation to Read

**In priority order:**

1. **IMMEDIATE:** `QUICK_START_FOUNDER_BRIEFING.md` (5 min read)
   - Executive summary of current state

2. **BEFORE KICKOFF:** `SERVICES.md` (15 min read)
   - Target architecture for rebuild
   - Share with all owners

3. **BEFORE WEEK 1:** `PARALLEL_REBUILD_EXECUTION_SUMMARY.md` (10 min read)
   - What was done today
   - Timeline and budget
   - Owner assignments

4. **FOR REFERENCE:** `CONTINUE_VS_REBUILD_DECISION.md`
   - Decision matrix (for context, we chose rebuild)

5. **FOR OPS TEAM:** `BACKUP_AND_MIGRATION_PLAN.md`
   - Data preservation strategy

---

## 🚨 If Anything Goes Wrong

### System is Down
- **Command:** `pm2 status` (check if services running)
- **Fallback:** `pm2 restart stream-listener milo-aggregator` (restart)
- **Contact:** dev@twzrd.xyz

### Team is Blocked on Code
- **Read:** `SERVICES.md` → Exact interfaces and schemas
- **Check:** `twzrd-stream-listener-README.md` → Setup instructions
- **Contact:** dev@twzrd.xyz

### Data Concerns
- **Backups exist at:** `/home/twzrd/milo-token/backups/`
- **Recovery plan:** See `BACKUP_AND_MIGRATION_PLAN.md`
- **Contact:** dev@twzrd.xyz

### Rebuild Stalls >2 Weeks
- **Decision point:** End of Week 2 (Dec 1)
- **If red metrics:** Pause build, fallback to legacy, do post-mortem
- **Escape hatch:** Continue stabilizing legacy services instead

---

## ✨ Quick Reference

### File Locations (All in `/home/twzrd/milo-token/`)

**Documentation:**
- `QUICK_START_FOUNDER_BRIEFING.md` ← START HERE
- `SERVICES.md` ← Share with team
- `PARALLEL_REBUILD_EXECUTION_SUMMARY.md` ← Handoff document
- `INFRASTRUCTURE_STATUS.md` ← Live system state
- `BACKUP_AND_MIGRATION_PLAN.md` ← Data safety

**Scaffolds (Copy to Private Repos):**
- `twzrd-stream-listener-*.json`, `-.ts`, `-.md`
- `twzrd-aggregator-package.json` (fixed)

**Config:**
- `.env` ← Contains secrets (do not commit)
- `backups/` ← PostgreSQL + SQLite snapshots

### System Status (Right Now)

```bash
# Check legacy services
pm2 status

# View aggregator logs
pm2 logs milo-aggregator --lines 20

# Check database
psql -U user -h localhost twzrd -c "SELECT COUNT(*) FROM claims;"

# Monitor queue depth
redis-cli LLEN twzrd:stream-events:*
```

### Git Status

```bash
# Latest public commit
git log --oneline -1 github/main
# → dedd482 chore: finalize workspace config for v0.2.1-clean

# Tag is locked
git show v0.2.1-clean:programs/src/lib.rs | grep declare_id
# → GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop ✅
```

---

## 📞 Contact & Support

- **Technical blocker:** dev@twzrd.xyz
- **Architecture question:** See SERVICES.md (comprehensive spec)
- **Data concern:** See BACKUP_AND_MIGRATION_PLAN.md
- **Status update:** Check this repo's docs daily

---

## ✅ Done

You now have:

1. ✅ Safe backups (legacy data preserved)
2. ✅ Public repo locked (v0.2.1-clean immutable)
3. ✅ Target architecture (SERVICES.md)
4. ✅ Stream Listener scaffold (ready to build)
5. ✅ Migration plan (3-phase cutover)
6. ✅ Team playbook (success criteria + fallback)
7. ✅ Full documentation (no ambiguity)

**Next:** Assign owners, kick off builds tomorrow.

---

**Get started:** Pick your owners list above and schedule kickoff.
