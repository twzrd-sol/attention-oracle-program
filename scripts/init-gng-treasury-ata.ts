#!/usr/bin/env tsx
/**
 * Initialize the GNG protocol treasury ATA for the Token-2022 mint.
 *
 * This creates the associated token account for:
 *   - Mint:  AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5  (CCM Token-2022)
 *   - Owner: ProtocolState PDA (derived from [b"protocol", mint])
 * under the Token-2022 program.
 *
 * It does NOT mint any tokens; it only creates the ATA if missing.
 *
 * Usage:
 *   RPC_URL=... npx tsx scripts/init-gng-treasury-ata.ts
 *   # Optional overrides:
 *   #   PROGRAM_ID   (defaults to GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop)
 *   #   MINT_PUBKEY  (defaults to CCM mint)
 *   #   PAYER_KEYPAIR (defaults to ~/.config/solana/id.json)
 */

import fs from 'fs';
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import {
  TOKEN_2022_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  getAccount,
  getAssociatedTokenAddress,
} from '@solana/spl-token';

const RPC_URL =
  process.env.RPC_URL ||
  'https://api.mainnet-beta.solana.com';

const PROGRAM_ID = new PublicKey(
  process.env.PROGRAM_ID ||
    'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop',
);

const MINT = new PublicKey(
  process.env.MINT_PUBKEY ||
    'AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5',
);

const PAYER_KEYPAIR_PATH =
  process.env.PAYER_KEYPAIR ||
  `${process.env.HOME}/.config/solana/id.json`;

async function main() {
  console.log('=== Init GNG Treasury ATA (Token-2022) ===\n');
  console.log('RPC URL:     ', RPC_URL);
  console.log('Program ID:  ', PROGRAM_ID.toBase58());
  console.log('Mint:        ', MINT.toBase58());
  console.log('Payer keypair:', PAYER_KEYPAIR_PATH);
  console.log('');

  const connection = new Connection(RPC_URL, 'confirmed');

  const payer = Keypair.fromSecretKey(
    Uint8Array.from(
      JSON.parse(fs.readFileSync(PAYER_KEYPAIR_PATH, 'utf-8')),
    ),
  );

  console.log('Payer pubkey:', payer.publicKey.toBase58(), '\n');

  // Derive ProtocolState PDA: seeds = [b"protocol", mint]
  const [protocolStatePda] = PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), MINT.toBuffer()],
    PROGRAM_ID,
  );

  console.log('ProtocolState PDA:', protocolStatePda.toBase58());

  // Derive the treasury ATA for Token-2022 mint owned by ProtocolState PDA
  const treasuryAta = await getAssociatedTokenAddress(
    MINT,
    protocolStatePda,
    true,
    TOKEN_2022_PROGRAM_ID,
  );

  console.log('Treasury ATA:      ', treasuryAta.toBase58(), '\n');

  // Check if the ATA already exists
  let exists = false;
  try {
    const acct = await getAccount(
      connection,
      treasuryAta,
      'confirmed',
      TOKEN_2022_PROGRAM_ID,
    );
    console.log(
      `Treasury ATA already exists with balance: ${
        Number(acct.amount) / 1e9
      } tokens`,
    );
    exists = true;
  } catch {
    console.log('Treasury ATA does not exist yet.');
  }

  if (exists) {
    console.log('\nNothing to do. Exiting.\n');
    return;
  }

  console.log('\nCreating treasury ATA...\n');

  const ix = createAssociatedTokenAccountInstruction(
    payer.publicKey, // payer
    treasuryAta, // ATA address
    protocolStatePda, // owner (PDA)
    MINT,
    TOKEN_2022_PROGRAM_ID,
  );

  const tx = new Transaction().add(ix);
  tx.feePayer = payer.publicKey;

  const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: 'confirmed',
  });

  console.log('✅ Treasury ATA created!');
  console.log('   Signature:', sig);
  console.log(
    '   Explorer:  https://explorer.solana.com/tx/' + sig + '\n',
  );

  // Verify final account
  const acct = await getAccount(
    connection,
    treasuryAta,
    'confirmed',
    TOKEN_2022_PROGRAM_ID,
  );
  console.log(
    'Final treasury ATA balance:',
    Number(acct.amount) / 1e9,
    'tokens\n',
  );
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
