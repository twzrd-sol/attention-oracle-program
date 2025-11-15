#!/usr/bin/env tsx
import 'dotenv/config'
import { Redis } from 'ioredis'

function makeRedis(): Redis {
  const url = process.env.REDIS_URL
  if (!url) throw new Error('REDIS_URL not set')
  return new Redis(url, { maxRetriesPerRequest: 1, connectTimeout: 2000 }) as unknown as Redis
}

async function main() {
  const redis = makeRedis()
  try {
    const prefix = 'bull:tree-builder'
    const keys = [`${prefix}:wait`, `${prefix}:active`, `${prefix}:delayed`]
    const [wait, active, delayed] = await Promise.all([
      redis.llen(keys[0]),
      redis.llen(keys[1]),
      redis.zcard(keys[2]),
    ])
    console.log(`WAIT=${wait} ACTIVE=${active} DELAYED=${delayed}`)
  } finally {
    ;(redis as any).disconnect?.()
  }
}

main().catch((e) => {
  console.log('WAIT=0 ACTIVE=0 DELAYED=0')
  process.exit(0)
})

