import 'dotenv/config';
import pino from 'pino';
import { Connection, PublicKey, Commitment } from '@solana/web3.js';
import { Queue } from 'bullmq';
import { createWriteStream } from 'fs';
import { mkdirSync } from 'fs';
import { resolve } from 'path';
import { StreamListener, StreamListenerConfig } from './listener.js';

// ============================================================================
// Configuration from Environment
// ============================================================================

const RPC_HTTP = process.env.ANCHOR_PROVIDER_URL || process.env.AO_RPC_URL;
const RPC_WS = process.env.AO_RPC_WS;
const PROGRAM_ID_STR = process.env.AO_PROGRAM_ID;
const REDIS_URL = process.env.REDIS_URL || 'redis://localhost:6379';
const BULLMQ_PREFIX = process.env.BULLMQ_PREFIX || 'twzrd';
const LOG_DIR = resolve(process.cwd(), process.env.LOG_DIR || './logs');
const LOG_LEVEL = process.env.LOG_LEVEL || 'info';
const COMMITMENT = (process.env.STREAM_COMMITMENT || 'confirmed') as Commitment;

// Validation
if (!RPC_HTTP) {
  throw new Error(
    'ANCHOR_PROVIDER_URL (or AO_RPC_URL) environment variable is required'
  );
}

if (!PROGRAM_ID_STR) {
  throw new Error('AO_PROGRAM_ID environment variable is required');
}

// ============================================================================
// Setup Logging & Directories
// ============================================================================

mkdirSync(LOG_DIR, { recursive: true });

const logger = pino(
  {
    level: LOG_LEVEL,
    transport:
      process.env.NODE_ENV === 'development'
        ? {
            target: 'pino-pretty',
            options: {
              colorize: true,
              singleLine: true,
            },
          }
        : undefined,
  },
  process.env.NODE_ENV === 'development'
    ? process.stderr
    : createWriteStream(`${LOG_DIR}/stream-listener.log`, { flags: 'a' })
);

// ============================================================================
// Setup Solana Connection
// ============================================================================

const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);

logger.info(
  {
    rpc: RPC_HTTP,
    program_id: PROGRAM_ID.toBase58(),
    commitment: COMMITMENT,
  },
  'Initializing Stream Listener'
);

const connection = new Connection(RPC_HTTP, {
  commitment: COMMITMENT,
  wsEndpoint: RPC_WS,
  disableRetryOnRateLimit: false,
});

// ============================================================================
// Setup Queue (BullMQ)
// ============================================================================

const eventQueue = new Queue('stream-events', {
  connection: {
    host: process.env.REDIS_HOST || 'localhost',
    port: parseInt(process.env.REDIS_PORT || '6379'),
  },
  defaultJobOptions: {
    attempts: 3,
    backoff: {
      type: 'exponential',
      delay: 2000,
    },
  },
});

eventQueue.on('error', (err) => {
  logger.error({ err }, 'Queue error');
});

// ============================================================================
// Setup Stream Listener
// ============================================================================

const listenerConfig: StreamListenerConfig = {
  connection,
  programId: PROGRAM_ID,
  logger,
  queue: eventQueue,
  logDir: LOG_DIR,
  commitment: COMMITMENT,
};

const listener = new StreamListener(listenerConfig);

// ============================================================================
// Graceful Shutdown
// ============================================================================

async function shutdown(signal: string) {
  logger.info({ signal }, 'Received shutdown signal');

  try {
    await listener.stop();
    await eventQueue.close();
    await connection.connection.close?.();

    logger.info('Graceful shutdown complete');
    process.exit(0);
  } catch (err) {
    logger.error({ err }, 'Error during shutdown');
    process.exit(1);
  }
}

process.on('SIGTERM', () => shutdown('SIGTERM'));
process.on('SIGINT', () => shutdown('SIGINT'));

process.on('uncaughtException', (err) => {
  logger.error({ err }, 'Uncaught exception');
  shutdown('uncaughtException').catch(console.error);
});

process.on('unhandledRejection', (reason, promise) => {
  logger.error({ reason, promise }, 'Unhandled rejection');
});

// ============================================================================
// Start Listener
// ============================================================================

async function main() {
  try {
    await listener.start();
    logger.info('Stream listener started successfully');
  } catch (err) {
    logger.error({ err }, 'Failed to start listener');
    process.exit(1);
  }
}

main().catch((err) => {
  logger.error({ err }, 'Fatal error');
  process.exit(1);
});

export { listener, eventQueue, connection, logger };
