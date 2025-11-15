# CLS Claim UI – Quick Reference

## 🚀 Get Started in 3 Steps

### Step 1: Run Locally
```bash
cd apps/claim-ui
npm install
npm run dev
```
→ Open `http://localhost:5173`

### Step 2: Load a Proof
- Upload `proof.json` or paste JSON
- See claim details appear

### Step 3: Connect & Claim
- Click "Connect Wallet" → Phantom
- Click "Submit Claim" → sign
- Wait ~30s → done!

---

## 📦 Deploy to Production

### Vercel (Recommended)
```bash
npm run build
# Push to GitHub, connect to Vercel
# Auto-deploys dist/ folder
```

### Any Static Host
```bash
npm run build
# Upload dist/ folder to your server
```

### Cloudflare Pages
```bash
npm run build
# Deploy dist/ folder via CLI or UI
```

---

## 🔗 Configuration

### Change Network (Devnet)
Edit `src/ClaimCLS.tsx`:
```typescript
const PROGRAM_ID = new PublicKey('YOUR_DEVNET_ID');
const RPC_URL = 'https://api.devnet.solana.com';
```

### Customize Styling
Edit `src/App.css` (light/dark mode variables at top)

---

## 📋 Proof JSON Format

```json
{
  "claimer": "wallet_address",
  "mint": "token_mint",
  "channel": "stableronaldo",
  "epoch": 1,
  "index": 0,
  "amount": "10000000000",
  "id": "channel:stableronaldo:alice",
  "root": "0x...",
  "proof": ["0x...", "0x..."]
}
```

See `sample-proof.json` for example.

---

## 🛠️ Development

### Install Dependencies
```bash
npm install
```

### Development Server
```bash
npm run dev
```
→ Vite serves on port 5173 with hot reload

### Production Build
```bash
npm run build
npm run preview  # Test locally
```

### Check Build Size
```bash
du -sh dist/
# Output: 448K (441K gzipped)
```

---

## ✅ Key Facts

| Aspect | Details |
|--------|---------|
| **Framework** | React + Vite |
| **Dependencies** | @solana/web3.js, @solana/spl-token, js-sha3 |
| **Size** | 441 KB gzipped |
| **Network** | Solana mainnet |
| **Program** | `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop` |
| **Wallet** | Phantom required |
| **Fee** | 1% (on-chain) |
| **Verification** | Merkle proof on-chain |

---

## 🆘 Troubleshooting

**"Phantom not detected"**
→ Install Phantom browser extension

**"Proof is for X, but you're using Y"**
→ Switch wallets in Phantom to match proof

**"You already claimed this epoch"**
→ Try next epoch (already claimed once)

**"Invalid proof JSON"**
→ Check all required fields present in JSON

**Build fails**
→ Run `npm install` first, then `npm run build`

---

## 📖 Documentation

- **User Guide**: `CLS_CLAIM_UI.md` (7 KB)
- **Dev Guide**: `README.md` (3.9 KB)
- **Sample Proof**: `sample-proof.json`

---

## 🎯 What Happens

1. **Client**: Parse proof, derive PDAs, build instruction
2. **User**: Sign with Phantom
3. **Network**: Submit to Solana mainnet
4. **Program**: Verify proof, check bitmap, transfer tokens
5. **UI**: Show balance update & Explorer link

---

## 🔐 Security

- ✅ No private keys stored
- ✅ Phantom handles all signing
- ✅ Proofs verified on-chain only
- ✅ Double-claim prevention (bitmap guard)

---

**Built:** October 31, 2025
**Status:** ✅ Production Ready
**Repo:** https://github.com/twzrd-sol/attention-oracle-program
