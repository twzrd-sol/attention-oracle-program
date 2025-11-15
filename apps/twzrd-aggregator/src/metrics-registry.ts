import client from 'prom-client'

// Shared registry so all metrics are exposed via the same /metrics endpoint
export const register = new client.Registry()

client.collectDefaultMetrics({
  register,
  prefix: 'twzrd_aggregator_',
})

export const publisherLoopTickCounter = new client.Counter({
  name: 'publisher_loop_tick_total',
  help: 'Total publisher loop ticks executed.',
  registers: [register],
})

export const publishSuccessCounter = new client.Counter({
  name: 'publish_success_total',
  help: 'Total successful on-chain publishes.',
  labelNames: ['channel'],
  registers: [register],
})

export const publishSkippedCounter = new client.Counter({
  name: 'publish_skipped_total',
  help: 'Total publishes skipped by reason.',
  labelNames: ['reason'],
  registers: [register],
})

export const publishFailedCounter = new client.Counter({
  name: 'publish_failed_total',
  help: 'Total publish attempts that failed.',
  labelNames: ['error_code'],
  registers: [register],
})

export const unpublishedBacklogGauge = new client.Gauge({
  name: 'unpublished_epoch_backlog',
  help: 'Current number of sealed epochs pending publication by channel group.',
  labelNames: ['group'],
  registers: [register],
})

export const aggregatorBacklogGauge = new client.Gauge({
  name: 'aggregator_backlog',
  help: 'Number of unpublished sealed epochs (legacy metric).',
  registers: [register],
})

export const aggregatorLastEpochGauge = new client.Gauge({
  name: 'aggregator_last_epoch',
  help: 'Last sealed epoch timestamp.',
  registers: [register],
})

export const aggregatorLastSealedAtGauge = new client.Gauge({
  name: 'aggregator_last_sealed_at',
  help: 'Unix timestamp of the last sealed epoch.',
  registers: [register],
})

export const aggregatorWalletSolGauge = new client.Gauge({
  name: 'aggregator_wallet_sol',
  help: 'Oracle authority wallet balance (SOL).',
  registers: [register],
})

export function incrementPublishSuccess(channel: string) {
  publishSuccessCounter.inc({ channel: channel.toLowerCase() })
}

export function incrementPublishSkipped(reason: 'stale_epoch' | 'offline_epoch' | 'non_milo' | 'anomaly') {
  publishSkippedCounter.inc({ reason })
}

export function incrementPublishFailure(errorCode: string) {
  publishFailedCounter.inc({ error_code: errorCode })
}

export function setBacklogCounts(entries: Array<{ group: string; count: number }>, total: number) {
  unpublishedBacklogGauge.reset()
  for (const { group, count } of entries) {
    unpublishedBacklogGauge.set({ group }, count)
  }
  aggregatorBacklogGauge.set(total)
}

export function setLastEpoch(epoch: number) {
  aggregatorLastEpochGauge.set(epoch)
}

export function setLastSealedAt(timestamp: number) {
  aggregatorLastSealedAtGauge.set(timestamp)
}

export function setWalletBalance(sol: number) {
  aggregatorWalletSolGauge.set(sol)
}

// Database pool metrics
export const dbPoolUtilizationGauge = new client.Gauge({
  name: 'db_pool_utilization',
  help: 'Current database pool utilization (used/total connections).',
  labelNames: ['pool_name'],
  registers: [register],
})

export const dbPoolWaitingCountGauge = new client.Gauge({
  name: 'db_pool_waiting_count',
  help: 'Number of requests waiting for a database connection.',
  labelNames: ['pool_name'],
  registers: [register],
})

// Tree building metrics
export const treeBuiltCounter = new client.Counter({
  name: 'tree_built_total',
  help: 'Total number of merkle trees built.',
  labelNames: ['channel'],
  registers: [register],
})

export const treeParticipantsHistogram = new client.Histogram({
  name: 'tree_participants',
  help: 'Distribution of participant counts in merkle trees.',
  labelNames: ['channel'],
  buckets: [0, 10, 50, 100, 250, 500, 1000, 1024, 2000, 5000],
  registers: [register],
})

// Epoch sealing metrics
export const epochSealedCounter = new client.Counter({
  name: 'epoch_sealed_total',
  help: 'Total number of epochs sealed.',
  labelNames: ['channel', 'token_group'],
  registers: [register],
})

export const epochParticipantsHistogram = new client.Histogram({
  name: 'epoch_participants',
  help: 'Distribution of participant counts in sealed epochs.',
  labelNames: ['channel'],
  buckets: [0, 10, 50, 100, 250, 500, 1000, 2000, 5000, 10000],
  registers: [register],
})

// RPC metrics
export const rpcRequestDurationHistogram = new client.Histogram({
  name: 'rpc_request_duration_seconds',
  help: 'Duration of RPC requests in seconds.',
  labelNames: ['method', 'status'],
  buckets: [0.1, 0.5, 1, 2, 5, 10],
  registers: [register],
})

// Helper functions for new metrics
export function recordTreeBuilt(channel: string, participantCount: number) {
  treeBuiltCounter.inc({ channel: channel.toLowerCase() })
  treeParticipantsHistogram.observe({ channel: channel.toLowerCase() }, participantCount)
}

export function recordEpochSealed(channel: string, tokenGroup: string, participantCount: number) {
  epochSealedCounter.inc({ channel: channel.toLowerCase(), token_group: tokenGroup })
  epochParticipantsHistogram.observe({ channel: channel.toLowerCase() }, participantCount)
}

export function recordRpcDuration(method: string, status: 'success' | 'error', durationSeconds: number) {
  rpcRequestDurationHistogram.observe({ method, status }, durationSeconds)
}

export function updateDbPoolMetrics(poolName: string, used: number, total: number, waiting: number) {
  dbPoolUtilizationGauge.set({ pool_name: poolName }, used / total)
  dbPoolWaitingCountGauge.set({ pool_name: poolName }, waiting)
}
