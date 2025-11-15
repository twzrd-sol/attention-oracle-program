import { RpcPool } from './RpcPool.js'

function parseEnvList(value?: string): string[] {
  if (!value) return []
  return value.split(',').map((v) => v.trim()).filter(Boolean)
}

const configured = parseEnvList(process.env.AGGREGATOR_RPC_URLS)
const legacy = parseEnvList(process.env.RPC_URLS)
const fallback = process.env.RPC_URL ? [process.env.RPC_URL] : []

const urls = [...configured, ...legacy, ...fallback]
if (urls.length === 0) {
  urls.push('https://api.mainnet-beta.solana.com')
}

export const aggregatorRpcPool = new RpcPool(urls)

