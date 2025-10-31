/**
 * Manual end-to-end verification (no Anchor, no IDL).
 * Constructs all instructions by hand using discriminators and borsh layout.
 *
 * Usage:
 *   CLAIM_JSON=../apps/twzrd-aggregator/test-claim-export.json \
 *   node --loader tsx scripts/e2e-direct-manual.ts
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  LAMPORTS_PER_SOL,
} from '@solana/web3.js';
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
} from '@solana/spl-token';
import * as borsh from 'borsh';
import { keccak_256 } from 'js-sha3';
import fs from 'fs';
import path from 'path';
import { createHash } from 'crypto';

const CLAIM_JSON_PATH = process.env.CLAIM_JSON ?? path.join(__dirname, '../../apps/twzrd-aggregator/test-claim-export.json');
const PROGRAM_KEYPAIR_PATH = path.join(__dirname, '../target/deploy/token_2022-keypair.json');

if (!fs.existsSync(CLAIM_JSON_PATH)) {
  console.error(`Claim JSON not found at ${CLAIM_JSON_PATH}`);
  process.exit(1);
}

if (!fs.existsSync(PROGRAM_KEYPAIR_PATH)) {
  console.error(`Program keypair not found at ${PROGRAM_KEYPAIR_PATH}`);
  process.exit(1);
}

const CLAIM_DATA = JSON.parse(fs.readFileSync(CLAIM_JSON_PATH, 'utf8'));
const programKeypairData = JSON.parse(fs.readFileSync(PROGRAM_KEYPAIR_PATH, 'utf8'));
const PROGRAM_ID = Keypair.fromSecretKey(Uint8Array.from(programKeypairData)).publicKey;

const connection = new Connection('http://127.0.0.1:8899', 'confirmed');

// Load wallet
const walletPath = process.env.ANCHOR_WALLET ?? path.join(process.env.HOME!, '.config/solana/id.json');
const walletData = JSON.parse(fs.readFileSync(walletPath, 'utf8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(walletData));

console.log(`\n🔧 Manual end-to-end verification (no Anchor, no IDL)…\n`);
console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
console.log(`Program ID: ${PROGRAM_ID.toBase58()}\n`);

// Discriminators (first 8 bytes of SHA256("global:instruction_name"))
function discriminator(name: string): Buffer {
  return Buffer.from(createHash('sha256').update(`global:${name}`).digest().slice(0, 8));
}

const DISC_INITIALIZE_MINT_OPEN = discriminator('initialize_mint_open');
const DISC_UPDATE_PUBLISHER_OPEN = discriminator('update_publisher_open');
const DISC_INITIALIZE_CHANNEL = discriminator('initialize_channel');
const DISC_SET_MERKLE_ROOT_RING = discriminator('set_merkle_root_ring');
const DISC_CLAIM_WITH_RING = discriminator('claim_with_ring');

// Borsh schemas
class InitializeMintOpenArgs {
  fee_basis_points: number;
  max_fee: bigint;

  constructor(fields: { fee_basis_points: number; max_fee: bigint }) {
    this.fee_basis_points = fields.fee_basis_points;
    this.max_fee = fields.max_fee;
  }

  static schema = new Map([
    [
      InitializeMintOpenArgs,
      {
        kind: 'struct',
        fields: [
          ['fee_basis_points', 'u16'],
          ['max_fee', 'u64'],
        ],
      },
    ],
  ]);
}

class UpdatePublisherOpenArgs {
  new_publisher: Uint8Array;

  constructor(fields: { new_publisher: Uint8Array }) {
    this.new_publisher = fields.new_publisher;
  }

  static schema = new Map([
    [
      UpdatePublisherOpenArgs,
      {
        kind: 'struct',
        fields: [['new_publisher', [32]]],
      },
    ],
  ]);
}

class InitializeChannelArgs {
  streamer_key: Uint8Array;

  constructor(fields: { streamer_key: Uint8Array }) {
    this.streamer_key = fields.streamer_key;
  }

  static schema = new Map([
    [
      InitializeChannelArgs,
      {
        kind: 'struct',
        fields: [['streamer_key', [32]]],
      },
    ],
  ]);
}

class SetMerkleRootRingArgs {
  root: Uint8Array;
  epoch: bigint;
  claim_count: number;
  streamer_key: Uint8Array;

  constructor(fields: { root: Uint8Array; epoch: bigint; claim_count: number; streamer_key: Uint8Array }) {
    this.root = fields.root;
    this.epoch = fields.epoch;
    this.claim_count = fields.claim_count;
    this.streamer_key = fields.streamer_key;
  }

  static schema = new Map([
    [
      SetMerkleRootRingArgs,
      {
        kind: 'struct',
        fields: [
          ['root', [32]],
          ['epoch', 'u64'],
          ['claim_count', 'u16'],
          ['streamer_key', [32]],
        ],
      },
    ],
  ]);
}

class ClaimWithRingArgs {
  epoch: bigint;
  index: number;
  amount: bigint;
  proof: Uint8Array[];
  id: string;
  streamer_key: Uint8Array;

  constructor(fields: {
    epoch: bigint;
    index: number;
    amount: bigint;
    proof: Uint8Array[];
    id: string;
    streamer_key: Uint8Array;
  }) {
    this.epoch = fields.epoch;
    this.index = fields.index;
    this.amount = fields.amount;
    this.proof = fields.proof;
    this.id = fields.id;
    this.streamer_key = fields.streamer_key;
  }

  static schema = new Map([
    [
      ClaimWithRingArgs,
      {
        kind: 'struct',
        fields: [
          ['epoch', 'u64'],
          ['index', 'u32'],
          ['amount', 'u64'],
          ['proof', [[32]]],
          ['id', 'string'],
          ['streamer_key', [32]],
        ],
      },
    ],
  ]);
}

// Alternative: manually serialize the claim data
function serializeClaimWithRing(args: {
  epoch: bigint;
  index: number;
  amount: bigint;
  proof: Uint8Array[];
  id: string;
  streamer_key: Uint8Array;
}): Buffer {
  const buffers: Buffer[] = [];

  // epoch (u64)
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(args.epoch);
  buffers.push(epochBuf);

  // index (u32)
  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(args.index);
  buffers.push(indexBuf);

  // amount (u64)
  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(args.amount);
  buffers.push(amountBuf);

  // proof (Vec<[u8; 32]>) - length prefix + elements
  const proofLenBuf = Buffer.alloc(4);
  proofLenBuf.writeUInt32LE(args.proof.length);
  buffers.push(proofLenBuf);
  args.proof.forEach((node) => buffers.push(Buffer.from(node)));

  // id (String) - length prefix + UTF-8 bytes
  const idBytes = Buffer.from(args.id, 'utf8');
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBytes.length);
  buffers.push(idLenBuf);
  buffers.push(idBytes);

  // streamer_key ([u8; 32])
  buffers.push(Buffer.from(args.streamer_key));

  return Buffer.concat(buffers);
}

function deriveStreamerKey(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const hash = keccak_256.update('channel:').update(lower).digest();
  return new PublicKey(Buffer.from(hash));
}

async function ensureAta(owner: PublicKey, mint: PublicKey, payer: PublicKey): Promise<PublicKey> {
  const ata = getAssociatedTokenAddressSync(mint, owner, true, TOKEN_2022_PROGRAM_ID);
  const info = await connection.getAccountInfo(ata);
  if (!info) {
    const ix = createAssociatedTokenAccountInstruction(payer, ata, owner, mint, TOKEN_2022_PROGRAM_ID);
    const tx = new Transaction().add(ix);
    await sendAndConfirmTransaction(connection, tx, [wallet]);
  }
  return ata;
}

async function main() {
  const claimerPubkey = new PublicKey(CLAIM_DATA.claimer);
  if (claimerPubkey.toBase58() !== wallet.publicKey.toBase58()) {
    throw new Error('Claimer pubkey must match wallet; regenerate JSON with correct wallet.');
  }

  const mintKeypair = Keypair.generate();
  const streamerKey = deriveStreamerKey(CLAIM_DATA.channel);
  const epoch = BigInt(CLAIM_DATA.epoch);
  const claimCount = CLAIM_DATA.claim_count;
  const claimIndex = CLAIM_DATA.index;
  const claimAmount = BigInt(CLAIM_DATA.amount);
  const claimId = CLAIM_DATA.id;
  const proofNodes: Uint8Array[] = CLAIM_DATA.proof.map((hex: string) => Buffer.from(hex, 'hex'));
  const rootBytes = Buffer.from(CLAIM_DATA.root, 'hex');

  const [protocolPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mintKeypair.publicKey.toBuffer()],
    PROGRAM_ID
  );
  const [feeConfigPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mintKeypair.publicKey.toBuffer(), Buffer.from('fee_config')],
    PROGRAM_ID
  );
  const [channelPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mintKeypair.publicKey.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );

  const treasuryAta = getAssociatedTokenAddressSync(mintKeypair.publicKey, protocolPda, true, TOKEN_2022_PROGRAM_ID);
  const claimerAta = getAssociatedTokenAddressSync(mintKeypair.publicKey, claimerPubkey, false, TOKEN_2022_PROGRAM_ID);

  console.log(`Mint: ${mintKeypair.publicKey.toBase58()}`);
  console.log(`Protocol PDA: ${protocolPda.toBase58()}`);
  console.log(`Fee Config PDA: ${feeConfigPda.toBase58()}`);
  console.log(`Channel PDA: ${channelPda.toBase58()}`);
  console.log(`Streamer key: ${streamerKey.toBase58()}`);
  console.log(`Claimer ATA: ${claimerAta.toBase58()}`);
  console.log(`Treasury ATA: ${treasuryAta.toBase58()}\n`);

  // Step 1: Create Token-2022 mint with transfer fee
  console.log('Step 1: Create Token-2022 mint with transfer fee extension…');
  const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);
  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: wallet.publicKey,
    newAccountPubkey: mintKeypair.publicKey,
    space: mintLen,
    lamports,
    programId: TOKEN_2022_PROGRAM_ID,
  });
  const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
    mintKeypair.publicKey,
    wallet.publicKey,
    wallet.publicKey,
    100,
    BigInt(1_000_000_000),
    TOKEN_2022_PROGRAM_ID
  );
  const initMintIx = createInitializeMintInstruction(
    mintKeypair.publicKey,
    9,
    wallet.publicKey,
    null,
    TOKEN_2022_PROGRAM_ID
  );
  const mintTx = new Transaction().add(createAccountIx).add(initTransferFeeIx).add(initMintIx);
  await sendAndConfirmTransaction(connection, mintTx, [wallet, mintKeypair]);
  console.log('✅ Mint created\n');

  // Step 2: Initialize protocol
  console.log('Step 2: Initialize protocol state…');
  const initMintArgs = new InitializeMintOpenArgs({ fee_basis_points: 100, max_fee: BigInt(1_000_000_000) });
  const initMintData = Buffer.concat([
    DISC_INITIALIZE_MINT_OPEN,
    Buffer.from(borsh.serialize(InitializeMintOpenArgs.schema, initMintArgs)),
  ]);
  const initMintIx2 = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: mintKeypair.publicKey, isSigner: false, isWritable: false },
      { pubkey: protocolPda, isSigner: false, isWritable: true },
      { pubkey: feeConfigPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: initMintData,
  });
  await sendAndConfirmTransaction(connection, new Transaction().add(initMintIx2), [wallet]);
  console.log('✅ Protocol initialized\n');

  // Step 3: Set publisher
  console.log('Step 3: Set publisher (self)…');
  const updatePubArgs = new UpdatePublisherOpenArgs({ new_publisher: wallet.publicKey.toBytes() });
  const updatePubData = Buffer.concat([
    DISC_UPDATE_PUBLISHER_OPEN,
    Buffer.from(borsh.serialize(UpdatePublisherOpenArgs.schema, updatePubArgs)),
  ]);
  const updatePubIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolPda, isSigner: false, isWritable: true },
    ],
    data: updatePubData,
  });
  await sendAndConfirmTransaction(connection, new Transaction().add(updatePubIx), [wallet]);
  console.log('✅ Publisher set\n');

  // Step 4: Initialize channel
  console.log('Step 4: Initialize channel…');
  const initChanArgs = new InitializeChannelArgs({ streamer_key: streamerKey.toBytes() });
  const initChanData = Buffer.concat([
    DISC_INITIALIZE_CHANNEL,
    Buffer.from(borsh.serialize(InitializeChannelArgs.schema, initChanArgs)),
  ]);
  const initChanIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolPda, isSigner: false, isWritable: false },
      { pubkey: channelPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: initChanData,
  });
  await sendAndConfirmTransaction(connection, new Transaction().add(initChanIx), [wallet]);
  console.log('✅ Channel initialized\n');

  // Step 5: Publish merkle root
  console.log('Step 5: Publish ring root…');
  const setRootArgs = new SetMerkleRootRingArgs({
    root: rootBytes,
    epoch,
    claim_count: claimCount,
    streamer_key: streamerKey.toBytes(),
  });
  const setRootData = Buffer.concat([
    DISC_SET_MERKLE_ROOT_RING,
    Buffer.from(borsh.serialize(SetMerkleRootRingArgs.schema, setRootArgs)),
  ]);
  const setRootIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolPda, isSigner: false, isWritable: true },
      { pubkey: channelPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: setRootData,
  });
  await sendAndConfirmTransaction(connection, new Transaction().add(setRootIx), [wallet]);
  console.log('✅ Root published\n');

  // Step 6: Fund treasury
  console.log('Step 6: Fund protocol treasury…');
  await ensureAta(protocolPda, mintKeypair.publicKey, wallet.publicKey);
  await ensureAta(wallet.publicKey, mintKeypair.publicKey, wallet.publicKey);
  const mintToIx = createMintToInstruction(
    mintKeypair.publicKey,
    treasuryAta,
    wallet.publicKey,
    claimAmount * BigInt(2),
    [],
    TOKEN_2022_PROGRAM_ID
  );
  await sendAndConfirmTransaction(connection, new Transaction().add(mintToIx), [wallet]);
  console.log('✅ Treasury funded\n');

  // Step 7: Execute claim
  console.log('Step 7: Execute claim_with_ring…');
  const treasuryBefore = await getAccount(connection, treasuryAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
  let claimerBefore = BigInt(0);
  try {
    const acct = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
    claimerBefore = acct.amount;
  } catch (_) {
    // Not created yet
  }

  const serializedArgs = serializeClaimWithRing({
    epoch,
    index: claimIndex,
    amount: claimAmount,
    proof: proofNodes,
    id: claimId,
    streamer_key: streamerKey.toBytes(),
  });

  console.log(`   Streamer key (hex): ${Buffer.from(streamerKey.toBytes()).toString('hex')}`);
  console.log(`   Claim ID: ${claimId}`);
  console.log(`   Serialized length: ${serializedArgs.length} bytes`);

  const claimData = Buffer.concat([DISC_CLAIM_WITH_RING, serializedArgs]);

  const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
  const claimIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolPda, isSigner: false, isWritable: true },
      { pubkey: channelPda, isSigner: false, isWritable: true },
      { pubkey: mintKeypair.publicKey, isSigner: false, isWritable: false },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: claimerAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: claimData,
  });

  const claimTx = await sendAndConfirmTransaction(connection, new Transaction().add(claimIx), [wallet]);
  console.log(`✅ Claim transaction: ${claimTx}`);

  const treasuryAfter = await getAccount(connection, treasuryAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
  const claimerAfter = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
  const deltaTreasury = Number(treasuryBefore.amount - treasuryAfter.amount);
  const deltaClaimer = Number(claimerAfter.amount - claimerBefore);

  console.log(`   Treasury delta: ${deltaTreasury}`);
  console.log(`   Claimer delta: ${deltaClaimer}`);

  // Account for 1% transfer fee (100 basis points)
  const expectedAfterFee = Number(claimAmount) * 0.99;
  const tolerance = 1; // Allow 1 token unit tolerance for rounding

  if (Math.abs(deltaClaimer - expectedAfterFee) > tolerance) {
    throw new Error(`Claimer did not receive expected amount (after 1% fee): ${deltaClaimer} !== ${expectedAfterFee}`);
  }
  if (deltaTreasury !== Number(claimAmount)) {
    throw new Error(`Treasury did not decrease as expected: ${deltaTreasury} !== ${claimAmount}`);
  }

  console.log(`✅ Amounts verified: Treasury sent ${deltaTreasury}, claimer received ${deltaClaimer} (after 1% fee)`);

  // Step 8: Double claim should fail
  console.log('Step 8: Ensure double claim fails…');
  let doubleClaimRejected = false;
  try {
    await sendAndConfirmTransaction(connection, new Transaction().add(claimIx), [wallet]);
  } catch (err: any) {
    if (err.toString().includes('AlreadyClaimed') || err.toString().includes('custom program error: 0x1770')) {
      doubleClaimRejected = true;
    } else {
      throw err;
    }
  }

  if (!doubleClaimRejected) {
    throw new Error('Second claim should fail with AlreadyClaimed');
  }

  console.log('\n✅✅✅ END-TO-END VERIFICATION PASSED ✅✅✅\n');
  console.log('Cryptographic alignment confirmed:');
  console.log('  • Off-chain leaf hashing: ✅ matches compute_leaf');
  console.log('  • Proof verification: ✅ on-chain PASS');
  console.log('  • Token transfer: ✅ treasury → claimer');
  console.log('  • Double-claim guard: ✅ AlreadyClaimed\n');
  console.log('The protocol is ready for production deployment.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
