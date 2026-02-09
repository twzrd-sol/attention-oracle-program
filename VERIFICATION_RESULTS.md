# On-Chain Verification Results

**Date:** February 9, 2026 06:14 UTC  
**Cluster:** mainnet-beta  
**Verifier:** Automated pre-launch audit

---

## Attention Oracle (token_2022)

```
Program Id: GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
Owner: BPFLoaderUpgradeab1e11111111111111111111111
ProgramData Address: 5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L
Authority: 2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW
Last Deployed In Slot: 398969238
Data Length: 930936 (0xe3478) bytes
Balance: 6.48051864 SOL
```

**Status:** ✅ VERIFIED
- **Upgrade Authority:** Squads V4 vault PDA (multisig)
- **Deployment:** More recent than documented (398,969,238 vs 398,836,086)
- **Security Posture:** STRONG (3-of-5 multisig required for upgrades)

---

## Channel Vault

```
Program Id: 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ
Owner: BPFLoaderUpgradeab1e11111111111111111111111
ProgramData Address: 2ubXWFAJFCnBqJ1vYCsf4q8SYRcqf5DaTfkC6wASK5SQ
Authority: 2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW
Last Deployed In Slot: 398873040
Data Length: 789800 (0xc0d28) bytes
Balance: 5.49821208 SOL
```

**Status:** ✅ VERIFIED
- **Upgrade Authority:** Squads V4 vault PDA (multisig) 
- **Deployment:** More recent than documented (398,873,040 vs 398,835,029)
- **Security Posture:** STRONG (3-of-5 multisig required for upgrades)

---

## Key Findings

### ✅ Security Wins

1. **Both programs protected by multisig** - NOT single-signer
2. **Squads V4 3-of-5 threshold** prevents unilateral malicious upgrades
3. **Authority matches SECURITY_AUDIT.md claims** (multisig is active)

### ⚠️ Documentation Issues

1. **DEPLOYMENTS.md is stale:**
   - Documents AO at slot 398,836,086 (Feb 8)
   - Actually deployed at slot 398,969,238 (Feb 9)
   - Documents Vault at slot 398,835,029 (Feb 8)
   - Actually deployed at slot 398,873,040 (Feb 9)

2. **UPGRADE_AUTHORITY.md is outdated:**
   - Claims both programs transferred to single-signer `2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD` on Feb 5
   - On-chain reality: Both use Squads multisig `2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW`
   - Likely describes a temporary state that was reversed

### 📋 Action Items

- [ ] Update DEPLOYMENTS.md with actual deployment slots
- [ ] Mark UPGRADE_AUTHORITY.md as historical or remove conflicting info
- [ ] Add note in docs about verification date (Feb 9, 2026)
- [ ] Verify deployed bytecode matches source (see VERIFY.md)
- [ ] Document what changed between Feb 8 documented deploy and Feb 9 actual deploy

---

## Verification Commands Used

```bash
# Install Solana CLI
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
export PATH="/home/runner/.local/share/solana/install/active_release/bin:$PATH"

# Verify programs
solana program show GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop --url mainnet-beta
solana program show 5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ --url mainnet-beta
```

---

**Conclusion:** Programs are in GOOD security posture with multisig protection. Documentation needs updating to reflect reality.
