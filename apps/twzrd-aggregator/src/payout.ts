import { WeightedParticipant } from './db-types.js'
import type { ChannelPayoutSnapshot } from './db-factory.js'

export const TOKEN_DECIMALS = Number(process.env.REWARD_DECIMALS || 9)
export const TOKEN_PRECISION = Math.pow(10, TOKEN_DECIMALS)
export const BASE_REWARD_PER_WEIGHT = Number(process.env.BASE_REWARD_PER_WEIGHT || 80)

const clampRatio = (value: number): number => {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}

const CLS_STREAMER_RATIO_DEFAULT = clampRatio(Number(process.env.CLS_STREAMER_RATIO || 0))

export type ChannelSplit = {
  viewerRatio: number
  streamerRatio: number
  type: 'cls' | 'default'
}

// CLS-only split logic. Optional category mode (e.g., aggregated "crypto")
export function getChannelSplit(channel: string, opts?: { category?: boolean }): ChannelSplit {
  const channelLower = channel.toLowerCase()

  if (opts?.category || channelLower === 'crypto' || channelLower === 'category:crypto' || channelLower === 'all') {
    return { viewerRatio: 1, streamerRatio: 0, type: 'default' }
  }

  const streamerRatio = CLS_STREAMER_RATIO_DEFAULT
  return { viewerRatio: clampRatio(1 - streamerRatio), streamerRatio, type: 'cls' }
}

export const computeViewerAmount = (weight: number, split: ChannelSplit): number => {
  if (split.viewerRatio <= 0) return 0
  return Math.round(weight * BASE_REWARD_PER_WEIGHT * split.viewerRatio * TOKEN_PRECISION)
}

export const computeStreamerAmount = (weight: number, split: ChannelSplit): number => {
  if (split.streamerRatio <= 0) return 0
  return Math.round(weight * BASE_REWARD_PER_WEIGHT * split.streamerRatio * TOKEN_PRECISION)
}

export function buildChannelPayoutSnapshot(
  epoch: number,
  channel: string,
  weighted: WeightedParticipant[],
  split: ChannelSplit
): ChannelPayoutSnapshot | null {
  if (!weighted || weighted.length === 0) return null
  const totalWeight = weighted.reduce((acc, row) => acc + row.weight, 0)

  return {
    epoch,
    channel,
    participantCount: weighted.length,
    totalWeight,
    viewerAmount: computeViewerAmount(totalWeight, split),
    streamerAmount: computeStreamerAmount(totalWeight, split),
    viewerRatio: split.viewerRatio,
    streamerRatio: split.streamerRatio,
  }
}
