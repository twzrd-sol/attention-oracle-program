import { PublicKey } from '@solana/web3.js';

// Program configuration
export const PROGRAM_ID = new PublicKey(
  import.meta.env.VITE_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop'
);

// Token program IDs
export const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBP4nEde2Kyn');
export const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
export const SYSTEM_PROGRAM_ID = new PublicKey('11111111111111111111111111111111');

// PDA Seeds
export const PROTOCOL_SEED = 'protocol';
export const CHANNEL_STATE_SEED = 'channel_state';

// Network configuration
export const RPC_URL = import.meta.env.VITE_SOLANA_RPC || 'https://api.mainnet-beta.solana.com';
export const NETWORK = (import.meta.env.VITE_SOLANA_NETWORK || 'mainnet-beta') as 'mainnet-beta' | 'devnet' | 'testnet';

// Explorer configuration
export const EXPLORER_URL = {
  'mainnet-beta': 'https://explorer.solana.com',
  'devnet': 'https://explorer.solana.com?cluster=devnet',
  'testnet': 'https://explorer.solana.com?cluster=testnet',
}[NETWORK];

// Transfer fee configuration
export const TRANSFER_FEE_BPS = 100; // 1% = 100 basis points

// Tier multipliers (basis points, 0-100)
export const TIER_MULTIPLIERS = {
  0: 0,    // Unverified: 0.0x
  1: 20,   // Emerging: 0.2x
  2: 40,   // Active: 0.4x
  3: 60,   // Established: 0.6x
  4: 80,   // Featured: 0.8x
  5: 100,  // Elite: 1.0x
} as const;

export const TIER_LABELS = {
  0: 'Unverified',
  1: 'Emerging',
  2: 'Active',
  3: 'Established',
  4: 'Featured',
  5: 'Elite',
} as const;
