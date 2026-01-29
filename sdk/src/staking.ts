/**
 * Channel Staking Client - Attention Oracle Protocol
 *
 * Provides helpers for interacting with the Token-2022 NonTransferable staking system.
 */

import {
  PublicKey,
  Connection,
  TransactionInstruction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { BN, Program, AnchorProvider } from "@coral-xyz/anchor";

// =============================================================================
// CONSTANTS
// =============================================================================

export const PROGRAM_ID = new PublicKey(
  "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
);

// PDA Seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const CHANNEL_USER_STAKE_SEED = Buffer.from("channel_user");
const STAKE_NFT_MINT_SEED = Buffer.from("stake_nft");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

// Boost tiers (slots per day ~ 216,000 at 400ms)
const SLOTS_PER_DAY = 216_000n;
const BOOST_TIERS = [
  { maxDays: 6, bps: 10_000 }, // 1.0x
  { maxDays: 29, bps: 12_500 }, // 1.25x
  { maxDays: 89, bps: 15_000 }, // 1.5x
  { maxDays: 179, bps: 20_000 }, // 2.0x
  { maxDays: 364, bps: 25_000 }, // 2.5x
  { maxDays: Infinity, bps: 30_000 }, // 3.0x
];

// =============================================================================
// PDA DERIVATION
// =============================================================================

export function deriveProtocolState(mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveChannelConfig(
  mint: PublicKey,
  channelName: string
): [PublicKey, number] {
  const channelHash = hashChannel(channelName);
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, mint.toBuffer(), channelHash],
    PROGRAM_ID
  );
}

