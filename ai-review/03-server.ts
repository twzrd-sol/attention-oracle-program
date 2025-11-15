#!/usr/bin/env ts-node

/**
 * Simple API server for CHAT claim system
 */

import express from 'express';
import { Pool } from 'pg';
import { Connection, PublicKey, Keypair, clusterApiUrl } from '@solana/web3.js'
import { getAssociatedTokenAddress, getMint, getOrCreateAssociatedTokenAccount, mintTo, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, createMint } from '@solana/spl-token'
import * as path from 'path';
import * as fs from 'fs';
import session from 'express-session';
import axios from 'axios';
import * as dotenv from 'dotenv';

// Load env from root .env and local .env.claim if present
dotenv.config({ path: path.join(__dirname, '..', '..', '.env') });
// Allow local claim env to override root values for devnet testing
dotenv.config({ path: path.join(__dirname, '..', '.env.claim'), override: true });
// Also load .env.local for Twitch OAuth settings
dotenv.config({ path: path.join(__dirname, '..', '.env.local'), override: true });

const app = express();
app.use(express.json());

  // Enable CORS for frontend
  app.use((req, res, next) => {
      res.header('Access-Control-Allow-Origin', '*');
      res.header('Access-Control-Allow-Headers', 'Content-Type');
      res.header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
      next();
  });

  // Serve static files
  app.use(express.static(path.join(__dirname, '../public')));

  // Quiet 404 noise from browsers requesting a favicon
  app.get('/favicon.ico', (_req, res) => res.status(204).end());

  // Friendly route for claim page (redirect to v2)
  app.get('/claim', (_req, res) => {
      res.redirect('/claim-v2');
  });
  // New gift-like claim flow
  app.get('/claim-v2', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/claim-v2.html'));
  });
  // Gentle guide and community (optional pages if present)
  app.get('/get-started.html', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/get-started.html'));
  });
  app.get('/community.html', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/community.html'));
  });
  // Friendly legal/brand routes without .html
  app.get('/terms', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/terms.html'));
  });
  app.get('/privacy', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/privacy.html'));
  });
  app.get('/cookies', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/cookies.html'));
  });
  app.get('/brand', (_req, res) => {
      res.sendFile(path.join(__dirname, '../public/brand.html'));
  });
  // Silence Chrome devtools probe (harmless in console)
  app.get('/.well-known/appspecific/com.chrome.devtools.json', (_req, res) => {
      res.setHeader('Content-Type', 'application/json');
      res.status(200).send('{}');
  });

  // Sessions (for Twitch login)
  const sessSecret = process.env.SESSION_SECRET || 'dev_session_secret_change_me'
  // Behind proxies (e.g., nginx), trust X-Forwarded-* so secure cookies work
  app.set('trust proxy', 1)
  app.use(session({
      secret: sessSecret,
      resave: false,
      saveUninitialized: true,
      cookie: {
        maxAge: 1000 * 60 * 60 * 24,
        sameSite: 'lax',
        secure: process.env.NODE_ENV === 'production',
        domain: process.env.SESSION_COOKIE_DOMAIN || undefined,
      }
  }));

  // Twitch OAuth endpoints
  const TWITCH_CLIENT_ID = process.env.TWITCH_CLIENT_ID || ''
  const TWITCH_CLIENT_SECRET = process.env.TWITCH_CLIENT_SECRET || ''
  const TWITCH_REDIRECT_URI = process.env.TWITCH_REDIRECT_URI || 'http://localhost:3000/api/auth/twitch/callback'

  app.get('/api/auth/twitch/login', (req, res) => {
      const state = Math.random().toString(36).slice(2)
      ;(req.session as any).oauth_state = state
      const scope = encodeURIComponent('user:read:email')
      const url = `https://id.twitch.tv/oauth2/authorize?client_id=${TWITCH_CLIENT_ID}&redirect_uri=${encodeURIComponent(TWITCH_REDIRECT_URI)}&response_type=code&scope=${scope}&state=${state}`
      res.redirect(url)
  })

  app.get('/api/auth/twitch/callback', async (req, res) => {
      try {
          const { code, state } = req.query as any
          if (!code || !state || state !== (req.session as any).oauth_state) {
              return res.status(400).send('Invalid OAuth state')
          }
          // Exchange code for token
          const tokenRes = await axios.post('https://id.twitch.tv/oauth2/token', null, {
              params: {
                  client_id: TWITCH_CLIENT_ID,
                  client_secret: TWITCH_CLIENT_SECRET,
                  code,
                  grant_type: 'authorization_code',
                  redirect_uri: TWITCH_REDIRECT_URI,
              }
          })
          const access_token = tokenRes.data.access_token
          // Fetch user
          const userRes = await axios.get('https://api.twitch.tv/helix/users', {
              headers: {
                  'Authorization': `Bearer ${access_token}`,
                  'Client-Id': TWITCH_CLIENT_ID,
              }
          })
          const user = userRes.data.data?.[0]
          if (!user) return res.status(400).send('Unable to fetch user')
          ;(req.session as any).twitch = { id: user.id, login: user.login }
          // Send back to claim page
          res.redirect('/claim')
      } catch (e:any) {
          console.error('[oauth] error', e?.response?.data || e.message)
          res.status(500).send('OAuth error')
      }
  })

  app.get('/api/auth/me', (req, res) => {
      const me = (req.session as any).twitch
      if (!me) return res.json({ loggedIn: false })
      res.json({ loggedIn: true, id: me.id, username: me.login })
  })

  // Minimal env surface for UI (cluster detection, etc.)
  app.get('/api/env', (_req, res) => {
      const rpc = RPC_URL
      const cluster = /devnet/i.test(rpc) ? 'devnet' : /mainnet/i.test(rpc) ? 'mainnet-beta' : (/localhost|127\.0\.0\.1|8899/.test(rpc) ? 'custom' : 'mainnet-beta')
      const tokenProgram = (process.env.TOKEN_PROGRAM || 'TOKEN_2022').toUpperCase()
      res.json({ rpcUrl: rpc, cluster, tokenProgram, chatMint: CHAT_MINT || null })
  })

  // Aliases for older configs
  app.get('/oauth/twitch/callback', (req, res) => {
      const qs = new URLSearchParams(req.query as any).toString()
      res.redirect(`/api/auth/twitch/callback?${qs}`)
  })
  app.get('/login', (_req, res) => res.redirect('/api/auth/twitch/login'))

