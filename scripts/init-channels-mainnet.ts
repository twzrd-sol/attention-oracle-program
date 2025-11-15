#!/usr/bin/env tsx
/**
 * Initialize channel_state PDAs for active channels on mainnet
 * Uses the aggregator's publishRootRing which auto-initializes channels
 */

import { publishRootRing } from '../apps/twzrd-aggregator/src/lib/publish.js';
import { Pool } from 'pg';
import dotenv from 'dotenv';
import { resolve } from 'path';

dotenv.config({ path: resolve(process.cwd(), '.env') });

const PROGRAM_ID = process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';
const MINT_PUBKEY = process.env.MINT_PUBKEY || 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5';
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const PAYER_KEYPAIR_PATH = process.env.HOME + '/.config/solana/id.json';

async function getChannelsToInitialize(): Promise<Array<{channel: string; epoch: number; root: string; count: number}>> {
  let DATABASE_URL = process.env.DATABASE_URL!;
  const url = new URL(DATABASE_URL);
  url.searchParams.delete('sslmode');
  DATABASE_URL = url.toString();

  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false }
  });

  const result = await pool.query(`
    SELECT
      se.channel,
      se.epoch,
      se.root,
      COUNT(sp.user_hash) as participant_count
    FROM sealed_epochs se
    JOIN sealed_participants sp
      ON se.epoch = sp.epoch
      AND se.channel = sp.channel
    WHERE se.published = 1
    GROUP BY se.channel, se.epoch, se.root
    ORDER BY se.channel ASC, se.epoch DESC
  `);

  await pool.end();

  // Get one epoch per channel (most recent)
  const channelMap = new Map<string, any>();
  for (const row of result.rows) {
    if (!channelMap.has(row.channel)) {
      channelMap.set(row.channel, {
        channel: row.channel,
        epoch: Number(row.epoch),
        root: row.root.replace(/^0x/, ''),
        count: Number(row.participant_count)
      });
    }
  }

  return Array.from(channelMap.values());
}

async function main() {
  console.log('🚀 Initializing Channel State PDAs');
  console.log('==================================');
  console.log('Program:', PROGRAM_ID);
  console.log('Mint:', MINT_PUBKEY);
  console.log('');

  const channels = await getChannelsToInitialize();
  console.log(`Found ${channels.length} channels to process`);
  console.log('');

  let successCount = 0;
  let errorCount = 0;

  for (const { channel, epoch, root, count } of channels) {
    try {
      console.log(`Initializing ${channel}...`);

      const signature = await publishRootRing({
        rpcUrl: RPC_URL,
        programId: PROGRAM_ID,
        mintPubkey: MINT_PUBKEY,
        payerKeypairPath: PAYER_KEYPAIR_PATH,
        channel,
        epoch,
        l2RootHex: root,
        claimCount: count
      });

      console.log(`✅ ${channel} - Initialized & root published!`);
      console.log(`   TX: ${signature}`);
      console.log(`   Explorer: https://explorer.solana.com/tx/${signature}`);
      successCount++;

      // Wait to avoid rate limits
      await new Promise(resolve => setTimeout(resolve, 2000));
    } catch (error: any) {
      console.error(`❌ ${channel} - Error:`, error.message);
      if (error.logs) {
        console.log('   Logs:', error.logs.slice(0, 3).join('\n   '));
      }
      errorCount++;
    }
    console.log('');
  }

  console.log('==================================');
  console.log('Summary:');
  console.log(`✅ Success: ${successCount}`);
  console.log(`❌ Errors: ${errorCount}`);
  console.log('');
  console.log('Channels are now ready for claims!');
}

main().catch(console.error);
