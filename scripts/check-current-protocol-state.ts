import { Connection, PublicKey } from '@solana/web3.js';

async function main() {
  const connection = new Connection('https://api.mainnet.solana.com', 'confirmed');
  const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
  const mint = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');

  const [protocolState] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mint.toBuffer()],
    programId
  );

  console.log('Protocol State PDA:', protocolState.toBase58());

  const accountInfo = await connection.getAccountInfo(protocolState);
  if (!accountInfo) {
    console.log('Account not found!');
    process.exit(1);
  }

  console.log('Account data length:', accountInfo.data.length);
  const admin = new PublicKey(accountInfo.data.slice(10, 42));
  console.log('Admin pubkey:', admin.toBase58());
  const publisher = new PublicKey(accountInfo.data.slice(42, 74));
  console.log('Publisher pubkey:', publisher.toBase58());
  const paused = accountInfo.data[72];
  console.log('Paused:', paused === 1);
  
  const oracleAuth = '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy';
  console.log('\nOracle Authority:', oracleAuth);
  console.log('Match admin?', admin.toBase58() === oracleAuth);
  console.log('Match publisher?', publisher.toBase58() === oracleAuth);
}

main().catch(console.error);
