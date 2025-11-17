# Migration Guide: Portal v3

**Purpose**: Consolidate claim-ui + portal-v2 into a single, modern portal-v3 application.

---

## 📊 Before vs After

### Before (Fragmented)

```
claim-ui/                    (Anchor hooks + 4-step flow)
├── src/hooks/              (useAnchorProgram, useWallet, useMerkleProof)
├── src/components/         (ProofUpload, WalletConnect, Review, Execution)
├── src/App.tsx             (4-step orchestrator)
├── package.json            (React + Anchor + wallet-adapter)
└── idl/                    (token-2022.json)

portal-v2/                   (Static pages)
├── index.html
├── claim.html
├── dashboard.html
└── [unstructured CSS/JS]

gateway/                     (Express backend)
├── /api/verification-status
├── /api/claim-cls
└── serves claim-ui as frontend
```

**Issues:**
- Fragmented codebase (2 separate UI apps)
- Claim-ui tied to Anchor IDL (harder to extend)
- Portal-v2 static (hard to maintain)
- No unified styling/UX
- Duplicate dependencies

### After (Unified - Portal v3)

```
portal-v3/                   (Single React + Vite app)
├── src/
│   ├── components/
│   │   └── ClaimCLS.tsx    (Verification + Claiming)
│   ├── lib/
│   │   ├── api.ts          (Backend client)
│   │   └── solana.ts       (Network config)
│   ├── App.tsx             (Shell)
│   └── main.tsx            (Entry point)
├── dist/                    (Built static assets)
└── package.json            (Clean dependencies)

gateway/                     (Express backend)
├── /api/verification-status
├── /api/claim-cls
└── app.use(express.static('portal-v3/dist'))
```

**Benefits:**
- ✅ Single, unified codebase
- ✅ Clean architecture (separation of concerns)
- ✅ Easier to extend (dashboards, tiers, etc.)
- ✅ Better DX (TypeScript strict, @ alias, Vite fast builds)
- ✅ Consistent styling
- ✅ No Anchor IDL coupling

---

## 🔄 Migration Steps

### Phase 1: Build Portal v3

✅ **COMPLETE** - All files created and documented

### Phase 2: Implement Backend Endpoints (Agent B)

**Implement these endpoints:**

```
GET /api/verification-status?wallet=<pubkey>
POST /api/claim-cls
```

See `README.md` → "API Contract" section

### Phase 3: Test on Devnet

1. Set `.env`:
   ```
   VITE_SOLANA_NETWORK=devnet
   VITE_SOLANA_RPC=https://api.devnet.solana.com
   VITE_PROGRAM_ID=<devnet-program-id>
   ```

2. Run dev server:
   ```
   npm run dev
   ```

3. Test full flow:
   - Wallet connect
   - Verification status
   - Refresh verification
   - Claim transaction
   - Confirmation

### Phase 4: Deploy to Production

1. Build:
   ```
   npm run build
   ```

2. Configure gateway:
   ```typescript
   app.use(express.static('portal-v3/dist'))
   ```

3. Test at production URL
4. Monitor logs for errors

### Phase 5: Decommission Old UIs

- Archive `claim-ui/` (reference only)
- Archive `portal-v2/` (reference only)
- Remove old routes from gateway
- Update documentation

---

## 📋 File Mapping

### Claim Flow

**Old (claim-ui):**
```
ProofUpload → WalletConnect → ClaimReview → ClaimExecution
```

**New (portal-v3):**
```
ClaimCLS (all-in-one)
├── Verification tiles (Twitter, Discord)
├── Epoch selector
└── Claim button
```

**Key Difference:**
- Old: Multi-step wizard (good for educational flow)
- New: Single card (good for quick claiming)

---

### Verification

**Old (claim-ui):**
- Verification was implicit (address matching)

**New (portal-v3):**
- Explicit social verification (Twitter, Discord)
- Refreshable status checks
- Visual badges

---

### API Integration

**Old (claim-ui):**
- Direct Anchor IDL calls
- Manual instruction building

**New (portal-v3):**
- Backend-agnostic HTTP API
- Base64-encoded transactions
- Wallet adapter for signing

---

## 🔑 Key Files to Replace

