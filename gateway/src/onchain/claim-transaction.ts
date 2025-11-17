/**
 * On-chain Transaction Building
 *
 * Builds a CLS claim transaction for the Token‑2022 program.
 *
 * Supports two modes:
 * 1. Simple (Claim #0001 style):
 *    - Fixed index, amount, id from env vars
 *    - Empty proof (single-leaf tree)
 *    - Best for simple fixed-allocation drops
 *
 * 2. Generalized (Multi-wallet epochs):
 *    - Per-wallet index, amount, proof from request
 *    - Full Merkle tree support
 *    - Best for proportional allocations from engagement data
 *
 * In both cases, the corresponding `EpochState` PDA must be initialized
 * on-chain with a Merkle root consistent with:
 *   leaf = keccak256(claimer || index || amount || id)
 */

import {
  Connection,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from '@solana/web3.js';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from '@solana/spl-token';
import { keccak_256 } from '@noble/hashes/sha3.js';
import crypto from 'crypto';

// Configuration
const PROGRAM_ID = new PublicKey(process.env.PROGRAM_ID!);
const RPC_URL = process.env.SOLANA_RPC || 'https://api.mainnet-beta.solana.com';
const connection = new Connection(RPC_URL, 'confirmed');

const MINT_PUBKEY = process.env.MINT_PUBKEY;
if (!MINT_PUBKEY) {
  throw new Error('MINT_PUBKEY env var is required for CLS claims');
}
const MINT = new PublicKey(MINT_PUBKEY);

const PROTOCOL_SEED = Buffer.from('protocol');
const CHANNEL_STATE_SEED = Buffer.from('channel_state');

/**
 * Build claim transaction (Extended 13-account variant)
 *
 * Includes full account set for sybil-resistant tier-based claims:
 * - Fee config for dynamic fee calculation
 * - Channel state for ring buffer epoch management
 * - Passport state for tier verification
 * - Creator pool for fee distribution
 *
 * @param args.wallet - Claimer wallet pubkey
 * @param args.epochId - Epoch to claim for
 * @param args.merkleRoot - Merkle root (from epochs table)
 * @param args.index - Claimer index in Merkle tree
 * @param args.amount - Claim amount in raw tokens (BigInt)
 * @param args.id - Leaf identifier string (must match off-chain tree)
 * @param args.proof - Merkle proof as array of hex strings
 * @param args.creatorPoolAta - (Optional) Creator fee pool ATA for fee distribution
 * @param args.passportState - (Optional) User's passport state for tier lookup
 * @param args.channelState - (Optional) Channel state for ring buffer management
 * @returns Unsigned transaction ready for client signing (13 accounts)
 */
export async function buildClaimTransaction(args: {
  wallet: PublicKey;
  epochId: number;
  merkleRoot: string;
  index: number;
  amount: bigint;
  id: string;
  proof: string[];
  creatorPoolAta?: PublicKey;
  passportState?: PublicKey;
  channelState?: PublicKey;
}): Promise<Transaction> {
  const { wallet, epochId, index, amount, id, proof, creatorPoolAta, passportState, channelState } = args;

  try {
    // Streamer namespace for CLS epochs (must match off-chain epoch init)
    const streamerName = (process.env.CLS_STREAMER_NAME || 'claim-0001-test').toLowerCase();

    const idBuf = Buffer.from(id, 'utf8');
    if (idBuf.length > 32) {
      throw new Error(
        `CLS claim id too long (max 32 bytes, got ${idBuf.length}). ` +
          `Shorten CLS_CLAIM_ID_PREFIX or override per-epoch logic.`,
      );
    }

    // Derive PDAs
    const [protocolState] = PublicKey.findProgramAddressSync(
      [PROTOCOL_SEED, MINT.toBuffer()],
      PROGRAM_ID,
    );

    // Derive fee_config PDA (new: required for 13-account variant)
    const [feeConfig] = PublicKey.findProgramAddressSync(
      [PROTOCOL_SEED, MINT.toBuffer(), Buffer.from('fee_config')],
      PROGRAM_ID,
    );

    const epochBuf = Buffer.alloc(8);
    epochBuf.writeBigUInt64LE(BigInt(epochId), 0);

    // Streamer key (keccak256("twitch:" || channel_lower))
    const streamerHash = keccak_256(
      Buffer.concat([Buffer.from('twitch:'), Buffer.from(streamerName, 'utf8')]),
    );
    const streamerKey = new PublicKey(streamerHash);

    // Derive channel_state PDA (ring buffer for this channel) - provided or derived
    const derivedChannelState = PublicKey.findProgramAddressSync(
      [CHANNEL_STATE_SEED, MINT.toBuffer(), streamerKey.toBuffer()],
      PROGRAM_ID,
    )[0];

    const effectiveChannelState = channelState || derivedChannelState;

    // ATAs (Token-2022)
    const treasuryAta = getAssociatedTokenAddressSync(
      MINT,
      protocolState,
      true,
      TOKEN_2022_PROGRAM_ID,
    );
    const claimerAta = getAssociatedTokenAddressSync(
      MINT,
      wallet,
      false,
      TOKEN_2022_PROGRAM_ID,
    );

    // Creator pool ATA (optional, for fee distribution)
    const effectiveCreatorPoolAta = creatorPoolAta || treasuryAta;

    // Derive epoch_state PDA (for claim_open variant)
    const [epochState] = PublicKey.findProgramAddressSync(
      [
        Buffer.from('epoch_state'),
        epochBuf,
        streamerKey.toBuffer(),
        MINT.toBuffer(),
      ],
      PROGRAM_ID,
    );

    // Instruction data for claim_open (extended 13-account variant)
    // discriminator = sha256("global:claim_open")[0..8]
    const disc = crypto
      .createHash('sha256')
      .update('global:claim_open')
      .digest()
      .subarray(0, 8);

    // Encode instruction args for claim_open
    // Args: index (u32), amount (u64), id (String), proof (Vec<[u8; 32]>)
    const epochArgBuf = Buffer.alloc(8);
    epochArgBuf.writeBigUInt64LE(BigInt(epochId), 0);

    const indexBuf = Buffer.alloc(4);
    indexBuf.writeUInt32LE(index, 0);

    const amountBuf = Buffer.alloc(8);
    amountBuf.writeBigUInt64LE(amount, 0);

    // Encode Merkle proof as Vec<[u8; 32]>
    // Format: length (u32) + elements (each 32 bytes)
    const proofLenBuf = Buffer.alloc(4);
    proofLenBuf.writeUInt32LE(proof.length, 0);

    // Convert hex-encoded proof strings to buffers
    const proofBufs: Buffer[] = [];
    for (const proofElement of proof) {
      const hex = proofElement.startsWith('0x') || proofElement.startsWith('0X')
        ? proofElement.slice(2)
        : proofElement;
      if (!/^[0-9a-f]{64}$/i.test(hex)) {
        throw new Error(
          `Invalid proof element: must be 64-char hex string (32 bytes), got "${proofElement}"`
        );
      }
      proofBufs.push(Buffer.from(hex, 'hex'));
    }

    // Optional local verification: ensure leaf+proof match DB root
    // NOTE: claim_with_ring does NOT use the id field in leaf hash (simpler verification)
    // SKIP VERIFICATION for test merkle roots (prefixed with "test_")
    const isTestRoot = args.merkleRoot && args.merkleRoot.startsWith('test_');

    if (!isTestRoot && (proof.length > 0 || args.merkleRoot)) {
      const leafIdxBuf = Buffer.alloc(4);
      leafIdxBuf.writeUInt32LE(index, 0);
      const leafAmtBuf = Buffer.alloc(8);
      leafAmtBuf.writeBigUInt64LE(amount, 0);
      // Leaf hash: keccak256(wallet || index || amount) - NO ID FIELD
      const leafPreimage = Buffer.concat([wallet.toBuffer(), leafIdxBuf, leafAmtBuf]);
      const leaf = keccak_256(leafPreimage);

      const rootClean =
        args.merkleRoot && (args.merkleRoot.startsWith('0x') || args.merkleRoot.startsWith('0X'))
          ? args.merkleRoot.slice(2)
          : args.merkleRoot;
      const rootBuf = Buffer.from(rootClean, 'hex');

      let hash = Buffer.from(leaf);
      for (const node of proofBufs) {
        const [a, b] = Buffer.compare(hash, node) <= 0 ? [hash, node] : [node, hash];
        hash = Buffer.from(keccak_256(Buffer.concat([a, b])));
      }

      if (!hash.equals(rootBuf)) {
        throw new Error('Provided proof does not match epoch merkle root');
      }
    } else if (isTestRoot) {
      console.log('[buildClaimTransaction] SKIPPING verification for test merkle root');
    }

    // Build instruction data for claim_with_ring
    // Format: discriminator + epoch + index + amount + proof_len + proof_elements + streamer_key
    const data = Buffer.concat([
      disc,
      epochArgBuf,
      indexBuf,
      amountBuf,
      proofLenBuf,
      ...proofBufs, // Concatenate all proof elements
      streamerKey.toBuffer(), // streamer_key arg (Pubkey = 32 bytes)
    ]);

    // Build instruction matching claim_open (extended 13-account variant)
    // Core accounts:
    // 1. claimer (mut, signer)
    // 2. protocol_state (mut)
    // 3. epoch_state (mut)
    // 4. mint
    // 5. treasury_ata (mut)
    // 6. claimer_ata (mut)
    // 7. token_program (Token-2022)
    // 8. associated_token_program
    // 9. system_program
    // Extended accounts (for tier/sybil resistance):
    // 10. fee_config (PDA)
    // 11. channel_state (ring buffer)
    // 12. passport_state (tier verification)
    // 13. creator_pool_ata (fee distribution)
    const ix = new TransactionInstruction({
      programId: PROGRAM_ID,
      keys: [
        { pubkey: wallet, isSigner: true, isWritable: true },                          // 1. claimer
        { pubkey: protocolState, isSigner: false, isWritable: true },                  // 2. protocol_state
        { pubkey: epochState, isSigner: false, isWritable: true },                     // 3. epoch_state (derived)
        { pubkey: MINT, isSigner: false, isWritable: false },                          // 4. mint
        { pubkey: treasuryAta, isSigner: false, isWritable: true },                    // 5. treasury_ata
        { pubkey: claimerAta, isSigner: false, isWritable: true },                     // 6. claimer_ata
        { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },         // 7. token_program
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },   // 8. associated_token_program
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },       // 9. system_program
        { pubkey: feeConfig, isSigner: false, isWritable: false },                     // 10. fee_config (PDA)
        { pubkey: effectiveChannelState, isSigner: false, isWritable: false },         // 11. channel_state (ring buffer)
        { pubkey: passportState || wallet, isSigner: false, isWritable: false },       // 12. passport_state (fallback: wallet)
        { pubkey: effectiveCreatorPoolAta, isSigner: false, isWritable: true },        // 13. creator_pool_ata
      ],
      data,
    });

    // Create transaction
    const tx = new Transaction();
    tx.add(ix);

    // Set fee payer if configured
    const feePayerKey = process.env.FEEPAYER_PUBKEY;
    if (feePayerKey) {
      tx.feePayer = new PublicKey(feePayerKey);
    } else {
      tx.feePayer = wallet;
    }

    // Get recent blockhash
    const { blockhash } = await connection.getLatestBlockhash('finalized');
    tx.recentBlockhash = blockhash;

    return tx;
  } catch (error) {
    console.error('[buildClaimTransaction] Error:', error);
    throw new Error(
      `Failed to build claim transaction: ${error instanceof Error ? error.message : 'Unknown error'}`
    );
  }
}

/**
 * Derive Protocol PDA
 *
 * Derives the protocol state PDA from mint address
 */
export function deriveProtocolPda(mint: PublicKey): [PublicKey, number] {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBuffer()],
    PROGRAM_ID
  );
  return [pda, bump];
}

/**
 * Derive Channel State PDA
 *
 * Derives the channel state PDA from mint and streamer key
 */
export function deriveChannelStatePda(
  mint: PublicKey,
  streamerKey: PublicKey
): [PublicKey, number] {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mint.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );
  return [pda, bump];
}
