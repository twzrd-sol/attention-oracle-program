/**
 * Default maximum claims per epoch; matches 1024-byte bitmap (8192 bits).
 */
export function getChannelMaxClaims(): number {
  const raw = Number(process.env.CHANNEL_MAX_CLAIMS || 8192)
  return Number.isFinite(raw) && raw > 0 ? Math.floor(raw) : 8192
}

/** Env gate for ring-claim API exposure */
export function isRingEnabled(): boolean {
  if (typeof process.env.RING_CLAIMS_ENABLED === 'string') {
    return process.env.RING_CLAIMS_ENABLED.toLowerCase() === 'true'
  }
  return (process.env.NODE_ENV || '').toLowerCase() === 'staging'
}
