import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import * as fs from 'fs';
import * as crypto from 'crypto';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const RPC_URL = 'https://api.mainnet-beta.solana.com';

const PROTOCOL_SEED = Buffer.from('protocol');
const EPOCH_STATE_SEED = Buffer.from('epoch_state');

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  
  // Test wallet
  const claimerKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8')))
  );

  console.log('Claimer:', claimerKeypair.publicKey.toBase58());

  // PDAs
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  const epoch = 424243;
  const epochBuf = Buffer.alloc(8);
  epochBuf.writeBigUInt64LE(BigInt(epoch), 0);

  const channel = 'claim-0001-test';
  const { keccak_256 } = require('@noble/hashes/sha3');
  const streamerKeyHash = keccak_256(Buffer.concat([Buffer.from('twitch:'), Buffer.from(channel.toLowerCase())]));
  const streamerKey = new PublicKey(Buffer.from(streamerKeyHash));

  const [epochState] = PublicKey.findProgramAddressSync(
    [EPOCH_STATE_SEED, epochBuf, streamerKey.toBuffer(), MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log('Protocol State:', protocolState.toBase58());
  console.log('Epoch State:', epochState.toBase58());

  // ATAs (just use known values for now)
  const claimerAta = new PublicKey('5UwVTFQKoJh1jyBV6pqZxwq4oQC1oWJDGaLv1MxPHLeq'); // placeholder
  const treasuryAta = new PublicKey('9C9SXs3k5bFCCKBqzUVLMhKFMDZvFXYQYC5YhJ4Y6xF9'); // placeholder

  // Build claim instruction
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

  const data = Buffer.concat([
    discriminator,
    streamerIndexBuf,
    indexBuf,
    amountBuf,
    idLenBuf,
    idBuf,
    proofCountBuf
  ]);

  const ix = new TransactionInstruction({
    keys: [
      { pubkey: claimerKeypair.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: true },
      { pubkey: epochState, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: claimerAta, isSigner: false, isWritable: true },
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS'), isSigner: false, isWritable: false },
      { pubkey: new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'), isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    programId: PROGRAM_ID,
    data,
  });

  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimerKeypair.publicKey;

  const serialized = tx.serialize({ requireAllSignatures: false });
  const base64 = serialized.toString('base64');
  
  console.log('\n✅ Unsigned transaction (base64):');
  console.log(base64);
  console.log('\n💡 Copy this and paste into Backpack to sign and send');
}

main().catch(console.error);
