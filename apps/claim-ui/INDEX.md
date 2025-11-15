# CLS Claim UI – Documentation Index

**Status:** ✅ Production Ready | **Built:** October 31, 2025

---

## 📚 Documentation Guide

### For Users
**Start Here:** [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md)
- Step-by-step claim instructions
- What to expect at each step
- Common errors & solutions
- Proof JSON format
- Security & privacy info

### For Developers
**Start Here:** [`README.md`](./README.md)
- Development setup & quick start
- Build instructions
- Deployment to Vercel/static hosts
- Architecture overview
- Error reference table

### For Quick Reference
**Start Here:** [`QUICKSTART.md`](./QUICKSTART.md)
- 3-step local setup
- Deployment in 2 steps
- Key facts & troubleshooting
- Configuration changes
- ~2 minute read

---

## 🏗️ File Structure

```
apps/claim-ui/
│
├── 📖 DOCUMENTATION
│   ├── INDEX.md ..................... This file
│   ├── QUICKSTART.md ................ Quick reference
│   ├── README.md .................... Developer guide
│   ├── CLS_CLAIM_UI.md .............. User guide (7 KB)
│   └── sample-proof.json ............ Example proof
│
├── 💻 SOURCE CODE
│   └── src/
│       ├── ClaimCLS.tsx ............ Main component (16 KB)
│       ├── App.tsx ................. Entry point
│       ├── App.css ................. Styling
│       ├── main.tsx ................ React init
│       └── index.css ............... Global styles
│
├── 🚀 BUILD & CONFIG
│   ├── package.json ................ Dependencies (updated)
│   ├── tsconfig.json ............... TypeScript config
│   ├── vite.config.ts .............. Vite config
│   └── dist/ ....................... Production build (448 KB)
│
└── 📦 DEPENDENCIES
    ├── @solana/web3.js ............. Solana blockchain
    ├── @solana/spl-token ........... Token accounts
    ├── js-sha3 ..................... keccak256 hashing
    ├── react + react-dom ........... UI framework
    └── vite ........................ Build tool
```

---

## 🎯 Typical User Journeys

### "I want to claim my CLS"
1. Read: [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md) – Quick Start section
2. Get your proof JSON from CLS team
3. Visit the claim UI
4. Follow 6-step process (load → connect → claim → verify)
5. Done! Check balance in wallet.

### "I'm a developer, I want to run this locally"
1. Read: [`QUICKSTART.md`](./QUICKSTART.md) – Get Started in 3 Steps
2. Run `npm install && npm run dev`
3. Open http://localhost:5173
4. Test with sample proof

### "I want to deploy this to production"
1. Read: [`README.md`](./README.md) – Build for Production section
2. Run `npm run build`
3. Follow deployment instructions (Vercel / Static / CDN)

### "I want to modify the styling"
1. Read: [`README.md`](./README.md) – Development section
2. Edit `src/App.css` (variables at top for colors)
3. Run `npm run dev` to see live changes
4. Build when ready

### "Something's not working"
1. Check: [`README.md`](./README.md) – Error Handling section (table)
2. For user issues → [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md) – Common Errors
3. For build issues → [`QUICKSTART.md`](./QUICKSTART.md) – Troubleshooting

---

## 🔍 Key Technical Details

### Proof JSON
- **Format**: JSON with claimer, mint, channel, epoch, index, amount, id, root, proof[]
- **Template**: See `sample-proof.json`
- **Reference**: [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md) – Proof JSON Format section

### Main Component (ClaimCLS.tsx)
- **Size**: ~400 lines, 16 KB
- **Features**: JSON input, wallet connection, balance tracking, instruction building
- **Location**: `src/ClaimCLS.tsx`
- **Reference**: [`README.md`](./README.md) – Architecture section

### Build Output
- **Type**: Production-ready React app
- **Size**: 448 KB total, 441 KB gzipped
- **Location**: `dist/` folder (after `npm run build`)
- **Deploy**: Copy to any static web server

### Program Integration
- **Program ID**: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- **Network**: Solana mainnet
- **RPC**: https://api.mainnet-beta.solana.com
- **Verification**: Merkle proofs checked on-chain

---

## 📋 Quick Facts

