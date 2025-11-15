#!/usr/bin/env tsx
/**
 * Quick batch publisher for MILO epochs that already have L2 cache
 * Publishes via aggregator API to avoid RPC flakiness
 */

import 'dotenv/config';
import { Pool } from 'pg';

const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:6432/twzrd_oracle';
const AGGREGATOR_URL = process.env.AGGREGATOR_URL || 'http://127.0.0.1:8080';

interface ReadyEpoch {
  channel: string;
  epoch: number;
  root: string;
  participant_count: number;
}

async function getReadyEpochs(pool: Pool): Promise<ReadyEpoch[]> {
  const result = await pool.query(`
    SELECT
      se.channel,
      se.epoch,
      l2.root,
      l2.participant_count
    FROM sealed_epochs se
    JOIN l2_tree_cache l2 ON se.epoch = l2.epoch AND se.channel = l2.channel
    WHERE (se.published IS NULL OR se.published = 0)
      AND l2.root IS NOT NULL
    ORDER BY se.sealed_at ASC
    LIMIT 12
  `);

  return result.rows.map(r => ({
    channel: r.channel,
    epoch: r.epoch,
    root: r.root,
    participant_count: r.participant_count,
  }));
}

async function publishEpoch(epoch: ReadyEpoch): Promise<boolean> {
  try {
    // Use aggregator's publish endpoint
    const url = `${AGGREGATOR_URL}/publish-channel-root`;

    const response = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        channel: epoch.channel,
        epoch: epoch.epoch,
        token_group: 'MILO',
        category: 'default',
      }),
    });

    if (!response.ok) {
      const text = await response.text();
      console.error(`❌ ${epoch.channel} epoch ${epoch.epoch} - HTTP ${response.status}: ${text}`);
      return false;
    }

    const result = await response.json();
    console.log(`✅ ${epoch.channel} epoch ${epoch.epoch} - ${result.signature || 'published'}`);
    return true;
  } catch (err) {
    console.error(`❌ ${epoch.channel} epoch ${epoch.epoch} - ${err}`);
    return false;
  }
}

async function main() {
  const pool = new Pool({
    connectionString: DATABASE_URL,
    max: 2,
  });

  try {
    const ready = await getReadyEpochs(pool);

    console.log(`🚀 Publishing ${ready.length} ready MILO epochs with L2 cache`);
    console.log('');

    let published = 0;
    let failed = 0;

    for (const epoch of ready) {
      const success = await publishEpoch(epoch);
      if (success) {
        published++;
      } else {
        failed++;
      }

      // Small delay to avoid overwhelming RPC
      await new Promise(resolve => setTimeout(resolve, 1000));
    }

    console.log('');
    console.log(`📊 Results: ${published} published, ${failed} failed`);
  } finally {
    await pool.end();
  }
}

main().catch(console.error);
