import { Transaction, Keypair, Connection } from '@solana/web3.js';
import * as fs from 'fs';
import * as bs58 from 'bs58';

const RPC_URL = 'https://api.mainnet-beta.solana.com';
const connection = new Connection(RPC_URL, 'confirmed');

// Load keypair
const keypairData = JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8'));
const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

// Unsigned transaction (base64 from gateway)
const txBase64 = 'AQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAAECuYCKWi+AsRBfXgnjZMLnW+QsRWpz4Fx2oVYnejIZMdjqeITzX/K67PkjBcMuPqE2zoT73xlM8Yua/aOCHaSwa2NP5C/MKokxeWIqsdMnw/GdXK4iniGtui14x7lEFg8lAQEBAAA=';

// Decode transaction
const txBuffer = Buffer.from(txBase64, 'base64');
const tx = Transaction.from(txBuffer);

// Sign it
tx.sign(keypair);

// Serialize and encode
const signedBuffer = tx.serialize();
const signedBase64 = signedBuffer.toString('base64');

console.log('✅ Transaction Signed!\n');
console.log('Signed Transaction (base64):');
console.log(signedBase64);
console.log('\n📤 Sending to mainnet...\n');

// Send to mainnet
(async () => {
  try {
    const sig = await connection.sendRawTransaction(signedBuffer);
    console.log('✅ Transaction Sent!');
    console.log('Signature:', sig);
    console.log('Explorer: https://explorer.solana.com/tx/' + sig);
    
    console.log('\n⏳ Waiting for confirmation...');
    await connection.confirmTransaction(sig, 'confirmed');
    console.log('✅ Confirmed!');
  } catch (err) {
    console.error('❌ Error:', err);
  }
})();
