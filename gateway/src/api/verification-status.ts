/**
 * GET /api/verification-status
 *
 * Returns Twitter & Discord verification status for a wallet
 */

import type { Request, Response } from 'express';
import bs58 from 'bs58';
import { db } from '../db.js';
import { verificationRequests } from '../metrics.js';

export interface VerificationStatusResponse {
  twitterFollowed: boolean;
  discordJoined: boolean;
  passportTier?: number | null;
  lastVerified?: string | null;
}

export async function getVerificationStatus(
  req: Request,
  res: Response<VerificationStatusResponse | { error: string }>
) {
  try {
    // 1) Extract wallet param
    const wallet = String(req.query.wallet || '').trim();

    if (!wallet) {
      return res.status(400).json({
        error: 'Missing wallet query parameter'
      });
    }

    // 2) Validate wallet is base58
    try {
      bs58.decode(wallet);
    } catch {
      return res.status(400).json({
        error: 'Invalid wallet public key (not valid base58)'
      });
    }

    // 3) Query verification status
    const row = await db.oneOrNone(
      `SELECT
        twitter_followed,
        discord_joined,
        passport_tier,
        last_verified
      FROM social_verification
      WHERE wallet = $1`,
      [wallet]
    );

    // 4) If not found, return all false
    if (!row) {
      return res.json({
        twitterFollowed: false,
        discordJoined: false,
        passportTier: null,
        lastVerified: null
      });
    }

    // 5) Return status
    verificationRequests.inc({ status: 'success' });
    res.json({
      twitterFollowed: row.twitter_followed,
      discordJoined: row.discord_joined,
      passportTier: row.passport_tier,
      lastVerified: row.last_verified?.toISOString() || null
    });
  } catch (err) {
    console.error('[getVerificationStatus] Error:', err);
    verificationRequests.inc({ status: 'error' });
    res.status(500).json({
      error: 'Internal server error'
    });
  }
}

export default getVerificationStatus;
