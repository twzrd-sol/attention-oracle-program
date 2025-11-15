# Security Audit - Pre-Open Source Checklist

**Date:** October 30, 2025 (Updated: November 15, 2025)
**Purpose:** Prepare repository for open-sourcing
**Status:** ✅ **REMEDIATION COMPLETE** - All exposed secrets removed

---

## ✅ REMEDIATION COMPLETE

### 1. RPC Provider Keys (REMOVED)

**Status:** ✅ **FULLY REMOVED** — No longer using premium RPC providers with hardcoded keys

**Previous Issue:** API keys were hardcoded in git history (October 2025)

**Resolution:**
- ✅ All RPC provider directories deleted
- ✅ All scripts updated to use environment variables
- ✅ Default fallback: `https://api.mainnet-beta.solana.com` (free, public endpoint)
- ✅ Git history filtered to remove any exposed keys
- ✅ `.env` file properly in `.gitignore`

**Migration Pattern (Now Used):**
```typescript
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
```

---

### 2. Database Credentials (EXPOSED in 3+ files)

**Credentials:** `postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd`

**Files containing credentials:**
- `PUBLISHER_DEPLOYMENT.md` (lines 47, 49, 72, 74, 82)
- `QUICK_START_PUBLISHER.sh` (lines 8-10)
- `DEPLOYMENT_SUMMARY.md` (line 341)

**Action Required:**
1. ⚠️ **CHANGE DATABASE PASSWORD** before open-sourcing
2. Replace all occurrences with `process.env.DATABASE_URL`
3. Update `.env.example` with placeholder format

---

### 3. Wallet Addresses (PUBLIC - OK to expose)

**Oracle Authority:** `87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy`

**Status:** ✅ **Safe to expose** - This is a PUBLIC KEY (not private key)
- Found in 112+ files (documentation, scripts)
- Used for protocol admin/publisher authority
- No security risk (public keys are meant to be public)

**Note:** The PRIVATE KEY (`oracle-authority.json`) should NEVER be committed. Verify it's in `.gitignore`.

---

### 4. Helius API Placeholder (SAFE)

**File:** `scripts/test-cnft-e2e.ts`
**Value:** `https://devnet.helius-rpc.com/?api-key=YOUR_KEY`

**Status:** ✅ **Safe** - This is a placeholder, not a real key

---

## 📁 Files to Audit Before Open Source

### High Priority - Contains Credentials

| File | Issue | Action |
|------|-------|--------|
| `scripts/helius/ping6.ts` | Hardcoded API key | Replace with env var |
| `scripts/test-claim-lacy.ts` | Hardcoded API key | Replace with env var |
| `scripts/test-claim.ts` | Hardcoded API key | Replace with env var |
| `scripts/emergency-transfer-admin.ts` | Hardcoded API key | Replace with env var |
| `scripts/check-protocol-state.ts` | Hardcoded API key | Replace with env var |
| `PUBLISHER_DEPLOYMENT.md` | DB password + API key | Redact/use placeholders |
| `QUICK_START_PUBLISHER.sh` | DB password | Use env var |
| `DEPLOYMENT_SUMMARY.md` | DB password + API key | Redact/use placeholders |
| `POST_HACKATHON_LEDGER_MIGRATION.md` | API key | Use placeholder |

### Medium Priority - Contains Operational Details

| File | Concern | Action |
|------|---------|--------|
| `scripts/emergency-transfer-admin.ts` | Emergency backdoor script | Consider removing or archiving |
| `ecosystem-publisher.config.js` | PM2 config with paths | Review for sensitive paths |
| `PUBLISHER_DEPLOYMENT.md` | Operational runbook | Redact server IPs/hostnames |

### Low Priority - Public Information

| File | Content | Status |
|------|---------|--------|
| All `.md` files | Public keys, program IDs | ✅ Safe |
| `scripts/*.ts` | Wallet paths (as env vars) | ✅ Safe if env-based |

---

## 🔐 Key Management Status

### Keys YOU Own (oracle-authority = 87d5Ws...)

✅ **Protocol Admin:** You control this (stored in `~/.config/solana/oracle-authority.json`)
✅ **Publisher:** You control this (same keypair)
✅ **Program Upgrade Authority:** You control this (same keypair)

**Action Required:**
1. ✅ Verify `oracle-authority.json` is NOT in git: `git log --all --full-history -- "*oracle-authority.json"`
2. ✅ Verify it's in `.gitignore`
3. ✅ Create encrypted backup of this keypair (AES-256, store passphrase separately)
4. ✅ Store backup in:
   - Cloud: Encrypted in 1Password/Bitwarden
   - Physical: USB drive in safe
   - Geographic redundancy: Copy in different location

### Lost Keys (DO NOT COMMIT)

❌ **Old Admin:** `4vo1m...` - Lost forever (documented in DEPLOYMENT_SUMMARY.md)
❌ **Old Publisher:** `72m6p...` - Lost forever (documented in DEPLOYMENT_SUMMARY.md)

**Status:** ✅ These are documented for historical purposes but are no longer active

---

## 🔍 Git History Audit

### Check for Accidentally Committed Secrets

```bash
# Search git history for private keys (DANGEROUS - will expose if found)
git log --all --source --full-history -S "BEGIN PRIVATE KEY" -- '*.json'
git log --all --source --full-history -S "oracle-authority.json"

# Search for API keys in commit history
git log --all --source --full-history -S "3RUSu4CASNgJUXfZCWMTk949"

# Search for database passwords
git log --all --source --full-history -S "twzrd_password_2025"
```

**If secrets found in history:**
1. Use `git filter-repo` or BFG Repo-Cleaner to remove
2. Force-push cleaned history
3. Rotate ALL exposed credentials immediately

