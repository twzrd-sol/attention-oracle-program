import { keccak_256 } from '@noble/hashes/sha3'

/**
 * Compute ring claim leaf: keccak256(claimer || index || amount || id_bytes)
 * - claimer: 32-byte wallet public key
 * - index: u32 LE
 * - amount: u64 LE (BigInt supported)
 * - id: UTF-8 string (<=32 bytes recommended)
 */
export function computeClaimLeaf(claimer: Uint8Array, index: number, amount: bigint | number, id: string): Uint8Array {
  if (!(claimer instanceof Uint8Array) || claimer.length !== 32) {
    throw new Error(`claimer must be 32 bytes, got ${claimer?.length}`)
  }
  const idxBuf = Buffer.alloc(4)
  idxBuf.writeUInt32LE(index >>> 0, 0)
  const amt = typeof amount === 'bigint' ? amount : BigInt(amount)
  const amtBuf = Buffer.alloc(8)
  amtBuf.writeBigUInt64LE(amt, 0)
  const idBuf = Buffer.from(id ?? '', 'utf8')
  const pre = Buffer.concat([Buffer.from(claimer), idxBuf, amtBuf, idBuf])
  return Uint8Array.from(keccak_256(pre))
}

