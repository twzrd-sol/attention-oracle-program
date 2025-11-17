#!/usr/bin/env tsx
/**
 * build-allocations-for-epoch.ts
 *
 * On-demand script: for a given channel+epoch,
 * - Joins sealed_participants + weighted_participants + user_mapping
 * - Computes amounts using BASE_PER_WEIGHT=80 logic
 * - Builds CLS Merkle tree with real claimer pubkeys
 * - Generates proofs and inserts into allocations for /api/claim-cls
 *
 * Env: DATABASE_URL – L2 Postgres
 * Usage: npx tsx scripts/build-allocations-for-epoch.ts --channel <name> --epoch <id>
 */

import { Pool } from 'pg';
import { PublicKey } from '@solana/web3.js';
import { makeClaimLeaf, buildTreeWithLevels, generateProofFromLevels } from '../apps/twzrd-aggregator/dist/merkle.js';

interface Participant {
  index: number;
  user_hash: string;
  weight: number;
  wallet?: string;
  username?: string;
}

async function main() {
  const DATABASE_URL = process.env.DATABASE_URL;
  if (!DATABASE_URL) {
    throw new Error('DATABASE_URL environment variable is required');
  }

  const pool = new Pool({ connectionString: DATABASE_URL });

  try {
    const args = process.argv.slice(2);
    const channelIdx = args.findIndex(a => a === '--channel' || a === '-c');
    const epochIdx = args.findIndex(a => a === '--epoch' || a === '-e');

    if (channelIdx === -1 || !args[channelIdx + 1]) {
      throw new Error('Require --channel <name>');
    }
    if (epochIdx === -1 || !args[epochIdx + 1]) {
      throw new Error('Require --epoch <id>');
    }

    const channel = args[channelIdx + 1];
    const epoch = Number(args[epochIdx + 1]);

    if (!Number.isInteger(epoch)) {
      throw new Error(`Invalid epoch: ${args[epochIdx + 1]}`);
    }

    console.log(`\n🔨 Building allocations for ${channel} epoch ${epoch}...\n`);

    // 1. Fetch participants + weights + wallets
    const res = await pool.query<Participant>(
      `
      SELECT
        sp.idx as index,
        sp.user_hash,
        wp.weight,
        um.username
      FROM sealed_participants sp
      LEFT JOIN weighted_participants wp
        ON sp.user_hash = wp.user_hash
        AND wp.channel = $1
        AND wp.epoch = $2
      LEFT JOIN user_mapping um
        ON sp.user_hash = um.user_hash
      WHERE sp.channel = $1 AND sp.epoch = $2
      ORDER BY sp.idx ASC
      LIMIT 1024
      `,
      [channel, epoch]
    );

    if (res.rows.length === 0) {
      console.log('  ⚠️  No participants found. Check sealed_participants table.');
      await pool.end();
      return;
    }

    console.log(`  📊 Found ${res.rows.length} participants`);

    // 2. Build Merkle tree (single pass with real amounts)
    const BASE_PER_WEIGHT = 80;
    const DECIMALS = 9;
    const MAX_CLAIMS = 1024;
    const limited = res.rows.slice(0, MAX_CLAIMS);

    type Item = { index: number; wallet: string; amount: bigint; id: string };
    const items: Item[] = [];

    for (const row of limited) {
      let walletStr = row.wallet;
      if (!walletStr && row.user_hash && /^0x[0-9a-fA-F]{64}$/.test(row.user_hash)) {
        try {
          walletStr = new PublicKey(Buffer.from(row.user_hash.slice(2), 'hex')).toBase58();
        } catch {
          // ignore
        }
      }
      if (!walletStr) {
        console.warn(`    ⚠️  Skipping index ${row.index}: no wallet found for user_hash ${row.user_hash}`);
        continue;
      }

      const weight = row.weight || 1;
      const amount = BigInt(Math.round(weight * BASE_PER_WEIGHT * Math.pow(10, DECIMALS)));
      const id = `twitch:${channel}:${(row.username || row.user_hash.slice(2, 18)).toLowerCase()}`;
      items.push({ index: row.index, wallet: walletStr, amount, id });
    }

    if (items.length === 0) {
      throw new Error('No valid participants to build tree');
    }

    console.log(`\n  🌳 Building Merkle tree...`);
    const leaves: Buffer[] = items.map((it) =>
      makeClaimLeaf({
        claimer: new PublicKey(it.wallet).toBytes(),
        index: it.index,
        amount: it.amount,
        id: it.id,
      })
    );

    const { root: rootBytes, levels: treeLevels } = buildTreeWithLevels(leaves);
    const root = '0x' + Buffer.from(rootBytes).toString('hex');
    console.log(`  ✅ Tree root: ${root}`);

    // 3. Second pass: generate proofs with real amounts and insert allocations
    console.log(`\n  💾 Inserting allocations...`);
    let inserted = 0;

    for (const it of items) {
      const proof = generateProofFromLevels(treeLevels, it.index);
      const proofHex = proof.map((b) => Buffer.from(b).toString('hex'));

      try {
        await pool.query(
          `
          INSERT INTO allocations (epoch_id, wallet, index, amount, id, proof_json)
          VALUES ($1, $2, $3, $4, $5, $6)
          ON CONFLICT (epoch_id, wallet) DO UPDATE
          SET amount = EXCLUDED.amount,
              id = EXCLUDED.id,
              proof_json = EXCLUDED.proof_json
          `,
          [epoch, it.wallet, it.index, it.amount.toString(), it.id, JSON.stringify(proofHex)]
        );
        inserted++;
        console.log(`    ✓ ${it.wallet}: ${it.amount} tokens`);
      } catch (err) {
        console.error(`    ❌ Failed to insert allocation for ${it.wallet}:`, err);
        throw err;
      }
    }

    // 4. Update sealed_epochs.root to match allocations tree
    console.log(`\n  📦 Updating sealed_epochs root...`);
    await pool.query(
      `
      UPDATE sealed_epochs
      SET root = $3
      WHERE epoch = $1 AND channel = $2
      `,
      [epoch, channel, root]
    );
    console.log(`    ✓ sealed_epochs updated with root ${root}`);

    console.log(`\n✅ Build complete!`);
    console.log(`\n   Summary:`);
    console.log(`   • Inserted: ${inserted} allocations`);
    console.log(`   • Root: ${root}`);
    console.log(`   • Ready for: npx tsx scripts/allocate-and-claim.ts --csv claims.csv\n`);

    await pool.end();
  } catch (error) {
    console.error('\n❌ Error:', error instanceof Error ? error.message : error);
    await pool.end();
    process.exit(1);
  }
}

main();
