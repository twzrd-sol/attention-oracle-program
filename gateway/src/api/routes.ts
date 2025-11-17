/**
 * API Routes Setup
 *
 * Registers all API endpoints:
 * - GET /api/verification-status
 * - POST /api/claim-cls
 */

import { Router } from 'express';
import getVerificationStatus from './verification-status.js';
import postClaimCls from './claim-cls.js';
import getVerificationHistory from './verification-history.js';
import postAdminAuditLog from './admin-audit-log.js';

export function setupApiRoutes(): Router {
  const router = Router();

  /**
   * GET /api/verification-status
   *
   * Query parameters:
   *   - wallet: string (base58 Solana pubkey)
   *
   * Response:
   *   {
   *     "twitterFollowed": boolean,
   *     "discordJoined": boolean,
   *     "passportTier": number | null,
   *     "lastVerified": string | null
   *   }
   */
  router.get('/verification-status', getVerificationStatus);

  /**
   * GET /api/verification-history
   *
   * Query parameters:
   *   - wallet: string (base58 Solana pubkey) [required]
   *   - limit: number (1-500, default 50) [optional]
   *
   * Response:
   *   {
   *     "wallet": "string",
   *     "changes": [
   *       {
   *         "fieldName": "string",
   *         "oldValue": "string | null",
   *         "newValue": "string | null",
   *         "changedBy": "string",
   *         "changeReason": "string",
   *         "changedAt": "ISO8601"
   *       }
   *     ]
   *   }
   */
  router.get('/verification-history', getVerificationHistory);

  /**
   * POST /api/claim-cls
   *
   * Request body:
   *   {
   *     "wallet": "string",
   *     "epochId": number
   *   }
   *
   * Response:
   *   {
   *     "transaction": "string (base64)",
   *     "signature": null
   *   }
   */
  router.post('/claim-cls', postClaimCls);

  /**
   * POST /api/admin/audit-log
   *
   * Manually log a verification change (for admin corrections, security incidents)
   * ⚠️ TODO: Add authorization middleware
   *
   * Request body:
   *   {
   *     "wallet": "string",
   *     "field": "string",
   *     "oldValue": "string | null",
   *     "newValue": "string | null",
   *     "reason": "string",
   *     "adminId": "string"
   *   }
   *
   * Response:
   *   {
   *     "success": true,
   *     "message": "string"
   *   }
   */
  router.post('/admin/audit-log', postAdminAuditLog);

  return router;
}

export default setupApiRoutes;
