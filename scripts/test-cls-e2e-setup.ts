#!/usr/bin/env tsx
/**
 * test-cls-e2e-setup.ts
 *
 * Inserts test data for end-to-end CLS flow testing:
 * - 3 test users with wallets (weights: 10, 20, 30)
 * - sealed_participants row for channel "test-cls", epoch 424245
 * - weighted_participants rows with engagement weights
 * - user_mapping for usernames
 * - sealed_epochs entry
 *
 * Env: DATABASE_URL
 * Usage: npx tsx scripts/test-cls-e2e-setup.ts
 */

import { Pool } from 'pg';
import { Keypair } from '@solana/web3.js';
import { keccak_256 } from '@noble/hashes/sha3.js';

const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) {
  throw new Error('DATABASE_URL environment variable is required');
}

interface TestUser {
  username: string;
  weight: number;
  keypair: Keypair;
}

async function main() {
  const pool = new Pool({ connectionString: DATABASE_URL });

  try {
    console.log('🧪 Setting up CLS end-to-end test data...\n');

    // Create 3 test users with different weights
    const testUsers: TestUser[] = [
      {
        username: 'alice-test',
        weight: 10,
        keypair: Keypair.generate(),
      },
      {
        username: 'bob-test',
        weight: 20,
        keypair: Keypair.generate(),
      },
      {
        username: 'charlie-test',
        weight: 30,
        keypair: Keypair.generate(),
      },
    ];

    const channel = 'test-cls';
    const epoch = 424245;

    console.log(`Channel: ${channel}`);
    console.log(`Epoch: ${epoch}\n`);

    // 1. Insert into user_mapping (user_hash derived from wallet pubkey for this synthetic test)
    console.log('📝 Inserting user_mapping...');
    for (const user of testUsers) {
      const userHash = '0x' + Buffer.from(user.keypair.publicKey.toBytes()).toString('hex');
      await pool.query(
        `INSERT INTO user_mapping (user_hash, username, first_seen)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_hash) DO UPDATE SET username = EXCLUDED.username`,
        [userHash, user.username, Math.floor(Date.now() / 1000)]
      );
      console.log(`  ✓ ${user.username} → ${userHash}`);
    }

    // 2. Insert into sealed_participants (user_hash keyed by wallet pubkey)
    console.log('\n📝 Inserting sealed_participants...');
    for (let i = 0; i < testUsers.length; i++) {
      const user = testUsers[i];
      const userHash = '0x' + Buffer.from(user.keypair.publicKey.toBytes()).toString('hex');
      await pool.query(
        `INSERT INTO sealed_participants (epoch, channel, idx, user_hash, username)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (epoch, channel, idx) DO NOTHING`,
        [epoch, channel, i, userHash, user.username]
      );
      console.log(`  ✓ Index ${i}: ${user.username}`);
    }

    // 3. Insert into weighted_participants (same user_hash as sealed_participants)
    console.log('\n📝 Inserting weighted_participants...');
    for (const user of testUsers) {
      const userHash = '0x' + Buffer.from(user.keypair.publicKey.toBytes()).toString('hex');
      await pool.query(
        `INSERT INTO weighted_participants (channel, epoch, user_hash, weight)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (channel, epoch, user_hash) DO UPDATE SET weight = $4`,
        [channel, epoch, userHash, user.weight]
      );
      console.log(`  ✓ ${user.username}: weight ${user.weight}`);
    }

    // 4. Insert into sealed_epochs
    console.log('\n📝 Inserting sealed_epochs...');
    const placeholderRoot = '0x' + '00'.repeat(32); // Will be updated by build-allocations
    await pool.query(
      `INSERT INTO sealed_epochs (epoch, channel, root, sealed_at, published)
       VALUES ($1, $2, $3, $4, 0)
       ON CONFLICT ON CONSTRAINT sealed_epochs_pkey DO UPDATE SET root = $3`,
      [epoch, channel, placeholderRoot, Math.floor(Date.now() / 1000)]
    );
    console.log(`  ✓ Epoch ${epoch} for channel ${channel}`);

    // 5. Output test wallets and keypairs for claims.csv
    console.log('\n\n🔑 Test Wallets & Keypairs (save for later):\n');
    console.log('wallet,epochs,keypair_path');
    for (let i = 0; i < testUsers.length; i++) {
      const user = testUsers[i];
      const keypairPath = `/tmp/test-cls-wallet-${i}.json`;
      const secret = Array.from(user.keypair.secretKey);
      fs.writeFileSync(keypairPath, JSON.stringify(secret));
      console.log(`${user.keypair.publicKey.toBase58()},${epoch},${keypairPath}`);
    }

    console.log('\n\n✅ Test data inserted successfully!');
    console.log('\nNext steps:');
    console.log(`1. Run: npx tsx scripts/build-allocations-for-epoch.ts --channel ${channel} --epoch ${epoch}`);
    console.log(`2. Create claims.csv with the wallets above`);
    console.log(`3. Run: npx tsx scripts/allocate-and-claim.ts --csv claims.csv`);

    await pool.end();
  } catch (error) {
    console.error('❌ Error:', error);
    await pool.end();
    process.exit(1);
  }
}

import fs from 'fs';
main();
