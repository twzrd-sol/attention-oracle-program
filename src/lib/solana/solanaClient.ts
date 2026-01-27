import { createSolanaClient } from 'gill';

const RPC_URL = (process.env.NEXT_PUBLIC_SOLANA_RPC && process.env.NEXT_PUBLIC_SOLANA_RPC.trim()) || process.env.SYNDICA_RPC!;
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

