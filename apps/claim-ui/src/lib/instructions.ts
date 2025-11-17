import { AnchorProvider, Program } from '@coral-xyz/anchor';
import { PublicKey, Transaction } from '@solana/web3.js';
import { getAssociatedTokenAddressSync } from '@solana/spl-token';
import { PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, PROTOCOL_SEED, CHANNEL_STATE_SEED } from './constants';
import type { MerkleProof } from '../hooks/useMerkleProof';

/**
 * Helper to derive protocol PDA
 */
export const deriveProtocolPDA = (mint: PublicKey): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(PROTOCOL_SEED), mint.toBuffer()],
    PROGRAM_ID
  );
};

/**
 * Helper to derive channel state PDA
 */
export const deriveChannelStatePDA = (mint: PublicKey, streamerKey: PublicKey): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(CHANNEL_STATE_SEED), mint.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );
};

/**
 * Helper to derive streamer key from channel name (keccak256 hash)
 */
export const deriveStreamerKey = (channel: string): PublicKey => {
  // Note: This is a simplified version. The actual implementation
  // uses keccak256 hashing. For production, match the on-chain implementation.
  const hash = Buffer.alloc(32);
  const channelStr = `channel:${channel.toLowerCase()}`;
  // Placeholder - use actual keccak256 in production
  hash.write(channelStr);
  return new PublicKey(hash);
};

/**
 * Build claim_with_ring instruction
 */
export const buildClaimWithRingInstruction = async (
  program: Program,
  proof: MerkleProof,
  claimer: PublicKey,
  streamerKey: PublicKey
) => {
  const mint = new PublicKey(proof.mint);
  const [protocolPda] = deriveProtocolPDA(mint);
  const [channelPda] = deriveChannelStatePDA(mint, streamerKey);

  const treasuryAta = getAssociatedTokenAddressSync(
    mint,
    protocolPda,
    true,
    TOKEN_2022_PROGRAM_ID
  );

  const claimerAta = getAssociatedTokenAddressSync(
    mint,
    claimer,
    false,
    TOKEN_2022_PROGRAM_ID
  );

  // Convert proof hex strings to buffers
  const proofNodes = proof.proof.map(hex =>
    Buffer.from(hex.replace('0x', ''), 'hex')
  );

  try {
    const tx = await program.methods
      .claimWithRing(
        new (BigInt as any)(proof.epoch),
        proof.index,
        new (BigInt as any)(proof.amount),
        proofNodes,
        streamerKey
      )
      .accounts({
        claimer,
        protocolState: protocolPda,
        channelState: channelPda,
        mint,
        treasuryAta,
        claimerAta,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: PublicKey.default,
      })
      .transaction();

    return tx;
  } catch (err) {
    throw new Error(`Failed to build claim instruction: ${err instanceof Error ? err.message : String(err)}`);
  }
};

/**
 * Submit claim transaction
 */
export const submitClaimTransaction = async (
  provider: AnchorProvider,
  transaction: Transaction
): Promise<string> => {
  try {
    const tx = await provider.sendAndConfirm(transaction);
    return tx;
  } catch (err) {
    throw new Error(`Failed to submit transaction: ${err instanceof Error ? err.message : String(err)}`);
  }
};

/**
 * Fetch balance for a token account
 */
export const fetchBalance = async (
  provider: AnchorProvider,
  tokenAccount: PublicKey
): Promise<bigint> => {
  try {
    // Placeholder - implement actual token account query
    return BigInt(0);
  } catch (err) {
    console.error('Failed to fetch balance:', err);
    return BigInt(0);
  }
};

/**
 * Calculate fees from transfer amount
 */
export const calculateFees = (
  amount: bigint,
  treasuryFeeBps: number = 5,
  creatorFeeBps: number = 5,
  tierMultiplier: number = 100
) => {
  const treasuryFee = (amount * BigInt(treasuryFeeBps)) / BigInt(10000);
  const creatorFee = (amount * BigInt(creatorFeeBps)) * BigInt(tierMultiplier) / BigInt(1000000);
  const totalFee = treasuryFee + creatorFee;
  const netAmount = amount - totalFee;

  return {
    gross: amount,
    treasuryFee,
    creatorFee,
    totalFee,
    net: netAmount,
  };
};
