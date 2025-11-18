import { Keccak } from 'sha3';
import { PublicKey } from '@solana/web3.js';

/**
 * Compute merkle leaf hash
 */
export function computeLeaf(
  claimer: PublicKey,
  index: number,
  amount: bigint,
  id: string
): Buffer {
  const hasher = new Keccak(256);

  const indexBytes = Buffer.alloc(4);
  indexBytes.writeUInt32LE(index);

  const amountBytes = Buffer.alloc(8);
  amountBytes.writeBigUInt64LE(amount);

  hasher.update(claimer.toBuffer());
  hasher.update(indexBytes);
  hasher.update(amountBytes);
  hasher.update(Buffer.from(id, 'utf-8'));

  return hasher.digest();
}

/**
 * Verify merkle proof
 */
export function verifyMerkleProof(
  leaf: Buffer,
  proof: Buffer[],
  root: Buffer
): boolean {
  let computedHash = leaf;

  for (const proofElement of proof) {
    if (Buffer.compare(computedHash, proofElement) < 0) {
      computedHash = keccakHash([computedHash, proofElement]);
    } else {
      computedHash = keccakHash([proofElement, computedHash]);
    }
  }

  return Buffer.compare(computedHash, root) === 0;
}

/**
 * Keccak256 hash helper
 */
export function keccakHash(inputs: Buffer[]): Buffer {
  const hasher = new Keccak(256);
  inputs.forEach(input => hasher.update(input));
  return hasher.digest();
}

/**
 * Parse tier multiplier from basis points
 */
export function parseTierMultiplier(bps: number): number {
  return bps / 10000;
}

/**
 * Calculate dynamic fee for a tier
 */
export function calculateDynamicFee(
  baseFeeBps: number,
  tierMultiplier: number
): number {
  return Math.floor(baseFeeBps * tierMultiplier);
}
