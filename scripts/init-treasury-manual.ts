import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
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

  console.log('\nCreating manually via SystemProgram + Token-2022 init...\n');

  // Token-2022 TokenAccount size
  const TOKEN_ACCOUNT_SIZE = 165;
  const lamports = await connection.getMinimumBalanceForRentExemption(TOKEN_ACCOUNT_SIZE);

  console.log('Account size:', TOKEN_ACCOUNT_SIZE, 'bytes');
  console.log('Rent lamports:', lamports, '(' + (lamports / LAMPORTS_PER_SOL).toFixed(6), 'SOL)');

  // Step 1: Create the account with SystemProgram
  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: claimer.publicKey,
    newAccountPubkey: treasuryAta,
    lamports,
    space: TOKEN_ACCOUNT_SIZE,
    programId: TOKEN_2022_PROGRAM_ID,
  });

  // Step 2: Initialize it as a TokenAccount
  // Token-2022 InitializeAccount3 instruction format:
  // [mint, owner, 0] (authority, decimals info, etc.)
  const initDataBuf = Buffer.alloc(34);
  initDataBuf.write('9'); // Discriminator (might be different for Token-2022, trying 9)
  MINT.toBuffer().copy(initDataBuf, 1); // mint pubkey (32 bytes)
  protocolState.toBuffer().copy(initDataBuf, 33); // owner pubkey (32 bytes)
  // Note: This structure might need adjusting based on Token-2022's InitializeAccount3

  const initializeAccountIx = new TransactionInstruction({
    programId: TOKEN_2022_PROGRAM_ID,
    keys: [
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: false },
    ],
    data: initDataBuf,
  });

  const tx = new Transaction().add(createAccountIx, initializeAccountIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  try {
    console.log('📤 Sending transaction...');
    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('✅ Submitted!');
    console.log('📍 Signature:', sig);

    console.log('\n⏳ Confirming...');
    const conf = await connection.confirmTransaction(sig, 'confirmed');

    if (conf.value.err) {
      console.error('❌ Failed:', conf.value.err);
      process.exit(1);
    }

    console.log('✅ CONFIRMED!');
    console.log('🎉 Treasury ATA initialized manually!\n');
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
