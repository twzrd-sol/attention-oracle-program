import { Connection, PublicKey } from '@solana/web3.js';

async function main() {
  const connection = new Connection(process.env.RPC_URL || 'https://api.mainnet.solana.com', 'confirmed');
  const programId = new PublicKey('4rArjoSZKrYkoE7hkvZNBP2Wpxovr78cfkxBnNwFNPn5');
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

  // Account layout: [discriminator (8), Borsh 0101 (2), admin (32), publisher (32), ...]
  // Admin starts at byte 10 (after discriminator + Borsh prefix)
  const admin = new PublicKey(accountInfo.data.slice(10, 42));
  console.log('Admin pubkey:', admin.toBase58());

  // Publisher starts at byte 42
  const publisher = new PublicKey(accountInfo.data.slice(42, 74));
  console.log('Publisher pubkey:', publisher.toBase58());

  // Paused flag
  const paused = accountInfo.data[72];
  console.log('Paused:', paused === 1);

  // Our oracle authority
  const oracleAuth = '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy';
  console.log('\nOracle Authority:', oracleAuth);
  console.log('Match admin?', admin.toBase58() === oracleAuth);
  console.log('Match publisher?', publisher.toBase58() === oracleAuth);
}

main().catch(console.error);
