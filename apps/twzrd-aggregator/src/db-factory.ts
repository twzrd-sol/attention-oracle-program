import { Pool } from 'pg'
import { TwzrdDBPostgres } from './db-pg.js'
import type { ParticipationRow, SignalRow, WeightedParticipant } from './db-types.js'

export interface ChannelPayoutSnapshot {
  epoch: number
  channel: string
  participantCount: number
  totalWeight: number
  viewerAmount: number
  streamerAmount: number
  viewerRatio: number
  streamerRatio: number
}

export interface ITwzrdDB {
  pool: Pool

  recordParticipation(rows: ParticipationRow[]): Promise<void>
  getParticipants(epoch: number, channel: string): Promise<string[]>
  getSealedParticipants(epoch: number, channel: string, tokenGroup?: string, category?: string): Promise<string[] | null>
  getWeightedParticipants(epoch: number, channel: string): Promise<WeightedParticipant[]>
  sealEpoch(epoch: number, channel: string, computeRoot: (users: string[]) => string, tokenGroup?: string, category?: string): Promise<void>
  getActiveChannels(epoch: number): Promise<string[]>
  getCachedL2Tree(epoch: number, channel: string): Promise<{ root: string; levels: Buffer[][]; participantCount: number; builtAt: number } | null>
  cacheL2Tree(epoch: number, channel: string, root: string, levels: Buffer[][], participantCount: number): Promise<void>
  recordSignals(rows: SignalRow[]): Promise<void>
  upsertUsernameMapping(userHash: string, username: string): Promise<void>
  recordChannelPayoutSnapshot(snapshot: ChannelPayoutSnapshot): Promise<void>
  getUnpublishedRoots(currentEpoch: number, limit: number): Promise<Array<{ epoch: number; channel: string; root: string; token_group: string; category: string }>>
  markRootAsPublished(epoch: number, channel: string, tokenGroup?: string, category?: string): Promise<void>
  getBacklogCount(): Promise<number>
  getBacklogCountsByGroup(): Promise<Array<{ group: string; count: number }>>
  getLastSealedEpoch(): Promise<{ epoch: number; sealed_at: number } | null>
  getRecentSealedEpochs(limit: number): Promise<number[]>
  getSealedParticipantCountsByChannel(epoch: number): Promise<Array<{ channel: string; cnt: number }>>
  getUsernameMapping(userHash: string): Promise<string | null>
  getSealedChannels(epoch: number): Promise<string[]>
  hasLiveOverlap?(epoch: number, channel: string, epochSeconds?: number): Promise<boolean>
  isSuppressed?(userHash: string): Promise<boolean>
  addSuppression?(userHash: string, username: string, reason?: string, ipHash?: string): Promise<void>
  getSuppressionStatus?(username: string): Promise<{ suppressed: boolean; requested_at?: number } | null>
  cleanupBefore?(epochCutoff: number): Promise<void>
  close(): Promise<void>
}

export function createDatabase(): ITwzrdDB {
  const connString = process.env.DATABASE_URL
  if (!connString) {
    throw new Error('DATABASE_URL required (PostgreSQL enforced)')
  }
  console.log(`[DB Factory] Using PostgreSQL: ${connString.replace(/:\/\/.+?@/, '://***@')}`)
  return new PostgresDBAdapter(connString)
}

class PostgresDBAdapter implements ITwzrdDB {
  private readonly pgDb: TwzrdDBPostgres

  constructor(private readonly postgresUrl: string) {
    this.pgDb = new TwzrdDBPostgres(this.postgresUrl)
  }

  get pool(): Pool {
    return (this.pgDb as any).pool
  }

  recordParticipation(rows: ParticipationRow[]): Promise<void> {
    return this.pgDb.recordParticipation(rows)
  }

  getParticipants(epoch: number, channel: string): Promise<string[]> {
    return this.pgDb.getParticipants(epoch, channel)
  }

  getSealedParticipants(epoch: number, channel: string, tokenGroup = 'MILO', category = 'default'): Promise<string[] | null> {
    return this.pgDb.getSealedParticipants(epoch, channel, tokenGroup, category)
  }

