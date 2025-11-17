/**
 * POST /api/claim-cls
 *
 * Requests a claim transaction for CLS tokens
 * - Validates wallet and epoch
 * - Ensures verification is satisfied
 * - Enforces one-claim-per-epoch-per-wallet
 * - Builds and returns base64-encoded transaction
 */

import type { Request, Response } from 'express';
import { PublicKey } from '@solana/web3.js';
import bs58 from 'bs58';
import { db } from '../db.js';
import { buildClaimTransaction } from '../onchain/claim-transaction.js';
import { claimRequests, claimLatency } from '../metrics.js';

export interface ClaimClsRequest {
  wallet: string;
  epochId: number;
  index?: number;                    // Optional: claimer index in Merkle tree (defaults to 0)
  amount?: string | number;          // Optional: claim amount in raw tokens (defaults to env CLS_CLAIM_AMOUNT)
  id?: string;                       // Optional: leaf identifier (defaults to "cls-epoch-{epochId}")
  proof?: string[];                  // Optional: Merkle proof as array of 64-char hex strings (defaults to empty for single-leaf)
}

export interface ClaimClsResponse {
  transaction: string;
  signature: string | null;
}

export interface ErrorResponse {
  error: string;
  details?: string;
}

type ClaimValidationResult = { reissue: boolean } | { error: ErrorResponse };

/**
 * Validate claim request
 */
async function validateClaimRequest(
  wallet: string,
  epochId: number
): Promise<ClaimValidationResult> {
  // 1) Wallet format
  if (!wallet || typeof wallet !== 'string') {
    return { error: { error: 'Missing wallet' } };
  }

  let pubkey: PublicKey;
  try {
    bs58.decode(wallet);
    pubkey = new PublicKey(wallet);
  } catch {
    return { error: { error: 'Invalid wallet public key' } };
  }

  // 2) Epoch ID format
  if (typeof epochId !== 'number' || !Number.isInteger(epochId) || epochId < 0) {
    return {
      error: { error: 'Invalid epochId (must be non-negative integer)' }
    };
  }

  // 3) Epoch exists and is open
  const epoch = await db.oneOrNone(
    'SELECT merkle_root, is_open FROM epochs WHERE epoch_id = $1',
    [epochId]
  );

  if (!epoch) {
    return { error: { error: 'Epoch not found' } };
  }

  if (!epoch.is_open) {
    return { error: { error: 'Epoch is closed' } };
  }

  // 4) Verification satisfied
  const sv = await db.oneOrNone(
    `SELECT twitter_followed, discord_joined
     FROM social_verification
     WHERE wallet = $1`,
    [wallet]
  );

  if (!sv || !sv.twitter_followed || !sv.discord_joined) {
    return {
      error: {
        error: 'Verification requirements not met',
        details: 'Must have followed @twzrd_xyz on Twitter and joined Discord'
      }
    };
  }

  // 5) One claim per epoch per wallet
  const existing = await db.oneOrNone(
    'SELECT tx_status FROM cls_claims WHERE wallet = $1 AND epoch_id = $2',
    [wallet, epochId]
  );

  if (existing) {
    const status = existing.tx_status;
    if (status === 'confirmed') {
      return {
        error: {
          error: 'Already claimed for this epoch'
        }
      };
    }

    return { reissue: true };
  }

  return { reissue: false };
}

/**
 * POST /api/claim-cls handler
 */
