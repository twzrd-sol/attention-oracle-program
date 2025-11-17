import { Connection, PublicKey } from '@solana/web3.js';

const conn = new Connection('https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973', 'confirmed');

async function main() {
  const sig = '4PSuyC5YsHG7W6Zm4sJTAgxHAh4by7vXPFrqzTcMoTG5gyarPdxhgC881RCTg3GJupDUusSnrHi6pzJevLmkKP3z';

  try {
    const tx = await conn.getParsedTransaction(sig, 'confirmed');

    if (!tx) {
      console.log('Transaction not found');
      return;
    }

    console.log('=== Transaction Details ===\n');
    const error = tx.meta?.err;
    console.log(`Status: ${error ? 'FAILED' : 'SUCCESS'}\n`);

    if (tx.transaction.message.instructions) {
      const instructions = tx.transaction.message.instructions;
      console.log(`Instructions: ${instructions.length}\n`);

      for (let i = 0; i < instructions.length; i++) {
        const instr = instructions[i];
        console.log(`Instruction ${i + 1}:`);
        console.log(JSON.stringify(instr, null, 2));
        console.log('');
      }
    }

    // Check account list
    console.log('=== Accounts Involved ===\n');
    const accts = tx.transaction.message.staticAccountKeys;
    for (let i = 0; i < accts.length; i++) {
      console.log(`[${i}] ${accts[i].toBase58()}`);
    }

    // Check pre and post balances
    console.log('\n=== SOL Balance Changes ===\n');
    if (tx.meta?.preBalances && tx.meta?.postBalances) {
      for (let i = 0; i < accts.length; i++) {
        const pre = tx.meta.preBalances[i];
        const post = tx.meta.postBalances[i];
        const change = post - pre;
        if (change !== 0) {
          console.log(`[${i}] ${accts[i].toBase58()}`);
          console.log(`  Pre:  ${(pre / 1e9).toFixed(6)} SOL`);
          console.log(`  Post: ${(post / 1e9).toFixed(6)} SOL`);
          console.log(`  Δ:    ${(change / 1e9).toFixed(6)} SOL\n`);
        }
      }
    }

  } catch (e: any) {
    console.log(`Error: ${e.message}`);
  }
}

main();
