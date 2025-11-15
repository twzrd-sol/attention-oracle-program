#!/bin/bash

# Deploy CLS Workers S2-S4 with proper configuration
# Each worker gets a unique port and shard assignment

echo "Deploying CLS Workers S2-S4..."

# Worker S2 - Port 8082, Shard 2/5
pm2 start apps/worker-v2/src/index.ts \
  --name cls-worker-s2 \
  --interpreter tsx \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "WORKER_PORT=8082" \
  --env "WORKER_SHARD=2" \
  --env "WORKER_TOTAL_SHARDS=5" \
  --env "CLS_CHANNEL_LIST_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "DISABLE_HTTP_SERVER=true" \
  --cwd /home/twzrd/milo-token

# Worker S3 - Port 8083, Shard 3/5
pm2 start apps/worker-v2/src/index.ts \
  --name cls-worker-s3 \
  --interpreter tsx \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "WORKER_PORT=8083" \
  --env "WORKER_SHARD=3" \
  --env "WORKER_TOTAL_SHARDS=5" \
  --env "CLS_CHANNEL_LIST_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "DISABLE_HTTP_SERVER=true" \
  --cwd /home/twzrd/milo-token

# Worker S4 - Port 8084, Shard 4/5
pm2 start apps/worker-v2/src/index.ts \
  --name cls-worker-s4 \
  --interpreter tsx \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "WORKER_PORT=8084" \
  --env "WORKER_SHARD=4" \
  --env "WORKER_TOTAL_SHARDS=5" \
  --env "CLS_CHANNEL_LIST_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "DISABLE_HTTP_SERVER=true" \
  --cwd /home/twzrd/milo-token

pm2 save

echo "Workers deployed. Checking status..."
pm2 list | grep cls-worker