import { createHash } from 'crypto';
import * as bs58 from 'bs58';

// Read allocation
const allocation = [
  {
    wallet: "DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1",
    amount: "100000000000",
    index: 0
  }
];

// Helper: sha256 hash (simplified - real implementation would use keccak256)
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

// For single-entry tree, root = leaf
const root = leaves[0];

console.log('=== Merkle Tree Generation ===\n');
console.log('Allocation:');
console.log(JSON.stringify(allocation, null, 2));
console.log('\nLeaves:');
leaves.forEach((leaf, i) => {
  console.log(`  Leaf ${i}: ${leaf.toString('hex')}`);
});
console.log('\nMerkle Root (hex):');
console.log(root.toString('hex'));
console.log('\nMerkle Root (base58 - for on-chain):');
console.log(bs58.default.encode(root));
console.log('\n=== Test Epoch Parameters ===');
console.log('Channel: claim-0001-test');
console.log('Epoch: 424242');
console.log('Merkle Root: ' + bs58.default.encode(root));
