import { keccak_256 } from '@noble/hashes/sha3';
import * as bs58 from 'bs58';

// Test allocation - must match program's compute_leaf exactly
const allocation = [
  {
    wallet: "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    amount: "100000000000",  // 100 CCM with 9 decimals
    index: 0,
    id: "claim-0001"  // CRITICAL: id parameter
  }
];

// Build leaf: keccak256(claimer || index_u32_le || amount_u64_le || id_bytes)
const leaves = allocation.map((entry) => {
  // Decode wallet pubkey
  const claimer = Buffer.from(bs58.default.decode(entry.wallet));
  if (claimer.length !== 32) throw new Error(`Invalid claimer length: ${claimer.length}`);

  // index as u32 little-endian (4 bytes)
  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(entry.index, 0);

  // amount as u64 little-endian (8 bytes)
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(BigInt(entry.amount), 0);

  // id as bytes
  const idBuf = Buffer.from(entry.id, 'utf8');

  // Concatenate: claimer || index || amount || id
  const leaf = Buffer.concat([claimer, indexBuf, amountBuf, idBuf]);
  const leafHash = keccak_256(leaf);

  console.log(`\nLeaf construction for ${entry.wallet}:`);
  console.log(`  Claimer (32): ${claimer.toString('hex')}`);
  console.log(`  Index u32LE (4): ${indexBuf.toString('hex')}`);
  console.log(`  Amount u64LE (8): ${amountBuf.toString('hex')}`);
  console.log(`  ID bytes: ${idBuf.toString('hex')} ("${entry.id}")`);
  console.log(`  Concatenated: ${leaf.toString('hex')}`);
  console.log(`  Leaf hash: ${leafHash.toString('hex')}`);

  return leafHash;
});

// For single-entry tree, root = leaf
const root = leaves[0];
const proof = []; // No proof needed for single entry

console.log('\n=== CORRECT Merkle Proof (Keccak256) ===\n');
console.log('Allocation:');
console.log(JSON.stringify(allocation, null, 2));

console.log('\nMerkle Root (hex):');
console.log(`  ${root.toString('hex')}`);
console.log('\nMerkle Root (base58):');
console.log(`  ${bs58.default.encode(root)}`);

console.log('\nProof Path (for on-chain verification):');
console.log(`  [${proof.map(p => `"${p.toString('hex')}"`).join(', ')}]`);
console.log('  (Empty for single-entry tree)');

console.log('\n=== Claim Parameters ===');
console.log(`Channel: claim-0001-test`);
console.log(`Epoch: 424242`);
console.log(`Wallet: ${allocation[0].wallet}`);
console.log(`Amount: ${allocation[0].amount} (100 CCM)`);
console.log(`Index: ${allocation[0].index}`);
console.log(`ID: ${allocation[0].id}`);
console.log(`Merkle Root (base58): ${bs58.default.encode(root)}`);
console.log(`Proof: []`);
