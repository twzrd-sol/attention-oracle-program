#!/bin/bash

# Deploy CLS Workers S2-S4 with correct environment variables
# Using compiled JavaScript and proper env vars

echo "Deploying CLS Workers S2-S4..."

# Load environment variables
source /home/twzrd/milo-token/.env

# Worker S2 - Shard 2/5
pm2 start /home/twzrd/milo-token/apps/worker-v2/dist/index.js \
  --name cls-worker-s2 \
  --cwd /home/twzrd/milo-token/apps/worker-v2 \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "TWZRD_URL=http://localhost:3070" \
  --env "SHARD_INDEX=2" \
  --env "TOTAL_SHARDS=5" \
  --env "CHANNELS_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "CLS_BLOCKLIST=threadguy,threadguys,thethreadguy,notthreadguy,counterpartytv"

# Worker S3 - Shard 3/5
pm2 start /home/twzrd/milo-token/apps/worker-v2/dist/index.js \
  --name cls-worker-s3 \
  --cwd /home/twzrd/milo-token/apps/worker-v2 \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "TWZRD_URL=http://localhost:3070" \
  --env "SHARD_INDEX=3" \
  --env "TOTAL_SHARDS=5" \
  --env "CHANNELS_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "CLS_BLOCKLIST=threadguy,threadguys,thethreadguy,notthreadguy,counterpartytv"

# Worker S4 - Shard 4/5
pm2 start /home/twzrd/milo-token/apps/worker-v2/dist/index.js \
  --name cls-worker-s4 \
  --cwd /home/twzrd/milo-token/apps/worker-v2 \
  --env "NODE_ENV=production" \
  --env "DATABASE_URL=$DATABASE_URL" \
  --env "TWZRD_URL=http://localhost:3070" \
  --env "SHARD_INDEX=4" \
  --env "TOTAL_SHARDS=5" \
  --env "CHANNELS_FILE=/home/twzrd/milo-token/config/cls-channels.json" \
  --env "CLS_BLOCKLIST=threadguy,threadguys,thethreadguy,notthreadguy,counterpartytv"

pm2 save

echo "Workers deployed. Checking status..."
pm2 list | grep cls-worker