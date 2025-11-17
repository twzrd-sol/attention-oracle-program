#!/usr/bin/env node
import pg from 'pg';
import { keccak_256 } from '@noble/hashes/sha3.js';

const { Pool } = pg;

// Database connection
const pool = new Pool({
  connectionString: 'postgresql://doadmin:AVNS_7OLyCRhJkIPcAKrZMoi@twzrd-prod-postgres-do-user-21113270-0.f.db.ondigitalocean.com:25061/twzrd-oracle-pool',
  ssl: { rejectUnauthorized: false }
});

//
// Merkle tree functions
//

function makeParticipationLeaf(params) {
  const { user_hash, channel, epoch } = params;
  const userHashBytes = Buffer.from(user_hash, 'hex');
  const channelBytes = Buffer.from(channel, 'utf8');
  const epochBytes = Buffer.alloc(8);
  epochBytes.writeBigUInt64LE(BigInt(epoch), 0);
  const preimage = Buffer.concat([userHashBytes, channelBytes, epochBytes]);
  return keccak_256(preimage);
}

function merkleRoot(leaves) {
  if (leaves.length === 0) throw new Error('No leaves');
  if (leaves.length === 1) return leaves[0];
  let level = [...leaves];
  while (level.length > 1) {
    const nextLevel = [];
    for (let i = 0; i < level.length; i += 2) {
      const left = level[i];
      const right = level[i + 1] || left;
      const [first, second] = Buffer.compare(Buffer.from(left), Buffer.from(right)) <= 0 ? [left, right] : [right, left];
      nextLevel.push(keccak_256(Buffer.concat([first, second])));
    }
    level = nextLevel;
  }
  return level[0];
}

function generateProof(leaves, targetIndex) {
  const proof = [];
  let level = [...leaves];
  let idx = targetIndex;
  while (level.length > 1) {
    const siblingIdx = idx % 2 === 0 ? idx + 1 : idx - 1;
    if (siblingIdx < level.length) proof.push(level[siblingIdx]);
    else proof.push(level[idx]);
    const nextLevel = [];
    for (let i = 0; i < level.length; i += 2) {
      const left = level[i];
      const right = level[i + 1] || left;
      const [first, second] = Buffer.compare(Buffer.from(left), Buffer.from(right)) <= 0 ? [left, right] : [right, left];
      nextLevel.push(keccak_256(Buffer.concat([first, second])));
    }
    level = nextLevel;
    idx = Math.floor(idx / 2);
  }
  return proof;
}

async function main() {
  try {
    console.log('Fetching marlon participants...');
    const result = await pool.query(`
      SELECT user_hash FROM sealed_participants
      WHERE epoch = 1762308000 AND channel = 'marlon' AND token_group = 'MILO' AND category = 'default'
      ORDER BY idx ASC
    `);
    console.log(`✅ Found ${result.rows.length} participants`);
    
    const leaves = result.rows.map(p => makeParticipationLeaf({ user_hash: p.user_hash, channel: 'marlon', epoch: 1762308000 }));
    const root = Buffer.from(merkleRoot(leaves)).toString('hex');
    console.log(`✅ Root: ${root}`);
    
    const expected = '6fce67da102af54283b0deb46e6d1880fb7670e6bbff240c149234f6333ee3b0';
    if (root === expected) {
      console.log('✅ ROOT MATCHES!');
      const proof = generateProof(leaves, 0);
      console.log(`✅ Proof generated (${proof.length} siblings)`);
      const proofHex = proof.map(p => Buffer.from(p).toString('hex'));
      const data = { channel: 'marlon', epoch: 1762308000, root: `0x${root}`, proof: proofHex.map(p => `0x${p}`), participant: result.rows[0].user_hash };
      const fs = await import('fs/promises');
      await fs.writeFile('/tmp/marlon-test-proof.json', JSON.stringify(data, null, 2));
      console.log('✅ Saved to /tmp/marlon-test-proof.json');
    } else {
      console.log(`❌ MISMATCH! Got ${root}, expected ${expected}`);
    }
  } catch (err) {
    console.error(err);
  } finally {
    await pool.end();
  }
}

main();
