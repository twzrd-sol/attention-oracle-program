import { PublicKey } from '@solana/web3.js';

/**
 * Passport tier levels (0-6)
 */
export enum PassportTier {
  Unverified = 0,
  Emerging = 1,
  Active = 2,
  Established = 3,
  Featured = 4,
  Elite = 5,
  Legendary = 6,
}

/**
 * Merkle proof for claiming tokens
 */
export interface MerkleProof {
  /** User's wallet address */
  claimer: PublicKey;
  /** Index in the merkle tree */
  index: number;
  /** Amount to claim (in lamports) */
  amount: bigint;
  /** Unique claim ID */
  id: string;
  /** Merkle proof hashes */
  proof: Buffer[];
  /** Epoch index */
  epochIndex: number;
}

/**
 * Channel configuration
 */
export interface ChannelConfig {
  /** Channel identifier (e.g., "kaicenat") */
  id: string;
  /** Channel authority pubkey */
  authority: PublicKey;
  /** Whether channel is active */
  active: boolean;
  /** Merkle root for current epoch */
  merkleRoot: Buffer;
}

/**
 * Passport account state
 */
export interface PassportState {
  /** Current tier */
  tier: PassportTier;
  /** Engagement points */
  points: bigint;
  /** Last update timestamp */
  lastUpdate: bigint;
  /** Oracle that issued passport */
  oracle: PublicKey;
}

/**
 * Claim receipt
 */
export interface ClaimReceipt {
  /** User who claimed */
  claimer: PublicKey;
  /** Channel ID */
  channelId: string;
  /** Amount claimed */
  amount: bigint;
  /** Epoch index */
  epochIndex: number;
  /** Transaction signature */
  signature: string;
  /** Timestamp */
  timestamp: number;
}

/**
 * Fee configuration
 */
export interface FeeConfig {
  /** Treasury fee basis points */
  treasuryFeeBps: number;
  /** Creator fee basis points */
  creatorFeeBps: number;
  /** Tier multipliers (array of 7 multipliers) */
  tierMultipliers: number[];
}