export async function postClaimCls(
  req: Request<{}, ClaimClsResponse | ErrorResponse, ClaimClsRequest>,
  res: Response<ClaimClsResponse | ErrorResponse>
) {
  const endTimer = claimLatency.startTimer();

  try {
    const { wallet, epochId, index, amount, id, proof } = req.body || {};

    // 1) Validate request
    const validationResult = await validateClaimRequest(wallet, epochId);
    if ('error' in validationResult) {
      // Track specific failure reasons
      if (validationResult.error.error === 'Already claimed for this epoch') {
        claimRequests.inc({ status: 'duplicate' });
      } else if (validationResult.error.error === 'Verification requirements not met') {
        claimRequests.inc({ status: 'unverified' });
      } else {
        claimRequests.inc({ status: 'error' });
      }

      const status = validationResult.error.error === 'Already claimed for this epoch' ? 409 : 400;
      endTimer();
      return res.status(status).json(validationResult.error);
    }

    const reissue = validationResult.reissue;

    // 2) Fetch epoch details
    const epochData = await db.one(
      'SELECT merkle_root FROM epochs WHERE epoch_id = $1',
      [epochId]
    );

    // 3) Derive claim parameters (index, amount, id, proof) with sane defaults
    // These can be provided by the off-chain allocator for multi-wallet epochs,
    // or will use env-based defaults for simple fixed-allocation drops.

    const claimIndex =
      typeof index === 'number' && Number.isInteger(index) && index >= 0 ? index : 0;

    let amountStr: string;
    if (typeof amount === 'number') {
      amountStr = amount.toString();
    } else if (typeof amount === 'string') {
      amountStr = amount;
    } else {
      amountStr = process.env.CLS_CLAIM_AMOUNT || '100000000000';
    }

    let claimAmount: bigint;
    try {
      claimAmount = BigInt(amountStr);
    } catch {
      return res.status(400).json({
        error: 'Invalid amount (must be numeric string or number)',
      });
    }

    const defaultIdPrefix = process.env.CLS_CLAIM_ID_PREFIX || 'cls-epoch';
    const claimId = (typeof id === 'string' && id.length > 0)
      ? id
      : `${defaultIdPrefix}-${epochId}`;

    if (Buffer.byteLength(claimId, 'utf8') > 32) {
      return res.status(400).json({
        error: 'Invalid id length (must be ≤ 32 bytes UTF-8)',
      });
    }

    // Validate proof format if provided
    const claimProof: string[] = Array.isArray(proof)
      ? proof.map((p) => String(p))
      : [];

    // Validate each proof element (if any) is 64-char hex
    for (const proofElement of claimProof) {
      if (!/^[0-9a-f]{64}$/i.test(proofElement)) {
        return res.status(400).json({
          error: `Invalid proof element: must be 64-char hex string (32 bytes), got "${proofElement}"`,
        });
      }
    }

    // 4) Build claim transaction
    // Passes through allocation data from off-chain allocator (or uses defaults)
    const walletPubkey = new PublicKey(wallet);
    const tx = await buildClaimTransaction({
      wallet: walletPubkey,
      epochId,
      merkleRoot: epochData.merkle_root,
      index: claimIndex,
      amount: claimAmount,
      id: claimId,
      proof: claimProof,
    });

    // 5) Serialize to base64
    const serialized = tx.serialize({ requireAllSignatures: false });
    const base64Tx = serialized.toString('base64');

    // 6) Record claim as "pending" (prevents duplicate tx generation)
    // Update to "confirmed" when on-chain confirmation arrives
    await db.none(
      `INSERT INTO cls_claims (wallet, epoch_id, amount, tx_status)
       VALUES ($1, $2, $3, 'pending')
       ON CONFLICT (wallet, epoch_id) DO UPDATE
       SET amount = EXCLUDED.amount,
           tx_status = 'pending',
           tx_signature = NULL,
           confirmed_at = NULL`,
      [wallet, epochId, amountStr]
    );

    if (reissue) {
      console.log('  ↺ Reissuing pending claim entry');
    }

    // 7) Return transaction
    claimRequests.inc({ status: 'success' });
    endTimer();

    res.json({
      transaction: base64Tx,
      signature: null
    });

    // NOTE: Claim recorded as "pending". On-chain confirmation webhook should update to "confirmed"
    // and set tx_signature when transaction lands on-chain.
  } catch (err) {
    console.error('[postClaimCls] Error:', err);
    claimRequests.inc({ status: 'error' });
    endTimer();

    // Check for specific error types
    if (err instanceof Error) {
      if (err.message.includes('Epoch not found')) {
        return res.status(400).json({ error: 'Epoch not found' });
      }
      if (err.message.includes('Verification')) {
        return res.status(403).json({ error: 'Verification check failed' });
      }
      if (err.message.includes('Provided proof does not match epoch merkle root')) {
        return res.status(400).json({
          error: 'Invalid proof for epoch',
          details: err.message,
        });
      }
    }

    res.status(500).json({
      error: 'Internal server error'
    });
  }
}

export default postClaimCls;
