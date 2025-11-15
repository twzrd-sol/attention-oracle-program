-- PostgreSQL schema for Twzrd aggregator
-- Migrated from SQLite with concurrency improvements

-- Per-channel participation tracking
CREATE TABLE IF NOT EXISTS channel_participation (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  user_hash TEXT NOT NULL,
  first_seen BIGINT NOT NULL,
  PRIMARY KEY (epoch, channel, user_hash)
);

CREATE INDEX IF NOT EXISTS idx_channel_epoch
  ON channel_participation(channel, epoch);

CREATE INDEX IF NOT EXISTS idx_epoch
  ON channel_participation(epoch);

-- Weighted signals (v0.1)
CREATE TABLE IF NOT EXISTS user_signals (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  user_hash TEXT NOT NULL,
  signal_type TEXT NOT NULL,
  value REAL NOT NULL,
  timestamp BIGINT NOT NULL,
  PRIMARY KEY (epoch, channel, user_hash, signal_type, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_signals_lookup
  ON user_signals(epoch, channel, user_hash);

-- Sealed epochs snapshot (prevents root churn during claims)
CREATE TABLE IF NOT EXISTS sealed_epochs (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  root TEXT NOT NULL,
  sealed_at BIGINT NOT NULL,
  published INTEGER DEFAULT 0,
  PRIMARY KEY (epoch, channel)
);

-- Frozen participant order for sealed epochs
CREATE TABLE IF NOT EXISTS sealed_participants (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  idx INTEGER NOT NULL,
  user_hash TEXT NOT NULL,
  username TEXT,
  PRIMARY KEY (epoch, channel, idx)
);

-- Username mapping for MILO compatibility
CREATE TABLE IF NOT EXISTS user_mapping (
  user_hash TEXT PRIMARY KEY,
  username TEXT NOT NULL,
  first_seen BIGINT NOT NULL
);

-- L2 Merkle tree cache (claim trees with index|amount|id format)
CREATE TABLE IF NOT EXISTS l2_tree_cache (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  root TEXT NOT NULL,
  levels_json TEXT NOT NULL,
  participant_count INTEGER NOT NULL,
  built_at BIGINT NOT NULL,
  PRIMARY KEY (epoch, channel)
);

-- Attention index (hourly publisher stub)
CREATE TABLE IF NOT EXISTS attention_index (
  epoch BIGINT NOT NULL,
  channel TEXT NOT NULL,
  value REAL NOT NULL,
  participants INTEGER NOT NULL,
  messages INTEGER NOT NULL,
  computed_at BIGINT NOT NULL,
  PRIMARY KEY (epoch, channel)
);

-- Grant permissions to twzrd user
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO twzrd;
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO twzrd;
