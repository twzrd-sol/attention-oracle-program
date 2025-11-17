-- Migration: Add verification audit trail
-- Date: 2025-11-15
-- Purpose: Enable traceability of all verification status changes

-- Add audit columns to social_verification
ALTER TABLE social_verification ADD COLUMN updated_by TEXT;
ALTER TABLE social_verification ADD COLUMN update_reason TEXT;

-- Create verification_audit table for immutable change history
CREATE TABLE IF NOT EXISTS verification_audit (
  id BIGSERIAL PRIMARY KEY,
  wallet TEXT NOT NULL,
  field_name VARCHAR(50) NOT NULL,
  old_value TEXT,
  new_value TEXT,
  changed_by TEXT NOT NULL,
  change_reason TEXT,
  changed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  FOREIGN KEY (wallet) REFERENCES social_verification(wallet)
);

-- Index for audit queries
CREATE INDEX IF NOT EXISTS idx_verification_audit_wallet
  ON verification_audit(wallet);

CREATE INDEX IF NOT EXISTS idx_verification_audit_changed_at
  ON verification_audit(changed_at DESC);

CREATE INDEX IF NOT EXISTS idx_verification_audit_changed_by
  ON verification_audit(changed_by);

-- View for recent changes
CREATE OR REPLACE VIEW verification_changes_recent AS
SELECT
  wallet,
  field_name,
  old_value,
  new_value,
  changed_by,
  change_reason,
  changed_at
FROM verification_audit
ORDER BY changed_at DESC
LIMIT 1000;
