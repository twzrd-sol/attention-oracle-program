import { Connection, PublicKey } from '@solana/web3.js';

const conn = new Connection('https://mainnet.helius-rpc.com/?api-key=1fc5da66-dd53-4041-9069-7300d1787973', 'confirmed');

const SOURCE_ADDR = '87d5WsriU5DiGPSFvojQJH525qsHNiHCP4m2Qa19ufdy';
const PROGRAM_ID = 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';
const MINT = 'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5';

async function main() {
  console.log(`=== Investigating Source Address ===\n`);
  console.log(`Address: ${SOURCE_ADDR}`);

  // Try to find this as a PDA
  const programId = new PublicKey(PROGRAM_ID);
  const mintPk = new PublicKey(MINT);
  const sourcePk = new PublicKey(SOURCE_ADDR);

  // Check if it's a valid PDA by trying different seeds
  const seeds = [
    Buffer.from('rent_receiver'),
    Buffer.from('protocol'),
    Buffer.from('treasury'),
    Buffer.from('harvest'),
    Buffer.from('harvest_pool'),
    Buffer.from('fees'),
  ];

  console.log('\nChecking if address is derived from common seeds:\n');

  for (const seed of seeds) {
    try {
      const [pda] = PublicKey.findProgramAddressSync(
        [seed, mintPk.toBuffer()],
        programId
      );

      if (pda.equals(sourcePk)) {
        console.log(`MATCH FOUND!`);
        console.log(`  Seed: "${seed.toString()}"`);
        console.log(`  Derived PDA: ${pda.toBase58()}`);
        return;
      }
    } catch (e) {
      // ignore
    }
  }

  console.log('Not a known program-derived address\n');

  // Check if it's the RENT_RECEIVER from the script
  console.log('=== Checking Script References ===\n');

  const fs = require('fs');
  try {
    const scriptPath = '/home/twzrd/milo-token/clean-hackathon/scripts/reclaim-channel-accounts.ts';
    const content = fs.readFileSync(scriptPath, 'utf-8');

    const lines = content.split('\n');
    for (const line of lines) {
      if (line.includes(SOURCE_ADDR)) {
        console.log(`Found in reclaim-channel-accounts.ts:`);
        console.log(`  ${line.trim()}`);
      }
    }
  } catch (e) {
    console.log(`Cannot read script file`);
  }
}

main().catch(console.error);
