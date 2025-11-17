/**
 * Prometheus Metrics for TWZRD Gateway
 *
 * Exposes /metrics endpoint for Prometheus scraping
 */

import { Request, Response } from 'express';
import client from 'prom-client';

// ===== Registry Setup =====

const register = new client.Registry();

// Collect default Node.js metrics (memory, CPU, event loop, etc.)
client.collectDefaultMetrics({ register });

// ===== Custom Metrics =====

/**
 * Counter: Total verification status requests
 */
export const verificationRequests = new client.Counter({
  name: 'twzrd_verification_requests_total',
  help: 'Total number of verification status API requests',
  labelNames: ['status'], // 'success', 'error'
  registers: [register],
});

/**
 * Counter: Total claim transaction requests
 */
export const claimRequests = new client.Counter({
  name: 'twzrd_claim_requests_total',
  help: 'Total number of claim transaction API requests',
  labelNames: ['status'], // 'success', 'duplicate', 'unverified', 'error'
  registers: [register],
});

/**
 * Histogram: Claim transaction build latency
 */
export const claimLatency = new client.Histogram({
  name: 'twzrd_claim_latency_seconds',
  help: 'Latency of building claim transactions',
  buckets: [0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0],
  registers: [register],
});

/**
 * Gauge: Last epoch sealed timestamp (Unix seconds)
 */
export const lastEpochSealed = new client.Gauge({
  name: 'twzrd_last_epoch_sealed_timestamp',
  help: 'Unix timestamp of the most recently sealed epoch',
  registers: [register],
});

/**
 * Gauge: Active viewers in current epoch
 */
export const activeViewers = new client.Gauge({
  name: 'twzrd_active_viewers',
  help: 'Number of active viewers in current epoch',
  labelNames: ['channel'],
  registers: [register],
});

// ===== Metrics Endpoint =====

/**
 * GET /metrics handler
 * Returns Prometheus-formatted metrics
 */
export async function metricsHandler(_req: Request, res: Response): Promise<void> {
  try {
    res.setHeader('Content-Type', register.contentType);
    const metrics = await register.metrics();
    res.send(metrics);
  } catch (error) {
    console.error('[Metrics] Error generating metrics:', error);
    res.status(500).send('Error generating metrics');
  }
}

/**
 * Optional: Update last_epoch_sealed from database
 * Call this periodically (e.g., every 60 seconds)
 */
export async function updateEpochMetrics(pool: any): Promise<void> {
  try {
    const result = await pool.one(
      'SELECT MAX(epoch) as last_epoch, MAX(sealed_at) as sealed_at FROM sealed_epochs'
    );

    if (result && result.sealed_at) {
      // Convert to Unix timestamp (seconds) if it's a Date object
      const timestamp = typeof result.sealed_at === 'object'
        ? Math.floor(result.sealed_at.getTime() / 1000)
        : Number(result.sealed_at);

      if (!isNaN(timestamp)) {
        lastEpochSealed.set(timestamp);
      }
    }
  } catch (error) {
    console.error('[Metrics] Error updating epoch metrics:', error);
  }
}

/**
 * Optional: Update active viewer count
 * Call this periodically or on-demand
 */
export async function updateViewerMetrics(pool: any): Promise<void> {
  try {
    const now = Math.floor(Date.now() / 1000);
    const currentEpoch = now - (now % 3600);

    const result = await pool.any(
      `SELECT channel, COUNT(DISTINCT user_hash) as count
       FROM channel_participation
       WHERE epoch = $1
       GROUP BY channel`,
      [currentEpoch]
    );

    // Reset all channels to 0 first (in case a channel has no activity)
    activeViewers.reset();

    for (const row of result) {
      activeViewers.set({ channel: row.channel }, parseInt(row.count));
    }
  } catch (error) {
    console.error('[Metrics] Error updating viewer metrics:', error);
  }
}

export { register };