const PORT = process.env.PORT || 3000;

  // Database connection (uses DATABASE_URL if provided)
  const DATABASE_URL = process.env.DATABASE_URL;
  const pool = new Pool(
      DATABASE_URL
          ? {
              connectionString: DATABASE_URL,
              ssl: process.env.DATABASE_SSL === 'true' ? { rejectUnauthorized: false } : undefined,
          }
          : {
              database: 'twzrd',
              user: 'twzrd',
              host: process.env.DATABASE_HOST || '/var/run/postgresql',
              ssl: false,
          }
  );

  const getClient = async () => pool.connect();

  // Solana helpers
  function loadKeypair(fp: string): Keypair {
      const raw = JSON.parse(require('fs').readFileSync(fp, 'utf8'))
      const sk = Array.isArray(raw) ? new Uint8Array(raw) : new Uint8Array(raw.secretKey)
      return Keypair.fromSecretKey(sk)
  }

  const RPC_URL = process.env.RPC_URL || clusterApiUrl('devnet')
  const PROMO_AMOUNT = parseInt(process.env.PROMO_AMOUNT || '100', 10)
  const PROMO_WINDOW_HOURS = parseInt(process.env.PROMO_WINDOW_HOURS || '12', 10)
  const CLAIM_RATE_LIMIT_MS = parseInt(process.env.CLAIM_RATE_LIMIT_MS || '5000', 10)
  const DECIMALS = parseInt(process.env.CHAT_DECIMALS || '6', 10)
  const TOKEN_PROG = (process.env.TOKEN_PROGRAM || 'TOKEN_2022').toUpperCase() === 'TOKEN_2022' ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID
  const MINT_AUTHORITY_PATH = process.env.MINT_AUTHORITY_PATH || path.join(__dirname, '../.keys/chat-mint-authority.json')
  const MINT_KEYPAIR_PATH = path.join(__dirname, '../.keys/chat-mint.json')
  const CHAT_MINT = process.env.CHAT_MINT || ''
  const ALLOW_CREATE_MINT_ON_MAINNET = (process.env.ALLOW_CREATE_MINT_ON_MAINNET === 'true')
  const claimRateLimiter = new Map<string, number>()

  async function ensureMintExists(connection: Connection): Promise<PublicKey> {
      // Prefer configured CHAT_MINT
      if (CHAT_MINT) {
          const pk = new PublicKey(CHAT_MINT)
          const info = await connection.getAccountInfo(pk)
          if (!info) throw new Error(`Configured CHAT_MINT not found on cluster: ${pk.toBase58()}`)
          return pk
      }
      // Fallback to local keypair (dev/test)
      const fs = require('fs')
      const mintJson = JSON.parse(fs.readFileSync(MINT_KEYPAIR_PATH, 'utf8'))
      const mintKp = 'secretKey' in mintJson ? Keypair.fromSecretKey(new Uint8Array(mintJson.secretKey)) : Keypair.generate()
      const mintPub = mintKp.publicKey
      const info = await connection.getAccountInfo(mintPub)
      if (info) return mintPub
      // Create only if not mainnet or explicitly allowed
      const isMainnet = /mainnet/i.test(RPC_URL)
      if (isMainnet && !ALLOW_CREATE_MINT_ON_MAINNET) {
          throw new Error(`Mint ${mintPub.toBase58()} not found. Set CHAT_MINT to an existing mainnet mint or set ALLOW_CREATE_MINT_ON_MAINNET=true (requires funded payer).`)
      }
      const payer = loadKeypair(MINT_AUTHORITY_PATH)
      const bal = await connection.getBalance(payer.publicKey)
      if (bal < 0.01 * 1e9) {
          throw new Error(`Mint authority underfunded (${(bal/1e9).toFixed(6)} SOL). Fund ${payer.publicKey.toBase58()} to create mint/ATAs.`)
      }
      const sig = await createMint(connection, payer, payer.publicKey, null, DECIMALS, mintKp, undefined, TOKEN_PROG)
      console.log('[mint] created CHAT mint', mintPub.toBase58(), 'tx', sig)
      return mintPub
  }

  async function mintChat(connection: Connection, mint: PublicKey, toWallet: PublicKey, amountUi: number): Promise<string> {
      const payer = loadKeypair(MINT_AUTHORITY_PATH)
      const ata = await getOrCreateAssociatedTokenAccount(connection, payer, mint, toWallet, undefined, undefined, undefined, TOKEN_PROG)
      const mintInfo = await getMint(connection, mint, undefined, TOKEN_PROG)
      const factor = BigInt(10) ** BigInt(mintInfo.decimals)
      const amountBn = BigInt(Math.floor(amountUi)) * factor
      const sig = await mintTo(connection, payer, mint, ata.address, payer, Number(amountBn), undefined, undefined, TOKEN_PROG)
      return sig
  }

