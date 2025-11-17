import { PublicKey } from '@solana/web3.js';

const TOKEN_2022 = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPvZeS';
const ASSOCIATED_TOKEN = 'ATokenGPvbdGVqstVQmcLsNZAqeEjlU23wWNHUaiP3c6Z';

console.log('Testing TOKEN_2022:', TOKEN_2022);
console.log('Testing ASSOCIATED_TOKEN:', ASSOCIATED_TOKEN);

try {
  const t1 = new PublicKey(TOKEN_2022);
  console.log('✅ TOKEN_2022 created:', t1.toBase58());
} catch (e: any) {
  console.error('❌ TOKEN_2022 failed:', e.message);
}

try {
  const t2 = new PublicKey(ASSOCIATED_TOKEN);
  console.log('✅ ASSOCIATED_TOKEN created:', t2.toBase58());
} catch (e: any) {
  console.error('❌ ASSOCIATED_TOKEN failed:', e.message);
}
