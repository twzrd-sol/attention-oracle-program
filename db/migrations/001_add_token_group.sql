-- Migration: 001_add_token_group.sql
-- Date: 2025-11-17
-- Description: Add token_group column to sealed_epochs and sealed_participants
--              to support multi-token tracking (MILO, CLS, etc.)

BEGIN;

-- Add token_group to sealed_epochs
ALTER TABLE sealed_epochs
  ADD COLUMN IF NOT EXISTS token_group VARCHAR(10) DEFAULT 'milo';

-- Add token_group to sealed_participants
ALTER TABLE sealed_participants
  ADD COLUMN IF NOT EXISTS token_group VARCHAR(10) DEFAULT 'milo';

COMMIT;
