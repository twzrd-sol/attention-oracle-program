# ChannelVault Deployments

## Mainnet

**Program ID:** `5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ`
**IDL Account:** `FuwoBwLPZkvKNpQYEJpXKBHWdWg2GA5dyYL162WtzMGj`
**Last Deploy Slot:** 396551576
**Binary SHA256:** `fb670591a4c7ec2ca81dca3c31aa95d66003a6972d080fe19986091c9b3bd7e1`
**Admin:** `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`

---

## Vaults

Vault addresses are derived on-chain from channel configs via PDA seeds.
See local ops config for vault inventory.

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
