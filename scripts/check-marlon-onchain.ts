import { Connection, PublicKey } from '@solana/web3.js';
import dotenv from 'dotenv';

dotenv.config();

const RPC_URL = process.env.RPC_URL!;
const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');

async function checkMarlon() {
  const connection = new Connection(RPC_URL, 'confirmed');

  const [channelStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), Buffer.from('marlon')],
    PROGRAM_ID
  );

  console.log(`Channel: marlon`);
  console.log(`PDA: ${channelStatePDA.toBase58()}`);

  const accountInfo = await connection.getAccountInfo(channelStatePDA);

  if (!accountInfo) {
    console.log(`❌ Account not found`);
    process.exit(1);
  }

  console.log(`✅ Account found`);
  console.log(`Size: ${accountInfo.data.length} bytes`);
  console.log(`Owner: ${accountInfo.owner.toBase58()}`);

  if (accountInfo.data.length === 10742) {
    console.log(`✅ V2 Account (8192 capacity)`);
  } else if (accountInfo.data.length === 1782) {
    console.log(`ℹ️  V1 Account (1024 capacity)`);
  }
}

checkMarlon();
