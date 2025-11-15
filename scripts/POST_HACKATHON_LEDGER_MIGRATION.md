# Post-Hackathon: Migrate to Hardware Wallet (Ledger)

## Goal
Move protocol admin authority from software keypair (oracle-authority.json) to hardware wallet (Ledger) for production security.

---

## Prerequisites

- [ ] Ledger device (Nano S Plus / Nano X)
- [ ] Ledger Live installed
- [ ] Solana app installed on Ledger
- [ ] Ledger firmware up to date
- [ ] Emergency upgrade deployed (oracle-authority.json is current admin)

---

## Step 1: Get Ledger Public Key

```bash
# Connect Ledger, unlock, open Solana app
# Get first derivation path address
solana-keygen pubkey usb://ledger?key=0/0

# Save this address - you'll use it for transfer
export LEDGER_PUBKEY=$(solana-keygen pubkey usb://ledger?key=0/0)
echo "Ledger pubkey: $LEDGER_PUBKEY"

# Verify you can sign with it
solana balance $LEDGER_PUBKEY --url mainnet-beta
```

---

## Step 2: Fund Ledger Address (Small Amount)

```bash
# Send 0.1 SOL to cover transaction fees
solana transfer $LEDGER_PUBKEY 0.1 \
  --from ~/.config/solana/oracle-authority.json \
  --url mainnet-beta

# Confirm it arrived
solana balance $LEDGER_PUBKEY --url mainnet-beta
```

---

## Step 3: Transfer Protocol Admin to Ledger

```bash
cd /home/twzrd/milo-token

# Run admin transfer (requires oracle-authority.json as current admin)
npx tsx scripts/transfer-admin-to-ledger.ts \
  --ledger-pubkey $LEDGER_PUBKEY \
  --current-admin ~/.config/solana/oracle-authority.json \
  --rpc-url https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973
```

Expected output:
```
✅ Current admin verified: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy
✅ Transferring to: <LEDGER_PUBKEY>
✅ Transaction sent: <signature>
✅ Confirmed!
✅ New admin: <LEDGER_PUBKEY>
```

---

## Step 4: Verify Transfer

```bash
# Check on-chain state
npx tsx scripts/check-protocol-state.ts

# Should show:
# Admin: <LEDGER_PUBKEY> (your Ledger)
# Publisher: 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy (oracle-authority)
```

---

## Step 5: Test Admin Functions with Ledger

```bash
# Test updating publisher (requires Ledger signature)
npx tsx scripts/test-ledger-admin.ts \
  --ledger-path usb://ledger?key=0/0 \
  --action update_publisher \
  --new-publisher 87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy

# Ledger will prompt: "Sign transaction?"
# Approve on device
# ✅ Success
```

---

## Step 6: Backup Ledger Recovery Phrase

⚠️ CRITICAL SECURITY STEP

1. Write down 24-word recovery phrase
2. Store in 3 separate physical locations:
   - Safe deposit box
   - Fireproof home safe
   - Trusted family member (sealed envelope)
3. NEVER store digitally (no photos, no cloud, no text files)

---

## Step 7: Secure Old Software Keypair

```bash
# Encrypt oracle-authority.json
gpg --symmetric --cipher-algo AES256 ~/.config/solana/oracle-authority.json

# Creates: oracle-authority.json.gpg

# Delete plaintext
shred -vfz -n 10 ~/.config/solana/oracle-authority.json

# Store encrypted backup in 3 locations:
# 1. External drive (offline)
# 2. Cloud storage (encrypted)
# 3. Paper backup (QR code + hex)
```

---

## Step 8: Update Operational Procedures

### For Publisher Operations (Daily):
- Publisher still uses oracle-authority.json (hot wallet)
- No Ledger needed for routine publishing

### For Admin Operations (Rare):
- Require Ledger connected
- Examples: update publisher, pause protocol, change fees

### For Program Upgrades (Rare):
- Still use oracle-authority.json (upgrade authority)
- Consider moving upgrade authority to Ledger later

---

## Architecture After Migration

```
┌─────────────────────────────────────────┐
│ Protocol Admin (Cold Storage)           │
│ Ledger: <LEDGER_PUBKEY>                 │
│ - update_publisher()                    │
│ - pause()                               │
│ - update_fees()                         │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Publisher (Hot Wallet)                  │
│ oracle-authority.json (87d5W...)        │
│ - set_merkle_root_ring()                │
│ - Runs 24/7 on server                   │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Program Upgrade Authority (Warm Storage)│
│ oracle-authority.json (87d5W...)        │
│ - program deploy                        │
│ - Only use for upgrades                 │
└─────────────────────────────────────────┘
```

