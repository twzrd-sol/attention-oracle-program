import { Connection, Keypair, PublicKey, Transaction } from '@solana/web3.js';
import * as fs from 'fs';

const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const RPC_URL = 'https://api.mainnet-beta.solana.com';

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');
  const claimer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8')))
  );

  const PROTOCOL_SEED = Buffer.from('protocol');
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  const [treasuryAta] = PublicKey.findProgramAddressSync(
    [MINT.toBuffer(), protocolState.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  console.log('Treasury ATA:', treasuryAta.toBase58());

  // Check if exists
  const exists = await connection.getAccountInfo(treasuryAta);
  if (exists) {
    console.log('✅ Already exists');
    process.exit(0);
  }

  console.log('Creating via ATA program...\n');

  // Use web3.js to call the ATA program's CreateIdempotent
  // This is the standard way to create ATAs
  const { createAssociatedTokenAccountIdempotentInstruction } = await import('@solana/spl-token').catch(() => ({
    createAssociatedTokenAccountIdempotentInstruction: null
  }));

  if (!createAssociatedTokenAccountIdempotentInstruction) {
    console.error('spl-token module not available, trying manual TX...');
    // Fallback: just submit a basic instruction and see what error we get
    // This will help us debug the right format
    const { TransactionInstruction } = await import('@solana/web3.js');
    const ix = new TransactionInstruction({
      programId: ASSOCIATED_TOKEN_PROGRAM_ID,
      keys: [
        { pubkey: claimer.publicKey, isSigner: true, isWritable: true },
        { pubkey: treasuryAta, isSigner: false, isWritable: true },
        { pubkey: protocolState, isSigner: false, isWritable: false },
        { pubkey: MINT, isSigner: false, isWritable: false },
        { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      ],
      data: Buffer.from([1]), // CreateIdempotent discriminator
    });

    const tx = new Transaction().add(ix);
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = claimer.publicKey;
    tx.sign(claimer);

    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('Tx:', sig);
    const conf = await connection.confirmTransaction(sig);
    console.log('Confirmed:', !conf.value.err);
    return;
  }

  const ix = createAssociatedTokenAccountIdempotentInstruction(
    claimer.publicKey,
    treasuryAta,
    protocolState,
    MINT,
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  const tx = new Transaction().add(ix);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  const sig = await connection.sendRawTransaction(tx.serialize());
  console.log('Tx:', sig);
  const conf = await connection.confirmTransaction(sig);
  console.log('Confirmed:', !conf.value.err);
}

main().catch(console.error);
