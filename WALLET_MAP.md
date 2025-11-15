# 🔑 Wallet Map - CLS Publishing System

## Overview

Three wallets are active in the CLS infrastructure. Here's their roles, locations, and usage.

---

## 1. Oracle Authority (Publisher/Payer)

**Address:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`
**Keypair Location:** `/home/twzrd/.config/solana/oracle-authority.json`
**Current Balance:** ~1.672 SOL

### Role
- **Primary publisher** for CLS epochs
- **Payer** for channel initializations (~0.04002 SOL rent per channel)
- **Transaction fees** for all publish operations

### Used By
- **PM2 Service:** `cls-aggregator` (via `PAYER_KEYPAIR` env var)
- **Scripts:** Any publish/init scripts that set `ADMIN_KEYPAIR` explicitly
- **On-Chain Authority:** Listed as `publisher` in both protocol PDAs

### Permissions
- Can publish merkle roots (authorized publisher on-chain)
- Can initialize new channels (pays rent)
- Can sign transactions on behalf of the aggregator

### Safety
- ✅ Isolated to server (never exposed client-side)
- ✅ Used by automated services only
- ✅ Has sufficient balance for operations

---

## 2. Protocol Admin (On-Chain Authority)

**Address:** `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD`
**Keypair Location:** `/home/twzrd/.config/solana/id.json` (Solana CLI default)
**Current Balance:** ~0.000 SOL (needs funding for admin ops)

### Role
- **Protocol admin** in on-chain ProtocolState (mint-keyed PDA)
- **Fallback signer** for CLI commands that don't specify a keypair
- **Emergency admin** for protocol updates/configuration

### Used By
- **Solana CLI:** Default keypair when no `--keypair` flag specified
- **Scripts:** Any script that doesn't explicitly set `ADMIN_KEYPAIR` or `PAYER_KEYPAIR`
- **On-Chain Authority:** Listed as `admin` in mint-keyed protocol PDA

### Permissions
- Can update publisher authority
- Can update protocol configuration
- Can pause/unpause protocol (emergency)

### Safety
- ⚠️ **Low balance** - fund before admin operations
- ⚠️ **CLI default** - scripts may accidentally use this instead of oracle-authority
- ✅ Keep as admin-only (don't use for routine publishing)

---

## 3. Legacy Admin (Maintenance Key)

**Address:** `AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv`
**Keypair Location:** `/home/twzrd/milo-token/keys/admin-keypair.json`
**Current Balance:** ~0.085 SOL

### Role
- **Legacy admin** for singleton protocol PDA (deprecated variant)
- **Maintenance key** for one-off admin scripts
- **Historical** - used during initial deployment/setup

### Used By
- **Scripts:** `set-publisher-singleton.ts` and other legacy admin scripts
- **On-Chain Authority:** Listed as `admin` in singleton protocol PDA (FcyW...)
- **Manual Operations:** Ad-hoc testing/fixes

### Permissions
- Can update singleton protocol (non-mint-keyed)
- Limited to specific legacy PDAs
- Not used by automated services

### Safety
- ✅ Not used by PM2 services (isolated)
- ⚠️ **Retirement candidate** - move to 2pHjZ... for consistency
- ✅ Keep for emergency recovery only

---

## Wallet Usage Matrix

| Operation | Preferred Wallet | Fallback | Script/Service |
|-----------|------------------|----------|----------------|
| Publish CLS epochs | 87d5...ufdy | None | `cls-aggregator` (PM2) |
| Initialize channels | 87d5...ufdy | None | `init-*.ts` scripts |
| Update publisher | 2pHjZ...ZZaD | AmMf...CsBv | `set-publisher-*.ts` |
| Update protocol config | 2pHjZ...ZZaD | None | Manual admin scripts |
| Emergency pause | 2pHjZ...ZZaD | None | Emergency only |
| Manual testing | AmMf...CsBv | 2pHjZ...ZZaD | One-off scripts |

---

## Current Balances (as of session)

```
87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy  1.672 SOL   ✅ Healthy
2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD  0.000 SOL   ⚠️  Fund before admin ops
AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv  0.085 SOL   ✅ OK for testing
```

**Check balances:**
```bash
solana balance 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy --url mainnet-beta
solana balance 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD --url mainnet-beta
solana balance AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv --url mainnet-beta
```

---

## PM2 Process → Wallet Mapping

| Process | Keypair Env Var | Wallet Used | Purpose |
|---------|----------------|-------------|---------|
| `cls-aggregator` | `PAYER_KEYPAIR` | 87d5...ufdy | Publish/init |
| `gateway` | None | N/A | Read-only |
| `cls-worker-s0` | None | N/A | Ingestion only |
| `cls-worker-s1` | None | N/A | Ingestion only |
| `epoch-watcher` | None | N/A | Monitoring only |
| `tree-builder` | None | N/A | Merkle computation |

**Only `cls-aggregator` signs transactions** - all other services are read-only or compute-only.

---

## On-Chain Authority Status

### Mint-Keyed Protocol PDA (Current/Active)
```
PDA: FEwsakAJZrEojzRrNJCwmS91Jn5gamuDjk1GGZXMNYwr
Seeds: ["protocol", mint]
Admin: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
Publisher: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
Paused: false
```

### Singleton Protocol PDA (Legacy)
```
PDA: FcyWuzYhxMnqPBvnMPXyyYPjpRvaweWku2qQo1a9HtuH
Seeds: ["protocol"]
Admin: AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv
Publisher: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
Paused: false
Status: Updated (was 11111... before session)
```

---

## Recommended Normalization

### ✅ Already Done
- Oracle authority (87d5...ufdy) set as publisher on both PDAs
- PM2 services explicitly use `PAYER_KEYPAIR` env var
- Strict publish mode enabled (prevents accidental inits)

### 🔄 Next Steps
1. **Fund admin wallet (2pHjZ...)**
   ```bash
   # Transfer 0.1 SOL from oracle-authority to admin for emergency ops
   solana transfer 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD 0.1 \
     --keypair ~/.config/solana/oracle-authority.json \
     --url mainnet-beta
   ```

2. **Audit all scripts** - ensure explicit `PAYER_KEYPAIR`/`ADMIN_KEYPAIR` (no defaults to id.json)

3. **Retire AmMf... usage** - update scripts to use 2pHjZ... for admin ops

4. **Add "whoami" logs** - print signer pubkey before any transaction

### 🔐 Safety Rails
- ❌ **Never commit keypair files** to git
- ❌ **Never use keypairs without explicit env var**
- ✅ **Log all transactions** before sending
- ✅ **Keep strict mode ON** (prevents accidental channel inits)

---

## Quick Reference Commands

### Check who a script will use
```bash
# Check default Solana CLI keypair
solana config get

# Override for specific command
solana balance --keypair /path/to/keypair.json
```

### Verify on-chain authorities
```bash
# Check mint-keyed protocol
solana account FEwsakAJZrEojzRrNJCwmS91Jn5gamuDjk1GGZXMNYwr --url mainnet-beta

# Check singleton protocol
solana account FcyWuzYhxMnqPBvnMPXyyYPjpRvaweWku2qQo1a9HtuH --url mainnet-beta
```

### Rotate publisher (emergency)
```bash
cd /home/twzrd/milo-token
npx tsx scripts/set-publisher-mainnet.ts  # Uses 2pHjZ... as admin
```

---

## Emergency Contacts

If wallets are compromised or need rotation:
1. **Pause protocol** using admin keypair (2pHjZ...)
2. **Transfer funds** to new secure wallet
3. **Update publisher** authority to new keypair
4. **Restart PM2 services** with new `PAYER_KEYPAIR`

**Recovery keypairs stored:** (off-server backup location TBD)

---

Last Updated: 2025-11-07 (Session continuation)
