# TWZRD Stream Listener

Clean TypeScript implementation of the Solana event listener for the Attention Oracle program.

## Overview

The Stream Listener is the **edge** of the TWZRD infrastructure. It:

1. **Connects to Solana** via `@solana/web3.js`
2. **Subscribes to program events** from the Attention Oracle program (GnGzNdsQ...)
3. **Normalizes events** into a standard StreamEvent format
4. **Queues events** for processing (via BullMQ/Redis)
5. **Logs events** to NDJSON for audit trail and replay

## Architecture

```
Solana Blockchain (Mainnet)
    ↓
Stream Listener (this service)
    ├─→ BullMQ Queue (Redis) → Aggregator
    └─→ NDJSON Log (disk) → Audit trail / replay
```

## Quick Start

### Prerequisites

- **Node.js** ≥ 18.0.0
- **Redis** (for BullMQ queue)
- **pnpm** or **npm**

### Installation

```bash
# Clone and navigate
git clone https://github.com/twzrd-sol/twzrd-stream-listener.git
cd twzrd-stream-listener

# Install dependencies
npm install

# Copy environment template
cp .env.example .env

# Edit .env with your configuration
# At minimum:
#   ANCHOR_PROVIDER_URL = <Solana RPC>
#   AO_PROGRAM_ID = GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop
```

### Running

**Development:**
```bash
npm run dev
```

**Production:**
```bash
npm run build
npm start
```

**Via PM2 (supervised):**
```bash
pm2 start dist/index.js --name stream-listener --env production
pm2 save
pm2 startup
```

## Configuration

See `.env.example` for all options. Key variables:

| Variable | Required | Default | Purpose |
|----------|----------|---------|---------|
| `ANCHOR_PROVIDER_URL` | Yes | — | Solana RPC endpoint |
| `AO_PROGRAM_ID` | Yes | `GnGzNdsQ...` | Attention Oracle program ID |
| `REDIS_URL` | No | `redis://localhost:6379` | Redis connection |
| `STREAM_COMMITMENT` | No | `confirmed` | Solana commitment level |
| `LOG_LEVEL` | No | `info` | Pino log level |

## Data Format

### StreamEvent (Queue + NDJSON)

```typescript
interface StreamEvent {
  // Metadata
  timestamp: string;           // ISO 8601
  slot: number;                // Solana slot
  signature: string;            // Transaction signature
  blockTime?: number;           // Unix timestamp

  // Instruction details
  instruction: {
    program: string;            // "Attention Oracle"
    programId: string;          // Base58 program address
    action: string;             // "claim", "finalize", "setPolicy", etc.
    data: Record<string, unknown>; // Decoded instruction args
  };

  // Context
  accounts?: string[];          // Instruction account keys
  meta?: {
    fee: number;
    preTokenBalances: unknown[];
    postTokenBalances: unknown[];
  };
}
```

### Example NDJSON Log Entry

```json
{"timestamp":"2025-11-18T22:30:45.123Z","slot":300000,"signature":"5P9z...","instruction":{"program":"Attention Oracle","programId":"GnGzNdsQ...","action":"claim","data":{"channel":"twitch_channel_123","user":"user_pubkey_123","amount":1000000}},"accounts":["GnGzNdsQ...","..."],"meta":{"fee":5000,"preTokenBalances":[],"postTokenBalances":[]}}
```

## Monitoring & Observability

### Logs

Logs are written to:
- **Console** (development): via `pino-pretty`
- **File** (production): `./logs/stream-listener.log`

Log levels: `debug`, `info`, `warn`, `error`

### Queue Monitoring

Check queue depth and job status:

```bash
# Using BullMQ Inspector (optional)
npm install bull-board
# Add to src/index.ts for HTTP UI

# Or via Redis CLI
redis-cli
> LLEN twzrd:stream-events:*
```

### Health Check

```bash
# Once health endpoint is added
curl http://localhost:4000/health
```

## Error Handling & Resilience

### Reconnection

If Solana connection drops:
- Exponential backoff (3s → 30s)
- Max 10 attempts before fatal error
- Graceful shutdown on SIGTERM/SIGINT

### Queue Retries

