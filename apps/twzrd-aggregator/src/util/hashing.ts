import { keccak_256 } from '@noble/hashes/sha3'
import { hashUser } from '../db-types.js'

export interface HashInputLike {
  user?: string
  username?: string
  userId?: string
  user_id?: string
}

/**
 * Canonical user hash for ring + participation flows.
 * - Prefer stable Twitch userId if present: keccak256("twitchId:" + id)
 * - Fallback to login/username via existing hashUser()
 */
export function canonicalUserHash(input: HashInputLike): string {
  const id = (input?.userId || input?.user_id || '').trim()
  if (id) {
    const pre = Buffer.from(`twitchId:${id}`)
    return Buffer.from(keccak_256(pre)).toString('hex')
  }
  const name = (input?.user || input?.username || '').trim()
  if (name) {
    return hashUser(name)
  }
  throw new Error('invalid_user_input')
}

/** Convenience wrapper for plain strings */
export function canonicalUserHashFromStrings(userId?: string | null, username?: string | null): string {
  if (userId && userId.trim()) return canonicalUserHash({ userId })
  if (username && username.trim()) return canonicalUserHash({ user: username })
  throw new Error('invalid_user_input')
}

