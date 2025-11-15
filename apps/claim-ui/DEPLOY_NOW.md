# 🚀 Deploy Claim UI to Netlify - Ready to Go!

## ✅ Pre-Flight Check

All systems are GO for deployment:

- ✅ Production build completed (`dist/` directory)
- ✅ Netlify CLI installed globally
- ✅ Environment variables configured (`.env.production`)
- ✅ RPC proxy function ready (`netlify/functions/rpc-proxy.js`)
- ✅ Program ID configured: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- ✅ Netlify configuration ready (`netlify.toml`)

## 🎯 Quick Deploy (2 commands)

```bash
cd /home/twzrd/milo-token/apps/claim-ui

# 1. Login to Netlify (opens browser)
netlify login

# 2. Deploy to production
netlify deploy --prod --dir=dist --functions=netlify/functions
```

When prompted:
- **Create & configure a new site?** → Yes
- **Team:** → Select your team
- **Site name:** → `milo-cls-claim` (or custom name)

## 🔐 Set Environment Variables (Required!)

After first deploy, set these in Netlify dashboard or via CLI:

```bash
# Server-side variables (for RPC proxy function)
netlify env:set RPC_URL "https://solana-mainnet.api.Helius.io"
netlify env:set CLAIM_UI_KEY "your-rpc-api-key-here"
netlify env:set AUTH_MODE ""  # or "bearer" if needed

# Redeploy to apply changes
netlify deploy --prod --dir=dist --functions=netlify/functions
```

## 🌐 Your Live URL

After deployment, you'll get:
```
https://<random-name>.netlify.app
```

Or set custom domain:
```bash
netlify domains:add claim.twzrd.xyz
```

## 📊 Deployment Summary

**What's Deployed:**
- Static frontend: `dist/` (440 KB total)
  - `index.html` (541 bytes)
  - `assets/index-*.css` (2.71 KB)
  - `assets/index-*.js` (438.68 KB)
- Serverless function: `rpc-proxy` (keeps API key server-side)

**Environment:**
- Program ID: `GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop`
- Network: Solana Mainnet
- RPC: Proxied via Netlify Function (API key hidden)

## 🧪 Post-Deployment Testing

1. **Visit the URL** - Should show claim interface
2. **Connect Phantom wallet** - Should connect successfully
3. **Upload a proof JSON** - Should parse and show details
4. **Test RPC proxy** - Check Network tab in DevTools for `/.netlify/functions/rpc-proxy`
5. **Submit claim** (with real proof) - Should create transaction

## 🔄 Update After Deploy

To update the UI:
```bash
cd /home/twzrd/milo-token/apps/claim-ui

# Make changes to src/
# Rebuild
npm run build

# Redeploy
netlify deploy --prod --dir=dist --functions=netlify/functions
```

## 📝 Important Notes

- **RPC_URL and CLAIM_UI_KEY** must be set in Netlify env vars (not in `.env` file!)
- Never commit `.env` files with API keys
- The RPC proxy keeps your API key secure (server-side only)
- Function logs available at: https://app.netlify.com → Functions tab

## 🎉 You're Ready!

Run the 2 commands above to deploy now!
