import { PublicKey, clusterApiUrl } from '@solana/web3.js';
import { WalletAdapterNetwork } from '@solana/wallet-adapter-base';

/**
 * Solana Network Configuration
 */

// Network (mainnet-beta, devnet, testnet)
export const NETWORK = (process.env.VITE_SOLANA_NETWORK || 'mainnet-beta') as WalletAdapterNetwork;

// RPC Endpoint
export const RPC_URL = process.env.VITE_SOLANA_RPC || clusterApiUrl(NETWORK);

// Program IDs
export const PROGRAM_ID = new PublicKey(
  process.env.VITE_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop'
);

export const TOKEN_2022_PROGRAM_ID = new PublicKey(
  'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBP4nEde2Kyn'
);

export const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey(
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
