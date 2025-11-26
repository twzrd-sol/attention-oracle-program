#!/usr/bin/env ts-node
/**
 * Initialize ExtraAccountMetaList for a Token-2022 CCM mint
 *
 * This script sets up the EAML PDA for a transfer hook-enabled mint,
 * allowing Token-2022 to automatically inject protocol_state, fee_config,
 * and system_program into every transfer_checked call.
 *
 * Usage:
 *   ts-node scripts/init-eaml.ts <MINT_ADDRESS>
 *
 * Example:
 *   ts-node scripts/init-eaml.ts 7XJ8...
 */

import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
} from '@solana/web3.js';
import { AnchorProvider, Program, Wallet } from '@coral-xyz/anchor';
import * as fs from 'fs';
import * as path from 'path';

async function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.error('❌ Usage: ts-node scripts/init-eaml.ts <MINT_ADDRESS>');
    console.error('');
    console.error('Example:');
    console.error('  ts-node scripts/init-eaml.ts 7XJ8KF3wYPn4YvD2jZqZ1z2qZ3Z4Z5Z6Z7Z8Z9ZaZ');
    process.exit(1);
  }

  const mintStr = args[0];
  let mint: PublicKey;

  try {
    mint = new PublicKey(mintStr);
  } catch (e) {
    console.error(`❌ Invalid mint address: ${mintStr}`);
    process.exit(1);
  }

  console.log('🚀 Initialize ExtraAccountMetaList for Transfer Hook\n');
  console.log(`📋 Mint: ${mint.toBase58()}`);

  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET?.replace('~', process.env.HOME || '')
    || path.join(process.env.HOME || '', '.config/solana/id.json');

  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf-8')))
  );
  console.log(`💼 Payer: ${walletKeypair.publicKey.toBase58()}`);

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || 'https://api.mainnet-beta.solana.com';
  const connection = new Connection(rpcUrl, 'confirmed');
  const wallet = new Wallet(walletKeypair);
  const provider = new AnchorProvider(connection, wallet, {
    commitment: 'confirmed',
  });

  // Load program IDL
  const programId = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
  const idlPath = path.join(process.cwd(), 'target/idl/token_2022.json');

  if (!fs.existsSync(idlPath)) {
    console.error(
      `❌ IDL not found at ${idlPath}. Run "anchor build" first.`
    );
    process.exit(1);
  }

  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));
  const program = new Program(idl, programId, provider);

  // Derive EAML PDA
  const [eamlPda] = PublicKey.findProgramAddressSync(
    [Buffer.from('extra-account-metas'), mint.toBuffer()],
    programId
  );

  console.log(`🔐 EAML PDA: ${eamlPda.toBase58()}\n`);

  // Check if EAML already exists
  const eamlAccount = await connection.getAccountInfo(eamlPda);
  if (eamlAccount) {
    console.log('✅ EAML already initialized for this mint.');
    console.log(`   Account: ${eamlPda.toBase58()}`);
    console.log(`   Size: ${eamlAccount.data.length} bytes`);
    return;
  }

  // Call initialize_extra_account_meta_list
  console.log('📡 Initializing EAML on-chain...');

  try {
    const tx = await program.methods
      .initializeExtraAccountMetaList()
      .accounts({
        payer: walletKeypair.publicKey,
        mint: mint,
        extraAccountMetaList: eamlPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(`✅ EAML initialized!\n`);
    console.log(`📦 Transaction: ${tx}`);
    console.log(`   EAML PDA: ${eamlPda.toBase58()}`);
    console.log(`\n🎉 Transfer hook is now ready for mint: ${mint.toBase58()}`);
  } catch (err: any) {
    console.error('❌ Failed to initialize EAML:', err.message);
    if (err.logs) {
      console.error('\nProgram logs:');
      err.logs.forEach((log: string) => console.error(`  ${log}`));
    }
    process.exit(1);
  }
}

main()
  .then(() => {
    console.log('\n✅ Done');
    process.exit(0);
  })
  .catch((err) => {
    console.error('\n❌ Error:', err);
    process.exit(1);
  });
