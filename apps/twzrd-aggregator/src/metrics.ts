import { register, Counter, Gauge, Histogram, collectDefaultMetrics } from 'prom-client';

// Collect default NodeJS metrics (memory, CPU, etc.)
collectDefaultMetrics({ register });

// Publisher metrics
export const publisherLoopTick = new Counter({
  name: 'publisher_loop_tick_total',
  help: 'Total number of publisher loop iterations',
  registers: [register],
});

export const publishSuccess = new Counter({
  name: 'publish_success_total',
  help: 'Total number of successful epoch publishes',
  labelNames: ['channel', 'token_group'],
  registers: [register],
});

export const publishFailed = new Counter({
  name: 'publish_failed_total',
  help: 'Total number of failed epoch publishes',
  labelNames: ['channel', 'error_type'],
  registers: [register],
});

export const publishSkipped = new Counter({
  name: 'publish_skipped_total',
  help: 'Total number of skipped epoch publishes',
  labelNames: ['reason'],
  registers: [register],
});

export const unpublishedEpochBacklog = new Gauge({
  name: 'unpublished_epoch_backlog',
  help: 'Current number of unpublished epochs in backlog',
  registers: [register],
});

// Database pool metrics
export const dbPoolUtilization = new Gauge({
  name: 'db_pool_utilization',
  help: 'Current database pool utilization',
  labelNames: ['pool_name'],
  registers: [register],
});

export const dbPoolWaitingCount = new Gauge({
  name: 'db_pool_waiting_count',
  help: 'Number of requests waiting for a connection',
  labelNames: ['pool_name'],
  registers: [register],
});

// Tree building metrics
export const treeBuilt = new Counter({
  name: 'tree_built_total',
  help: 'Total number of merkle trees built',
  labelNames: ['channel'],
  registers: [register],
});

export const treeParticipants = new Histogram({
  name: 'tree_participants',
  help: 'Number of participants in merkle trees',
  labelNames: ['channel'],
  buckets: [0, 10, 50, 100, 250, 500, 1000, 1024, 2000, 5000],
  registers: [register],
});

// Epoch sealing metrics
export const epochSealed = new Counter({
  name: 'epoch_sealed_total',
  help: 'Total number of epochs sealed',
  labelNames: ['channel', 'token_group'],
  registers: [register],
});

export const epochParticipants = new Histogram({
  name: 'epoch_participants',
  help: 'Number of participants in sealed epochs',
  labelNames: ['channel'],
  buckets: [0, 10, 50, 100, 250, 500, 1000, 2000, 5000, 10000],
  registers: [register],
});

// RPC metrics
export const rpcRequestDuration = new Histogram({
  name: 'rpc_request_duration_seconds',
  help: 'Duration of RPC requests in seconds',
  labelNames: ['method', 'status'],
  buckets: [0.1, 0.5, 1, 2, 5, 10],
  registers: [register],
});

// Export the registry for the /metrics endpoint
export { register };