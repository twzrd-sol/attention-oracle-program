# 🔍 Keypair Usage Audit - CLS Scripts

**Audit Date:** 2025-11-07
**Objective:** Identify scripts that default to `id.json` and add explicit keypair requirements

---

## ⚠️ Scripts Defaulting to id.json (NEEDS FIXING)

These scripts will use `~/.config/solana/id.json` (2pHjZ...) if no env var is set:

### High Priority (Used Frequently)

1. **set-publisher-mainnet.ts**
   ```typescript
   const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR || `${process.env.HOME}/.config/solana/id.json`;
   ```
   **Risk:** Updates publisher authority using wrong wallet
   **Fix:** Require explicit `ADMIN_KEYPAIR` env var

2. **init-protocol-open.ts**
   ```typescript
   const ADMIN_KEYPAIR = process.env.ADMIN_KEYPAIR || process.env.PAYER_KEYPAIR || path.join(process.env.HOME || '', '.config/solana/id.json')
   ```
   **Risk:** Protocol initialization with wrong admin
   **Fix:** Require explicit `ADMIN_KEYPAIR`

3. **anchor-milo-root.ts**
   - Defaults to id.json
   **Fix:** Add explicit keypair requirement

4. **claim-airdrop.ts**
   - Defaults to id.json
   **Fix:** Add explicit keypair requirement

5. **emergency-pause.ts**
   - Defaults to id.json
   **Risk:** HIGH - Emergency operations must use correct admin
   **Fix:** URGENT - require explicit admin keypair

### Medium Priority (Admin Operations)

6. **create-and-fund-treasury.ts** - Defaults to id.json
7. **create-ctw-points-mint.ts** - Defaults to id.json
8. **create-points-mint.ts** - Defaults to id.json
9. **create-test-epoch.ts** - Defaults to id.json (test only, OK)
10. **create-test-mints.ts** - Defaults to id.json (test only, OK)
11. **fund-new-treasury.ts** - Defaults to id.json
12. **init-channels-mainnet.ts** - Defaults to id.json

---

## ✅ Scripts with Explicit Keypair (GOOD)

These scripts require explicit env vars or have safe defaults:

1. **publish-root-mainnet.ts**
   ```typescript
   const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR || process.env.HOME + '/milo-token/keys/admin-keypair.json';
   ```
   ✅ Defaults to AmMf... (admin-keypair.json), not id.json

2. **init-top-3-channels.ts**
   ```typescript
   ADMIN_KEYPAIR: '/home/twzrd/.config/solana/oracle-authority.json'
   ```
   ✅ Hardcoded to oracle-authority

3. **set-publisher-singleton.ts**
   ```typescript
   const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR || `${process.env.HOME}/milo-token/keys/admin-keypair.json`;
   ```
   ✅ Defaults to AmMf... (admin-keypair.json)

---

## 🛠️ Recommended Fixes

### Pattern 1: Require Explicit Env Var (Safest)

```typescript
// BEFORE (unsafe)
const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR || `${process.env.HOME}/.config/solana/id.json`;

// AFTER (safe)
const ADMIN_KEYPAIR_PATH = process.env.ADMIN_KEYPAIR;
if (!ADMIN_KEYPAIR_PATH) {
  console.error('❌ ADMIN_KEYPAIR environment variable is required');
  console.error('Usage: ADMIN_KEYPAIR=/path/to/keypair.json npm run script');
  process.exit(1);
}
```

### Pattern 2: Add "Whoami" Log Before Transactions

```typescript
const admin = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(fs.readFileSync(ADMIN_KEYPAIR_PATH, 'utf8')))
);

console.log('🔑 Signing with:', admin.publicKey.toBase58());
console.log('   Keypair:', ADMIN_KEYPAIR_PATH);

// Confirm before on-chain operations
const rl = readline.createInterface({
  input: process.stdin,
  output: process.stdout
});

await new Promise(resolve => {
  rl.question('Continue with this wallet? (y/N): ', (answer) => {
    if (answer.toLowerCase() !== 'y') {
      console.log('Aborted');
      process.exit(0);
    }
    rl.close();
    resolve(undefined);
  });
});
```

### Pattern 3: Safe Default for Specific Operations

```typescript
// For publish operations (routine, automated)
const PAYER_KEYPAIR = process.env.PAYER_KEYPAIR || '/home/twzrd/.config/solana/oracle-authority.json';

// For admin operations (rare, manual)
const ADMIN_KEYPAIR = process.env.ADMIN_KEYPAIR;
if (!ADMIN_KEYPAIR) {
  throw new Error('ADMIN_KEYPAIR required for admin operations');
}
```

---

## 📋 Action Items

### Immediate (Before Next Use)

- [ ] **emergency-pause.ts** - URGENT: Require explicit ADMIN_KEYPAIR
- [ ] **set-publisher-mainnet.ts** - Require explicit ADMIN_KEYPAIR
- [ ] **init-protocol-open.ts** - Require explicit ADMIN_KEYPAIR

### Short-term (This Week)

- [ ] Add "whoami" logs to all scripts that sign transactions
- [ ] Update remaining admin scripts to require explicit keypairs
- [ ] Add interactive confirmation for irreversible operations

### Long-term (Before Production Scale)

- [ ] Migrate to multisig for protocol admin
- [ ] Implement transaction approval workflow
- [ ] Add audit logging for all on-chain operations

---

## 🚨 Safety Checklist

Before running any script:

1. ✅ Check what keypair it will use:
   ```bash
   grep "ADMIN_KEYPAIR\|PAYER_KEYPAIR" script.ts
   ```

2. ✅ Set explicit keypair if needed:
   ```bash
   export ADMIN_KEYPAIR=/path/to/correct/keypair.json
   ```

3. ✅ Verify wallet has sufficient balance:
   ```bash
   solana balance $(solana-keygen pubkey $ADMIN_KEYPAIR) --url mainnet-beta
   ```

4. ✅ Dry-run if possible (devnet first)

5. ✅ Have emergency contacts ready if something goes wrong

---

## 📝 Script Usage Guidelines

### For Automated Publishing (PM2 Services)
```bash
# Always use oracle-authority
PAYER_KEYPAIR=/home/twzrd/.config/solana/oracle-authority.json
```

### For Admin Operations (Manual)
```bash
# Explicitly set admin keypair
ADMIN_KEYPAIR=/home/twzrd/.config/solana/id.json  # (2pHjZ... - protocol admin)
# OR
ADMIN_KEYPAIR=/home/twzrd/milo-token/keys/admin-keypair.json  # (AmMf... - legacy admin)
```

### For Testing (Devnet)
```bash
# OK to use defaults on devnet
ADMIN_KEYPAIR=/home/twzrd/milo-token/keys/publisher-devnet.json
```

---

## 🔄 Migration Path

### Phase 1: Immediate Safety (This Session) ✅
- [x] Document wallet roles (WALLET_MAP.md)
- [x] Audit keypair usage (this file)
- [ ] Fix critical scripts (emergency-pause, set-publisher)

### Phase 2: Short-term Hardening (This Week)
- [ ] Add explicit keypair requirements to all admin scripts
- [ ] Add "whoami" logs and confirmations
- [ ] Test emergency procedures

### Phase 3: Production Hardening (Before Scale)
- [ ] Implement multisig for protocol admin
- [ ] Add transaction approval workflow
- [ ] Set up monitoring/alerting for unauthorized keypair usage

---

Last Updated: 2025-11-07
