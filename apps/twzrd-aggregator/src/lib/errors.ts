// Utility helpers for RPC error handling

/**
 * Determines whether an RPC error should trigger a cooldown for the endpoint.
 * We only penalize errors that indicate server-side issues or rate limiting.
 */
export function isReportableRpcError(error: any): boolean {
  if (!error) return false;

  const message = typeof error.message === 'string' ? error.message.toLowerCase() : '';
  const rpcCode = (error as { code?: number }).code;

  // Solana-specific throttling code.
  if (rpcCode === -32005) return true;

  // HTTP status hints embedded in error messages.
  if (message.includes('429')) return true;
  if (message.includes('503')) return true;

  // Network / transport issues.
  if (message.includes('failed to fetch')) return true;
  if (message.includes('network error')) return true;
  if (message.includes('server-side error')) return true;
  if (message.includes('timeout')) return true;

  return false;
}

