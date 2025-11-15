import { PublicKey } from '@solana/web3.js';

/**
 * Channel state account data
 */
export interface ChannelState {
  streamer: PublicKey;
  mint: PublicKey;
  currentEpoch: number;
  totalMinted: bigint;
  active: boolean;
}

/**
 * User claim data
 */
export interface ClaimData {
  user: PublicKey;
  channel: PublicKey;
  amount: bigint;
  timestamp: number;
}
