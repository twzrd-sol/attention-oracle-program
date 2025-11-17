import { Connection, Keypair, PublicKey, Transaction, TransactionInstruction, SystemProgram } from '@solana/web3.js';
import * as fs from 'fs';

const MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
const TOKEN_2022_PROGRAM_ID = new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS');
const ASSOCIATED_TOKEN_PROGRAM_ID = new PublicKey('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const RPC_URL = 'https://api.mainnet-beta.solana.com';

async function main() {
  const connection = new Connection(RPC_URL, 'confirmed');

  // Load claimer keypair
  const claimerData = JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8'));
  const claimer = Keypair.fromSecretKey(Uint8Array.from(claimerData));

  // Derive protocol state
  const PROTOCOL_SEED = Buffer.from('protocol');
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, MINT.toBuffer()],
    PROGRAM_ID
  );

  console.log('🔐 Claimer:', claimer.publicKey.toBase58());
  console.log('📍 Protocol State:', protocolState.toBase58());

  // Derive treasury ATA - get both address and bump
  const [treasuryAta, bump] = PublicKey.findProgramAddressSync(
    [MINT.toBuffer(), protocolState.toBuffer(), TOKEN_2022_PROGRAM_ID.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  console.log('💰 Treasury ATA:', treasuryAta.toBase58());
  console.log('📌 Bump:', bump);

  // Check if it exists
  const accountInfo = await connection.getAccountInfo(treasuryAta);
  if (accountInfo) {
    console.log('✅ Treasury ATA already exists!');
    return;
  }

  console.log('❌ Treasury ATA does not exist. Creating via PublicKey instruction...\n');

  // Get account size and lamports needed
  const ACCOUNT_SIZE = 165; // TokenAccount size
  const lamports = await connection.getMinimumBalanceForRentExemption(ACCOUNT_SIZE);

  // Create account
  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: claimer.publicKey,
    newAccountPubkey: treasuryAta,
    lamports,
    space: ACCOUNT_SIZE,
    programId: TOKEN_2022_PROGRAM_ID,
  });

  // Initialize token account
  const initTokenIx = new TransactionInstruction({
    programId: TOKEN_2022_PROGRAM_ID,
    keys: [
      { pubkey: treasuryAta, isSigner: false, isWritable: true },
      { pubkey: MINT, isSigner: false, isWritable: false },
      { pubkey: protocolState, isSigner: false, isWritable: false },
    ],
    data: Buffer.from([3]), // InitializeMint2 discriminator is [3] for Token-2022
  });

  const tx = new Transaction().add(createAccountIx, initTokenIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  try {
    console.log('📤 Sending transaction...');
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

    console.log('✅ CONFIRMED!');
    console.log('🎉 Treasury ATA initialized successfully!\n');
  } catch (err: any) {
    console.error('❌ Error:', err.message);
    if (err.logs) {
      console.log('\nLogs:');
      console.log(err.logs.join('\n'));
    }
    process.exit(1);
  }
}

main().catch((err) => {
  console.error('❌ Error:', err.message);
  process.exit(1);
});
