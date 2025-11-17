import { PublicKey } from '@solana/web3.js';

const ASSOCIATED_TOKEN = 'ATokenGPvbdGVqstVQmcLsNZAqeEjlU23wWNHUaiP3c6Z';

// Test each character
const BASE58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

console.log('Testing each character in the string:');
for (let i = 0; i < ASSOCIATED_TOKEN.length; i++) {
  const char = ASSOCIATED_TOKEN[i];
  const valid = BASE58.includes(char);
  if (!valid) {
    console.log(`[${i}] '${char}': ❌ INVALID (code: ${char.charCodeAt(0)})`);
  }
}

// Also test the correct known address
const CORRECT = 'ATokenGPvbdGVqstVQmcLsNZAqeEjlU23wWNHUaiP3c6Z';
console.log('\nAttempting correct address...');
try {
  const pk = new PublicKey(CORRECT);
  console.log('Success:', pk.toBase58());
} catch (e: any) {
  console.log('Error:', e.message);
  // Try alternate format
  const CORRECT2 = 'ATokenGPvbdGVqstVQmcLsNZAqeEjlU23wWNHUaiP3c6Z';
  try {
    const pk2 = new PublicKey(CORRECT2);
    console.log('Alternate worked:', pk2.toBase58());
  } catch {
    console.log('Both failed');
  }
}
