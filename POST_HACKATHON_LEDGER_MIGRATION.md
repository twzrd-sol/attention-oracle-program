# Post-Hackathon: Ledger Hardware Wallet Migration Guide

## Overview

This guide walks you through transferring protocol admin authority from your current hot wallet (`87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`) to a Ledger hardware wallet for enhanced security.

## Current State

✅ **Protocol Admin:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy` (YOU own this)
✅ **Publisher:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy` (YOU own this)
✅ **Program Upgrade Authority:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy` (YOU own this)
✅ **Emergency Backdoor:** REMOVED (program is production-ready)
✅ **Admin Transfer Capability:** `update_admin_open` instruction DEPLOYED

**Program ID:** `4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5`
**Protocol State PDA:** `3RhGhHjdzYCCeT9QY1mdBoe8t7XkAaHH225nfQUmH4RX`
**Last Deployed Slot:** 376,838,981
**Program Size:** 636KB (658,512 bytes)

## Security Architecture (Post-Migration)

```
┌─────────────────────────────────────────────────────┐
│                  LEDGER (Cold Storage)               │
│  • Protocol Admin Authority                         │
│  • Signs: admin transfers, policy changes, pauses   │
│  • Requires physical confirmation                   │
└─────────────────────────────────────────────────────┘
                           │
                           │
┌─────────────────────────────────────────────────────┐
│           HOT WALLET (oracle-authority.json)        │
│  • Publisher Authority (unchanged)                  │
│  • Signs: merkle root publications                  │
│  • Automated oracle operations                      │
└─────────────────────────────────────────────────────┘
                           │
                           │
┌─────────────────────────────────────────────────────┐
│              PROGRAM UPGRADE AUTHORITY               │
│  • Consider: Ledger or multi-sig                    │
│  • Signs: program code upgrades                     │
│  • Most sensitive operations                        │
└─────────────────────────────────────────────────────┘
```

## Prerequisites

### 1. Get Your Ledger Ready

Connect your Ledger device and install the Solana app:
1. Connect Ledger via USB
2. Open Ledger Live
3. Install "Solana" app if not already installed
4. Open the Solana app on your Ledger

### 2. Get Ledger Public Key

```bash
# Using Solana CLI (will prompt for Ledger confirmation)
solana-keygen pubkey usb://ledger?key=0/0

# Save this address - you'll need it for the migration
```

**Example output:** `ABC123...XYZ` ← This will be your new admin address

### 3. Fund the Migration Transaction

Ensure `oracle-authority.json` has at least **0.01 SOL** for transaction fees:

```bash
solana balance ~/.config/solana/oracle-authority.json
```

If needed, send SOL from your Phantom wallet to `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`.

## Migration Steps

### Step 1: DRY RUN (Simulation)

**ALWAYS test first!** Run the migration script in simulation mode:

```bash
npx tsx scripts/transfer-admin-to-ledger.ts \
  --ledger-pubkey YOUR_LEDGER_ADDRESS_FROM_STEP_2 \
  --current-admin ~/.config/solana/oracle-authority.json \
  --rpc-url https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973 \
  --dry-run
```

**Expected output:**
```
=== Transfer Admin to Hardware Wallet ===

Current admin: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
New admin (Ledger): YOUR_LEDGER_ADDRESS
RPC: https://solana-mainnet...
Dry run: true

Protocol state PDA: 3RhGhHjdzYCCeT9QY1mdBoe8t7XkAaHH225nfQUmH4RX
Current admin (on-chain): 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
✅ Admin verified

🔍 Simulating transaction...
✅ Simulation successful!

Logs:
  Program 4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5 invoke [1]
  Program log: Instruction: UpdateAdminOpen
  Program 4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5 success

⚠️  Dry run complete. Run without --dry-run to execute.
```

### Step 2: Execute Migration

**THIS IS IRREVERSIBLE!** Once executed, only the Ledger will control admin operations.

```bash
npx tsx scripts/transfer-admin-to-ledger.ts \
  --ledger-pubkey YOUR_LEDGER_ADDRESS_FROM_STEP_2 \
  --current-admin ~/.config/solana/oracle-authority.json \
  --rpc-url https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973
```

**Expected output:**
```
=== Transfer Admin to Hardware Wallet ===
...
📤 Sending transaction...
Transaction sent: ABC123DEF456...
https://solscan.io/tx/ABC123DEF456...

⏳ Confirming...
✅ Transaction confirmed!

New admin (on-chain): YOUR_LEDGER_ADDRESS
✅ Admin successfully transferred to Ledger!

⚠️  IMPORTANT: Test admin operations with Ledger before securing old keypair.
```

### Step 3: Verify Migration

Check the protocol state to confirm the new admin:

```bash
tsx scripts/check-protocol-state.ts
```

**Expected output:**
```
Protocol State PDA: 3RhGhHjdzYCCeT9QY1mdBoe8t7XkAaHH225nfQUmH4RX
Account data length: 141
Admin pubkey: YOUR_LEDGER_ADDRESS  ← Should match!
Publisher pubkey: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy  ← Unchanged!
Paused: false