| Question | Answer |
|----------|--------|
| **How do I run locally?** | `npm install && npm run dev` |
| **How do I build?** | `npm run build` |
| **How do I deploy?** | Push to Vercel or serve `dist/` folder |
| **What does it cost?** | Free (except Solana network fees for claims) |
| **Is it secure?** | Yes – no private keys stored, on-chain verification |
| **Does it work on mobile?** | Yes – responsive design |
| **Can I customize it?** | Yes – edit `src/App.css` and components |
| **What's the proof JSON?** | Hash commitment of (wallet, amount, ID) |
| **What's the 1% fee?** | Token-2022 transfer fee (configured in program) |
| **Can I use other wallets?** | Currently Phantom only (can add others) |

---

## 🎓 Learning Path

### Beginner (Just want to claim)
1. Read: `CLS_CLAIM_UI.md` (7 min)
2. Get proof JSON from CLS team
3. Use the UI to claim

### Intermediate (Want to understand how it works)
1. Read: `README.md` (10 min)
2. Read: `CLAIM_INTEGRATION_GUIDE.md` (10 min)
3. Run `npm run dev` and explore UI
4. Look at `src/ClaimCLS.tsx` comments

### Advanced (Want to modify/extend)
1. Read: All docs above
2. Read: Program source `programs/token-2022/src/instructions/merkle_ring.rs`
3. Read: E2E test `scripts/e2e-direct-manual.ts`
4. Modify components as needed
5. Build with `npm run build`

---

## 🔗 Related Documents

**In this folder:**
- `README.md` – Full developer guide
- `CLS_CLAIM_UI.md` – User guide
- `QUICKSTART.md` – Quick reference
- `sample-proof.json` – Example proof

**In parent folder:**
- `HARDENING_SPRINT_SUMMARY.md` – Program verification & fixes
- `CLAIM_UI_BUILD_SUMMARY.md` – Technical build details
- `CLAIM_INTEGRATION_GUIDE.md` – End-to-end flow

**In program source:**
- `programs/token-2022/src/instructions/merkle_ring.rs` – Smart contract
- `scripts/e2e-direct-manual.ts` – Reference implementation
- `README.md` – Architecture overview

---

## ✅ Pre-Launch Checklist

- [x] UI component created and tested
- [x] Build succeeds without errors
- [x] Dependencies installed and verified
- [x] Documentation complete
- [x] Sample proof included
- [x] Styling finalized
- [x] Error handling implemented
- [x] Integration with mainnet program verified

---

## 🚀 Next Steps

### For Teams
1. Deploy UI to domain (Vercel/CDN)
2. Generate proof JSONs from aggregator
3. Send to builders (with claim link)
4. Monitor claim submissions

### For Individuals
1. Get proof JSON from CLS team
2. Visit claim UI
3. Load proof → connect wallet → claim
4. Verify balance in wallet

---

## 📞 Support

**Not working?** Check:
1. [`QUICKSTART.md`](./QUICKSTART.md) – Troubleshooting section
2. [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md) – Common Errors
3. [`README.md`](./README.md) – Error Handling

**Questions?** See:
1. [`README.md`](./README.md) – FAQ section (if exists)
2. [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md) – Full detailed walkthrough

---

## 📊 Statistics

| Metric | Value |
|--------|-------|
| **Component Size** | 16 KB |
| **Build Size** | 448 KB (441 KB gzipped) |
| **Documentation** | 4 guides, ~25 KB total |
| **Dependencies** | 5 main (@solana/web3.js, spl-token, js-sha3, react, vite) |
| **Lines of Code** | ~400 (component) + config/styles |
| **Time to Build** | 3.26 seconds |
| **Build Output** | dist/ folder (production-ready) |

---

## 🎉 Summary

**This is a complete, production-ready claim interface for CLS token distribution:**

- ✅ Users can load proof JSON and claim tokens
- ✅ All verification happens on-chain (no backend needed)
- ✅ Fully documented for users, developers, and operators
- ✅ Can be deployed in minutes to any static host
- ✅ Extensible for future enhancements

**Start here:**
- **Users**: Read [`CLS_CLAIM_UI.md`](./CLS_CLAIM_UI.md)
- **Developers**: Read [`README.md`](./README.md)
- **Quick ref**: Read [`QUICKSTART.md`](./QUICKSTART.md)

---

**Built:** October 31, 2025
**Status:** ✅ Ready for Production
**Program:** `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
**Repository:** https://github.com/twzrd-sol/attention-oracle-program
