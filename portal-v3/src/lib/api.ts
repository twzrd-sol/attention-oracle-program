/**
 * Typed API client for TWZRD gateway backend
 *
 * Handles:
 * - Verification status (Twitter, Discord)
 * - Claim transaction building
 * - Error handling with detailed messages
 */

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

/**
 * Fetch verification status for a wallet
 */
export async function getVerificationStatus(wallet: string): Promise<VerificationStatus> {
  try {
    const response = await fetch(`/api/verification-status?wallet=${encodeURIComponent(wallet)}`);

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
    const response = await fetch('/api/claim-cls', {
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
