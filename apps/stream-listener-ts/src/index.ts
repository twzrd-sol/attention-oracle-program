import fs from 'node:fs';
import path from 'node:path';
import fetch from 'node-fetch';
import pino from 'pino';
import { Connection, PublicKey, Keypair } from '@solana/web3.js';
import { AnchorProvider, Program, Wallet, EventParser } from '@coral-xyz/anchor';
import { fileURLToPath } from 'url';
// Load local vendored IDL JSON (placed by anchor build)
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const IDL_PATH = path.join(__dirname, '../idl/token_2022.json');
const IDL = JSON.parse(fs.readFileSync(IDL_PATH, 'utf8'));
import { env } from './env.js';
import { metrics, startMetricsServer } from './metrics.js';

const logger = pino({ level: process.env.LOG_LEVEL || 'info' });
const PROGRAM_ID = new PublicKey(env.PROGRAM_ID);

// Connection with optional explicit WS endpoint
const connection = new Connection(env.RPC_URL, {
  wsEndpoint: env.RPC_URL_WS,
  commitment: env.STREAM_COMMITMENT as any,
});

// Read-only provider and Anchor program for typed event parsing
const provider = new AnchorProvider(connection, new Wallet(Keypair.generate()), {});
// Explicit any to avoid constructor overload inference issues in TS
const program = new (Program as any)(IDL as any, PROGRAM_ID, provider);
const parser = new EventParser(PROGRAM_ID, program.coder);

// Output setup
fs.mkdirSync(env.LOG_DIR, { recursive: true });
const LOG_FILE = path.join(env.LOG_DIR, 'events.ndjson');
const fileStream = fs.createWriteStream(LOG_FILE, { flags: 'a' });

logger.info({ PROGRAM_ID: PROGRAM_ID.toBase58(), RPC_URL: env.RPC_URL, LOG_FILE }, 'listener:init');

function writeNdjson(obj: unknown) {
  try {
    fileStream.write(JSON.stringify(obj) + '\n');
  } catch (err: any) {
    logger.warn({ err: err?.message || String(err) }, 'ndjson_write_failed');
  }
}

async function forward(payload: unknown) {
  if (!env.GATEWAY_URL) return;
  try {
    const res = await fetch(`${env.GATEWAY_URL.replace(/\/$/, '')}/internal/event`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        ...(env.INTERNAL_EVENT_TOKEN ? { 'x-internal-token': env.INTERNAL_EVENT_TOKEN } : {}),
      },
      body: JSON.stringify(payload),
    });
    if (res.ok) metrics.inc('gateway_success'); else metrics.inc('gateway_failure');
  } catch (err: any) {
    metrics.inc('gateway_failure');
    logger.warn({ err: err?.message || String(err) }, 'gateway_forward_failed');
  }
}

async function main() {
  const removers: Array<() => Promise<void> | void> = [];
  // Start metrics HTTP server on a distinct port
  startMetricsServer(9091);
  const subId = await connection.onLogs(
    PROGRAM_ID,
    (logs, ctx) => {
      if (logs.err) return;
      const events = [...parser.parseLogs(logs.logs)];
      if (events.length === 0) return;
      for (const ev of events) {
        const line = {
          ts: new Date().toISOString(),
          signature: logs.signature,
          slot: ctx?.slot ?? null,
          name: ev.name,
          data: ev.data,
        };
        writeNdjson(line);
        metrics.inc('events_emitted');
        void forward(line);
      }
    },
    env.STREAM_COMMITMENT as any,
  );
  logger.info({ subId }, 'listener:subscribed');
  removers.push(() => connection.removeOnLogsListener(subId));

  // ProtocolState account subscription (singleton or mint-keyed)
  try {
    const mint = env.MINT_PUBKEY ? new PublicKey(env.MINT_PUBKEY) : null;
    const seeds = mint ? [Buffer.from('protocol'), mint.toBuffer()] : [Buffer.from('protocol')];
    const [protocolPda] = PublicKey.findProgramAddressSync(seeds, PROGRAM_ID);
    const accId = await connection.onAccountChange(
      protocolPda,
      (info, ctx) => {
        try {
          let state: any | null = null;
          try { state = program.coder.accounts.decode('protocolState', info.data); } catch {}
          if (!state) {
            try { state = program.coder.accounts.decode('ProtocolState', info.data); } catch {}
          }
          const payload = {
            ts: new Date().toISOString(),
            slot: ctx.slot,
            type: 'protocol_update',
            account: protocolPda.toBase58(),
            data: state ?? null,
          };
          writeNdjson(payload);
          metrics.inc('events_emitted');
          metrics.inc('protocol_updates');
          void forward(payload);
        } catch (e: any) {
          logger.warn({ err: e?.message || String(e) }, 'protocol_decode_failed');
        }
      },
      env.STREAM_COMMITMENT as any,
    );
    removers.push(() => connection.removeAccountChangeListener(accId));
  logger.info({ account: protocolPda.toBase58() }, 'listener:protocol_subscribed');
  } catch (e: any) {
    logger.warn({ err: e?.message || String(e) }, 'listener:protocol_subscribe_failed');
  }

  // Optional: ChannelState account subscriptions (if mint + channels provided)
  try {
    if (env.MINT_PUBKEY && env.STREAM_CHANNELS && env.STREAM_CHANNELS.length > 0) {
      const mint = new PublicKey(env.MINT_PUBKEY);
      const channels = env.STREAM_CHANNELS;
      for (const channel of channels) {
        try {
          const lower = channel.trim().toLowerCase();
          // Derive streamer key using keccak('twitch:' + lower)
          const pre = Buffer.from(`twitch:${lower}`);
          const { keccak_256 } = await import('@noble/hashes/sha3');
          const hash = keccak_256(pre);
          const streamerKey = new PublicKey(Buffer.from(hash));
          const [pda] = PublicKey.findProgramAddressSync(
            [Buffer.from('channel_state'), mint.toBuffer(), streamerKey.toBuffer()],
            PROGRAM_ID,
          );
          const id = await connection.onAccountChange(
            pda,
            (info, ctx) => {
              const payload = {
                ts: new Date().toISOString(),
                slot: ctx.slot,
                type: 'channel_update',
                channel: lower,
                account: pda.toBase58(),
                data: info.data.toString('base64'),
              };
              writeNdjson(payload);
              metrics.inc('events_emitted');
              void forward(payload);
            },
            env.STREAM_COMMITMENT as any,
          );
          removers.push(() => connection.removeAccountChangeListener(id));
          logger.info({ channel: lower, account: pda.toBase58() }, 'listener:channel_subscribed');
        } catch (e: any) {
          logger.warn({ channel, err: e?.message || String(e) }, 'listener:channel_subscribe_failed');
        }
      }
    }
  } catch (e: any) {
    logger.warn({ err: e?.message || String(e) }, 'listener:channels_block_failed');
  }

  const shutdown = async () => {
    for (const rm of removers.reverse()) {
      try { await rm(); } catch {}
    }
    fileStream.end();
    process.exit(0);
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}

main().catch((err) => {
  logger.error({ err: err?.message || String(err) }, 'listener:fatal');
  process.exit(1);
});
