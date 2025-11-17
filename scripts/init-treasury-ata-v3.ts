import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
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

  console.log('🔐 Claimer:', claimer.publicKey.toBase58());
  console.log('📍 Protocol State:', protocolState.toBase58());
  console.log('💰 Treasury ATA to create:', treasuryAta.toBase58());

  // Check if already exists
  const existing = await connection.getAccountInfo(treasuryAta);
  if (existing) {
    console.log('✅ Already exists!');
    process.exit(0);
  }

  console.log('\nAttempting ATA creation with proper account order...\n');

  // Solana ATA program's CreateIdempotent instruction:
  // [
  //   payer (signer, writable),
  //   ATA account (writable),
  //   wallet/authority,
  //   mint,
  //   system_program,
  //   token_program,
  // ]

  const createIdempotentIx = new TransactionInstruction({
    programId: ASSOCIATED_TOKEN_PROGRAM_ID,
    keys: [
      // payer
      {
        pubkey: claimer.publicKey,
        isSigner: true,
        isWritable: true,
      },
      // ATA account to create
      {
        pubkey: treasuryAta,
        isSigner: false,
        isWritable: true,
      },
      // owner/authority (protocol_state)
      {
        pubkey: protocolState,
        isSigner: false,
        isWritable: false,
      },
      // mint
      {
        pubkey: MINT,
        isSigner: false,
        isWritable: false,
      },
      // system program
      {
        pubkey: SystemProgram.programId,
        isSigner: false,
        isWritable: false,
      },
      // token program
      {
        pubkey: TOKEN_2022_PROGRAM_ID,
        isSigner: false,
        isWritable: false,
      },
    ],
    data: Buffer.alloc(0), // No instruction data needed
  });

  const tx = new Transaction().add(createIdempotentIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  try {
    console.log('📤 Sending CreateIdempotent transaction...');
    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('✅ Submitted!');
    console.log('📍 Signature:', sig);
    console.log('🔗 Explorer: https://explorer.solana.com/tx/' + sig);

    console.log('\n⏳ Confirming...');
    const conf = await connection.confirmTransaction(sig, 'confirmed');

    if (conf.value.err) {
      console.error('❌ Failed:', conf.value.err);
      process.exit(1);
    }

    console.log('✅ CONFIRMED!');
    console.log('🎉 Treasury ATA initialized successfully!\n');
  } catch (err: any) {
    console.error('❌ Error:', err.message);
    if (err.logs) {
      console.log('\nProgram logs:');
      err.logs.forEach((log: string) => console.log('  ' + log));
    }
    process.exit(1);
  }
}

main().catch(console.error);