// GET /api/eligibility
  app.get('/api/eligibility', async (req, res) => {
      const { epoch, channel } = req.query as any;
      const sess = (req.session as any).twitch
      if (!sess) return res.status(401).json({ error: 'Login with Twitch required' })
      const username = String(sess.login).toLowerCase()

    const client = await getClient();

      try {
          // Ensure a base promo allocation exists for the current 12h window
          const windowSec = Math.max(1, PROMO_WINDOW_HOURS) * 3600
          const nowSec = Math.floor(Date.now() / 1000)
          const promoEpoch = Math.floor(nowSec / windowSec) * windowSec
          try {
              await client.query(
                `INSERT INTO claimable_allocations (epoch, channel, username, amount)
                 VALUES ($1, 'promo', $2, $3)
                 ON CONFLICT DO NOTHING`,
                [promoEpoch, username, PROMO_AMOUNT]
              )
          } catch (e:any) {
              console.error('[eligibility] promo insert failed', e.message)
          }

          // Special testing crutch: always keep 100 CHAT available for 'zowzrd'
          try {
              if (username === 'zowzrd') {
                  const hasUnclaimed = await client.query(
                    `SELECT 1 FROM claimable_allocations WHERE username = $1 AND redeemed_at IS NULL LIMIT 1`,
                    [username]
                  )
                  if (hasUnclaimed.rowCount === 0) {
                      const specialEpoch = nowSec; // unique primary key component
                      await client.query(
                        `INSERT INTO claimable_allocations (epoch, channel, username, amount)
                         VALUES ($1, 'promo', $2, $3)
                         ON CONFLICT DO NOTHING`,
                        [specialEpoch, username, PROMO_AMOUNT]
                      )
                  }
              }
          } catch (e:any) {
              console.error('[eligibility] zowzrd crutch insert failed', e.message)
          }

          let query = `
              SELECT *
              FROM claimable_allocations
              WHERE username = $1
          `;
          const params: any[] = [username];

        if (epoch) {
            query += ' AND epoch = $2';
            params.push(epoch);
        }
        if (channel) {
            query += ' AND channel = $3';
            params.push(channel);
        }

        // Stable ordering: unclaimed first, newest epoch first
        const ordered = `${query}\nORDER BY (redeemed_at IS NULL) DESC, epoch DESC`;
        const result = await client.query(ordered, params);

          if (result.rows.length === 0) {
              return res.json({
                  eligible: false,
                  username,
                  message: 'No allocations found'
              });
          }

        const allocations = result.rows;
        const unclaimed = allocations.filter(a => !a.redeemed_at);

        return res.json({
            eligible: unclaimed.length > 0,
            username,
            allocations: allocations.map(a => ({
                epoch: a.epoch,
                channel: a.channel,
                amount: a.amount,
                claimed: !!a.redeemed_at,
                wallet: a.wallet
            }))
        });
      } finally {
          client.release();
      }
  });

