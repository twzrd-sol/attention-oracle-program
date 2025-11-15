import { Commitment, Connection } from '@solana/web3.js'
import { isReportableRpcError } from './errors.js'

const DEFAULT_COOLDOWN_MS = 60_000

interface RpcEndpoint {
  url: string
  connection: Connection
  cooldownUntil: number
}

function parseUrls(urls: string[]): string[] {
  return Array.from(new Set(urls.map((u) => u.trim()).filter(Boolean)))
}

export class RpcPool {
  private endpoints: RpcEndpoint[]
  private lastUsedIndex = 0

  constructor(
    rpcUrls: string[],
    private readonly cooldownMs = DEFAULT_COOLDOWN_MS,
    commitment: Commitment = 'confirmed'
  ) {
    const parsed = parseUrls(rpcUrls)
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
    return this.endpoints.map((e) => e.url)
  }

  public getConnection(): Connection {
    const now = Date.now()
    const healthy = this.endpoints.filter((endpoint) => endpoint.cooldownUntil <= now)

    if (healthy.length > 0) {
      this.lastUsedIndex = (this.lastUsedIndex + 1) % healthy.length
      return healthy[this.lastUsedIndex].connection
    }

    // If everything is cooling down, return the one that becomes ready soonest.
    const soonest = [...this.endpoints].sort((a, b) => a.cooldownUntil - b.cooldownUntil)[0]
    return soonest.connection
  }

  public reportFailure(connection: Connection, error: unknown) {
    if (!isReportableRpcError(error)) return

    const endpoint = this.endpoints.find((item) => item.connection === connection)
    if (!endpoint) return

    endpoint.cooldownUntil = Date.now() + this.cooldownMs
    console.warn(
      `[RpcPool] Endpoint ${endpoint.url} reported failure; cooling down for ${this.cooldownMs / 1000}s`
    )
  }

  public async tryWithPool<T>(callback: (connection: Connection) => Promise<T>): Promise<T> {
    const connection = this.getConnection()
    try {
      return await callback(connection)
    } catch (error) {
      this.reportFailure(connection, error)
      throw error
    }
  }
}
