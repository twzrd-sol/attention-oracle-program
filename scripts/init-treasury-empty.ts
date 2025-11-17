import { Connection, Keypair, PublicKey, Transaction, SystemProgram, LAMPORTS_PER_SOL } from '@solana/web3.js';
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
  const existing = await connection.getAccountInfo(treasuryAta);
  if (existing) {
    console.log('✅ Already exists!');
    process.exit(0);
  }

  console.log('Creating empty account via SystemProgram...\n');

  const lamports = 0.00015 * LAMPORTS_PER_SOL; // Just enough for empty account

  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: claimer.publicKey,
    newAccountPubkey: treasuryAta,
    lamports: Math.floor(lamports),
    space: 0, // Empty account
    programId: TOKEN_2022_PROGRAM_ID,
  });

  const tx = new Transaction().add(createAccountIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = claimer.publicKey;
  tx.sign(claimer);

  try {
    console.log('📤 Sending...');
    const sig = await connection.sendRawTransaction(tx.serialize());
    console.log('✅ Submitted:', sig);

    const conf = await connection.confirmTransaction(sig, 'confirmed');
    if (conf.value.err) {
      console.error('❌ Failed:', conf.value.err);
    } else {
      console.log('✅ Created empty account');
    }
  } catch (err: any) {
    console.error('❌ Error:', err.message);
  }
}

main().catch(console.error);
