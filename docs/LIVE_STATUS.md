# Live Status

This document tracks **on-chain deployment facts** and the current **verification status** for the programs in this repository.

**Last updated:** 2026-01-27

---

## Quick Status

| Program | Status | Program ID |
|---------|--------|------------|
| token_2022 | 🟢 Active | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |

---

## Mint Extensions (Mainnet, if applicable)

If your deployment uses a Token-2022 mint, verify its configured extensions:

```bash
spl-token display --program-2022 -u mainnet-beta -v <MINT_ADDRESS>
```

---

## token_2022 (Active)

**Program ID:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`

| Property | Value |
|----------|-------|
| Status | 🟢 Active on mainnet |
| Upgrade Authority | See `DEPLOYMENTS.md` |

---

## Verification Commands

### Get on-chain executable hash
```bash
solana-verify get-program-hash -u https://api.mainnet-beta.solana.com GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

### Build and verify
See `VERIFY.md` for the recommended verification workflows.

---

## Legend

| Status | Meaning |
|--------|---------|
| 🟢 Active | Program is live on mainnet |
