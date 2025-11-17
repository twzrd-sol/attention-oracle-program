/**
 * POST /api/admin/audit-log
 *
 * Manual audit logging for administrative changes
 * Requires: Authorization header (not implemented here - add in production)
 *
 * Used for:
 * - Manual verification status updates
 * - Corrections to test data
 * - Security incidents (e.g., revoking compromised wallets)
 */

import type { Request, Response } from 'express';
import { logVerificationChange, VerificationChange } from '../services/verification-audit.js';
import { db } from '../db.js';

export interface AdminAuditLogRequest {
  wallet: string;
  field: string;
  oldValue: string | null;
  newValue: string | null;
  reason: string;
  adminId: string;
}

export async function postAdminAuditLog(
  req: Request<{}, { success: boolean; message: string } | { error: string }, AdminAuditLogRequest>,
  res: Response<{ success: boolean; message: string } | { error: string }>
) {
  try {
    const { wallet, field, oldValue, newValue, reason, adminId } = req.body;

    // TODO: Add authorization check
    // if (!isAuthorizedAdmin(req.headers.authorization)) {
    //   return res.status(401).json({ error: 'Unauthorized' });
    // }

    // Validate inputs
    if (!wallet || !field || !reason || !adminId) {
      return res.status(400).json({ error: 'Missing required fields: wallet, field, reason, adminId' });
    }

    // Verify wallet exists
    const walletExists = await db.oneOrNone(
      'SELECT wallet FROM social_verification WHERE wallet = $1',
      [wallet]
    );

    if (!walletExists) {
      return res.status(404).json({ error: 'Wallet not found in verification system' });
    }

    // Log the change
    await logVerificationChange(
      wallet,
      [{ fieldName: field, oldValue, newValue }],
      adminId,
      reason
    );

    res.json({
      success: true,
      message: `Logged change for wallet ${wallet}: ${field} ${oldValue} → ${newValue}`,
    });
  } catch (err) {
    console.error('[postAdminAuditLog] Error:', err);
    res.status(500).json({ error: 'Internal server error' });
  }
}

export default postAdminAuditLog;
