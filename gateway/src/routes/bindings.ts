import { Router, Request, Response } from 'express';
import rateLimit from 'express-rate-limit';
import { PublicKey } from '@solana/web3.js';
import { db } from '../db.js';
import { canonicalUserHash } from '../util/hashing.js';
import { requireTwitchAuth } from '../middleware/require-twitch-auth.js';

const router = Router();

const bindLimiter = rateLimit({
  windowMs: 60 * 60 * 1000,
  limit: 10, // Increased from 1 to 10 for testing
  standardHeaders: 'draft-7',
  legacyHeaders: false,
  keyGenerator: (req: Request) => {
    const twitchId = typeof req.body?.twitch_id === 'string' ? req.body.twitch_id.trim() : '';
    return twitchId || req.ip;
  },
});

let ensureTablePromise: Promise<void> | null = null;
async function ensureBindingsTable() {
  if (!ensureTablePromise) {
    ensureTablePromise = (async () => {
      await db.none(`
        CREATE TABLE IF NOT EXISTS twitch_wallet_bindings (
          twitch_id TEXT PRIMARY KEY,
          login TEXT NOT NULL,
          wallet TEXT NOT NULL,
          created_at TIMESTAMPTZ DEFAULT NOW(),
          updated_at TIMESTAMPTZ DEFAULT NOW()
        );
      `);
      await db.none(`
        CREATE UNIQUE INDEX IF NOT EXISTS idx_wallet_unique ON twitch_wallet_bindings (wallet);
      `);
    })().catch((error) => {
      ensureTablePromise = null;
      throw error;
    });
  }
  return ensureTablePromise;
}

// CORS preflight handler for bind-wallet
router.options('/bind-wallet', (req: Request, res: Response) => {
  const origin = req.headers.origin || 'https://twzrd.xyz';
  res.header('Access-Control-Allow-Origin', origin);
  res.header('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  res.header('Access-Control-Allow-Credentials', 'true');
  res.status(204).end();
});

router.post('/bind-wallet', bindLimiter, requireTwitchAuth, async (req: Request, res: Response) => {
  try {
    await ensureBindingsTable();
    const { twitch_id, login, wallet } = req.body ?? {};
    if (typeof wallet !== 'string') {
      return res.status(400).json({ error: 'missing_twitch_id_or_wallet' });
    }

    const submittedTwitchId = typeof twitch_id === 'string' ? twitch_id.trim() : '';
    const submittedLogin = typeof login === 'string' ? login.trim().toLowerCase() : '';
    const auth = req.twitchAuth;

    if (!auth) {
      return res.status(401).json({ error: 'unauthorized' });
    }

    const tokenTwitchId = (auth.userId || '').trim();
    const tokenLogin = (auth.login || '').trim().toLowerCase();

    if (submittedTwitchId && tokenTwitchId && submittedTwitchId !== tokenTwitchId) {
      return res.status(403).json({ error: 'twitch_id_mismatch' });
    }

    if (submittedLogin && tokenLogin && submittedLogin !== tokenLogin) {
      return res.status(403).json({ error: 'login_mismatch' });
    }

    const finalTwitchId = tokenTwitchId || submittedTwitchId;
    let finalLogin = submittedLogin || tokenLogin;

    if (!finalTwitchId) {
      return res.status(400).json({ error: 'missing_twitch_id_or_wallet' });
    }

    if (!finalLogin) {
      return res.status(400).json({ error: 'missing_login' });
    }

    let normalizedWallet: string;
    try {
      normalizedWallet = new PublicKey(wallet).toBase58();
    } catch {
      return res.status(400).json({ error: 'invalid_wallet' });
    }

    const userHash = canonicalUserHash({ userId: finalTwitchId, user: finalLogin });

    await db.none(
      `INSERT INTO twitch_wallet_bindings (twitch_id, login, wallet, updated_at)
       VALUES ($1, $2, $3, NOW())
       ON CONFLICT (twitch_id) DO UPDATE SET
         login = EXCLUDED.login,
         wallet = EXCLUDED.wallet,
         updated_at = NOW()`,
      [finalTwitchId, finalLogin, normalizedWallet]
    );

    return res.json({ ok: true, userHash });
  } catch (error: any) {
    if (error?.code === '23505') {
      return res.status(409).json({ error: 'wallet_already_bound' });
    }
    console.error('[POST /api/bindings/bind-wallet] error', error);
    return res.status(500).json({ error: 'internal_error' });
  }
});

// CORS preflight handler for bound-wallet
router.options('/bound-wallet', (req: Request, res: Response) => {
  const origin = req.headers.origin || 'https://twzrd.xyz';
  res.header('Access-Control-Allow-Origin', origin);
  res.header('Access-Control-Allow-Methods', 'GET, OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type, Authorization');
  res.header('Access-Control-Allow-Credentials', 'true');
  res.status(204).end();
});

router.get('/bound-wallet', requireTwitchAuth, async (req: Request, res: Response) => {
  try {
    await ensureBindingsTable();
    const twitchQuery = typeof req.query.twitch_id === 'string' ? req.query.twitch_id.trim() : '';
    const loginQuery = typeof req.query.login === 'string' ? req.query.login.trim().toLowerCase() : '';

    const auth = req.twitchAuth;
    if (!auth) {
      return res.status(401).json({ error: 'unauthorized' });
    }

    const tokenTwitchId = (auth.userId || '').trim();
    const tokenLogin = (auth.login || '').trim().toLowerCase();

    if (twitchQuery && tokenTwitchId && twitchQuery !== tokenTwitchId) {
      return res.status(403).json({ error: 'twitch_id_mismatch' });
    }

    if (loginQuery && tokenLogin && loginQuery !== tokenLogin) {
      return res.status(403).json({ error: 'login_mismatch' });
    }

    let resolvedTwitchId = twitchQuery || tokenTwitchId || null;
    let lookupLogin = loginQuery || tokenLogin || null;

    if (!resolvedTwitchId && lookupLogin) {
      const lookup = await db.oneOrNone<{ twitch_id: string }>(
        'SELECT twitch_id FROM twitch_wallet_bindings WHERE login = $1',
        [lookupLogin]
      );
      resolvedTwitchId = lookup?.twitch_id ?? null;
    }

    if (!lookupLogin && resolvedTwitchId === tokenTwitchId) {
      lookupLogin = tokenLogin;
    }

    let userHash: string | null = null;
    try {
      userHash = canonicalUserHash({ userId: resolvedTwitchId ?? undefined, user: lookupLogin ?? undefined });
    } catch {
      userHash = null;
    }

    if (!resolvedTwitchId) {
      return res.json({ wallet: null, userHash });
    }

    const binding = await db.oneOrNone<{ wallet: string }>(
      'SELECT wallet FROM twitch_wallet_bindings WHERE twitch_id = $1',
      [resolvedTwitchId]
    );
    if (!binding) {
      return res.json({ wallet: null, userHash });
    }

    // Normalize empty string to null
    const wallet = binding.wallet && binding.wallet.trim().length > 0 ? binding.wallet : null;
    return res.json({ wallet, userHash });
  } catch (error) {
    console.error('[GET /api/bindings/bound-wallet] error', error);
    return res.status(500).json({ error: 'internal_error' });
  }
});

export default router;
