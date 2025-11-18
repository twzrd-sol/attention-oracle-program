import { Connection, PublicKey, Keypair, Transaction, TransactionInstruction } from '@solana/web3.js';
import { Program, AnchorProvider, Idl } from '@coral-xyz/anchor';
import { MerkleProof, PassportTier } from './types';

/**
 * Main client for interacting with Attention Oracle program
 */
export class AttentionOracleClient {
  readonly program: Program;
  readonly connection: Connection;
  readonly programId: PublicKey;

  constructor(
    connection: Connection,
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop'),
    idl?: Idl
  ) {
    this.connection = connection;
    this.programId = programId;

    // If IDL provided, create program instance
    if (idl) {
      const provider = new AnchorProvider(connection, {} as any, {});
      this.program = new Program(idl, provider);
    }
  }

  /**
   * Derive PDA for a passport account
   */
  static derivePassportPda(
    user: PublicKey,
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from('passport'), user.toBuffer()],
      programId
    );
  }

  /**
   * Derive PDA for a channel account
   */
  static deriveChannelPda(
    channelId: string,
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from('channel'), Buffer.from(channelId)],
      programId
    );
  }

  /**
   * Derive PDA for a ring buffer epoch
   */
  static deriveEpochPda(
    channel: PublicKey,
    epochIndex: number,
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): [PublicKey, number] {
    const epochBytes = Buffer.alloc(4);
    epochBytes.writeUInt32LE(epochIndex);

    return PublicKey.findProgramAddressSync(
      [Buffer.from('epoch'), channel.toBuffer(), epochBytes],
      programId
    );
  }

  /**
   * Derive treasury PDA
   */
  static deriveTreasuryPda(
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from('treasury')],
      programId
    );
  }

  /**
   * Derive creator pool PDA
   */
  static deriveCreatorPoolPda(
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from('creator_pool')],
      programId
    );
  }

  /**
   * Fetch passport account data
   */
  async getPassport(user: PublicKey): Promise<{
    tier: PassportTier;
    points: number;
    lastUpdate: number;
  } | null> {
    const [passportPda] = AttentionOracleClient.derivePassportPda(user, this.programId);

    try {
      const accountInfo = await this.connection.getAccountInfo(passportPda);
      if (!accountInfo) return null;

      // Parse account data (simplified - use anchor deserialization in real impl)
      const data = accountInfo.data;
      return {
        tier: data[8] as PassportTier, // Assuming discriminator is 8 bytes
        points: data.readBigUInt64LE(9).valueOf() as number,
        lastUpdate: data.readBigUInt64LE(17).valueOf() as number,
      };
    } catch (e) {
      return null;
    }
  }

  /**
   * Fetch channel info
   */
  async getChannel(channelId: string): Promise<{
    authority: PublicKey;
    active: boolean;
    totalDistributed: bigint;
  } | null> {
    const [channelPda] = AttentionOracleClient.deriveChannelPda(channelId, this.programId);

    try {
      const accountInfo = await this.connection.getAccountInfo(channelPda);
      if (!accountInfo) return null;

      const data = accountInfo.data;
      return {
        authority: new PublicKey(data.slice(8, 40)),
        active: data[40] === 1,
        totalDistributed: data.readBigUInt64LE(41),
      };
    } catch (e) {
      return null;
    }
  }

  /**
   * Check if user has claimed from a specific epoch
   */
  async hasUserClaimed(
    user: PublicKey,
    channelId: string,
    epochIndex: number
  ): Promise<boolean> {
    const [channelPda] = AttentionOracleClient.deriveChannelPda(channelId, this.programId);
    const [epochPda] = AttentionOracleClient.deriveEpochPda(channelPda, epochIndex, this.programId);

    try {
      const accountInfo = await this.connection.getAccountInfo(epochPda);
      if (!accountInfo) return false;

      // Parse bitmap to check if user's index is claimed
      // (Simplified - real impl would check specific bit)
      return true; // Placeholder
    } catch (e) {
      return false;
    }
  }
}

/**
 * Builder for claim instructions
 */
export class ClaimBuilder {
  private instructions: TransactionInstruction[] = [];

  /**
   * Add a merkle claim instruction
   */
  addClaim(
    user: PublicKey,
    channelId: string,
    proof: MerkleProof,
    programId: PublicKey = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop')
  ): ClaimBuilder {
    // Build instruction (simplified - use anchor in real impl)
    const [passportPda] = AttentionOracleClient.derivePassportPda(user, programId);
    const [channelPda] = AttentionOracleClient.deriveChannelPda(channelId, programId);
    const [epochPda] = AttentionOracleClient.deriveEpochPda(
      channelPda,
      proof.epochIndex,
      programId
    );

    // Placeholder instruction
    const ix = new TransactionInstruction({
      keys: [
        { pubkey: user, isSigner: true, isWritable: true },
        { pubkey: passportPda, isSigner: false, isWritable: true },
        { pubkey: channelPda, isSigner: false, isWritable: false },
        { pubkey: epochPda, isSigner: false, isWritable: true },
      ],
      programId,
      data: Buffer.from([]), // Encode proof data here
    });

    this.instructions.push(ix);
    return this;
  }

  /**
   * Build final transaction
   */
  build(): Transaction {
    const tx = new Transaction();
    this.instructions.forEach(ix => tx.add(ix));
    return tx;
  }
}