  getWeightedParticipants(epoch: number, channel: string): Promise<WeightedParticipant[]> {
    return this.pgDb.getWeightedParticipants(epoch, channel)
  }

  sealEpoch(epoch: number, channel: string, computeRoot: (users: string[]) => string, tokenGroup = 'OTHER', category = 'default'): Promise<void> {
    return this.pgDb.sealEpoch(epoch, channel, computeRoot, tokenGroup, category)
  }

  getActiveChannels(epoch: number): Promise<string[]> {
    return this.pgDb.getActiveChannels(epoch)
  }

  getCachedL2Tree(epoch: number, channel: string) {
    return this.pgDb.getCachedL2Tree(epoch, channel)
  }

  cacheL2Tree(epoch: number, channel: string, root: string, levels: Buffer[][], participantCount: number): Promise<void> {
    return this.pgDb.cacheL2Tree(epoch, channel, root, levels, participantCount)
  }

  recordSignals(rows: SignalRow[]): Promise<void> {
    return this.pgDb.recordSignals(rows)
  }

  upsertUsernameMapping(userHash: string, username: string): Promise<void> {
    return this.pgDb.upsertUsernameMapping(userHash, username)
  }

  recordChannelPayoutSnapshot(snapshot: ChannelPayoutSnapshot): Promise<void> {
    return this.pgDb.recordChannelPayoutSnapshot(snapshot)
  }

  getUnpublishedRoots(currentEpoch: number, limit: number) {
    return this.pgDb.getUnpublishedRoots(currentEpoch, limit)
  }

  markRootAsPublished(epoch: number, channel: string, tokenGroup = 'MILO', category = 'default') {
    return this.pgDb.markRootAsPublished(epoch, channel, tokenGroup, category)
  }

  getBacklogCount(): Promise<number> {
    return this.pgDb.getBacklogCount()
  }

  getBacklogCountsByGroup(): Promise<Array<{ group: string; count: number }>> {
    return this.pgDb.getBacklogCountsByGroup()
  }

  getLastSealedEpoch() {
    return this.pgDb.getLastSealedEpoch()
  }

  getRecentSealedEpochs(limit: number): Promise<number[]> {
    return this.pgDb.getRecentSealedEpochs(limit)
  }

  getSealedParticipantCountsByChannel(epoch: number) {
    return this.pgDb.getSealedParticipantCountsByChannel(epoch)
  }

  getUsernameMapping(userHash: string) {
    return this.pgDb.getUsernameMapping(userHash)
  }

  getSealedChannels(epoch: number): Promise<string[]> {
    return this.pgDb.getSealedChannels(epoch)
  }

  hasLiveOverlap(epoch: number, channel: string, epochSeconds?: number) {
    if (!this.pgDb.hasLiveOverlap) return Promise.resolve(true)
    return this.pgDb.hasLiveOverlap(epoch, channel, epochSeconds)
  }

  isSuppressed(userHash: string): Promise<boolean> {
    if (!this.pgDb.isSuppressed) return Promise.resolve(false)
    return this.pgDb.isSuppressed(userHash)
  }

  addSuppression(userHash: string, username: string, reason?: string, ipHash?: string): Promise<void> {
    if (!this.pgDb.addSuppression) return Promise.resolve()
    return this.pgDb.addSuppression(userHash, username, reason, ipHash)
  }

  getSuppressionStatus(username: string): Promise<{ suppressed: boolean; requested_at?: number } | null> {
    if (!this.pgDb.getSuppressionStatus) return Promise.resolve(null)
    return this.pgDb.getSuppressionStatus(username)
  }

  cleanupBefore(epochCutoff: number): Promise<void> {
    if (typeof (this.pgDb as any).cleanupBefore !== 'function') {
      return Promise.resolve()
    }
    return (this.pgDb as any).cleanupBefore(epochCutoff)
  }

  close(): Promise<void> {
    return this.pgDb.close()
  }
}
