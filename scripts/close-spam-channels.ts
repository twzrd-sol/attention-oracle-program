#!/usr/bin/env tsx
import { Connection, PublicKey, Keypair, Transaction, SystemProgram } from '@solana/web3.js';
import { getConnection } from '../apps/gateway/src/lib/rpc.js';
import bs58 from 'bs58';
import 'dotenv/config';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const SPAM_CHANNELS = ['threadguy_live', 'thread_guytv'];

async function main() {
  const conn = getConnection();

  // Get admin keypair
  const adminPrivateKey = process.env.ADMIN_PRIVATE_KEY;
  if (!adminPrivateKey) {
    console.error('❌ ADMIN_PRIVATE_KEY not set');
    process.exit(1);
  }

  let adminKeypair: Keypair;
  try {
    if (adminPrivateKey.startsWith('[')) {
      adminKeypair = Keypair.fromSecretKey(new Uint8Array(JSON.parse(adminPrivateKey)));
    } else {
      adminKeypair = Keypair.fromSecretKey(bs58.decode(adminPrivateKey));
    }
  } catch (err) {
    console.error('❌ Failed to parse ADMIN_PRIVATE_KEY:', err);
    process.exit(1);
  }

  console.log('Admin wallet:', adminKeypair.publicKey.toBase58());
  console.log('\n=== Checking Spam Channel States ===\n');

  for (const channel of SPAM_CHANNELS) {
    const [channelStatePda] = PublicKey.findProgramAddressSync(
      [Buffer.from('channel_state'), Buffer.from(channel)],
      PROGRAM_ID
    );

    console.log(`${channel}:`);
    console.log(`  PDA: ${channelStatePda.toBase58()}`);

    const account = await conn.getAccountInfo(channelStatePda);
    if (!account) {
      console.log(`  Status: ✅ No on-chain account (nothing to close)\n`);
      continue;
    }

    const rentSOL = account.lamports / 1e9;
    console.log(`  Rent: ${rentSOL.toFixed(6)} SOL`);
    console.log(`  Size: ${account.data.length} bytes`);
    console.log(`  Owner: ${account.owner.toBase58()}`);

    // Note: Closing channel_state requires admin authority + proper instruction
    // This would need the actual close_channel instruction from the program
    console.log(`  ⚠️  Account exists but requires program instruction to close`);
    console.log(`      (Not a standard rent-recoverable account)\n`);
  }

  console.log('=== Summary ===');
  console.log('Spam channels checked. If accounts exist, they require');
  console.log('program-specific close instructions (not standard closeAccount).');
  console.log('\nBest practice: Blocklist at ingestion to prevent future data.');
}

main().catch(console.error);
