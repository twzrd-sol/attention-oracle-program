import { Commitment, Connection } from '@solana/web3.js'
import { isReportableRpcError } from './errors.js'

const DEFAULT_COOLDOWN_MS = 60_000

interface RpcEndpoint {
  url: string
  connection: Connection
  cooldownUntil: number
}

function parseUrls(raw: string[]): string[] {
  return Array.from(new Set(raw.map((u) => u.trim()).filter(Boolean)))
}

export class RpcPool {
  private endpoints: RpcEndpoint[]
  private lastUsedIndex = 0

  constructor(urls: string[], private cooldownMs = DEFAULT_COOLDOWN_MS, commitment: Commitment = 'confirmed') {
    const parsed = parseUrls(urls)
    if (parsed.length === 0) {
      throw new Error('RpcPool: no RPC URLs provided')
    }

    this.endpoints = parsed.map((url) => ({
      url,
      connection: new Connection(url, { commitment }),
      cooldownUntil: 0,
    }))
  }

  public getUrls(): string[] {
    return this.endpoints.map((endpoint) => endpoint.url)
  }

  public getConnection(): Connection {
    const now = Date.now()
    const healthy = this.endpoints.filter((endpoint) => endpoint.cooldownUntil <= now)

    if (healthy.length > 0) {
      this.lastUsedIndex = (this.lastUsedIndex + 1) % healthy.length
      return healthy[this.lastUsedIndex].connection
    }

    const soonest = [...this.endpoints].sort((a, b) => a.cooldownUntil - b.cooldownUntil)[0]
    return soonest.connection
  }

  public async tryWithPool<T>(fn: (connection: Connection) => Promise<T>): Promise<T> {
    const connection = this.getConnection()
    try {
      return await fn(connection)
    } catch (error) {
      this.reportFailure(connection, error)
      throw error
    }
  }

  public reportFailure(connection: Connection, error: unknown) {
    if (!isReportableRpcError(error)) return

    const endpoint = this.endpoints.find((item) => item.connection === connection)
    if (!endpoint) return
    endpoint.cooldownUntil = Date.now() + this.cooldownMs
    console.warn(`[RpcPool] Endpoint ${endpoint.url} failed; cooling down for ${this.cooldownMs / 1000}s`)
  }
}

