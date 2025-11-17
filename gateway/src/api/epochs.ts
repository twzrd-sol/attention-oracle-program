/**
 * GET /api/epochs
 *
 * Returns list of all epochs with status and details
 * - Supports filtering by status (open, closed, all)
 * - Includes claim count and merkle root
 * - Paginated with limit/offset
 */

import type { Request, Response } from 'express';
import { db } from '../db.js';

export interface Epoch {
  epochId: number;
  channel: string;
  status: 'open' | 'closed';
  merkleRoot: string;
  createdAt: string;
  closedAt: string | null;
  totalClaims: number;
  totalAllocated: string; // in lamports
}

export interface EpochsResponse {
  epochs: Epoch[];
  total: number;
  limit: number;
  offset: number;
}

export interface ErrorResponse {
  error: string;
}

/**
 * GET /api/epochs handler
 */
export async function getEpochs(
  req: Request,
  res: Response<EpochsResponse | ErrorResponse>
) {
  try {
    const {
      status = 'all',
      limit = '50',
      offset = '0',
    } = req.query;

    // Validate parameters
    const limitNum = Math.min(Math.max(parseInt(limit as string, 10) || 50, 1), 100);
    const offsetNum = Math.max(parseInt(offset as string, 10) || 0, 0);

    // Build WHERE clause based on status filter
    let whereClause = '';
    if (status === 'open') {
      whereClause = 'WHERE e.is_open = true';
    } else if (status === 'closed') {
      whereClause = 'WHERE e.is_open = false';
    }
    // 'all' = no WHERE clause

    // Query epochs with participant counts (from sealed_participants)
    const epochs = await db.manyOrNone<{
      epoch: number;
      channel: string;
      root: string;
      sealed_at: number;
      participant_count: number;
    }>(
      `SELECT
        se.epoch,
        se.channel,
        se.root,
        se.sealed_at,
        COUNT(sp.idx) AS participant_count
       FROM sealed_epochs se
       LEFT JOIN sealed_participants sp ON se.epoch = sp.epoch AND se.channel = sp.channel
       GROUP BY se.epoch, se.channel, se.root, se.sealed_at
       ORDER BY se.epoch DESC
       LIMIT $1 OFFSET $2`,
      [limitNum, offsetNum]
    );

    // Get total count (for pagination)
    const totalResult = await db.one<{ count: string }>(
      `SELECT COUNT(DISTINCT epoch, channel) AS count FROM sealed_epochs`
    );

    const total = parseInt(totalResult.count, 10);

    // Map to response format
    const mappedEpochs: Epoch[] = epochs.map((row) => ({
      epochId: row.epoch,
      channel: row.channel || 'unknown',
      status: 'open', // sealed_epochs are published, treat as open
      merkleRoot: row.root,
      createdAt: new Date(row.sealed_at * 1000).toISOString(),
      closedAt: null,
      totalClaims: row.participant_count || 0,
      totalAllocated: '0', // Not tracked in sealed_epochs
    }));

    res.json({
      epochs: mappedEpochs,
      total,
      limit: limitNum,
      offset: offsetNum,
    });
  } catch (err) {
    console.error('[getEpochs] Error:', err);
    res.status(500).json({ error: 'Internal server error' });
  }
}

export default getEpochs;