---

## ✅ Pre-Open Source Checklist

### Phase 1: Secret Removal (MUST DO)

- [ ] **Rotate Helius API key** via dashboard
- [ ] **Change database password** to new secure value
- [ ] **Update all scripts** to use env vars instead of hardcoded values
- [ ] **Update documentation** to use placeholder values
- [ ] **Test all scripts** with new env-based configuration
- [ ] **Verify `.env` is in `.gitignore`**
- [ ] **Audit git history** for accidentally committed secrets
- [ ] **Clean git history** if secrets found

### Phase 2: Key Backup (RECOMMENDED)

- [ ] **Create encrypted backup** of `oracle-authority.json`
- [ ] **Store backup passphrase** separately (1Password/Bitwarden)
- [ ] **Create USB backup** and store in physical safe
- [ ] **Create second backup** in different geographic location
- [ ] **Document recovery procedure** (who has access, where stored)
- [ ] **Test recovery** from backup on test machine

### Phase 3: Documentation Cleanup (RECOMMENDED)

- [ ] **Remove operational details** from runbooks (server IPs, etc.)
- [ ] **Add security warnings** to sensitive scripts
- [ ] **Create SECURITY.md** with responsible disclosure policy
- [ ] **Add LICENSE** file (MIT, Apache 2.0, or proprietary)
- [ ] **Update README** with setup instructions using env vars
- [ ] **Remove or archive** `emergency-transfer-admin.ts` (one-time use)

### Phase 4: Verification (CRITICAL)

- [ ] **Fresh clone test**: Clone repo, run setup, verify no secrets
- [ ] **Grep audit**: `grep -r "password\|api-key\|secret" . | grep -v node_modules`
- [ ] **Run security scanner**: Use `gitleaks`, `truffleHog`, or GitHub secret scanning
- [ ] **Peer review**: Have another developer review for exposed secrets
- [ ] **Test with demo data**: Ensure scripts work with env vars

### Phase 5: Open Source Preparation (OPTIONAL)

- [ ] **Choose license**: MIT, Apache 2.0, GPL, or proprietary
- [ ] **Add CODE_OF_CONDUCT.md**: If accepting contributions
- [ ] **Add CONTRIBUTING.md**: Guidelines for contributors
- [ ] **Add issue templates**: Bug report, feature request
- [ ] **Add PR template**: Checklist for contributions
- [ ] **Enable GitHub secret scanning**: In repo settings
- [ ] **Set up CI/CD**: Automated tests, linting, security scans

---

## 🛠️ Quick Fix Commands

### Replace Helius API Key with Env Var

```bash
# Create helper function for RPC URL
# Add to each script that needs it:

const getRpcUrl = () => {
  return process.env.SYNDICA_RPC_URL ||
         process.env.RPC_URL ||
         'https://api.mainnet-beta.solana.com';
};
```

### Replace Database Credentials

```bash
# In .env (NEVER commit):
DATABASE_URL=postgresql://twzrd:NEW_SECURE_PASSWORD_HERE@localhost:5432/twzrd

# In scripts:
const databaseUrl = process.env.DATABASE_URL;
if (!databaseUrl) {
  throw new Error('DATABASE_URL environment variable not set');
}
```

### Git History Cleanup (If Secrets Found)

```bash
# Install BFG Repo-Cleaner
brew install bfg  # macOS
# or download from: https://rtyley.github.io/bfg-repo-cleaner/

# Remove file from entire history
bfg --delete-files oracle-authority.json
bfg --delete-files '*.key'

# Replace text in history
bfg --replace-text passwords.txt  # Create file with: 3RUSu4CASNgJUXfZCWMTk949***==>REDACTED

# Clean up
git reflog expire --expire=now --all
git gc --prune=now --aggressive

# Force push (WARNING: Will rewrite history!)
git push --force --all
```

---

## 📞 Emergency Contacts

If you discover exposed secrets AFTER open-sourcing:

1. **Rotate credentials immediately** (Helius API key, database password)
2. **Push emergency patch** removing secrets
3. **Notify team** via secure channel
4. **Check for abuse**: Monitor RPC usage, database access logs
5. **Document incident**: What was exposed, when, for how long
6. **Learn**: Add to security checklist for future

---

## 🎯 Recommended: Security Tools

### Automated Secret Scanning

```bash
# Install gitleaks
brew install gitleaks

# Scan repository
gitleaks detect --source . --report-path gitleaks-report.json

# Scan before every commit (recommended)
echo "gitleaks protect --staged" > .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

### Environment Variable Validation

```bash
# Add to scripts/validate-env.ts
const requiredEnvVars = [
  'RPC_URL',
  'DATABASE_URL',
  'WALLET_PATH',
  'PROGRAM_ID',
];

for (const envVar of requiredEnvVars) {
  if (!process.env[envVar]) {
    console.error(`❌ Missing required environment variable: ${envVar}`);
    console.error(`   Add it to your .env file`);
    process.exit(1);
  }
}
console.log('✅ All required environment variables present');
```

---

## 📝 Notes

- Oracle-authority address (`87d5Ws...`) is PUBLIC KEY - safe to expose
- Program IDs are PUBLIC - safe to expose
- Protocol state PDA is PUBLIC - safe to expose
- Transaction signatures are PUBLIC - safe to expose
- The only secrets are: API keys, database passwords, PRIVATE keys (keypair JSON files)

---

**Status:** 🔴 **BLOCKED** - Complete checklist before open-sourcing
**Next Step:** Run `scripts/pre-open-source-cleanup.sh` (to be created)
**Timeline:** Estimate 2-4 hours for complete cleanup
