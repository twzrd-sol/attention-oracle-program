import { PublicKey } from '@solana/web3.js';

// The correct SPL Associated Token Program ID with '1' instead of 'l'
const correctedAddr = 'ATokenGPvbdGVqstVQmcLsNZAqeEj1U23wWNHUaiP3c6Z';

console.log('Testing corrected address with 1 instead of l:');
console.log('Address:', correctedAddr);

try {
  const pk = new PublicKey(correctedAddr);
  console.log('✅ VALID!');
  console.log('Decoded:', pk.toBase58());
} catch (e: any) {
  console.log('❌ Still invalid:', e.message);

  // List all invalid chars
  const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  console.log('\nInvalid characters found:');
  for (let i = 0; i < correctedAddr.length; i++) {
    if (!BASE58.includes(correctedAddr[i])) {
      const code = correctedAddr.charCodeAt(i);
      console.log(`  Position ${i}: '${correctedAddr[i]}' (code: ${code})`);
    }
  }
}
