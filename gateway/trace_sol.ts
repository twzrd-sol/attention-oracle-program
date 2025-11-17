import { Connection } from '@solana/web3.js';

const conn = new Connection('https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973', 'confirmed');

const txs = [
  '3PaRSrSaqEMqFFfSFJsYEgRzcjfAC47Ju6o4ztQC7t1MunxE8MyD5yg8WKPHi3maJtVnNGMZDpzpWA8Tmcj27x6q',
  '4Qk4HhKpq3bokB1E54BJ4jhVJHKaY4bNjJFt1Skk8CNz48mektuuy3uETttP2pvMeEDs63kmYkGmFzdPkpCaMRQW',
  '2pnVtRAi2RSFgFwBq4nZZzcoVwhVtcLQcGDDStCdXyYEw89GiZV77V5DJxq9BfGCY8RgebFVEpLtAp7nVrBLzwbh',
  '46eYCWyyxFuZkdi6Q5JBhdDos6A8pxN5Gmgk63bNvgG4kv9NLQVnqbqtvEd8G4v5aFaz4ak3uoRdcuQXJvHvWSui',
  '4fMNeMg5scEu6pTDPDkaf86MEKye8R8rdYdMbFpZ8dNZ9LxA4w6kEdor22CR8n1n7zW5NYPQUuHQz5XT8crTavz7',
];

async function main() {
  console.log('=== SOL Transfer Trace ===\n');

  for (let i = 0; i < txs.length; i++) {
    try {
      const tx = await conn.getParsedTransaction(txs[i], 'confirmed');

      if (!tx) {
        console.log(`[${i + 1}] Transaction not found`);
        continue;
      }

      const status = tx.meta?.err ? 'FAILED' : 'SUCCESS';
      const fee = tx.meta?.fee || 0;

      console.log(`[${i + 1}] ${status}`);
      console.log(`    Fee: ${(fee / 1e9).toFixed(6)} SOL`);

      if (tx.transaction.message.instructions) {
        const accts = tx.transaction.message.staticAccountKeys;
        const pre = tx.meta?.preBalances || [];
        const post = tx.meta?.postBalances || [];

        let hasTransfer = false;
        for (let j = 0; j < accts.length; j++) {
          const change = post[j] - pre[j];
          if (Math.abs(change) > 0) {
            hasTransfer = true;
            const addr = accts[j].toBase58();
            const prefix = addr.slice(0, 10);
            const suffix = addr.slice(-10);
            console.log(`    [${j}] ${prefix}...${suffix}`);
            console.log(`        ${(change / 1e9).toFixed(6)} SOL`);
          }
        }
        if (!hasTransfer) {
          console.log(`    No SOL balance changes`);
        }
      }
      console.log('');
    } catch (e: any) {
      console.log(`[${i + 1}] Error: ${e.message}\n`);
    }
  }
}

main();
