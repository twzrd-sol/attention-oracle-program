import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import { TOKEN_2022_PROGRAM_ID as TOKEN_2022_FROM_SPL, getAssociatedTokenAddress } from '@solana/spl-token';
import * as fs from 'fs';
import * as crypto from 'crypto';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const RPC_URL = 'https://api.mainnet-beta.solana.com';
// Use the TOKEN_2022_PROGRAM_ID from @solana/spl-token (imported above)
// Note: TOKEN_2022_FROM_SPL = TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb (correct)
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

const PROTOCOL_SEED = Buffer.from('protocol');
const EPOCH_STATE_SEED = Buffer.from('epoch_state');

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');

  // Load claimer keypair
  const claimerData = JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8'));
  const claimer = Keypair.fromSecretKey(Uint8Array.from(claimerData));

  console.log('🔐 Claimer:', claimer.publicKey.toBase58());
  console.log('📍 Claiming 100 CCM for epoch 424243\n');

  // Derive PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  const epoch = 424243;
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(epoch), 0);

  const { keccak_256 } = require('@noble/hashes/sha3');
  const channel = 'claim-0001-test';
  const channelBytes = Buffer.from(channel.toLowerCase());
  const streamerKeyHash = keccak_256(Buffer.concat([Buffer.from('twitch:'), channelBytes]));
  const streamerKey = new PublicKey(Buffer.from(streamerKeyHash));

  const [epochState] = PublicKey.findProgramAddressSync(
    [EPOCH_STATE_SEED, epochBuf, streamerKey.toBuffer(), MINT.toBuffer()],
    PROGRAM_ID
  );

  // Derive ATAs using the authoritative @solana/spl-token method
  // (This matches how Anchor's associated_token macro derives them)
  const treasuryAta = await getAssociatedTokenAddress(
    MINT,
    protocolState,
    true, // allowOwnerOffCurve - required for PDAs
    TOKEN_2022_FROM_SPL,
  );

  const claimerAta = await getAssociatedTokenAddress(
    MINT,
    claimer.publicKey,
    false, // allowOwnerOffCurve - not needed for user accounts
    TOKEN_2022_FROM_SPL,
  );

  console.log('Derived Accounts:');
  console.log('  Protocol State:', protocolState.toBase58());
  console.log('  Epoch State:', epochState.toBase58());
  console.log('  Claimer ATA:', claimerAta.toBase58());
  console.log('  Treasury ATA:', treasuryAta.toBase58());
  console.log();

  // Build claim_open instruction
  const hash = crypto.createHash('sha256').update('global:claim_open').digest();
  const discriminator = hash.slice(0, 8);

  const index = 0;
  const amount = BigInt('100000000000');
  const id = 'claim-0001';

  const indexBuf = Buffer.alloc(4);
  indexBuf.writeUInt32LE(index, 0);

  const amountBuf = Buffer.alloc(8);
  amountBuf.writeBigUInt64LE(amount, 0);

  const idBuf = Buffer.from(id);
  const idLenBuf = Buffer.alloc(4);
  idLenBuf.writeUInt32LE(idBuf.length, 0);

  const proofCountBuf = Buffer.alloc(4);
  proofCountBuf.writeUInt32LE(0, 0);

  const streamerIndexBuf = Buffer.alloc(1);
  streamerIndexBuf.writeUInt8(0, 0);

  // Optional parameters (all None = 0)
  const channelOption = Buffer.from([0]); // None
  const epochOption = Buffer.from([0]); // None
  const receiptOption = Buffer.from([0]); // None

  const data = Buffer.concat([
    discriminator,
    streamerIndexBuf,
    indexBuf,
    amountBuf,
    idLenBuf,
    idBuf,
    proofCountBuf,
    channelOption,
    epochOption,
    receiptOption
  ]);

  // Build instruction
  const ASSOCIATED_TOKEN_PROGRAM_ID_PK = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: claimer.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: epochState, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: claimerAta, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_FROM_SPL, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID_PK, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  // Build and sign tx
  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  console.log('✅ Transaction built and signed\n');
  console.log('📤 Submitting to mainnet...');

  try {
    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('✅ Submitted!');
    console.log('📍 Signature:', sig);
    console.log('🔗 Explorer: https://explorer.solana.com/tx/' + sig + '\n');

    console.log('⏳ Confirming...');
    const confirmation = await connection.confirmTransaction(sig, 'confirmed');

    if (confirmation.value.err) {
      console.error('❌ Transaction failed:', confirmation.value.err);
      process.exit(1);
    }

    console.log('✅ CONFIRMED!\n');
    console.log('🎉 Claim #0001 Successful!');
    console.log('💰 Check balance: https://solscan.io/token/AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5?owner=DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1\n');

    // Update DB
    console.log('📝 Updating database...');
    const { Pool } = require('pg');
    const pool = new Pool({
      connectionString: 'postgresql://postgres:postgres@localhost:5432/twzrd'
    });
    await pool.query(
      'UPDATE cls_claims SET tx_status = $1, tx_signature = $2, confirmed_at = NOW() WHERE wallet = $3 AND epoch_id = $4',
      ['confirmed', sig, 'DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1', 424243]
    );
    await pool.end();
    console.log('✅ Database updated\n');
    
  } catch (err: any) {
    console.error('❌ Error:', err.message);
    process.exit(1);
  }
}

main();
