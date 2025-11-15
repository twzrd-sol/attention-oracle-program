import { Connection, PublicKey } from '@solana/web3.js';
import 'dotenv/config';

const RPC = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const conn = new Connection(RPC, 'confirmed');

const wallets = [
  { name: 'Admin/Lacy', address: 'AmMftc4zHgR4yYfv29awV9Q46emo2aGPFW8utP81CsBv' },
  { name: 'Oracle/Publisher', address: '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy' },
  { name: 'Spending', address: '2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD' },
];

async function main() {
  console.log('=== Closable Token Accounts ===\n');

  let totalRecoverable = 0;

  for (const wallet of wallets) {
    const pubkey = new PublicKey(wallet.address);
    const tokenAccounts = await conn.getParsedTokenAccountsByOwner(pubkey, {
      programId: new PublicKey('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'),
    });

    const closable: any[] = [];

    for (const { pubkey: accountPubkey, account } of tokenAccounts.value) {
      const data = account.data.parsed.info;
      const balance = Number(data.tokenAmount.amount);
      const rentSOL = account.lamports / 1e9;

      // Consider closable if balance < 1 token (in raw amount)
      if (balance < 1_000_000) {
        closable.push({
          account: accountPubkey.toBase58(),
          mint: data.mint,
          balance,
          rentSOL,
        });
        totalRecoverable += rentSOL;
      }
    }

    if (closable.length > 0) {
      console.log(`${wallet.name} (${wallet.address}):`);
      for (const acc of closable) {
        console.log(`  Account: ${acc.account}`);
        console.log(`  Mint: ${acc.mint}`);
        console.log(`  Balance: ${acc.balance} (raw amount)`);
        console.log(`  Recoverable: ${acc.rentSOL.toFixed(6)} SOL\n`);
      }
    }
  }

  console.log(`=== Total Recoverable: ${totalRecoverable.toFixed(6)} SOL ===`);
}

main().catch(console.error);
