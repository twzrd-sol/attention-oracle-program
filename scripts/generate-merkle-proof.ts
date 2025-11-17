import { createHash } from 'crypto';
import * as bs58 from 'bs58';

// Test allocation (single entry)
const allocation = [
  {
    wallet: "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    amount: "100000000000",  // 100 CCM with 9 decimals
    index: 0
  }
];

// Helper: sha256 hash
function sha256(data: Buffer): Buffer {
  return createHash('sha256').update(data).digest();
}

// Build leaf nodes: hash(index || wallet || amount)
const leaves = allocation.map((entry) => {
  const buffer = Buffer.alloc(8 + 32 + 32); // index (8) + wallet pubkey (32) + amount (32)

  // Index (little-endian u64)
  buffer.writeBigUInt64LE(BigInt(entry.index), 0);

  // Wallet (base58 decode to bytes)
  const walletBytes = Buffer.from(bs58.default.decode(entry.wallet));
  walletBytes.copy(buffer, 8);

  // Amount (as bytes, u128)
  const amountBN = BigInt(entry.amount);
  for (let i = 0; i < 16; i++) {
    buffer[24 + i] = Number((amountBN >> BigInt(i * 8)) & BigInt(0xff));
  }

  const leaf = sha256(buffer);
  return leaf;
});

// For single-entry tree, proof is empty (root = leaf)
const root = leaves[0];
const proof = []; // No proof needed for single entry - leaf IS the root

console.log('=== Merkle Proof Generation ===\n');
console.log('Allocation:');
console.log(JSON.stringify(allocation, null, 2));
console.log('\nLeaf (hex):');
console.log(`  ${leaves[0].toString('hex')}`);
console.log('\nMerkle Root (hex):');
console.log(`  ${root.toString('hex')}`);
console.log('\nMerkle Root (base58):');
console.log(`  ${bs58.default.encode(root)}`);
console.log('\nProof Path (for on-chain verification):');
console.log(`  [${proof.map(p => `"${p.toString('hex')}"`).join(', ')}]`);
console.log('  (Empty for single-entry tree - leaf IS root)');

console.log('\n=== Claim Parameters ===');
console.log(`Channel: claim-0001-test`);
console.log(`Epoch: 424242`);
console.log(`Wallet: ${allocation[0].wallet}`);
console.log(`Amount: ${allocation[0].amount} (100 CCM)`);
console.log(`Index: ${allocation[0].index}`);
console.log(`Merkle Root: ${bs58.default.encode(root)}`);
console.log(`Proof: []`);
