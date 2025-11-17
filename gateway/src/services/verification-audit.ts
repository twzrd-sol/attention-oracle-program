/**
 * Verification Audit Service
 *
 * Logs all changes to verification status for compliance and debugging
 */

import { db } from '../db.js';

export interface VerificationChange {
  fieldName: string;
  oldValue: string | null;
  newValue: string | null;
}

/**
 * Log a verification status change
 *
 * @param wallet - The wallet address being modified
 * @param changes - Array of field changes
 * @param changedBy - Who made the change (user ID, API call, system, etc.)
 * @param reason - Why the change was made
 */
export async function logVerificationChange(
  wallet: string,
  changes: VerificationChange[],
  changedBy: string,
  reason: string
): Promise<void> {
  try {
    // Update the main table with audit metadata
    await db.none(
      'UPDATE social_verification SET updated_by = $1, update_reason = $2, updated_at = NOW() WHERE wallet = $3',
      [changedBy, reason, wallet]
    );

    // Log each individual field change to audit table (immutable history)
    for (const change of changes) {
      await db.none(
        `INSERT INTO verification_audit (wallet, field_name, old_value, new_value, changed_by, change_reason)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [wallet, change.fieldName, change.oldValue, change.newValue, changedBy, reason]
      );
    }

    console.log(`[Audit] Logged change for wallet ${wallet}: ${reason} (by ${changedBy})`);
  } catch (error) {
    console.error('[Audit] Failed to log verification change:', error);
    // Don't throw—audit failures shouldn't break the main flow
    // But definitely log them for manual review
  }
}

/**
 * Get audit history for a wallet
 *
 * @param wallet - The wallet to get history for
 * @param limit - Number of recent changes to return (default 50)
 */
export async function getVerificationHistory(wallet: string, limit: number = 50) {
  try {
    const history = await db.any(
      `SELECT field_name, old_value, new_value, changed_by, change_reason, changed_at
       FROM verification_audit
       WHERE wallet = $1
       ORDER BY changed_at DESC
       LIMIT $2`,
      [wallet, limit]
    );
    return history;
  } catch (error) {
    console.error('[Audit] Failed to fetch verification history:', error);
    return [];
  }
}

/**
 * Get recent changes across all wallets
 *
 * @param limit - Number of recent changes to return (default 100)
 */
export async function getRecentChanges(limit: number = 100) {
  try {
    const changes = await db.any(
      `SELECT wallet, field_name, old_value, new_value, changed_by, change_reason, changed_at
       FROM verification_audit
       ORDER BY changed_at DESC
       LIMIT $1`,
      [limit]
    );
    return changes;
  } catch (error) {
    console.error('[Audit] Failed to fetch recent changes:', error);
    return [];
  }
}

/**
 * Get changes by a specific modifier
 *
 * @param changedBy - The modifier to filter by (e.g., "oauth:twitter", "admin:user123")
 * @param limit - Number of changes to return
 */
export async function getChangesByModifier(changedBy: string, limit: number = 100) {
  try {
    const changes = await db.any(
      `SELECT wallet, field_name, old_value, new_value, changed_at, change_reason
       FROM verification_audit
       WHERE changed_by = $1
       ORDER BY changed_at DESC
       LIMIT $2`,
      [changedBy, limit]
    );
    return changes;
  } catch (error) {
    console.error('[Audit] Failed to fetch changes by modifier:', error);
    return [];
  }
}
