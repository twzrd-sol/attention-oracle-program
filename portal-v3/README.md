# TWZRD Portal v3

**A modern React + Vite web portal for claiming Attention Oracle CLS tokens with Twitter and Discord verification.**

---

## 🎯 Overview

Portal v3 is a single, unified React application that replaces fragmented UI pieces (claim-ui, static portal-v2). It provides:

- **Wallet Connection** - Multi-wallet support (Phantom, Solflare, Torus, Backpack)
- **Verification** - Twitter follow + Discord join status checks
- **Token Claiming** - On-chain CLS token claims via Anchor
- **Extensibility** - Easy to add creator dashboards, viewer UIs, passport tiers
- **Static Deployment** - Builds to `/dist` for serving from gateway backend

---

## 📋 Quick Start

### Installation

```bash
cd portal-v3
npm install
```

### Development Server

```bash
npm run dev
```

Opens at http://localhost:3000

### Production Build

```bash
npm run build
npm run preview
```

Output: `dist/` (ready for gateway static serving)

---

## 🏗️ Architecture

### Directory Structure

```
portal-v3/
├── package.json              # Dependencies & scripts
├── vite.config.ts           # Vite configuration
├── tsconfig.json            # TypeScript configuration
├── index.html               # HTML entry point
├── .env.example             # Environment variables template
├── src/
│   ├── main.tsx            # React entry + wallet providers
│   ├── App.tsx             # Shell (header, content, footer)
│   ├── index.css           # Global styles
│   ├── components/
│   │   └── ClaimCLS.tsx    # Main claim flow component
│   └── lib/
│       ├── solana.ts       # Network & RPC configuration
│       └── api.ts          # Typed API client for backend
└── dist/                   # Production build (generated)
```

### Component Tree

```
main.tsx
├── ConnectionProvider (RPC)
├── WalletProvider (wallet adapters)
├── WalletModalProvider (UI)
└── App.tsx
    ├── Header (TWZRD branding + wallet button)
    ├── ClaimCLS (main content)
    │   ├── Verification Status
    │   │   ├── Twitter tile (X)
    │   │   └── Discord tile
    │   ├── Epoch Selector
    │   └── Claim Button
    └── Footer (links + network info)
```

---

## 🔧 Configuration

### Environment Variables

Create `.env` from `.env.example`:

```bash
cp .env.example .env
```

Edit `.env`:

```bash
# Solana Network (mainnet-beta, devnet, testnet)
VITE_SOLANA_NETWORK=mainnet-beta

# RPC Endpoint
VITE_SOLANA_RPC=https://api.mainnet-beta.solana.com

# Attention Oracle Program ID
VITE_PROGRAM_ID=GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

---

## 🧩 Component Reference

### ClaimCLS.tsx (Main Component)

The core claim flow component with 3 sections:

#### 1. **Verification Status**

Two verification tiles:
- **Follow on X** (@twzrd_xyz)
  - Status badge: ✓ Verified / Not Verified
  - Click to open Twitter profile
- **Join Discord**
  - Status badge: ✓ Verified / Not Verified
  - Click to open Discord invite

**Refresh Verification Button**
- Re-fetches verification status after user completes steps
- Disabled during refresh

#### 2. **Epoch Selector**

- Number input for selecting epoch
- Hint text explaining epoch concept
- Disabled when claim is in progress

#### 3. **Claim Button**

- **Disabled conditions:**
  - Wallet not connected
  - Either verification flag is false
- **States:**
  - `idle` - Ready to claim
  - `claiming` - Building/signing transaction
  - `confirming` - Awaiting on-chain confirmation
  - `success` - Transaction confirmed with signature link
  - `error` - Error occurred with details

---

## 🔗 API Contract

### Backend Endpoints Required

#### GET /api/verification-status

Fetch verification status for a wallet.

**Query Parameters:**
- `wallet` (string) - Solana wallet address (base58)

**Response:**
```json
{
  "twitterFollowed": boolean,
  "discordJoined": boolean,
  "passportTier": number,      // optional
  "lastVerified": string       // optional ISO timestamp
}
```

**Errors:**
```json
{
  "error": "string",
  "details": "string",
  "code": number
}
```

---

#### POST /api/claim-cls

Request a claim transaction.

**Request Body:**
```json
{
  "wallet": "string",  // Solana wallet address (base58)
  "epochId": number    // Epoch ID to claim for
}
```

**Response:**
```json
{
  "transaction": "string",  // base64-encoded transaction
  "signature": "string"     // optional, if pre-signed
}
```

**Errors:**
```json
{
  "error": "string",
  "details": "string",
  "code": number
}
```

**Error Examples:**
- 400: Invalid wallet address or epoch
- 402: User not eligible for this epoch
- 409: Already claimed this epoch
- 500: Backend error

---

## 🎯 Claim Flow

```
1. User connects wallet (Phantom, Solflare, Torus, Backpack)
   ↓
2. App fetches verification status
   ↓
3. User sees Twitter & Discord verification tiles
   ↓
4. User follows X and joins Discord
   ↓
5. User clicks "Refresh Verification Status"
   ↓
6. App re-fetches status (both flags should be true now)
   ↓
7. User selects epoch (default: 0)
   ↓
8. User clicks "Claim CLS Tokens"
   ↓
9. App requests transaction from backend: POST /api/claim-cls
   ↓
10. Backend returns base64-encoded transaction
    ↓
