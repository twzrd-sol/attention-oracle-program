// API Request/Response Types

export interface VerificationStatusResponse {
  twitterFollowed: boolean;
  discordJoined: boolean;
  passportTier: number | null;
  lastVerified: string | null;
}

export interface ClaimRequest {
  wallet: string;
  epochId: number;
}

export interface ClaimResponse {
  transaction: string; // base64-encoded transaction
  signature: null;
}

export interface ApiError {
  error: string;
  details?: string;
}

export interface ValidationError extends ApiError {
  status: number;
}

// Database row types

export interface SocialVerificationRow {
  wallet: string;
  twitter_handle: string | null;
  twitter_followed: boolean;
  discord_id: string | null;
  discord_joined: boolean;
  passport_tier: number | null;
  last_verified: string;
  created_at: string;
  updated_at: string;
}

export interface EpochRow {
  epoch_id: number;
  merkle_root: string;
  is_open: boolean;
  total_allocation: number | null;
  created_at: string;
  closed_at: string | null;
}

export interface ClsClaimRow {
  id: number;
  wallet: string;
  epoch_id: number;
  amount: number | null;
  tx_signature: string | null;
  tx_status: string;
  created_at: string;
  confirmed_at: string | null;
}