export function deriveStakePool(
  channelConfig: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveUserStake(
  channelConfig: PublicKey,
  user: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_USER_STAKE_SEED, channelConfig.toBuffer(), user.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveStakeNftMint(
  stakePool: PublicKey,
  user: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [STAKE_NFT_MINT_SEED, stakePool.toBuffer(), user.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveStakeVault(stakePool: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, stakePool.toBuffer()],
    PROGRAM_ID
  );
}

// Hash channel name using Keccak256 (matches on-chain)
function hashChannel(channelName: string): Buffer {
  // Use browser-compatible keccak or import from ethers/noble
  const { keccak256 } = require("js-sha3");
  return Buffer.from(keccak256(channelName), "hex");
}

// =============================================================================
// BOOST CALCULATION
// =============================================================================

/**
 * Calculate boost multiplier in basis points based on lock duration.
 * @param lockSlots - Lock duration in slots
 * @returns Boost in basis points (10000 = 1.0x, 30000 = 3.0x)
 */
export function calculateBoostBps(lockSlots: bigint): number {
  const days = Number(lockSlots / SLOTS_PER_DAY);
  for (const tier of BOOST_TIERS) {
    if (days <= tier.maxDays) return tier.bps;
  }
  return 30_000; // Max boost
}

/**
 * Convert days to slots for lock duration.
 */
export function daysToSlots(days: number): bigint {
  return BigInt(days) * SLOTS_PER_DAY;
}

// =============================================================================
// ACCOUNT TYPES
// =============================================================================

export interface ChannelStakePool {
  bump: number;
  channel: PublicKey;
  mint: PublicKey;
  vault: PublicKey;
  totalStaked: bigint;
  totalWeighted: bigint;
  stakerCount: bigint;
}

export interface UserChannelStake {
  bump: number;
  user: PublicKey;
  channel: PublicKey;
  amount: bigint;
  startSlot: bigint;
  lockEndSlot: bigint;
  multiplierBps: bigint;
  nftMint: PublicKey;
}

export interface StakePosition {
  /** User wallet */
  user: PublicKey;
  /** Channel config pubkey */
  channel: PublicKey;
  /** Staked amount (raw, divide by 10^9 for CCM) */
  amount: bigint;
  /** Lock end slot (0 = no lock) */
  lockEndSlot: bigint;
  /** Boost multiplier (10000 = 1x) */
  multiplierBps: bigint;
  /** Soulbound NFT mint */
  nftMint: PublicKey;
  /** Is currently locked? */
  isLocked: boolean;
  /** Effective boost multiplier (e.g., 1.5) */
  boostMultiplier: number;
}

// =============================================================================
// ACCOUNT FETCHERS
// =============================================================================

/**
 * Fetch stake pool for a channel.
 */
export async function fetchStakePool(
  connection: Connection,
  channelConfig: PublicKey
): Promise<ChannelStakePool | null> {
  const [stakePoolPda] = deriveStakePool(channelConfig);
  const accountInfo = await connection.getAccountInfo(stakePoolPda);
  if (!accountInfo) return null;

  // Deserialize (skip 8-byte discriminator)
  const data = accountInfo.data.slice(8);
  return {
    bump: data[0],
    channel: new PublicKey(data.slice(1, 33)),
    mint: new PublicKey(data.slice(33, 65)),
    vault: new PublicKey(data.slice(65, 97)),
    totalStaked: data.readBigUInt64LE(97),
    totalWeighted: data.readBigUInt64LE(105),
    stakerCount: data.readBigUInt64LE(113),
  };
}

/**
 * Fetch user's stake position on a channel.
 */
export async function fetchUserStake(
  connection: Connection,
  channelConfig: PublicKey,
  user: PublicKey
): Promise<StakePosition | null> {
  const [userStakePda] = deriveUserStake(channelConfig, user);
  const accountInfo = await connection.getAccountInfo(userStakePda);
  if (!accountInfo) return null;

  // Deserialize (skip 8-byte discriminator)
  const data = accountInfo.data.slice(8);
  const stake: UserChannelStake = {
    bump: data[0],
    user: new PublicKey(data.slice(1, 33)),
    channel: new PublicKey(data.slice(33, 65)),
    amount: data.readBigUInt64LE(65),
    startSlot: data.readBigUInt64LE(73),
    lockEndSlot: data.readBigUInt64LE(81),
    multiplierBps: data.readBigUInt64LE(89),
    nftMint: new PublicKey(data.slice(97, 129)),
  };

  // Get current slot to determine lock status
  const slot = await connection.getSlot();
  const isLocked = stake.lockEndSlot > 0n && BigInt(slot) < stake.lockEndSlot;

  return {
    user: stake.user,
    channel: stake.channel,
    amount: stake.amount,
    lockEndSlot: stake.lockEndSlot,
    multiplierBps: stake.multiplierBps,
    nftMint: stake.nftMint,
    isLocked,
    boostMultiplier: Number(stake.multiplierBps) / 10_000,
  };
}

/**
 * Fetch all stake positions for a user across channels.
 */
export async function fetchAllUserStakes(
  connection: Connection,
  user: PublicKey
): Promise<StakePosition[]> {
  const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
    filters: [
      { dataSize: 137 }, // UserChannelStake size
      { memcmp: { offset: 9, bytes: user.toBase58() } }, // user field at offset 9
    ],
  });

  const currentSlot = await connection.getSlot();

  return accounts.map(({ account }) => {
    const data = account.data.slice(8);
    const lockEndSlot = data.readBigUInt64LE(81);
    const multiplierBps = data.readBigUInt64LE(89);

    return {
      user: new PublicKey(data.slice(1, 33)),
      channel: new PublicKey(data.slice(33, 65)),
      amount: data.readBigUInt64LE(65),
      lockEndSlot,
      multiplierBps,
      nftMint: new PublicKey(data.slice(97, 129)),
      isLocked: lockEndSlot > 0n && BigInt(currentSlot) < lockEndSlot,
      boostMultiplier: Number(multiplierBps) / 10_000,
    };
  });
}

// =============================================================================
// INSTRUCTION HELPERS (for manual TX building)
// =============================================================================