Events that fail processing:
- Retry up to 3 times
- Exponential backoff (2s initial delay)
- Moved to dead-letter queue after exhaustion

### Crash Recovery

If the listener crashes:
1. PM2 auto-restarts (if configured)
2. Resume from last confirmed Solana slot
3. Replay from NDJSON log if needed

## Integration with Aggregator

The Stream Listener feeds two downstream components:

### 1. BullMQ Queue
```typescript
// Aggregator subscribes to this queue
const queue = new Queue('stream-events', { connection });
queue.process(async (job) => {
  const event = job.data as StreamEvent;
  // Process event (build Merkle tree, etc.)
});
```

### 2. NDJSON Log
```bash
# Aggregator can replay from log
cat logs/stream-events.ndjson | jq '.instruction.action' | sort | uniq -c

# Or tail for real-time monitoring
tail -f logs/stream-events.ndjson
```

## Development

### Build

```bash
npm run build          # Compile TypeScript to dist/
npm run dev            # Watch mode
npm run lint           # Check code style
npm run test           # Run tests (if added)
```

### Project Structure

```
.
├── src/
│   ├── index.ts        # Entry point
│   ├── listener.ts     # StreamListener class
│   └── types.ts        # TypeScript interfaces
├── dist/               # Compiled output
├── logs/               # Event logs (NDJSON)
├── package.json        # Dependencies
├── tsconfig.json       # TypeScript config
└── .env.example        # Environment template
```

### Types

Export from `src/types.ts`:

```typescript
export interface StreamEvent { ... }
export interface StreamListenerConfig { ... }
export type Action = 'claim' | 'finalize' | 'setPolicy';
```

## Deployment

### Docker

```dockerfile
FROM node:20-alpine

WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production

COPY dist ./dist
COPY .env ./.env

EXPOSE 4000
CMD ["node", "dist/index.js"]
```

### systemd

```ini
[Unit]
Description=TWZRD Stream Listener
After=network.target redis.service

[Service]
Type=simple
User=twzrd
WorkingDirectory=/home/twzrd/stream-listener
ExecStart=/usr/bin/node /home/twzrd/stream-listener/dist/index.js
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Then:
```bash
sudo systemctl enable stream-listener
sudo systemctl start stream-listener
sudo systemctl status stream-listener
```

## Troubleshooting

### "Cannot connect to Redis"
```bash
# Check Redis is running
redis-cli ping
# Output: PONG

# Or start Redis
redis-server
```

### "Program subscription failed"
- Verify `AO_PROGRAM_ID` is correct
- Check RPC endpoint is accessible
- Try different RPC (Helius, QuickNode, etc.)

### "Events not queueing"
- Verify Redis connection works
- Check `REDIS_URL` in .env
- View queue stats: `redis-cli LLEN twzrd:stream-events:*`

### "Log file too large"
- Implement log rotation (see Production setup)
- Or use `./logs/stream-events.ndjson` → Archive daily

## Performance Characteristics

| Metric | Target | Notes |
|--------|--------|-------|
| Event ingest latency | <1s | From blockchain to queue |
| Queue throughput | >100 events/sec | BullMQ capable of 1000s/sec |
| Memory footprint | <200MB | Node.js heap |
| Disk I/O (logs) | ~10MB/hour | ~240MB/day (rotatable) |
| CPU usage | <5% idle | Minimal when no events |

## Roadmap

- [ ] Health check endpoint (`/health`)
- [ ] Prometheus metrics export
- [ ] Event filtering by instruction type
- [ ] Batch event publishing (reduce queue size)
- [ ] Multi-region failover support
- [ ] Event deduplication
- [ ] Automated NDJSON log rotation

## Contributing

1. Fork this repo
2. Create feature branch (`git checkout -b feat/xyz`)
3. Commit changes (`git commit -am 'Add xyz'`)
4. Push to branch (`git push origin feat/xyz`)
5. Open PR with description

## License

MIT

## Support

Issues or questions? Contact: dev@twzrd.xyz

---

**Part of the TWZRD Parallel Rebuild (v0.2.1-clean)**
Clean, typed, production-ready Solana infrastructure.
