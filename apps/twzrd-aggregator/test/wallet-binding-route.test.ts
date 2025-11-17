import assert from 'assert'
import express from 'express'
import request from 'supertest'
import bs58 from 'bs58'
import { createWalletBindingRouter, WalletBindingStore } from '../src/routes/wallet-binding.js'
import { canonicalUserHash } from '../src/util/hashing.js'
import { computeClaimLeaf } from '../src/claims.js'

class InMemoryBindingStore implements WalletBindingStore {
  private map = new Map<string, string>()

  async bindWallet(params: { userId?: string; username?: string; wallet: string; verified?: boolean; source?: string }): Promise<void> {
    const hash = canonicalUserHash({ userId: params.userId, user: params.username })
    this.map.set(hash, params.wallet)
  }

  async getWalletForUserHash(userHash: string): Promise<string | null> {
    return this.map.get(userHash) || null
  }
}

const TEST_TOKEN = 'test-secret'
const TEST_WALLET = bs58.encode(Buffer.from(Array.from({ length: 32 }, (_, i) => i % 256)))

const db = new InMemoryBindingStore()
const app = express()
app.use(express.json())
app.use('/', createWalletBindingRouter({ db, apiToken: TEST_TOKEN }))

async function run() {
  const twitchId = '123456'
  const username = 'TestUser'

  const bindResp = await request(app)
    .post('/bind-wallet')
    .set('x-bind-token', TEST_TOKEN)
    .set('x-twitch-user-id', twitchId)
    .set('x-twitch-login', username)
    .send({ twitch_id: twitchId, login: username, wallet: TEST_WALLET })
    .expect(200)

  const canonicalHash = canonicalUserHash({ userId: twitchId, user: username })
  assert.equal(bindResp.body.userHash, canonicalHash)

  const fetchResp = await request(app)
    .get('/bound-wallet')
    .set('x-bind-token', TEST_TOKEN)
    .query({ twitch_id: twitchId })
    .expect(200)

  assert.equal(fetchResp.body.wallet, TEST_WALLET)
  assert.equal(fetchResp.body.userHash, canonicalHash)

  const claimer = bs58.decode(TEST_WALLET)
  const id = `twitch:${username.toLowerCase()}`
  const leaf = computeClaimLeaf(claimer, 0, 1n, id)
  assert.ok(leaf)
  assert.equal(Buffer.from(leaf).length, 32)

  console.log('wallet-binding-route.test.ts: OK')
}

run().catch(err => {
  console.error(err)
  process.exit(1)
})
