import { PublicKey } from '@solana/web3.js';

const correctAddr = 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL';

try {
  const pk = new PublicKey(correctAddr);
  console.log('✅ VALID!');
  console.log('Address:', pk.toBase58());
} catch (e: any) {
  console.log('❌ Invalid:', e.message);
}
