import { Transaction, Keypair, Connection } from '@solana/web3.js';
import * as fs from 'fs';
import * as https from 'https';

const RPC_URL = 'https://api.mainnet-beta.solana.com';
const GATEWAY_URL = 'http://localhost:5000/api/claim-cls';
const connection = new Connection(RPC_URL, 'confirmed');

// Load keypair
const keypairData = JSON.parse(fs.readFileSync('/home/twzrd/.config/solana/cls-claim-0001.json', 'utf-8'));
const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

console.log('🔐 Wallet:', keypair.publicKey.toBase58());
console.log('📍 Claiming 100 CCM for epoch 424243\n');

// Step 1: Request fresh unsigned transaction from gateway
console.log('1️⃣  Requesting unsigned transaction from gateway...');

const payload = JSON.stringify({
  wallet: 'DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1',
  channelName: 'claim-0001-test',
  epochId: 424243,
  amount: '100000000000',
  index: 0,
  proof: []
});

const options = {
  hostname: 'localhost',
  port: 5000,
  path: '/api/claim-cls',
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Content-Length': payload.length
  }
};

const req = require('http').request(options, (res: any) => {
  let data = '';
  res.on('data', (chunk: any) => { data += chunk; });
  res.on('end', async () => {
    try {
      const response = JSON.parse(data);
      
      if (!response.transaction) {
        console.error('❌ No transaction in response:', response);
        process.exit(1);
      }
      
      console.log('✅ Received unsigned transaction\n');
      
      // Step 2: Decode and sign
      console.log('2️⃣  Signing transaction...');
      const txBuffer = Buffer.from(response.transaction, 'base64');
      const tx = Transaction.from(txBuffer);
      tx.sign(keypair);
      console.log('✅ Signed\n');
      
      // Step 3: Submit
      console.log('3️⃣  Submitting to mainnet...');
      const signedBuffer = tx.serialize();
      const sig = await connection.sendRawTransaction(signedBuffer);
      console.log('✅ Submitted!');
      console.log('📍 Signature:', sig);
      console.log('🔗 Link: https://explorer.solana.com/tx/' + sig + '\n');
      
      // Step 4: Confirm
      console.log('4️⃣  Confirming...');
      const confirmation = await connection.confirmTransaction(sig, 'confirmed');
      
      if (confirmation.value.err) {
        console.error('❌ Transaction failed:', confirmation.value.err);
        process.exit(1);
      }
      
      console.log('✅ CONFIRMED!\n');
      console.log('🎉 Claim #0001 Successful!');
      console.log('💰 Check balance: https://solscan.io/token/AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5?owner=DV879FdRD3LAot5MTfzftrVmMv9WC9giUeYsDnmTSZh1\n');
      
      process.exit(0);
    } catch (err) {
      console.error('❌ Error:', err);
      process.exit(1);
    }
  });
});

req.on('error', (err: any) => {
  console.error('❌ Gateway error:', err.message);
  process.exit(1);
});

req.write(payload);
req.end();