---

## Scripts to Create

### 1. `scripts/transfer-admin-to-ledger.ts`

```typescript
#!/usr/bin/env tsx
import { Connection, Keypair, PublicKey } from '@solana/web3.js'
import fs from 'fs'
import { program } from 'commander'

program
  .requiredOption('--ledger-pubkey <pubkey>')
  .requiredOption('--current-admin <path>')
  .requiredOption('--rpc-url <url>')
  .parse()

const opts = program.opts()

async function main() {
  const connection = new Connection(opts.rpcUrl, 'confirmed')
  const ledgerPubkey = new PublicKey(opts.ledgerPubkey)
  const currentAdmin = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(opts.currentAdmin, 'utf-8')))
  )

  console.log('Current admin:', currentAdmin.publicKey.toBase58())
  console.log('Transferring to:', ledgerPubkey.toBase58())

  // Call update_protocol with new admin
  // (Implementation depends on your program's update_protocol instruction)

  console.log('✅ Admin transferred to Ledger')
}

main().catch(console.error)
```

### 2. `scripts/test-ledger-admin.ts`

```typescript
#!/usr/bin/env tsx
// Test admin operations with Ledger
// Requires Ledger connected and unlocked
```

### 3. `scripts/emergency-recover-from-ledger.ts`

```typescript
#!/usr/bin/env tsx
// Emergency procedure if Ledger is lost
// Uses backup recovery phrase to restore admin access
```

---

## Emergency Recovery Procedure

If Ledger is lost/broken:

1. Get new Ledger device
2. Restore using 24-word recovery phrase
3. Verify same public key: `solana-keygen pubkey usb://ledger?key=0/0`
4. Admin access restored (same public key = same on-chain authority)

---

## Cost Analysis

| Item | Cost | Frequency |
|------|------|-----------|
| Ledger Nano S Plus | $79 | One-time |
| Safe deposit box | $50/year | Annual |
| Admin transactions | ~0.001 SOL | Rare |

**ROI:** Protects protocol worth potentially millions. Essential for production.

---

## Timeline

- **During hackathon:** Use oracle-authority.json (software keypair)
- **Day 1 post-hackathon:** Order Ledger (arrives in 3-5 days)
- **Day 7 post-hackathon:** Receive Ledger, complete migration
- **Day 8 post-hackathon:** Backup recovery phrase (3 locations)
- **Day 9 post-hackathon:** Test admin operations with Ledger
- **Day 10 post-hackathon:** Secure old keypair, update docs

---

## Checklist

### Pre-Migration
- [ ] Ledger device purchased
- [ ] Ledger firmware updated
- [ ] Solana app installed
- [ ] Recovery phrase written down (24 words)
- [ ] Recovery phrase stored in 3 locations

### Migration
- [ ] Get Ledger public key
- [ ] Fund Ledger address (0.1 SOL)
- [ ] Transfer admin authority
- [ ] Verify on-chain state
- [ ] Test admin operations

### Post-Migration
- [ ] Encrypt old keypair
- [ ] Delete plaintext keypair
- [ ] Store encrypted backups (3 locations)
- [ ] Update operational procedures
- [ ] Document Ledger derivation path
- [ ] Test emergency recovery procedure

---

## Reference: Ledger Commands

```bash
# List Ledger devices
solana-keygen pubkey usb://ledger --list

# Get pubkey at derivation path 0/0
solana-keygen pubkey usb://ledger?key=0/0

# Sign transaction with Ledger
solana transfer <recipient> <amount> \
  --keypair usb://ledger?key=0/0 \
  --url mainnet-beta

# Check balance
solana balance usb://ledger?key=0/0 --url mainnet-beta
```

---

## Security Best Practices

1. **Never expose seed phrase digitally**
   - No photos
   - No typing on internet-connected devices
   - No cloud storage

2. **Test recovery before trusting**
   - Set up Ledger
   - Write down seed
   - Wipe Ledger
   - Restore from seed
   - Verify same addresses

3. **Redundant backups**
   - 3+ physical locations
   - Different geographic regions
   - Protected from fire/water/theft

4. **Operational security**
   - Keep Ledger firmware updated
   - Verify transactions on device screen
   - Never approve unknown transactions

---

**Status:** Ready to implement post-hackathon (7-10 days after submission)