export interface StakeChannelParams {
  user: PublicKey;
  mint: PublicKey;
  channelConfig: PublicKey;
  amount: bigint;
  lockDuration: bigint;
}

/**
 * Get all accounts needed for stake_channel instruction.
 */
export function getStakeChannelAccounts(params: StakeChannelParams) {
  const { user, mint, channelConfig } = params;

  const [protocolState] = deriveProtocolState(mint);
  const [stakePool] = deriveStakePool(channelConfig);
  const [userStake] = deriveUserStake(channelConfig, user);
  const [vault] = deriveStakeVault(stakePool);
  const [nftMint] = deriveStakeNftMint(stakePool, user);

  const userTokenAccount = getAssociatedTokenAddressSync(
    mint,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  const nftAta = getAssociatedTokenAddressSync(
    nftMint,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  return {
    user,
    protocolState,
    channelConfig,
    mint,
    stakePool,
    userStake,
    vault,
    userTokenAccount,
    nftMint,
    nftAta,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
    rent: SYSVAR_RENT_PUBKEY,
  };
}

export interface UnstakeChannelParams {
  user: PublicKey;
  mint: PublicKey;
  channelConfig: PublicKey;
  nftMint: PublicKey;
}

/**
 * Get all accounts needed for unstake_channel instruction.
 */
export function getUnstakeChannelAccounts(params: UnstakeChannelParams) {
  const { user, mint, channelConfig, nftMint } = params;

  const [stakePool] = deriveStakePool(channelConfig);
  const [userStake] = deriveUserStake(channelConfig, user);
  const [vault] = deriveStakeVault(stakePool);

  const userTokenAccount = getAssociatedTokenAddressSync(
    mint,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  const nftAta = getAssociatedTokenAddressSync(
    nftMint,
    user,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  return {
    user,
    channelConfig,
    mint,
    stakePool,
    userStake,
    vault,
    userTokenAccount,
    nftMint,
    nftAta,
    tokenProgram: TOKEN_2022_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  };
}

/**
 * Get all accounts needed for emergency_unstake_channel instruction.
 * Same as regular unstake, allows early exit with 20% penalty.
 */
export function getEmergencyUnstakeChannelAccounts(params: UnstakeChannelParams) {
  // Emergency unstake uses same accounts as regular unstake
  return getUnstakeChannelAccounts(params);
}

/**
 * Calculate emergency unstake penalty and returned amount.
 * @param stakedAmount - Original staked amount
 * @returns { penalty, returnAmount } - 20% penalty, 80% returned
 */
export function calculateEmergencyUnstakePenalty(stakedAmount: bigint): {
  penalty: bigint;
  returnAmount: bigint;
} {
  const penalty = (stakedAmount * 20n) / 100n;
  const returnAmount = stakedAmount - penalty;
  return { penalty, returnAmount };
}

// =============================================================================
// EVENT PARSING
// =============================================================================

export interface ChannelStakedEvent {
  user: PublicKey;
  channel: PublicKey;
  amount: bigint;
  nftMint: PublicKey;
  lockDuration: bigint;
  boostBps: bigint;
  timestamp: bigint;
}

export interface ChannelUnstakedEvent {
  user: PublicKey;
  channel: PublicKey;
  amount: bigint;
  nftMint: PublicKey;
  timestamp: bigint;
}

export interface ChannelEmergencyUnstakedEvent {
  user: PublicKey;
  channel: PublicKey;
  stakedAmount: bigint;
  penaltyAmount: bigint;
  returnedAmount: bigint;
  nftMint: PublicKey;
  remainingLockSlots: bigint;
  timestamp: bigint;
}

// Event discriminators (first 8 bytes of sha256("event:EventName"))
const CHANNEL_STAKED_DISCRIMINATOR = Buffer.from([
  175, 97, 90, 63, 76, 199, 203, 0,
]);
const CHANNEL_UNSTAKED_DISCRIMINATOR = Buffer.from([
  227, 238, 141, 136, 146, 34, 74, 204,
]);
const CHANNEL_EMERGENCY_UNSTAKED_DISCRIMINATOR = Buffer.from([
  130, 174, 184, 149, 211, 66, 126, 79,
]);

/**
 * Parse ChannelStaked event from transaction logs.
 */
export function parseChannelStakedEvent(
  data: Buffer
): ChannelStakedEvent | null {
  if (!data.slice(0, 8).equals(CHANNEL_STAKED_DISCRIMINATOR)) return null;

  const offset = 8;
  return {
    user: new PublicKey(data.slice(offset, offset + 32)),
    channel: new PublicKey(data.slice(offset + 32, offset + 64)),
    amount: data.readBigUInt64LE(offset + 64),
    nftMint: new PublicKey(data.slice(offset + 72, offset + 104)),
    lockDuration: data.readBigUInt64LE(offset + 104),
    boostBps: data.readBigUInt64LE(offset + 112),
    timestamp: data.readBigInt64LE(offset + 120),
  };
}

/**
 * Parse ChannelUnstaked event from transaction logs.
 */
export function parseChannelUnstakedEvent(
  data: Buffer
): ChannelUnstakedEvent | null {
  if (!data.slice(0, 8).equals(CHANNEL_UNSTAKED_DISCRIMINATOR)) return null;

  const offset = 8;
  return {
    user: new PublicKey(data.slice(offset, offset + 32)),
    channel: new PublicKey(data.slice(offset + 32, offset + 64)),
    amount: data.readBigUInt64LE(offset + 64),
    nftMint: new PublicKey(data.slice(offset + 72, offset + 104)),
    timestamp: data.readBigInt64LE(offset + 104),
  };
}

/**
 * Parse ChannelEmergencyUnstaked event from transaction logs.
 */
export function parseChannelEmergencyUnstakedEvent(
  data: Buffer
): ChannelEmergencyUnstakedEvent | null {
  if (!data.slice(0, 8).equals(CHANNEL_EMERGENCY_UNSTAKED_DISCRIMINATOR))
    return null;

  const offset = 8;
  return {
    user: new PublicKey(data.slice(offset, offset + 32)),
    channel: new PublicKey(data.slice(offset + 32, offset + 64)),
    stakedAmount: data.readBigUInt64LE(offset + 64),
    penaltyAmount: data.readBigUInt64LE(offset + 72),
    returnedAmount: data.readBigUInt64LE(offset + 80),
    nftMint: new PublicKey(data.slice(offset + 88, offset + 120)),
    remainingLockSlots: data.readBigUInt64LE(offset + 120),
    timestamp: data.readBigInt64LE(offset + 128),
  };
}

// =============================================================================
// UTILITIES
// =============================================================================

/**
 * Format CCM amount for display (9 decimals).
 */
export function formatCCM(amount: bigint): string {
  const whole = amount / 1_000_000_000n;
  const frac = amount % 1_000_000_000n;
  if (frac === 0n) return whole.toString();
  return `${whole}.${frac.toString().padStart(9, "0").replace(/0+$/, "")}`;
}

/**
 * Parse CCM amount from string (9 decimals).
 */
export function parseCCM(amount: string): bigint {
  const [whole, frac = ""] = amount.split(".");
  const fracPadded = frac.padEnd(9, "0").slice(0, 9);
  return BigInt(whole) * 1_000_000_000n + BigInt(fracPadded);
}

/**
 * Estimate lock end time from slot.
 * @param lockEndSlot - Lock end slot
 * @param currentSlot - Current slot
 * @returns Estimated unlock timestamp (Date)
 */
export function estimateUnlockTime(
  lockEndSlot: bigint,
  currentSlot: bigint
): Date {
  const remainingSlots = lockEndSlot - currentSlot;
  const remainingMs = Number(remainingSlots) * 400; // ~400ms per slot
  return new Date(Date.now() + remainingMs);
}
