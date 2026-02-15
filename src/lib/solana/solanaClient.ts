import { createSolanaClient } from 'gill';

/**
 * Resolve the Solana RPC endpoint from available env vars.
 * Priority: NEXT_PUBLIC_SOLANA_RPC > RPC_URL > SOLANA_RPC_URL > mainnet default.
 */
function resolveRpcUrl(): string {
  const candidates = [
    process.env.NEXT_PUBLIC_SOLANA_RPC,
    process.env.RPC_URL,
    process.env.SOLANA_RPC_URL,
  ];
  for (const url of candidates) {
    if (url && url.trim()) return url.trim();
  }
  return 'https://api.mainnet-beta.solana.com';
}

const RPC_URL = resolveRpcUrl();
const WS_URL = (process.env.NEXT_PUBLIC_SOLANA_WS && process.env.NEXT_PUBLIC_SOLANA_WS.trim()) || undefined;

export const solanaClient = createSolanaClient({
  urlOrMoniker: RPC_URL,
  websocketUrl: WS_URL,
  config: {
    commitment: 'confirmed',
    confirmOptions: {
      skipPreflight: false,
    },
  },
});

export const RPC_ENDPOINT = RPC_URL;
export const WS_ENDPOINT = WS_URL;
