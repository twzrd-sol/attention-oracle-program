# Incident: Schema Mismatch on Production DB (`token_group`)

**Date:** 2025-11-17
**Status:** Resolved
**Severity:** Medium (pipeline disruption, no data loss)

---

## Summary

The off-chain aggregator failed in production due to a **schema mismatch**: it expected a `token_group` column on key tables that did not exist in the managed PostgreSQL database. The incident was resolved by applying a **safe, non-destructive schema extension** (`ALTER TABLE ... ADD COLUMN IF NOT EXISTS ... DEFAULT 'milo'`) and verifying that all existing data remained intact.

No records were deleted or corrupted. Aggregator processing resumed normally after the fix.

---

## Impact

- **Affected components:** Off-chain aggregator and any downstream consumers relying on fresh epoch/participant aggregates.
- **Data:**
  - 1,230 epochs sealed
  - 961,567 participants
  - 2,738,092 channel participation records
  - Latest epoch as of incident: 2025-11-16
- **User-visible:** Internal only (devnet / infra), no external users impacted yet.

---

## Timeline (UTC)

- **2025-11-17 ~14:30** – Aggregator begins failing with errors referencing `token_group` (e.g. `column "token_group" does not exist`).
- **2025-11-17 ~14:35** – Confirmed DB reachability; DB listed and healthy via `doctl databases list` and `psql`.
- **2025-11-17 ~14:40** – Identified mismatch between expected schema (code) and actual production schema:
  - Code paths and queries referenced `token_group`.
  - `information_schema.columns` confirmed `sealed_epochs` and `sealed_participants` lacked that column.
- **2025-11-17 ~14:45** – Applied non-destructive schema extension using `ALTER TABLE ... ADD COLUMN IF NOT EXISTS ... DEFAULT 'milo'`.
- **2025-11-17 ~14:50** – Validated:
  - Row counts unchanged.
  - New column populated with default `'milo'` for existing rows.
  - Aggregator resumed processing without errors.

---

## Root Cause

**Schema drift** between code and production DB:

- Recent aggregator changes assumed the presence of a `token_group` column on:
  - `sealed_epochs`
  - `sealed_participants`
- These schema changes were either:
  - applied in a local/dev environment only, or
  - never formally migrated into the production database.
- Production DB continued operating with the older schema until the upgraded aggregator attempted to read/write `token_group`, causing runtime errors.

---

## Technical Details

### Symptoms

- Aggregator logs showed errors similar to:
  - `ERROR: column "token_group" does not exist`
  - Failing queries against `sealed_epochs` / `sealed_participants`.

### Diagnosis

1. Verified DB cluster health:
   - `doctl databases list` – cluster present and online.
   - `psql "$DATABASE_URL" -c "SELECT now();"` – connectivity OK.
2. Inspected schema:
   ```sql
   SELECT table_name, column_name
   FROM information_schema.columns
   WHERE table_name IN ('sealed_epochs', 'sealed_participants')
     AND column_name = 'token_group';
   ```
   * Result: no rows → column missing.

3. Confirmed code expectations:
   * Aggregator queries / models referenced `token_group` for grouping token families (e.g. `'milo'`, `'cls'`).

### Resolution

Applied safe schema extension:

```sql
ALTER TABLE sealed_epochs
  ADD COLUMN IF NOT EXISTS token_group VARCHAR(10) DEFAULT 'milo';

ALTER TABLE sealed_participants
  ADD COLUMN IF NOT EXISTS token_group VARCHAR(10) DEFAULT 'milo';
```

Properties of this change:

* **Non-destructive:** no columns dropped; no rows deleted or modified except for adding a default value on a new column.
* **Backwards compatible:** existing code that doesn't reference `token_group` continues to work.
* **Forward compatible:** aggregator logic can now group by `token_group` as intended.

---

## Verification

### Schema Validation

```sql
SELECT
  table_name,
  column_name,
  data_type,
  column_default
FROM information_schema.columns
WHERE table_name IN ('sealed_epochs', 'sealed_participants')
  AND column_name = 'token_group'
ORDER BY table_name;

SELECT 'sealed_epochs' AS table_name, COUNT(*) AS row_count FROM sealed_epochs
UNION ALL
SELECT 'sealed_participants', COUNT(*) FROM sealed_participants;
```

* Confirmed:
  * `token_group` exists on both tables.
  * `data_type = character varying`.
  * `column_default = 'milo'::character varying`.
  * Row counts unchanged.

### Aggregator Health

* `pm2 logs milo-aggregator` – no further `token_group`-related errors.
* New epochs continue to be sealed and ingested successfully.
* Participant / channel participation counts continue to grow as expected.

---

## Data Loss Assessment

* **Records deleted:** 0
* **Records corrupted:** 0
* **Recovery actions required:** None

All changes were additive and reversible via:

```sql
ALTER TABLE sealed_epochs DROP COLUMN token_group;
ALTER TABLE sealed_participants DROP COLUMN token_group;
```

(if ever needed).

---

## Prevention & Follow-ups

### 1. Schema Migration Discipline

* Introduce a **versioned migration system** (e.g. `db/migrations/*.sql` or a tool like `dbmate`, `golang-migrate`, etc.).
* Enforce a policy:
  * **No code that depends on a schema change is deployed** unless its migration has been run on the target environment.

### 2. Schema Validation Script

* Add a lightweight schema validation step to deployment:
  * Script compares **expected columns** (from code/migrations) to `information_schema.columns` for key tables.
  * Deployment fails if required columns are missing.

### 3. Documentation

* Document future schema changes in:
  * `CHANGELOG.md` or
  * `docs/db-schema-changes.md`
* Include:
  * when the change was applied,
  * which component depends on it,
  * any backfill/default semantics.

### 4. Testing

* Add an integration test (or CI job) that:
  * Spins up a test DB,
  * Applies migrations,
  * Runs aggregator against that DB,
  * Fails if migrations and code expectations diverge.

---

## Lessons Learned

* "Just one extra column" is enough to break a production pipeline when migrations and code are not kept in lockstep.
* Quick, **non-destructive** fixes (`ADD COLUMN IF NOT EXISTS ... DEFAULT ...`) are invaluable under pressure, especially when data volume is high.
* Having good observability (aggregator logs + DB introspection + Slack signals) made this a **surgical** incident, not a crisis.

**Status:** 🟢 **Resolved — All systems nominal; aggregator resumed processing.**
