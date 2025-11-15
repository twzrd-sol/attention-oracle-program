#!/usr/bin/env node
/**
 * Test tree builder with hardened leaf hashing (includes claimer pubkey)
 * Generates a production-ready proof for end-to-end verification
 */
import { PublicKey } from '@solana/web3.js';
import { makeClaimLeaf, buildTreeWithLevels, generateProofFromLevels, hex } from './merkle.js';
import * as fs from 'fs';

// Test configuration - REAL WALLET FOR LOCALNET TEST
const TEST_CLAIMER = new PublicKey('2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD'); // Default Solana CLI wallet
const TEST_EPOCH = 1761818400n;
const TEST_CHANNEL = 'stableronaldo';

// Sample participants with known amounts
const TEST_PARTICIPANTS = [
  { username: 'alice', amount: 10000n, id: 'twitch:stableronaldo:alice' },
  { username: 'bob', amount: 15000n, id: 'twitch:stableronaldo:bob' },
  { username: 'charlie', amount: 20000n, id: 'twitch:stableronaldo:charlie' },
  { username: 'dave', amount: 12500n, id: 'twitch:stableronaldo:dave' },
  { username: 'eve', amount: 18000n, id: 'twitch:stableronaldo:eve' },
];

async function main() {
  console.log('🔧 Building hardened test tree with claimer pubkeys...\n');

  const claimerBytes = TEST_CLAIMER.toBytes();
  console.log(`Claimer: ${TEST_CLAIMER.toBase58()}`);
  console.log(`Claimer bytes (${claimerBytes.length}): ${hex(claimerBytes)}\n`);

  // Build leaves using hardened leaf function
  const leaves: Uint8Array[] = [];
  for (let i = 0; i < TEST_PARTICIPANTS.length; i++) {
    const participant = TEST_PARTICIPANTS[i];
    const leaf = makeClaimLeaf({
      claimer: claimerBytes,
      index: i,
      amount: participant.amount,
      id: participant.id,
    });
    leaves.push(leaf);
    console.log(`Leaf ${i} (${participant.username}): ${hex(leaf)}`);
    console.log(`  - claimer: ${TEST_CLAIMER.toBase58()}`);
    console.log(`  - index: ${i}`);
    console.log(`  - amount: ${participant.amount}`);
    console.log(`  - id: ${participant.id}\n`);
  }

  // Build tree
  console.log('Building merkle tree...');
  const { root, levels } = buildTreeWithLevels(leaves);
  const rootHex = hex(root);

  // Debug: Show tree structure
  console.log('\n📊 Tree structure:');
  for (let i = 0; i < levels.length; i++) {
    console.log(`  Level ${i}: ${levels[i].length} nodes`);
    if (levels[i].length <= 6) {
      levels[i].forEach((node, idx) => {
        console.log(`    [${idx}] ${hex(node).slice(0, 16)}...`);
      });
    }
  }
  console.log(`\n✅ Root: ${rootHex}`);
  console.log(`   (from levels[${levels.length-1}][0]: ${hex(levels[levels.length-1][0])})\n`);

  // Generate proof for first participant (index 0)
  const targetIndex = 0;
  const targetParticipant = TEST_PARTICIPANTS[targetIndex];
  const proof = generateProofFromLevels(levels, targetIndex);
  console.log(`Generated proof for index ${targetIndex} (${targetParticipant.username}):`);
  console.log(`Proof length: ${proof.length} nodes\n`);

  // Verify proof locally using same logic as on-chain
  let hash = leaves[targetIndex];
  console.log(`Starting hash (leaf ${targetIndex}): ${hex(hash)}`);
  for (let i = 0; i < proof.length; i++) {
    const sibling = proof[i];
    const [a, b] = Buffer.compare(Buffer.from(hash), Buffer.from(sibling)) <= 0
      ? [hash, sibling]
      : [sibling, hash];
    const { keccak_256 } = await import('@noble/hashes/sha3');
    const nextHash = keccak_256(Buffer.concat([Buffer.from(a), Buffer.from(b)]));
    console.log(`  Level ${i}: ${hex(hash).slice(0, 8)}... + ${hex(sibling).slice(0, 8)}...`);
    console.log(`         → [${hex(a).slice(0, 8)}..., ${hex(b).slice(0, 8)}...] → ${hex(nextHash).slice(0, 16)}...`);
    hash = nextHash;
  }
  console.log(`\nFinal hash:  ${hex(hash)}`);
  console.log(`Root:        ${rootHex}`);
  console.log(`✅ Verified: ${hex(hash) === rootHex}\n`);

  // Export claim as JSON
  const exportedClaim = {
    channel: TEST_CHANNEL,
    epoch: TEST_EPOCH.toString(),
    root: rootHex,
    claim_count: TEST_PARTICIPANTS.length,
    claimer: TEST_CLAIMER.toBase58(),
    index: targetIndex,
    amount: targetParticipant.amount.toString(),
    id: targetParticipant.id,
    proof: proof.map(p => hex(p)),
  };

  const outputPath = './test-claim-export.json';
  fs.writeFileSync(outputPath, JSON.stringify(exportedClaim, null, 2));
  console.log(`📄 Exported claim to: ${outputPath}\n`);
  console.log(JSON.stringify(exportedClaim, null, 2));

  console.log('\n✅ Test tree generation complete!');
  console.log('\nNext steps:');
  console.log('1. Replace TEST_CLAIMER with your test wallet pubkey');
  console.log('2. Run this script to regenerate the proof');
  console.log('3. Use clean-hackathon/scripts/claim-with-ring.ts to test on localnet');
}

main().catch(console.error);
