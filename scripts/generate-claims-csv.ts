#!/usr/bin/env tsx
/**
 * generate-claims-csv.ts
 *
 * Generates claims.csv from allocations table for a given epoch.
 * Keypairs are expected to exist in the specified keypair directory.
 *
 * Env: DATABASE_URL, KEYPAIR_DIR (optional, default /tmp)
 * Usage: npx tsx scripts/generate-claims-csv.ts --epoch 424245 --output claims.csv
 */

import { Pool } from 'pg';
import fs from 'fs';
import path from 'path';

async function main() {
  const DATABASE_URL = process.env.DATABASE_URL;
  if (!DATABASE_URL) {
    throw new Error('DATABASE_URL environment variable is required');
  }

  const pool = new Pool({ connectionString: DATABASE_URL });

  try {
    const args = process.argv.slice(2);
    const epochIdx = args.findIndex(a => a === '--epoch' || a === '-e');
    const outputIdx = args.findIndex(a => a === '--output' || a === '-o');

    if (epochIdx === -1 || !args[epochIdx + 1]) {
      throw new Error('Require --epoch <id>');
    }

    const epoch = Number(args[epochIdx + 1]);
    const outputFile = args[outputIdx + 1] || 'claims.csv';

    if (!Number.isInteger(epoch)) {
      throw new Error(`Invalid epoch: ${args[epochIdx + 1]}`);
    }

    console.log(`\n📋 Generating claims.csv for epoch ${epoch}...\n`);

    // Fetch all allocations for this epoch
    const res = await pool.query(
      `SELECT DISTINCT wallet FROM allocations WHERE epoch_id = $1 ORDER BY wallet`,
      [epoch]
    );

    if (res.rows.length === 0) {
      console.log('  ⚠️  No allocations found for this epoch.');
      await pool.end();
      return;
    }

    const wallets = res.rows.map((r: any) => r.wallet);
    console.log(`  Found ${wallets.length} wallets\n`);

    // Generate CSV
    const lines = ['wallet,epochs,keypair_path'];
    let foundCount = 0;

    for (const wallet of wallets) {
      // Look for keypair file (common patterns)
      const patterns = [
        `/tmp/test-cls-wallet-*.json`,
        `/tmp/${wallet}.json`,
        `${process.env.HOME}/.config/solana/${wallet}.json`,
      ];

      let keypairPath = null;
      for (const pattern of patterns) {
        if (pattern.includes('*')) {
          // Glob pattern - list /tmp and match
          try {
            const dir = path.dirname(pattern);
            const prefix = path.basename(pattern).replace('*', '');
            const files = fs.readdirSync(dir);
            const match = files.find(f => f.startsWith('test-cls-wallet-') && f.endsWith('.json'));
            if (match) {
              keypairPath = path.join(dir, match);
              break;
            }
          } catch {
            // dir doesn't exist
          }
        } else if (fs.existsSync(pattern)) {
          keypairPath = pattern;
          break;
        }
      }

      if (!keypairPath) {
        console.log(`  ⚠️  No keypair found for ${wallet}`);
        // Still add it with a placeholder - user can fix manually
        keypairPath = `/path/to/${wallet}.json`;
      }

      lines.push(`${wallet},${epoch},${keypairPath}`);
      foundCount++;
    }

    fs.writeFileSync(outputFile, lines.join('\n'));
    console.log(`✅ Written ${foundCount} claims to ${outputFile}\n`);
    console.log('Preview:');
    console.log(lines.slice(0, Math.min(5, lines.length)).join('\n'));
    if (lines.length > 5) {
      console.log(`... and ${lines.length - 5} more\n`);
    }

    console.log('\nNext: npx tsx scripts/allocate-and-claim.ts --csv ' + outputFile + '\n');

    await pool.end();
  } catch (error) {
    console.error('❌ Error:', error instanceof Error ? error.message : error);
    await pool.end();
    process.exit(1);
  }
}

main();
