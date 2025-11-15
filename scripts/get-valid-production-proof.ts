#!/usr/bin/env tsx
import { Pool } from 'pg';
import { keccak_256 } from '@noble/hashes/sha3';
import { MerkleTree } from 'merkletreejs';
import fs from 'fs';

// --- Config ---
const CHANNEL = 'jasontheween';
const EPOCH = 1762362000;
const PROOF_FILE = '/tmp/claim-jasontheween-valid-proof.json';
const TEST_INDEX = 5; // Use a low, stable index (< 1024)

// --- Hashing functions (must match on-chain & aggregator) ---
function hashLeaf(leaf: Buffer): Buffer {
  return Buffer.from(keccak_256(leaf));
}

function makeParticipationLeaf(params: {
  user_hash: string;
  channel: string;
  epoch: number;
}): Buffer {
  const userHashBytes = Buffer.from(params.user_hash, 'hex');
  const channelBytes = Buffer.from(params.channel.toLowerCase(), 'utf8');
  const epochBytes = Buffer.alloc(8);
  epochBytes.writeBigUInt64LE(BigInt(params.epoch));

  // This hash must match the L2 builder's leaf generation
  // keccak256(user_hash || channel || epoch)
  return hashLeaf(Buffer.concat([userHashBytes, channelBytes, epochBytes]));
}

// --- Main execution ---
async function main() {
  console.log(`\n🔍 Generating valid proof for:`);
  console.log(`   Channel: ${CHANNEL}`);
  console.log(`   Epoch: ${EPOCH}`);

  const pool = new Pool({
    connectionString: process.env.DATABASE_URL,
    ssl: { rejectUnauthorized: false } // Required for DigitalOcean managed DB
  });
  const client = await pool.connect();

  try {
    // 1. Fetch all participants for this epoch
    console.log('Fetching participants from DB...');
    const res = await client.query(
      `SELECT user_hash FROM sealed_participants
       WHERE channel = $1 AND epoch = $2
       ORDER BY user_hash ASC`, // IMPORTANT: Must match L2 builder's sort order
      [CHANNEL, EPOCH]
    );

    const participants = res.rows.map(r => r.user_hash);
    if (participants.length === 0) {
      throw new Error('No participants found for this epoch.');
    }
    console.log(`   Found ${participants.length} participants.`);

    // 2. Select a valid test candidate
    if (participants.length <= TEST_INDEX) {
      throw new Error(`Not enough participants to test index ${TEST_INDEX}.`);
    }
    const testUserHash = participants[TEST_INDEX];
    console.log(`   Using test user at index ${TEST_INDEX}: ${testUserHash.slice(0, 12)}...`);

    // 3. Build the Merkle tree (off-chain)
    console.log('Building Merkle tree...');
    const leaves = participants.map(user_hash =>
      makeParticipationLeaf({ user_hash, channel: CHANNEL, epoch: EPOCH })
    );

    // Note: The L2 builder might sort leaves *before* hashing.
    // We must replicate that exact behavior.
    const tree = new MerkleTree(leaves, keccak_256, {
      hashLeaves: false, // We are providing already hashed leaves
      sort: true,        // IMPORTANT: Must match L2 builder
    });
    const root = tree.getRoot().toString('hex');

    // 4. Generate the proof
    const testLeaf = leaves[TEST_INDEX];
    const proof = tree.getProof(testLeaf).map(p => '0x' + p.data.toString('hex'));

    // 5. Verify proof locally
    if (!tree.verify(tree.getProof(testLeaf), testLeaf, tree.getRoot())) {
      throw new Error('Local proof verification failed!');
    }
    console.log(`   Local proof verified!`);

    // 6. Check against the on-chain root
    const onChainRootRes = await client.query(
      `SELECT root FROM sealed_epochs
       WHERE channel = $1 AND epoch = $2`,
      [CHANNEL, EPOCH]
    );
    if (onChainRootRes.rows.length === 0) {
      throw new Error('Epoch not found in sealed_epochs!');
    }
    const onChainRoot = onChainRootRes.rows[0].root;

    if (root !== onChainRoot) {
      console.error('ROOT MISMATCH!');
      console.error(`   Off-chain Root: ${root}`);
      console.error(`   On-chain Root:  ${onChainRoot}`);
      throw new Error('Merkle root mismatch. Data desync. The tree logic is wrong.');
    }
    console.log(`   ✅ Root matches on-chain root: 0x${root.slice(0, 12)}...`);

    // 7. Save the proof data
    const proofData = {
      channel: CHANNEL,
      epoch: EPOCH,
      root: '0x' + root,
      index: TEST_INDEX,
      amount: "1000000000", // 1 MILO (test amount)
      // This ID must match the leaf structure for the on-chain program
      id: testUserHash, // Just the user hash, no prefix
      proof: proof,
      participant: testUserHash // For reference
    };

    fs.writeFileSync(PROOF_FILE, JSON.stringify(proofData, null, 2));
    console.log(`\n✅ Success! Valid proof saved to ${PROOF_FILE}`);

  } finally {
    client.release();
    await pool.end();
  }
}

main().catch(console.error);
