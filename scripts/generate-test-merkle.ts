import { keccak_256 } from '@noble/hashes/sha3';
import * as bs58 from 'bs58';

const wallet = "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD";
const amount = "100000000"; // 100M tokens (with 9 decimals = 100 tokens)
const index = 0;

// Build leaf: hash(wallet || index || amount || id)
const id = "test-claim-424245";
const idBuf = Buffer.from(id, 'utf8');

// Wallet (base58 decode, 32 bytes)
const walletBytes = Buffer.from(bs58.default.decode(wallet));

// Index (little-endian u32, 4 bytes)
const indexBuf = Buffer.alloc(4);
indexBuf.writeUInt32LE(index, 0);

// Amount (little-endian u64, 8 bytes)
const amountBuf = Buffer.alloc(8);
amountBuf.writeBigUInt64LE(BigInt(amount), 0);

// Leaf preimage: wallet || index || amount || id
const buffer = Buffer.concat([walletBytes, indexBuf, amountBuf, idBuf]);

const leaf = Buffer.from(keccak_256(buffer));
const root = leaf; // Single-entry tree: root = leaf

console.log('Wallet:', wallet);
console.log('Amount:', amount);
console.log('Index:', index);
console.log('ID:', id);
console.log('\nLeaf preimage (hex):', buffer.toString('hex'));
console.log('Leaf (hex):', leaf.toString('hex'));
console.log('Root (hex):', root.toString('hex'));
console.log('\n--- SQL Update Command ---');
console.log(`UPDATE epochs SET merkle_root = '${root.toString('hex')}' WHERE epoch_id = 424245;`);