11. App decodes transaction and sends via wallet adapter
    ↓
12. User signs in wallet
    ↓
13. App waits for on-chain confirmation
    ↓
14. Success! Display transaction signature with Solscan link
```

---

## 🚀 Gateway Integration

### Step 1: Build Portal

```bash
cd portal-v3
npm run build
```

Generates: `portal-v3/dist/`

### Step 2: Configure Gateway

In gateway backend (Express):

```typescript
import path from 'path';
import express from 'express';

const app = express();

const portalPath = path.join(__dirname, '..', 'portal-v3', 'dist');

// Serve static assets
app.use(express.static(portalPath));

// Serve index.html for all routes (SPA catch-all)
app.get('*', (_req, res) => {
  res.sendFile(path.join(portalPath, 'index.html'));
});

// Your API routes
app.get('/api/verification-status', (req, res) => {
  const wallet = req.query.wallet as string;
  // Implementation...
});

app.post('/api/claim-cls', (req, res) => {
  const { wallet, epochId } = req.body;
  // Implementation...
});
```

### Step 3: Verify

- Visit http://localhost:5000 (or your gateway URL)
- Portal loads from static assets
- API calls go to /api/* routes
- Works offline if cached

---

## 🛠️ Development

### Type Checking

```bash
npm run type-check
```

### Modifying for Devnet

In `.env`:

```bash
VITE_SOLANA_NETWORK=devnet
VITE_SOLANA_RPC=https://api.devnet.solana.com
VITE_PROGRAM_ID=<YOUR_DEVNET_PROGRAM_ID>
```

Then rebuild:

```bash
npm run build
```

### Adding New Sections

To add a new verification step (e.g., Passport tier):

1. Update `lib/api.ts`:
   - Add field to `VerificationStatus`
   - Update backend contract

2. Update `components/ClaimCLS.tsx`:
   - Add new `VerificationTile` for new check
   - Update claim eligibility logic

3. Example:

```typescript
{/* GitHub Verification Tile */}
<VerificationTile
  icon="🐙"
  label="Star on GitHub"
  verified={state.verification?.githubStarred || false}
  url="https://github.com/twzrd-sol/attention-oracle-program"
  onOpen={() => window.open(...)}
/>
```

---

## 📊 File Reference

### src/lib/solana.ts

Network configuration constants:
- `NETWORK` - Solana cluster (mainnet-beta, devnet, testnet)
- `RPC_URL` - RPC endpoint
- `PROGRAM_ID` - Attention Oracle program
- `TOKEN_2022_PROGRAM_ID` - Token-2022 program
- `ASSOCIATED_TOKEN_PROGRAM_ID` - Associated Token Program
- `getExplorerUrl(signature)` - Solscan explorer link
- `getClusterName()` - Display name
- `isMainnet()` - Boolean check

### src/lib/api.ts

Typed API client:
- `VerificationStatus` - Interface
- `ClaimRequest` - Request payload
- `ClaimResponse` - Response payload
- `getVerificationStatus(wallet)` - Fetch status
- `requestClaimTransaction(wallet, epochId)` - Request transaction
- `getTwitterUrl()` - Twitter profile link
- `getDiscordInviteUrl()` - Discord invite link

### src/components/ClaimCLS.tsx

Main component:
- `ClaimCLS` - Main component
- `VerificationTile` - Sub-component for verification tiles
- `ClaimState` - State interface
- Comprehensive error handling
- Transaction signing & confirmation

---

## 🔐 Security

- **No Private Keys** - Uses wallet adapter for signing
- **HTTPS Only** - All RPC calls over HTTPS
- **No Backend State** - Stateless transactions
- **Address Verification** - Transaction verified on-chain
- **Base64 Encoding** - Safe transaction transport

---

## 🐛 Troubleshooting

| Issue | Cause | Fix |
|-------|-------|-----|
| Wallet not connecting | Adapter not installed | Install Phantom/Solflare |
| "Verification status error" | Backend endpoint missing | Implement /api/verification-status |
| "Failed to request claim transaction" | POST /api/claim-cls missing | Implement claim-cls endpoint |
| Transaction fails | Invalid epoch or not eligible | Check /api/verification-status response |
| "Cannot find module '@'" | TypeScript path alias issue | Ensure tsconfig.json has `@` mapping |

---

## 📚 Resources

- **Vite Docs**: https://vite.dev/
- **React**: https://react.dev/
- **Solana Docs**: https://docs.solana.com/
- **Wallet Adapter**: https://github.com/solana-labs/wallet-adapter
- **Anchor**: https://www.anchor-lang.com/

---

## 📝 Environment Variables

Required:
- `VITE_SOLANA_NETWORK` - Solana cluster
- `VITE_SOLANA_RPC` - RPC endpoint
- `VITE_PROGRAM_ID` - Program address

Optional:
- `VITE_API_BASE_URL` - Backend URL (if different from domain)

---

## 🚢 Deployment

### Production Build

```bash
npm run build
```

Output: `dist/` directory with:
- `index.html` - Entry point
- `assets/` - JS/CSS bundles (optimized)
- `vite.svg` - Favicon

### Serve with Gateway

Copy `dist/` contents to gateway static directory and configure catch-all route.

### Cloudflare Pages / Vercel

```bash
npm run build
# Deploy dist/ folder
```

---

## 📄 License

See `../../LICENSE`

---

**Status**: ✅ Ready for production
**Last Updated**: November 15, 2025
