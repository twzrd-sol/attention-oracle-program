import assert from 'assert'
import { computeClaimLeaf } from '../src/claims.js'

// Deterministic example
const claimer = new Uint8Array(32).fill(7) // 0x07 * 32
const index = 5
const amount = 123_456n
const id = 'twitch:unit:alice'

const leaf = computeClaimLeaf(claimer, index, amount, id)
assert.equal(leaf.length, 32)

// Recompute manually to verify implementation parity
import { keccak_256 } from '@noble/hashes/sha3'
const idxBuf = Buffer.alloc(4); idxBuf.writeUInt32LE(index, 0)
const amtBuf = Buffer.alloc(8); amtBuf.writeBigUInt64LE(amount, 0)
const idBuf = Buffer.from(id, 'utf8')
const pre = Buffer.concat([Buffer.from(claimer), idxBuf, amtBuf, idBuf])
const expected = keccak_256(pre)
assert.equal(Buffer.from(leaf).toString('hex'), Buffer.from(expected).toString('hex'))

console.log('claims-leaf.test.ts: OK')

