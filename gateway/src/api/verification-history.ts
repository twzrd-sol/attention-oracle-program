/**
 * GET /api/verification-history
 *
 * Returns audit trail of verification status changes for a wallet
 * Useful for compliance, debugging, and detecting anomalies
 */

import type { Request, Response } from 'express';
import bs58 from 'bs58';
import { getVerificationHistory } from '../services/verification-audit.js';

export interface VerificationHistoryResponse {
  wallet: string;
  changes: Array<{
    fieldName: string;
    oldValue: string | null;
    newValue: string | null;
    changedBy: string;
    changeReason: string;
    changedAt: string;
  }>;
}

export async function getVerificationHistoryHandler(
  req: Request,
  res: Response<VerificationHistoryResponse | { error: string }>
) {
  try {
    const wallet = String(req.query.wallet || '').trim();
    const limit = parseInt(String(req.query.limit || '50'), 10);

    // Validate wallet
    if (!wallet) {
      return res.status(400).json({ error: 'Missing wallet query parameter' });
    }

    try {
      bs58.decode(wallet);
    } catch {
      return res.status(400).json({ error: 'Invalid wallet public key (not valid base58)' });
    }

    // Validate limit
    if (isNaN(limit) || limit < 1 || limit > 500) {
      return res.status(400).json({ error: 'Invalid limit (must be 1-500)' });
    }

    // Fetch history
    const history = await getVerificationHistory(wallet, limit);

    res.json({
      wallet,
      changes: history,
    });
  } catch (err) {
    console.error('[getVerificationHistory] Error:', err);
    res.status(500).json({ error: 'Internal server error' });
  }
}

export default getVerificationHistoryHandler;
