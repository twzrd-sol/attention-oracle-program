import { keccak_256 } from '@noble/hashes/sha3'

export interface ParticipationRow {
  epoch: number
  channel: string
  user_hash: string
  first_seen: number
  token_group: string
  category: string
}

export interface SignalRow {
  epoch: number
  channel: string
  user_hash: string
  signal_type: 'presence' | 'sub' | 'resub' | 'gift' | 'bits' | 'raid' | 'message'
  value: number
  timestamp: number
}

export interface WeightedParticipant {
  user_hash: string
  weight: number
  signals: {
    presence: number
    sub: number
    resub: number
    gift: number
    bits: number
    raid: number
  }
}

export function hashUser(username: string | null | undefined): string {
  if (typeof username !== 'string' || username.trim().length === 0) {
    throw new Error('Invalid username for hashing')
  }
  const lower = username.toLowerCase()
  return Buffer.from(keccak_256(Buffer.from(lower))).toString('hex')
}
