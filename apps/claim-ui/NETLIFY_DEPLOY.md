# 🚀 Netlify Deployment Guide

## ✅ Pre-Deployment Checklist

Your claim UI is **ready to deploy** with:

- ✅ Netlify configuration (`netlify.toml`)
- ✅ RPC proxy function (keeps API key server-side)
- ✅ Environment variable support
- ✅ SPA routing configured
- ✅ Asset caching optimized
- ✅ Security headers added

---

## 🌐 Deploy to Netlify (5 minutes)

### Option A: Deploy via Netlify UI (Easiest)

1. **Create Netlify Account**
   - Go to https://app.netlify.com
   - Sign up with GitHub

2. **Import Repository**
   - Click "Add new site" → "Import an existing project"
   - Choose "GitHub" and authorize
   - Select your repo: `twzrd-sol/twzrd-backend` (or wherever this lives)
   - Set **Base directory**: `apps/claim-ui`

3. **Configure Build Settings**
   ```
   Build command: npm run build
   Publish directory: dist
   Functions directory: netlify/functions
   ```

4. **Set Environment Variables**
   Go to Site settings → Environment variables → Add variables:

   ```bash
   # Build-time variables (VITE_* are embedded in build)
   NODE_VERSION = 20
   VITE_PROGRAM_ID = GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
   VITE_SOLANA_RPC = /.netlify/functions/rpc-proxy
   VITE_EXPLORER_BASE = https://explorer.solana.com

   # Server-side variables (used by Netlify function)
   RPC_URL = https://solana-mainnet.api.Helius.io
   CLAIM_UI_KEY = 3RUSu4CASNgJUXfZCWMTk949UtkS4WVh1JzngExSKLcu89P7hMD39PLWdqBfA6uneHhaM64FqgteGUYPsdyVhpfJwQd8Mht48q4
   AUTH_MODE = (leave empty for x-api-key, or set to "bearer" if needed)
   ```

5. **Deploy!**
   - Click "Deploy site"
   - Wait ~2 minutes for build
   - Get your URL: `https://<random-name>.netlify.app`

6. **Custom Domain (Optional)**
   - Go to Site settings → Domain management
   - Add custom domain: `claim.twzrd.xyz` or similar
   - Update DNS records as shown

---

### Option B: Deploy via Netlify CLI

```bash
# Install Netlify CLI
npm install -g netlify-cli

# Login
netlify login

# Initialize site
cd apps/claim-ui
netlify init

# Set environment variables
netlify env:set NODE_VERSION 20
netlify env:set VITE_PROGRAM_ID GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
netlify env:set VITE_SOLANA_RPC /.netlify/functions/rpc-proxy
netlify env:set VITE_EXPLORER_BASE https://explorer.solana.com
netlify env:set RPC_URL https://solana-mainnet.api.Helius.io
netlify env:set CLAIM_UI_KEY your-api-key-here

# Deploy
netlify deploy --prod
```

---

## 🧪 Test Your Deployment

1. **Check the site loads**
   ```
   https://<your-site>.netlify.app
   ```

2. **Test wallet connection**
   - Click "Connect Wallet"
   - Phantom should prompt
   - Wallet address should appear

3. **Test RPC proxy**
   - Open DevTools → Network tab
   - Upload a proof JSON
   - You should see requests to `/.netlify/functions/rpc-proxy`
   - Status should be 200 OK

4. **Test claim flow (with real proof)**
   - Upload valid proof JSON
   - Connect wallet
   - Click "Submit Claim"
   - Should see transaction signature + Explorer link

---

## 🔐 Security Notes

### ✅ What's Protected

- **RPC API Key** is server-side only (in Netlify function)
- **API Key never appears** in browser bundle or network requests
- **CORS headers** prevent unauthorized domains from calling your function
- **Security headers** added (X-Frame-Options, CSP, etc.)

### ⚠️ Important

1. **Never commit `.env` to git** - It contains your API key
2. **RPC_URL and CLAIM_UI_KEY** must be set in Netlify environment variables
3. **Rotate keys** if they're ever exposed

---

## 📊 Monitoring

### Netlify Dashboard

- **Functions tab**: See RPC proxy invocations, errors, logs
- **Analytics**: Page views, bandwidth usage
- **Deploy log**: Build output, errors

### Check for Issues

```bash
# View function logs
netlify functions:log rpc-proxy

# Check deploy status
netlify status

# View recent deploys
netlify deploy:list
```

---

## 🛠️ Troubleshooting

### Issue: "Proxy not configured" error

**Fix:** Set `RPC_URL` and `CLAIM_UI_KEY` in Netlify environment variables

### Issue: 401/403 from RPC

**Fix:** Check that `CLAIM_UI_KEY` matches your RPC provider's key

### Issue: 404 on page refresh

**Fix:** Verify `netlify.toml` has the SPA redirect rule (it does!)

### Issue: Slow asset loading

**Fix:** Check that `netlify.toml` has asset caching headers (it does!)

### Issue: Function timeout

**Fix:** Netlify functions have 10s timeout (enough for RPC). If timing out:
- Check RPC provider status
- Try a different RPC endpoint

---

## 📝 Environment Variables Reference

### Build-time (VITE_*)

These are embedded in the JavaScript bundle at build time:

| Variable | Value | Purpose |
|----------|-------|---------|
| `VITE_PROGRAM_ID` | `GnGz...VZop` | Solana program address |
| `VITE_SOLANA_RPC` | `/.netlify/functions/rpc-proxy` | RPC endpoint (points to function) |
| `VITE_EXPLORER_BASE` | `https://explorer.solana.com` | Explorer URL for tx links |

### Server-side (Function)

These stay server-side and are used by the Netlify function:

| Variable | Example | Purpose |
|----------|---------|---------|
| `RPC_URL` | `https://solana-mainnet.api.Helius.io` | Your RPC provider base URL |
| `CLAIM_UI_KEY` | `3RUSu4C...` | Your RPC API key |
| `AUTH_MODE` | `bearer` or empty | How to send the key (x-api-key or Authorization: Bearer) |

---

## 🔄 Updating After Deploy

### Update Environment Variables

```bash
netlify env:set CLAIM_UI_KEY new-key-here
```

### Redeploy

```bash
netlify deploy --prod
```

Or just push to GitHub - Netlify auto-deploys on push!

---

## 📦 What Gets Deployed

```
Netlify Site
├── index.html                (541 bytes)
├── assets/
│   ├── index-*.css          (2.71 KB)
│   └── index-*.js           (441 KB)
└── /.netlify/functions/
    └── rpc-proxy            (Serverless function)
```

---

## 🎯 Next Steps After Deploy

1. ✅ **Test thoroughly** with real claims
2. ✅ **Add custom domain** (claim.twzrd.xyz)
3. ✅ **Set up monitoring** (Netlify Analytics)
4. ✅ **Share with users** 🚀

---

## 📚 Additional Resources

- [Netlify Docs](https://docs.netlify.com/)
- [Netlify Functions](https://docs.netlify.com/functions/overview/)
- [Netlify CLI Reference](https://docs.netlify.com/cli/get-started/)
- [Custom Domains](https://docs.netlify.com/domains-https/custom-domains/)

---

**Your claim UI is production-ready!** 🎉

Need help? Check the troubleshooting section or reach out.
