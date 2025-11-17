# DEPRECATED GATEWAY

**Date**: 2025-11-17
**Status**: Archived (do not use)

## Why Deprecated

This gateway implementation had incomplete TypeScript source code:
- Only 3 source files in `src/`
- ~97 compiled JavaScript files in `dist/`
- Mixed `@noble/hashes` import styles (v1 vs v2)
- Non-recoverable compilation state

## Replacement

Use the canonical gateway at:
```
/home/twzrd/milo-token/gateway/
```

Running in PM2 as `gateway` (ID: 59)

## What Happened

During metrics instrumentation (Nov 17, 2025):
1. Attempted to fix `@noble/hashes` v2 breaking changes
2. Discovered source tree was incomplete/missing
3. Switched to working Express gateway at root `/gateway/`
4. All functionality migrated successfully

## Safe to Delete

This directory can be deleted after confirming:
- No scripts reference `/apps/gateway/`
- PM2 is pointing to `/home/twzrd/milo-token/gateway/`
- All documentation updated

---
For questions, see: `/home/twzrd/milo-token/gateway/docs/gateway.md`
