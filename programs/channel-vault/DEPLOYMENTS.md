# ChannelVault Deployments

## Mainnet

**Program ID:** `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ`
**IDL Account:** `FuwoBwLPZkvKNpQYEJpXKBHWdWg2GA5dyYL162WtzMGj`
**Last Deploy Slot:** 396551576
**Binary SHA256:** `fb670591a4c7ec2ca81dca3c31aa95d66003a6972d080fe19986091c9b3bd7e1`
**Admin:** `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`

---

## Trial Vaults (Active)

Min deposit: 10 CCM

| Channel | Lock/Queue | Vault | vLOFI Mint | Channel Config |
|---------|------------|-------|------------|----------------|
| `lofi-vault-3h` | 27,000 slots (3h) | `7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw` | `E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS` | `J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW` |
| `lofi-vault-6h` | 54,000 slots (6h) | `3BumiGZYw96eiyHEjy3wkjnrBTgcUspYmFHHptMpHof9` | `pZ5RyPEB9CS9SBjtidHARtQHqaqFT9qWKLLzohJSn4H` | `dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy` |
| `lofi-vault-9h` | 81,000 slots (9h) | `BnN5JfewvFZ93RFsduKyYbBc3NYvVc4xuYRDsMptEWu8` | `HUhqcKzaYabscWm31YsJYLn4kRxsNrKYgLmJu69fRdCp` | `2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM` |
| `lofi-vault-12h` | 108,000 slots (12h) | `8j7M2aQg7FdaN6dTW33km2zfJX5USVqQwSZ2WPA4kaPz` | `FWKim8StacRqPQ5Cq9QhMwbqHciCC4M1jj56B2FKq63p` | `GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP` |

---

## Legacy Vaults (Deprecated)

### attention:audio

| Component | Address | Status |
|-----------|---------|--------|
| Channel Config | `4Vkv6AsPRnGXQBmDhGsHBrb4xbpLUQ6rhSwYAHkuJoC7` | Active |
| Vault | `CD7hwmJpES9a86NE7nan8tXKWPfWY5yagEt7ESAqzK38` | Old struct |
| vLOFI Mint | `dY2Y11ANxWw4TBbqjwd3gcGaQDEDEYbQr1xFqHJvu4n` | |
| CCM Buffer | `5de4cYnt85HBxoRsQC6LJdyqzKWi6o8SvSSvre9632Jv` | |
| Oracle Position | `BqCVT1XWFVVjJZYJMq6KPWYcXXnat9yzgnauUTnX9Q78` | |

**Note:** This vault uses the old account layout (no `lock_duration_slots`/`withdraw_queue_slots`). May require migration or re-init.

### spotify:oracle (Closed)

The original `spotify:oracle` channel config has been closed on mainnet. Old PDAs are inert.

---

## Related Addresses

| Component | Address |
|-----------|---------|
| CCM Mint (Token-2022) | `Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM` |
| Attention Oracle Program | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| Protocol State | `596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3` |

---

## Version History

| Version | Slot | Binary SHA256 | Changes |
|---------|------|---------------|---------|
| v1.2.1 | 396551576 | `fb670591a4c7ec2ca81dca3c31aa95d66003a6972d080fe19986091c9b3bd7e1` | Reserve-first draw, transfer fee handling |
| v1.2.0 | 396549992 | `aa72f084a0a34656325e86f6a149f2b20acbc10426db8efe2062de2270c59d5b` | Per-vault lock config, Option B reserve |
| v1.1.0 | 396544983 | `6d4dcc5a78961c8ec67b176efc296f4bbcf371fb481de8e4d79e5f2d40e2bc68` | Accounting fixes, instant_redeem |
| v1.0.0 | 396474303 | `1ac295cb584c7c1e34d0179e1cd44ba9463e8b7a6e3ff45c2a0cd0bd3f5610cd` | Initial deploy |

### v1.2.1 Changes (2026-01-28)
- **Fixed:** `instant_redeem` now measures actual user balance change post-transfer (handles Token-2022 fees)
- **Fixed:** Reserve-first draw for instant exits (draws from `emergency_reserve` before `pending_deposits`)
- **Fixed:** `pending_deposits` adjusted when penalty moves to reserve (prevents double-counting in NAV)

### v1.2.0 Changes (2026-01-28)
- **Added:** `lock_duration_slots` and `withdraw_queue_slots` per-vault config (no more hardcoded constants)
- **Changed:** Reserve included in NAV (Option B) - `net_assets = staked + pending + reserve - withdrawals`
- **Added:** `VaultInsolvent` error when `pending_withdrawals > gross`
- **Breaking:** Account layout changed - existing vaults need migration or fresh init

### v1.1.0 Changes (2026-01-28)
- **Fixed:** Share accounting now subtracts `pending_withdrawals` from total assets
- **Fixed:** Compound reserves CCM for pending withdrawals (doesn't stake reserved funds)
- **Fixed:** Complete withdraw reconciles actual unstake receipts
- **Added:** `emergency_reserve` field for instant redeem liquidity buffer
- **Added:** `instant_redeem` - user exit from buffer/reserve with 20% penalty (no Oracle touch)
- **Added:** `admin_emergency_unstake` - admin break-glass for Oracle emergency unstake
- **Added:** `close_vault` - close empty vaults and reclaim rent

---

## Verification

The program can be verified using Anchor Verified Build:

```bash
anchor verify 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ --provider.cluster mainnet
```

Or using Solana Verify:

```bash
solana-verify verify-from-repo \
  --remote https://github.com/twzrd/attention-oracle-program \
  --program-id 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ \
  --mount-path programs/channel-vault
```