if (process.env.ENABLE_DEBUG === 'true') {
// GET /api/debug/claim - Debug endpoint for verifying claim system
app.get('/api/debug/claim', async (req, res) => {
    const connection = new Connection(RPC_URL, 'confirmed');

    try {
        // 1. Check RPC connection
        const version = await connection.getVersion();
        const slot = await connection.getSlot();

        // 2. Check mint exists
        let mintInfo = null;
        let mintSupply = null;
        let mintAuthority = null;

        if (CHAT_MINT) {
            try {
                const mintPub = new PublicKey(CHAT_MINT);
                const mintAccount = await connection.getAccountInfo(mintPub);
                if (mintAccount) {
                    // For Token-2022, we need special parsing
                    mintInfo = {
                        exists: true,
                        address: CHAT_MINT,
                        programId: mintAccount.owner.toString()
                    };
                    // Try to get supply (simplified check)
                    mintSupply = 'Unable to decode Token-2022 supply directly';
                }
            } catch (e) {
                mintInfo = { exists: false, error: (e as any).message };
            }
        } else {
            mintInfo = { exists: false, note: 'CHAT_MINT not configured' };
        }

        // 3. Check mint authority
        let authorityInfo = null;
        if (fs.existsSync(MINT_AUTHORITY_PATH)) {
            try {
                const authorityKeypair = Keypair.fromSecretKey(
                    new Uint8Array(JSON.parse(fs.readFileSync(MINT_AUTHORITY_PATH, 'utf-8')))
                );
                const balance = await connection.getBalance(authorityKeypair.publicKey);
                authorityInfo = {
                    pubkey: authorityKeypair.publicKey.toString(),
                    balance: balance / 1e9, // Convert to SOL
                    funded: balance > 0.01 * 1e9 // Need at least 0.01 SOL
                };
            } catch (e) {
                authorityInfo = { error: 'Could not load authority keypair' };
            }
        } else {
            authorityInfo = { error: `Authority keypair not found at ${MINT_AUTHORITY_PATH}` };
        }

        // 4. Check database connection
        let dbStatus = null;
        try {
            const client = await getClient();
            await client.query('SELECT 1');
            client.release();
            dbStatus = { connected: true };
        } catch (e) {
            dbStatus = { connected: false, error: (e as any).message };
        }

        // 5. Check for test allocation
        let testAllocation = null;
        if (dbStatus?.connected) {
            try {
                const client = await getClient();
                const result = await client.query(`
                    SELECT username, channel, epoch, amount, redeemed_at
                    FROM claimable_allocations
                    WHERE username = 'zowzrd'
                    ORDER BY epoch DESC
                    LIMIT 1
                `);
                client.release();

                if (result.rows.length > 0) {
                    testAllocation = result.rows[0];
                }
            } catch (e) {
                testAllocation = { error: (e as any).message };
            }
        }

        return res.json({
            timestamp: new Date().toISOString(),
            environment: {
                cluster: RPC_URL,
                network: RPC_URL.includes('mainnet') ? 'mainnet' : RPC_URL.includes('devnet') ? 'devnet' : 'custom',
                solana_version: version,
                current_slot: slot
            },
            mint: {
                configured_address: CHAT_MINT || 'NOT_SET',
                ...mintInfo,
                supply: mintSupply,
                decimals: DECIMALS,
                allow_create_on_mainnet: process.env.ALLOW_CREATE_MINT_ON_MAINNET === 'true'
            },
            authority: authorityInfo,
            database: dbStatus,
            test_allocation: testAllocation,
            status: {
                ready: !!(mintInfo?.exists && authorityInfo?.funded && dbStatus?.connected),
                issues: [
                    !mintInfo?.exists && 'Mint does not exist',
                    !authorityInfo?.funded && 'Authority needs funding',
                    !dbStatus?.connected && 'Database connection failed'
                ].filter(Boolean)
            }
        });
    } catch (e) {
        return res.status(500).json({
            error: 'Debug check failed',
            details: (e as any).message
        });
    }
});
}