Oracle Authority: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
Match admin? false  ← Expected! Admin is now Ledger
Match publisher? true  ← Expected! Publisher still hot wallet
```

### Step 4: Test Ledger Admin Operations

**CRITICAL:** Test that you can perform admin operations with the Ledger BEFORE securing the old keypair.

Try pausing/unpausing the protocol (safe operation):

```bash
# This will require Ledger confirmation
solana program execute \
  --keypair usb://ledger?key=0/0 \
  --program-id 4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5 \
  ... (instruction data for set_paused_open)
```

If you can successfully sign with the Ledger, you're good to go!

### Step 5: Secure Old Keypair

Once you've verified Ledger admin control:

1. **Backup `oracle-authority.json`:** Copy to encrypted storage (it still controls publisher!)
2. **Document Recovery:** Write down Ledger seed phrase backup location
3. **Update Documentation:** Record new admin address in team docs

**DO NOT DELETE `oracle-authority.json`!** It's still your publisher key and needed for automated oracle operations.

## Post-Migration Operations

### Pausing the Protocol (Ledger Required)

```bash
# Requires physical Ledger confirmation
anchor run pause-protocol --provider.wallet usb://ledger?key=0/0
```

### Updating Publisher (Ledger Required)

```bash
# Requires physical Ledger confirmation
tsx scripts/update-publisher.ts \
  --admin usb://ledger?key=0/0 \
  --new-publisher NEW_HOT_WALLET_ADDRESS
```

### Publishing Roots (No Ledger - Still Uses Hot Wallet)

```bash
# Business as usual - no Ledger needed!
tsx scripts/publisher/publish-category-root.ts
```

## Rollback Plan (Emergency Only)

If you need to transfer admin back to a hot wallet:

1. **Connect Ledger** with current admin authority
2. **Run transfer script** from Ledger to new hot wallet address
3. **Use `update_admin_open`** instruction (same as original migration)

**Script example:**
```typescript
// Use Ledger as signer instead of keypair
const ledgerWallet = new LedgerWallet(derivationPath);
await program.methods.updateAdminOpen(newAdminPubkey)
  .accounts({ admin: ledgerPubkey, protocolState })
  .signers([ledgerWallet])
  .rpc();
```

## Program Upgrade Authority Migration (Optional)

Consider also migrating program upgrade authority to Ledger or multi-sig:

```bash
solana program set-upgrade-authority \
  4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5 \
  --new-upgrade-authority YOUR_LEDGER_OR_MULTISIG \
  --keypair ~/.config/solana/oracle-authority.json
```

## Troubleshooting

### Simulation Fails

**Error:** `Admin mismatch!`
**Fix:** Ensure `oracle-authority.json` still controls the admin. Check `scripts/check-protocol-state.ts`.

**Error:** `Protocol state account not found!`
**Fix:** Verify program ID and mint are correct in script.

### Ledger Connection Issues

**Error:** `hidapi error`
**Fix:** Install/update Ledger udev rules on Linux:
```bash
wget -q -O - https://raw.githubusercontent.com/LedgerHQ/udev-rules/master/add_udev_rules.sh | sudo bash
```

**Error:** `Device not found`
**Fix:**
1. Unlock Ledger
2. Open Solana app
3. Enable "Allow blind signing" in Solana app settings

### Transaction Confirmation Timeout

**Error:** `Transaction not confirmed after 60s`
**Fix:** Check transaction manually on explorer. It may have succeeded despite timeout.

## Security Best Practices

1. **Seed Phrase Security:**
   - Store Ledger seed phrase in fireproof safe
   - Use metal backup (Cryptosteel, etc.)
   - Never store digitally

2. **Operational Security:**
   - Always verify transaction details on Ledger screen
   - Use separate Ledger for different protocols
   - Keep firmware updated

3. **Key Separation:**
   - Admin = Ledger (cold storage, rare use)
   - Publisher = Hot wallet (automated, frequent use)
   - Upgrade Authority = Ledger or multi-sig (critical operations)

4. **Testing:**
   - Always use `--dry-run` first
   - Test on devnet before mainnet for complex operations
   - Verify state changes after each admin operation

## Support

If you encounter issues during migration:

1. **Check protocol state:** `tsx scripts/check-protocol-state.ts`
2. **Verify Ledger address:** `solana-keygen pubkey usb://ledger?key=0/0`
3. **Review transaction logs:** Check Solscan for detailed error messages
4. **Backup plan:** Keep `oracle-authority.json` secure until migration is fully verified

---

**Document Version:** 1.0
**Last Updated:** 2025-10-30
**Program Version:** 636KB (with `update_admin_open` instruction)
**Deployment Tx:** `tanrNtT7JbLt3aorxZLUGQTrnJkNeFeqeHKtZ6S9VVfZDWNmqCSxVR9HVYAw1hGyTS5vgpfgyiu7i2tG6SxbeSA`
