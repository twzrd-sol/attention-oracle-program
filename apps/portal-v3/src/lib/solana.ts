import { PublicKey as OriginalPublicKey, clusterApiUrl } from '@solana/web3.js';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';

// Re-export PublicKey for convenience
export { PublicKey } from '@solana/web3.js';

/**
 * Solana Network Configuration
 */

// Network (mainnet-beta, devnet, testnet)
const NETWORK_VALUE = import.meta.env.VITE_SOLANA_NETWORK || 'mainnet-beta';
export const NETWORK = NETWORK_VALUE as WalletAdapterNetwork;

// RPC Endpoint
const RPC_VALUE = import.meta.env.VITE_SOLANA_RPC || clusterApiUrl(NETWORK);
export const RPC_URL = RPC_VALUE;

// Program IDs
const PROGRAM_ID_VALUE = import.meta.env.VITE_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';
export const PROGRAM_ID = new OriginalPublicKey(PROGRAM_ID_VALUE);

export const TOKEN_2022_PROGRAM_ID = new OriginalPublicKey(
  'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBP4nEde2Kyn'
);

export const ASSOCIATED_TOKEN_PROGRAM_ID = new OriginalPublicKey(
  'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'
);

/**
 * Explorer URL for transactions
 */
export const getExplorerUrl = (signature: string): string => {
  const cluster = NETWORK === 'mainnet-beta' ? '' : `?cluster=${NETWORK}`;
  return `https://solscan.io/tx/${signature}${cluster}`;
};

/**
 * Cluster display name
 */
export const getClusterName = (): string => {
  switch (NETWORK) {
    case 'mainnet-beta':
      return 'Mainnet Beta';
    case 'devnet':
      return 'Devnet';
    case 'testnet':
      return 'Testnet';
    default:
      return NETWORK;
  }
};

/**
 * Is mainnet?
 */
export const isMainnet = (): boolean => NETWORK === 'mainnet-beta';
