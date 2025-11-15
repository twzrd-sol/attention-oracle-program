import { Connection, PublicKey } from '@solana/web3.js';
import 'dotenv/config';

const RPC = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const conn = new Connection(RPC, 'confirmed');

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const OLD_TWZRD_PROGRAM = new PublicKey('FHFCPLierqNwqMkATmnCbT2ZPnnQ9j1AWWydKAUEB6Cj'); // Reward mint, not program

const wallets = [
  { name: 'Treasury', address: 'CSqL9UjtTKc3pFVkt7FFsCJbWKpwxfJZcycpgWeVVTTJ' },
  { name: 'Admin/Lacy', address: 'AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv' },
  { name: 'Oracle/Publisher', address: '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy' },
  { name: 'Spending', address: '2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD' },
  { name: 'Unknown', address: 'Hrbu75aVwq5sMd7u1z7gGLs1BmHbyGca1gLg5EL87EZF' },
];

async function checkProgramAccounts() {
  console.log('=== Checking Program-Owned Accounts ===\n');

  // Check for accounts owned by MILO program
  try {
    const accounts = await conn.getProgramAccounts(PROGRAM_ID, {
      filters: [
        {
          dataSize: 200, // Approximate size of ProtocolState or ChannelState
        },
      ],
    });

    console.log(`Found ${accounts.length} accounts owned by MILO program`);

    let totalRent = 0;
    for (const { pubkey, account } of accounts) {
      totalRent += account.lamports;
      console.log(`  ${pubkey.toBase58()}: ${(account.lamports / 1e9).toFixed(6)} SOL (${account.data.length} bytes)`);
    }

    console.log(`Total rent locked: ${(totalRent / 1e9).toFixed(6)} SOL\n`);
  } catch (err: any) {
    console.log(`Error checking program accounts: ${err.message}\n`);
  }
}

async function checkTokenAccounts() {
  console.log('=== Checking Token Accounts ===\n');

  const MILO_MINT = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');
  const CLS_MINT = new PublicKey('FZnUPK6eRWSQFEini3Go11JmVEqRNAQZgDP7q1DhyaKo');

  for (const wallet of wallets) {
    try {
      const pubkey = new PublicKey(wallet.address);
      const tokenAccounts = await conn.getParsedTokenAccountsByOwner(pubkey, {
        programId: new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'), // Token-2022
      });

      if (tokenAccounts.value.length > 0) {
        console.log(`${wallet.name} (${wallet.address}):`);
        for (const { account } of tokenAccounts.value) {
          const data = account.data.parsed.info;
          const mint = data.mint;
          const balance = data.tokenAmount.uiAmount;
          const rentSOL = account.lamports / 1e9;

          let mintName = 'Unknown';
          if (mint === MILO_MINT.toBase58()) mintName = 'MILO';
          if (mint === CLS_MINT.toBase58()) mintName = 'CLS';

          console.log(`  ${mintName}: ${balance} tokens, ${rentSOL.toFixed(6)} SOL rent`);
        }
      }
    } catch (err: any) {
      console.log(`${wallet.name}: Error - ${err.message}`);
    }
  }
}

async function checkChannelPDAs() {
  console.log('\n=== Checking Channel State PDAs ===\n');

  const testChannels = ['lacy', 'marlon', 'adapt', 'kaysan'];

  for (const channel of testChannels) {
    try {
      const [channelStatePda] = PublicKey.findProgramAddressSync(
        [Buffer.from('channel_state'), Buffer.from(channel)],
        PROGRAM_ID
      );

      const account = await conn.getAccountInfo(channelStatePda);
      if (account) {
        console.log(`${channel}: ${(account.lamports / 1e9).toFixed(6)} SOL (${account.data.length} bytes)`);
      }
    } catch (err) {
      // Skip
    }
  }
}

async function main() {
  console.log('Scanning for stale SOL and closable accounts...\n');

  await checkProgramAccounts();
  await checkTokenAccounts();
  await checkChannelPDAs();

  console.log('\n=== Summary ===');
  console.log('Total wallet balances: ~6.56 SOL');
  console.log('Check output above for closable accounts.');
}

main().catch(console.error);
