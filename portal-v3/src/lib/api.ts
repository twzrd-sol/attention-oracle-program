/**
 * Typed API client for TWZRD gateway backend
 *
 * Handles:
 * - Verification status (Twitter, Discord)
 * - Claim transaction building
 * - Error handling with detailed messages
 */

/**
 * API base URL - supports separate domain deployments
 * Falls back to relative paths if VITE_GATEWAY_URL not set
 */
const API_BASE = import.meta.env.VITE_GATEWAY_URL?.replace(/\/+$/, '') || '';

const api = (path: string): string => {
  if (!API_BASE) return path;
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${API_BASE}${normalizedPath}`;
};

export interface VerificationStatus {
  twitterFollowed: boolean;
  discordJoined: boolean;
  passportTier?: number;
  lastVerified?: string;
}

export interface ClaimRequest {
  wallet: string;
  epochId: number;
}

export interface ClaimResponse {
  transaction: string; // base64-encoded transaction
  signature?: string;
}

export interface ApiError {
  error: string;
  details?: string;
  code?: number;
}

export interface BindingResponse {
  ok?: boolean;
  userHash?: string;
}

export interface BoundWalletResponse {
  wallet: string | null;
  userHash?: string | null;
}

function authHeaders(token?: string): Record<string, string> {
  if (!token) return {};
  return { Authorization: `Bearer ${token}` };
}

/**
 * Fetch verification status for a wallet
 */
export async function getVerificationStatus(wallet: string): Promise<VerificationStatus> {
  try {
    const response = await fetch(api(`/api/verification-status?wallet=${encodeURIComponent(wallet)}`));

    if (!response.ok) {
      const error = (await response.json()) as ApiError;
      throw new Error(error.details || error.error || 'Failed to fetch verification status');
    }

    return (await response.json()) as VerificationStatus;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    throw new Error(`Verification status error: ${message}`);
  }
}

/**
 * Request a claim transaction
 */
export async function requestClaimTransaction(
  wallet: string,
  epochId: number
): Promise<ClaimResponse> {
  try {
    const response = await fetch(api('/api/claim-cls'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        wallet,
        epochId,
      } as ClaimRequest),
    });

    if (!response.ok) {
      const error = (await response.json()) as ApiError;
      throw new Error(error.details || error.error || 'Failed to request claim transaction');
    }

    return (await response.json()) as ClaimResponse;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    throw new Error(`Claim request error: ${message}`);
  }
}

export async function bindWalletWithTwitch(token: string, wallet: string): Promise<BindingResponse> {
  const response = await fetch(api('/api/bindings/bind-wallet'), {
    method: 'POST',
    headers: Object.assign({ 'Content-Type': 'application/json' }, authHeaders(token)),
    body: JSON.stringify({ wallet }),
  });

  if (!response.ok) {
    const error = (await response.json()) as ApiError;
    throw new Error(error.details || error.error || 'Failed to bind wallet');
  }

  return (await response.json()) as BindingResponse;
}

export async function fetchBoundWallet(token: string): Promise<BoundWalletResponse> {
  const response = await fetch(api('/api/bindings/bound-wallet'), {
    headers: authHeaders(token),
  });

  if (!response.ok) {
    const error = (await response.json()) as ApiError;
    throw new Error(error.details || error.error || 'Failed to fetch bound wallet');
  }

  return (await response.json()) as BoundWalletResponse;
}

/**
 * Check Twitter follow status
 */
export function getTwitterUrl(): string {
  return 'https://twitter.com/twzrd_xyz';
}

/**
 * Check Discord join status
 */
export function getDiscordInviteUrl(): string {
  return 'https://discord.gg/twzrd';
}

// ============================================================================
// EPOCH & CLAIM HISTORY TYPES (Week 4)
// ============================================================================

export interface Epoch {
  epoch_id: number;
  merkle_root: string;
  is_open: boolean;
  created_at: string;
  total_claimers: number;
  total_amount: number;
}

export interface EpochsResponse {
  epochs: Epoch[];
  total: number;
  limit: number;
  offset: number;
}

export interface ClaimRecord {
  epoch_id: number;
  amount: string;
  tx_signature: string | null;
  status: 'pending' | 'confirmed' | 'failed';
  claimed_at: string;
  tier?: number;
  fee_amount?: string;
}

export interface ClaimHistoryResponse {
  wallet: string;
  claims: ClaimRecord[];
  total: number;
  limit: number;
  offset: number;
}

// ============================================================================
// EPOCH & CLAIM HISTORY ENDPOINTS (Week 4)
// ============================================================================

/**
 * Get list of available epochs for claiming
 */
export async function getEpochs(limit = 50, offset = 0): Promise<EpochsResponse> {
  try {
    const params = new URLSearchParams({
      limit: limit.toString(),
      offset: offset.toString(),
    });

    const response = await fetch(api(`/api/epochs?${params.toString()}`));

    if (!response.ok) {
      const error = (await response.json()) as ApiError;
      throw new Error(error.details || error.error || 'Failed to fetch epochs');
    }

    return (await response.json()) as EpochsResponse;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    throw new Error(`Epochs fetch error: ${message}`);
  }
}

/**
 * Get single epoch details
 */
export async function getEpoch(epochId: number): Promise<Epoch> {
  try {
    const response = await fetch(api(`/api/epochs/${epochId}`));

    if (!response.ok) {
      const error = (await response.json()) as ApiError;
      throw new Error(error.details || error.error || 'Failed to fetch epoch');
    }

    return (await response.json()) as Epoch;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    throw new Error(`Epoch fetch error: ${message}`);
  }
}

/**
 * Get claim history for a wallet
 */
export async function getClaimHistory(
  wallet: string,
  limit = 50,
  offset = 0
): Promise<ClaimHistoryResponse> {
  try {
    const params = new URLSearchParams({
      wallet,
      limit: limit.toString(),
      offset: offset.toString(),
    });

    const response = await fetch(api(`/api/claims/history?${params.toString()}`));

    if (!response.ok) {
      const error = (await response.json()) as ApiError;
      throw new Error(error.details || error.error || 'Failed to fetch claim history');
    }

    return (await response.json()) as ClaimHistoryResponse;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown error';
    throw new Error(`Claim history fetch error: ${message}`);
  }
}
