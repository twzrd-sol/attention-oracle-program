import assert from 'assert'
import { getChannelMaxClaims, isRingEnabled } from '../src/constants.js'

// Default behavior
delete (process.env as any).CHANNEL_MAX_CLAIMS
assert.equal(getChannelMaxClaims(), 8192)

process.env.CHANNEL_MAX_CLAIMS = '4096'
assert.equal(getChannelMaxClaims(), 4096)

delete (process.env as any).RING_CLAIMS_ENABLED
process.env.NODE_ENV = 'production'
assert.equal(isRingEnabled(), false)
process.env.RING_CLAIMS_ENABLED = 'true'
assert.equal(isRingEnabled(), true)
delete (process.env as any).RING_CLAIMS_ENABLED
process.env.NODE_ENV = 'staging'
assert.equal(isRingEnabled(), true)

console.log('constants.test.ts: OK')
