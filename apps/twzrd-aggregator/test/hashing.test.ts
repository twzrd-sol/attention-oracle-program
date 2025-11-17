import assert from 'assert'
import { canonicalUserHash, canonicalUserHashFromStrings } from '../src/util/hashing.js'

const isHex = (s: string) => /^[0-9a-f]+$/i.test(s)

// Prefer userId
{
  const h = canonicalUserHash({ userId: '123456', user: 'SomeName' })
  assert.equal(h.length, 64)
  assert.ok(isHex(h))
}

// Fallback to username
{
  const h = canonicalUserHash({ user: 'SomeName' })
  assert.equal(h.length, 64)
}

// Convenience wrapper
{
  const h = canonicalUserHashFromStrings('42', null)
  assert.equal(h.length, 64)
}

console.log('hashing.test.ts: OK')

