import { PublicKey } from '@solana/web3.js';

// Test candidates - replacing 'l' with '1'
const candidates = [
  'ATokenGPvbdGVqstVQmcLsNZAqeEjlU231WNHUaiP3c6Z', // Try with '1'
  'ATokenGPvbdGVqstVQmcLsNZAqeEjlU23lWNHUaiP3c6Z', // Original with 'l'
];

for (const addr of candidates) {
  try {
    const pk = new PublicKey(addr);
    console.log('✅ Valid:', addr);
    console.log('   Decoded:', pk.toBase58());
  } catch (e: any) {
    console.log('❌ Invalid:', addr);
    console.log('   Error:', e.message);
  }
}