// POST /api/claim (with enhanced error codes)
  app.post('/api/claim', async (req, res) => {
      const { wallet, epoch, channel } = req.body;
      const sess = (req.session as any).twitch
      if (!sess) return res.status(401).json({
          error: 'Login with Twitch required',
          code: 'AUTH_REQUIRED'
      })
      const username = String(sess.login).toLowerCase()

      if (!wallet || !epoch || !channel) {
          return res.status(400).json({
              error: 'Missing required fields',
              code: 'MISSING_FIELDS',
              required: ['wallet','epoch','channel']
          })
      }

      // Simple rate limit per Twitch user (fallback to IP)
      const rateKey = sess?.id ? `user:${sess.id}` : `ip:${req.ip}`
      const now = Date.now()
      const last = claimRateLimiter.get(rateKey) || 0
      if (now - last < CLAIM_RATE_LIMIT_MS) {
          return res.status(429).json({
              error: 'Too many claim attempts. Please wait a moment.',
              code: 'RATE_LIMIT'
          })
      }
      claimRateLimiter.set(rateKey, now)

      // Validate wallet (base58 + curve)
      let owner: PublicKey
      try {
          owner = new PublicKey(wallet)
          if (!PublicKey.isOnCurve(owner.toBytes())) {
              return res.status(400).json({
                  error: 'Invalid wallet address',
                  code: 'INVALID_WALLET'
              })
          }
      } catch (e) {
          return res.status(400).json({
              error: 'Invalid wallet address',
              code: 'INVALID_WALLET'
          })
      }

      const client = await getClient();

      try {
          // Find allocation
          const allocResult = await client.query(`
              SELECT *
              FROM claimable_allocations
              WHERE username = $1
                AND epoch = $2
                AND channel = $3
                AND redeemed_at IS NULL
          `, [username, epoch, channel]);

          if (allocResult.rows.length === 0) {
              // Check if it was already claimed
              const claimedResult = await client.query(`
                  SELECT redeemed_at
                  FROM claimable_allocations
                  WHERE username = $1 AND epoch = $2 AND channel = $3 AND redeemed_at IS NOT NULL
              `, [username, epoch, channel]);

              if (claimedResult.rows.length > 0) {
                  return res.status(400).json({
                      error: 'Allocation already claimed',
                      code: 'ALREADY_CLAIMED',
                      claimed_at: claimedResult.rows[0].redeemed_at
                  });
              }

              return res.status(400).json({
                  error: 'No unclaimed allocation found',
                  code: 'NO_ALLOCATION'
              });
          }

          const allocation = allocResult.rows[0];

          // Real mint on devnet (or RPC_URL)
          const connection = new Connection(RPC_URL, 'confirmed')

          let mintPub: PublicKey;
          try {
              mintPub = await ensureMintExists(connection)
          } catch (e: any) {
              console.error('[mint] Mint not found or creation failed:', e.message)
              return res.status(500).json({
                  error: 'Mint not found or could not be created',
                  code: 'MINT_NOT_FOUND',
                  details: e.message
              })
          }

          let sig = ''
          try {
              sig = await mintChat(connection, mintPub, owner, Number(allocation.amount))
          } catch (e:any) {
              console.error('[mint] Mint transaction failed:', e.message)

              // Parse specific error types
              if (e.message?.includes('insufficient funds') || e.message?.includes('0x1')) {
                  return res.status(500).json({
                      error: 'Mint authority has insufficient funds',
                      code: 'INSUFFICIENT_FUNDS',
                      details: 'The mint authority wallet needs SOL to pay for transaction fees'
                  })
              }

              if (e.message?.includes('blockhash')) {
                  return res.status(500).json({
                      error: 'Network error - please retry',
                      code: 'RPC_ERROR',
                      details: 'Could not fetch recent blockhash'
                  })
              }

              return res.status(500).json({
                  error: 'Mint transaction failed',
                  code: 'MINT_FAILED',
                  details: e.message
              })
          }

          await client.query(`
              UPDATE claimable_allocations
              SET
                  redeemed_at = NOW(),
                  wallet = $1,
                  tx_signature = $2
              WHERE username = $3
                AND epoch = $4
                AND channel = $5
          `, [wallet, sig, username, epoch, channel]);

          return res.json({
              success: true,
              receipt: {
                  username,
                  channel,
                  epoch,
                  amount: allocation.amount,
                  wallet,
                  signature: sig,
                  claimed_at: new Date().toISOString()
              },
              message: `Successfully claimed ${allocation.amount} CHAT tokens!`
          });
      } finally {
          client.release();
      }
  });

// Health check
app.get('/health', (req, res) => {
    res.json({ status: 'ok', service: 'chat-claim-api' });
});

app.listen(PORT, () => {
    console.log(`🚀 CHAT Claim API running on http://localhost:${PORT}`);
    console.log(`
📍 Endpoints:`);
    console.log(`  GET  /api/eligibility?username=test_user`);
    console.log(`  POST /api/claim`);
    console.log(`  GET  /health`);
});
