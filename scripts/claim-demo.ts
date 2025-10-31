/**
 * Minimal CLS claim demo – shows the live claim flow
 *
 * Usage:
 *   export CLAIM_JSON=../path/to/claim-export.json
 *   export RPC_URL=https://api.devnet.solana.com (or localhost for local)
 *   tsx scripts/claim-demo.ts
 *
 * What it does:
 * 1. Loads proof from CLAIM_JSON
 * 2. Fetches current wallet balance
 * 3. Constructs claim_with_ring instruction manually (no Anchor)
 * 4. Signs and submits to blockchain
 * 5. Waits for confirmation
 * 6. Fetches new balance and displays delta
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
import { getAssociatedTokenAddressSync, TOKEN_2022_PROGRAM_ID, getAccount } from '@solana/spl-token';
import * as borsh from 'borsh';
import { keccak_256 } from 'js-sha3';
import fs from 'fs';
import path from 'path';
import { createHash } from 'crypto';

const CLAIM_JSON_PATH = process.env.CLAIM_JSON ?? path.join(__dirname, '../../apps/twzrd-aggregator/test-claim-export.json');
const PROGRAM_KEYPAIR_PATH = path.join(__dirname, '../target/deploy/token_2022-keypair.json');
const RPC_URL = process.env.RPC_URL ?? 'http://127.0.0.1:8899';

if (!fs.existsSync(CLAIM_JSON_PATH)) {
  console.error(`\n❌ Claim JSON not found at ${CLAIM_JSON_PATH}`);
  console.error(`   Set CLAIM_JSON env or ensure test-claim-export.json exists\n`);
  process.exit(1);
}

if (!fs.existsSync(PROGRAM_KEYPAIR_PATH)) {
  console.error(`\n❌ Program keypair not found at ${PROGRAM_KEYPAIR_PATH}`);
  console.error(`   Make sure you've built and deployed the program\n`);
  process.exit(1);
}

const CLAIM_DATA = JSON.parse(fs.readFileSync(CLAIM_JSON_PATH, 'utf8'));
const programKeypairData = JSON.parse(fs.readFileSync(PROGRAM_KEYPAIR_PATH, 'utf8'));
const PROGRAM_ID = Keypair.fromSecretKey(Uint8Array.from(programKeypairData)).publicKey;

const connection = new Connection(RPC_URL, 'confirmed');
const walletPath = process.env.ANCHOR_WALLET ?? path.join(process.env.HOME!, '.config/solana/id.json');
const walletData = JSON.parse(fs.readFileSync(walletPath, 'utf8'));
const wallet = Keypair.fromSecretKey(Uint8Array.from(walletData));

function discriminator(name: string): Buffer {
  return Buffer.from(createHash('sha256').update(`global:${name}`).digest().slice(0, 8));
}

function deriveStreamerKey(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const hash = keccak_256.update('channel:').update(lower).digest();
  return new PublicKey(Buffer.from(hash));
}

function serializeClaimWithRing(args: {
  epoch: bigint;
  index: number;
  amount: bigint;
  proof: Uint8Array[];
  id: string;
  streamer_key: Uint8Array;
}): Buffer {
  const buffers: Buffer[] = [];

  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(args.epoch);
  buffers.push(epochBuf);

  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(args.index);
  buffers.push(indexBuf);

  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(args.amount);
  buffers.push(amountBuf);

  const proofLenBuf = Buffer.alloc(4);
  proofLenBuf.writeUInt32LE(args.proof.length);
  buffers.push(proofLenBuf);
  args.proof.forEach((node) => buffers.push(Buffer.from(node)));

  const idBytes = Buffer.from(args.id, 'utf8');
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBytes.length);
  buffers.push(idLenBuf);
  buffers.push(idBytes);

  buffers.push(Buffer.from(args.streamer_key));

  return Buffer.concat(buffers);
}

async function main() {
  console.log('\n🎬 CLS Claim Demo\n');
  console.log(`Wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`RPC: ${RPC_URL}`);
  console.log(`Program: ${PROGRAM_ID.toBase58()}\n`);

  const claimerPubkey = new PublicKey(CLAIM_DATA.claimer);
  if (claimerPubkey.toBase58() !== wallet.publicKey.toBase58()) {
    console.error(`❌ Claim is for ${claimerPubkey.toBase58()}`);
    console.error(`   But wallet is ${wallet.publicKey.toBase58()}`);
    console.error(`   Regenerate JSON with correct wallet\n`);
    process.exit(1);
  }

  const mintPubkey = new PublicKey(CLAIM_DATA.mint); // Assuming claim data includes mint
  const streamerKey = deriveStreamerKey(CLAIM_DATA.channel);
  const epoch = BigInt(CLAIM_DATA.epoch);
  const claimIndex = CLAIM_DATA.index;
  const claimAmount = BigInt(CLAIM_DATA.amount);
  const claimId = CLAIM_DATA.id;
  const proofNodes: Uint8Array[] = CLAIM_DATA.proof.map((hex: string) => Buffer.from(hex, 'hex'));

  const [protocolPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mintPubkey.toBuffer()],
    PROGRAM_ID
  );

  const [channelPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mintPubkey.toBuffer(), streamerKey.toBuffer()],
    PROGRAM_ID
  );

  const treasuryAta = getAssociatedTokenAddressSync(mintPubkey, protocolPda, true, TOKEN_2022_PROGRAM_ID);
  const claimerAta = getAssociatedTokenAddressSync(mintPubkey, claimerPubkey, false, TOKEN_2022_PROGRAM_ID);

  console.log(`📋 Claim Details:`);
  console.log(`   Channel: ${CLAIM_DATA.channel}`);
  console.log(`   Epoch: ${epoch}`);
  console.log(`   Amount: ${claimAmount}`);
  console.log(`   ID: ${claimId}\n`);

  // Fetch balance before
  console.log(`💰 Checking balance…`);
  let balanceBefore = BigInt(0);
  try {
    const acct = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
    balanceBefore = acct.amount;
    console.log(`   Before: ${balanceBefore}`);
  } catch (_) {
    console.log(`   Before: 0 (ATA not created yet)`);
  }

  // Construct instruction
  console.log(`\n🔨 Constructing claim_with_ring instruction…`);
  const serializedArgs = serializeClaimWithRing({
    epoch,
    index: claimIndex,
    amount: claimAmount,
    proof: proofNodes,
    id: claimId,
    streamer_key: streamerKey.toBytes(),
  });

  const DISC_CLAIM_WITH_RING = discriminator('claim_with_ring');
  const claimData = Buffer.concat([DISC_CLAIM_WITH_RING, serializedArgs]);

  const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
  const claimIx = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolPda, isSigner: false, isWritable: true },
      { pubkey: channelPda, isSigner: false, isWritable: true },
      { pubkey: mintPubkey, isSigner: false, isWritable: false },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: claimerAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data: claimData,
  });

  console.log(`   ✓ Instruction ready`);
  console.log(`   Proof nodes: ${proofNodes.length}`);
  console.log(`   Serialized length: ${serializedArgs.length} bytes\n`);

  // Submit transaction
  console.log(`🚀 Submitting claim transaction…`);
  try {
    const claimTx = await sendAndConfirmTransaction(connection, new Transaction().add(claimIx), [wallet]);
    console.log(`   ✓ Signature: ${claimTx}\n`);

    // Fetch balance after
    console.log(`💰 Checking balance after claim…`);
    const acctAfter = await getAccount(connection, claimerAta, 'confirmed', TOKEN_2022_PROGRAM_ID);
    const balanceAfter = acctAfter.amount;
    const delta = balanceAfter - balanceBefore;

    console.log(`   After: ${balanceAfter}`);
    console.log(`   Delta: +${delta}\n`);

    console.log(`✅ Claim successful!\n`);
    console.log(`📊 Summary:`);
    console.log(`   • Received: ${delta} tokens (after transfer fee)`);
    console.log(`   • Wallet: ${wallet.publicKey.toBase58()}`);
    console.log(`   • Proof verified on-chain`);
    console.log(`   • Claim locked to prevent double-spending\n`);
  } catch (err: any) {
    if (err.toString().includes('AlreadyClaimed')) {
      console.error(`\n❌ Claim rejected: Already claimed this epoch`);
      console.error(`   Double-claim guard is working correctly\n`);
    } else {
      console.error(`\n❌ Error:`, err.message, '\n');
    }
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
