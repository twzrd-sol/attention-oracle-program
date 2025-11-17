-- Migration: Create social_verification and cls_claims tables
-- Date: 2025-11-15
-- Owner: Agent B

-- Table: social_verification
-- Purpose: Track off-chain verification (Twitter, Discord)
CREATE TABLE IF NOT EXISTS social_verification (
  wallet              TEXT PRIMARY KEY,
  twitter_handle      TEXT,
  twitter_followed    BOOLEAN NOT NULL DEFAULT FALSE,
  discord_id          TEXT,
  discord_joined      BOOLEAN NOT NULL DEFAULT FALSE,
  passport_tier       INTEGER,
  last_verified       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_social_verification_wallet
  ON social_verification(wallet);

CREATE INDEX IF NOT EXISTS idx_social_verification_discord_id
  ON social_verification(discord_id);

CREATE INDEX IF NOT EXISTS idx_social_verification_twitter_handle
  ON social_verification(twitter_handle);

-- Table: epochs
-- Purpose: Store epoch metadata (merkle roots, status)
CREATE TABLE IF NOT EXISTS epochs (
  epoch_id            INTEGER PRIMARY KEY,
  merkle_root         TEXT NOT NULL,
  is_open             BOOLEAN NOT NULL DEFAULT TRUE,
  total_allocation    BIGINT,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  closed_at           TIMESTAMPTZ
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_epochs_is_open
  ON epochs(is_open);

-- Table: cls_claims
-- Purpose: Track claims (enforce one-per-epoch-per-wallet)
CREATE TABLE IF NOT EXISTS cls_claims (
  id                  BIGSERIAL PRIMARY KEY,
  wallet              TEXT NOT NULL,
  epoch_id            INTEGER NOT NULL,
  amount              BIGINT,
  tx_signature        TEXT,
  tx_status           VARCHAR(20) DEFAULT 'pending',
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  confirmed_at        TIMESTAMPTZ,
  UNIQUE(wallet, epoch_id),
  FOREIGN KEY (wallet) REFERENCES social_verification(wallet),
  FOREIGN KEY (epoch_id) REFERENCES epochs(epoch_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_cls_claims_wallet
  ON cls_claims(wallet);

CREATE INDEX IF NOT EXISTS idx_cls_claims_epoch
  ON cls_claims(epoch_id);

CREATE INDEX IF NOT EXISTS idx_cls_claims_signature
  ON cls_claims(tx_signature);

CREATE INDEX IF NOT EXISTS idx_cls_claims_status
  ON cls_claims(tx_status);

CREATE INDEX IF NOT EXISTS idx_cls_claims_created_at
  ON cls_claims(created_at);
