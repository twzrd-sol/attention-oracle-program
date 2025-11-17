import { Router, Request, Response, NextFunction } from 'express'
import rateLimit from 'express-rate-limit'
import bs58 from 'bs58'
import { canonicalUserHash } from '../util/hashing.js'

export interface WalletBindingStore {
  bindWallet(params: { userId?: string; username?: string; wallet: string; verified?: boolean; source?: string }): Promise<void>
  getWalletForUserHash(userHash: string): Promise<string | null>
}

interface RouterOptions {
  db: WalletBindingStore
  apiToken?: string
}

function normalizeTwitchId(input: any): string | null {
  const id = (input?.twitch_id || input?.userId || input?.user_id || '').trim()
  return id || null
}

function normalizeUsername(input: any): string | null {
  const name = (input?.username || input?.login || input?.user || '').trim()
  return name || null
}

function ensureAuth(token?: string) {
  return (req: Request, res: Response, next: NextFunction) => {
    if (!token) return next()
    const header = String(req.headers['x-bind-token'] || '').trim()
    if (header && header === token) return next()
    return res.status(401).json({ error: 'unauthorized' })
  }
}

function requireTwitchHeaders(req: Request, res: Response, next: NextFunction) {
  const headerId = String(req.headers['x-twitch-user-id'] || '').trim()
  const headerLogin = String(req.headers['x-twitch-login'] || '').trim()
  if (!headerId || !headerLogin) {
    return res.status(401).json({ error: 'missing_twitch_auth' })
  }
  const bodyId = normalizeTwitchId(req.body)
  const bodyLogin = normalizeUsername(req.body)
  if (bodyId && bodyId !== headerId) {
    return res.status(403).json({ error: 'twitch_id_mismatch' })
  }
  if (bodyLogin && bodyLogin.toLowerCase() !== headerLogin.toLowerCase()) {
    return res.status(403).json({ error: 'twitch_login_mismatch' })
  }
  req.body.twitch_id = bodyId || headerId
  req.body.login = bodyLogin || headerLogin
  next()
}

function ensureWalletFormat(wallet: string) {
  try {
    const decoded = bs58.decode(wallet)
    if (decoded.length !== 32) {
      throw new Error('wallet must decode to 32 bytes')
    }
  } catch (err) {
    throw new Error('invalid_wallet')
  }
}

export function createWalletBindingRouter({ db, apiToken }: RouterOptions) {
  const router = Router()
  const auth = ensureAuth(apiToken)
  const bindLimiter = rateLimit({
    windowMs: 60_000,
    max: 10,
    keyGenerator: req => normalizeTwitchId(req.body) || req.ip,
    standardHeaders: true,
    legacyHeaders: false,
  })

  router.post('/bind-wallet', bindLimiter, auth, requireTwitchHeaders, async (req, res) => {
    const twitchId = normalizeTwitchId(req.body)
    if (!twitchId) return res.status(400).json({ error: 'missing_twitch_id' })
    const wallet = String(req.body?.wallet || '').trim()
    if (!wallet) return res.status(400).json({ error: 'missing_wallet' })
    try {
      ensureWalletFormat(wallet)
    } catch (err: any) {
      return res.status(400).json({ error: err.message || 'invalid_wallet' })
    }
    const username = normalizeUsername(req.body)
    let userHash: string
    try {
      userHash = canonicalUserHash({ userId: twitchId, user: username || undefined })
    } catch {
      return res.status(400).json({ error: 'invalid_user' })
    }
    try {
      await db.bindWallet({ userId: twitchId, username: username || undefined, wallet, verified: true, source: 'api' })
      return res.json({ ok: true, wallet, userHash })
    } catch (err: any) {
      return res.status(500).json({ error: 'bind_failed', message: err?.message })
    }
  })

  router.get('/bound-wallet', auth, async (req, res) => {
    const twitchId = normalizeTwitchId(req.query)
    const username = normalizeUsername(req.query)
    if (!twitchId && !username) {
      return res.status(400).json({ error: 'missing_identity' })
    }
    let userHash: string
    try {
      userHash = canonicalUserHash({ userId: twitchId || undefined, user: username || undefined })
    } catch {
      return res.status(400).json({ error: 'invalid_user' })
    }
    try {
      const wallet = await db.getWalletForUserHash(userHash)
      if (!wallet) return res.status(404).json({ error: 'not_found' })
      return res.json({ wallet, userHash })
    } catch (err: any) {
      return res.status(500).json({ error: 'lookup_failed', message: err?.message })
    }
  })

  return router
}

export default createWalletBindingRouter
