import { Transaction, Keypair, Connection } from '@solana/web3.js';
import * as fs from 'fs';

const RPC_URL = 'https://api.mainnet-beta.solana.com';
const connection = new Connection(RPC_URL, 'confirmed');

// Load keypair
const keypairData = JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8'));
const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

// Fresh transaction (base64 from gateway - just read)
const txBase64 = fs.readFileSync('/tmp/fresh-tx.b64', 'utf-8').trim();

// Decode transaction
const txBuffer = Buffer.from(txBase64, 'base64');
const tx = Transaction.from(txBuffer);

// Sign it
tx.sign(keypair);

// Serialize
const signedBuffer = tx.serialize();

console.log('✅ Transaction Signed!');
console.log('📤 Sending to mainnet...\n');

// Send to mainnet
(async () => {
  try {
    const sig = await connection.sendRawTransaction(signedBuffer);
    console.log('✅ Signature:', sig);
    console.log('🔗 Explorer: https://explorer.solana.com/tx/' + sig + '\n');
    
    console.log('⏳ Confirming...');
    const confirmation = await connection.confirmTransaction(sig, 'confirmed');
    
    if (confirmation.value.err) {
      console.log('❌ Transaction failed:', confirmation.value.err);
    } else {
      console.log('✅ CONFIRMED!\n');
      console.log('Claim #0001 successful!');
      console.log('Check CCM balance: https://solscan.io/token/AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5?owner=DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1');
    }
  } catch (err: any) {
    console.error('❌ Error:', err.message);
  }
})();
