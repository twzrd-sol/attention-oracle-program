# [UNBLOCK] Aggregator Rebuild — Safe Schema & Event Sample Extraction

Team,

We're ready to scaffold the Rust Aggregator rebuild. To ensure the new service matches production exactly (preserves Merkle proofs), we need two small forensic outputs:

1) Database schema DDL for: `merkle_trees`, `roots`, `claims`, `wallet_bindings`.
2) A 10-line NDJSON event sample from the listener.

Safety First:
- No credentials in chat. No binary scraping. Read-only access only.

## Instructions (run on milo-token prod host)

Prerequisites:
- You have `DATABASE_URL` or PG* env vars + `PGPASSWORD` in your shell (from secrets manager, PM2 env, or `.pgpass`).
- DB user has read-only access to metadata (schema). If TLS is required, export `PGSSLMODE`/`PGSSLROOTCERT`.

### Step 1: Extract Schema (DDL only)

```bash
scripts/forensics/dump_schema.sh
```

Expected: creates `schema_dump.sql` and prints the first 120 lines.

### Step 2: Extract NDJSON Sample

```bash
scripts/forensics/ndjson_sample.sh
```

Expected: prints last 10 lines from the most recent NDJSON file.

## Deliverables
- Paste back:
  - First ~120 lines of `schema_dump.sql` (or full if <200 lines)
  - The 10-line NDJSON sample

## Next Steps (once received)
- Generate SQLx migrations and types for `twzrd-aggregator-rs`.
- Scaffold ingest + proof endpoints and parity harness.
- In parallel, deploy the new TypeScript listener with vendored IDL via PM2.