| Old | New | Purpose |
|-----|-----|---------|
| claim-ui/src/App.tsx | portal-v3/src/App.tsx | Shell layout |
| claim-ui/src/components/* | portal-v3/src/components/ClaimCLS.tsx | Claim flow |
| N/A | portal-v3/src/lib/api.ts | **NEW:** Backend client |
| claim-ui/src/lib/* | portal-v3/src/lib/solana.ts | Network config |

---

## 🚀 Quick Migration Checklist

- [ ] Portal v3 created with all files
- [ ] Dependencies installed (`npm install`)
- [ ] Dev server works (`npm run dev`)
- [ ] Type checking passes (`npm run type-check`)
- [ ] Build successful (`npm run build`)
- [ ] dist/ generated with index.html + assets
- [ ] Backend endpoints implemented (/api/*)
- [ ] Gateway configured to serve dist/
- [ ] SPA catch-all route added
- [ ] Tested on devnet
- [ ] Tested on mainnet
- [ ] Old UIs archived
- [ ] Documentation updated

---

## 🧪 Testing Migration

### Before Deploying

1. **Local Testing**
   ```bash
   cd portal-v3
   npm install
   npm run dev
   # Test in browser at http://localhost:3000
   ```

2. **Build Testing**
   ```bash
   npm run build
   npm run preview
   # Test built version at http://localhost:4173
   ```

3. **Backend Integration**
   ```bash
   # Start gateway with static serving
   node gateway/index.js
   # Test at http://localhost:5000
   ```

4. **Devnet Testing**
   - Update .env to devnet
   - Rebuild
   - Connect Phantom to devnet
   - Test full claim flow
   - Verify tokens received

5. **Mainnet Testing**
   - Update .env to mainnet
   - Rebuild
   - Test with real tokens
   - Monitor for errors

---

## 📝 Configuration Changes

### Before (claim-ui)

```typescript
// claim-ui/src/lib/constants.ts
PROGRAM_ID = GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
RPC_URL = https://api.mainnet-beta.solana.com
```

### After (portal-v3)

```typescript
// portal-v3/.env
VITE_SOLANA_NETWORK=mainnet-beta
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com
VITE_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

---

## 🔗 API Contract Changes

### Claim Transaction Building

**Before (claim-ui):**
```typescript
// Direct Anchor IDL call
const tx = await program.methods
  .claimWithRing(epoch, index, amount, proof, streamerKey)
  .accounts({ ... })
  .transaction()
```

**After (portal-v3):**
```typescript
// Backend builds transaction
const response = await fetch('/api/claim-cls', {
  method: 'POST',
  body: JSON.stringify({ wallet, epochId })
})
const { transaction } = await response.json()
// transaction is base64-encoded
```

---

## 📚 Documentation Updates

Update these files in your docs:

1. **README.md** - Point to portal-v3
2. **SETUP.md** - Use `npm run dev` in portal-v3
3. **DEPLOYMENT.md** - Update build/serve instructions
4. **API.md** - Document new /api/verification-status and /api/claim-cls

---

## 🎯 Benefits of Migration

| Aspect | Before | After |
|--------|--------|-------|
| **Codebase** | Fragmented | Unified |
| **Build Tool** | Vite (claim-ui) | Vite (all) |
| **Type Safety** | Partial | Full (TypeScript strict) |
| **Extensibility** | Medium | High |
| **Styling** | Inline (claim-ui) | Consistent |
| **API Coupling** | High (Anchor IDL) | Low (REST) |
| **Dev Experience** | Good | Excellent |
| **Bundle Size** | Large (Anchor) | Smaller |

---

## ⚠️ Migration Risks

### Risk 1: Backend Endpoints Not Ready

**Impact:** Portal won't work
**Mitigation:** Implement /api/verification-status and /api/claim-cls before deploying

### Risk 2: Network Configuration Mismatch

**Impact:** Claiming fails
**Mitigation:** Double-check VITE_SOLANA_RPC and VITE_PROGRAM_ID

### Risk 3: Gateway Static Serving Broken

**Impact:** Portal v3 doesn't load
**Mitigation:** Test gateway static serving before deploying

### Risk 4: Old Routes Still Active

**Impact:** Users confused by multiple portals
**Mitigation:** Remove old routes after testing new portal

---

## 🔄 Rollback Plan

If issues arise:

1. Keep `claim-ui/dist/` for fallback
2. Update gateway to serve old portal:
   ```typescript
   app.use(express.static('claim-ui/dist'))
   ```
3. Debug portal-v3 issues
4. Redeploy once fixed

---

## 📞 Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| "Cannot GET /" | SPA catch-all missing | Add gateway route |
| 404 on /api/* | Backend endpoints missing | Implement endpoints |
| Blank page | dist/ not built | Run `npm run build` |
| Wallet not connecting | RPC endpoint wrong | Check VITE_SOLANA_RPC |

---

## ✅ Success Criteria

✓ Portal v3 loads at gateway URL
✓ Wallet connects successfully
✓ Verification status loads
✓ Claim transaction completes
✓ No console errors
✓ Tokens received in wallet
✓ No old portals still active

---

**Status**: Ready to migrate

**Timeline**: 1-2 weeks (depends on backend endpoint implementation)

**Owner**: Agent B (backend) + Agent C (UI)
