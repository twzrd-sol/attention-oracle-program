#!/usr/bin/env ts-node
/**
 * Minimal: Initialize ExtraAccountMetaList for CCM transfer hook
 *
 * Env vars: ANCHOR_PROVIDER_URL, ANCHOR_WALLET
 * Usage: ts-node scripts/init-eaml-simple.ts <MINT>
 */

import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import { AnchorProvider, Program, Wallet } from '@coral-xyz/anchor';
import * as fs from 'fs';
import * as path from 'path';

const PROGRAM_ID = 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';

async function main() {
  const mint = new PublicKey(process.argv[2] || '');
  if (mint.equals(PublicKey.default)) {
    console.error('❌ Usage: ts-node scripts/init-eaml-simple.ts <MINT_ADDRESS>');
    process.exit(1);
  }

  const rpcUrl = process.env.SYNDICA_RPC!;
  const walletPath = (process.env.ANCHOR_WALLET || '~/.config/solana/id.json').replace('~', process.env.HOME || '');

  const keypair = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf-8'))));
  const connection = new Connection(rpcUrl, 'confirmed');
  const provider = new AnchorProvider(connection, new Wallet(keypair), { commitment: 'confirmed' });

  const idlPath = path.join(path.dirname(__dirname), 'target/idl/token_2022.json');
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));
  const program = new Program(idl, PROGRAM_ID, provider);

  const [eaml] = PublicKey.findProgramAddressSync(
    [Buffer.from('extra-account-metas'), mint.toBuffer()],
    new PublicKey(PROGRAM_ID)
  );

  console.log(`\n🚀 Initialize EAML for mint: ${mint.toBase58()}`);
  console.log(`   EAML PDA: ${eaml.toBase58()}\n`);

  const existing = await connection.getAccountInfo(eaml);
  if (existing) {
    console.log('✅ EAML already initialized.\n');
    return;
  }

  console.log('📡 Sending initialize_extra_account_meta_list...');
  try {
    const tx = await program.methods
      .initializeExtraAccountMetaList()
      .accounts({
        payer: keypair.publicKey,
        mint,
        extraAccountMetaList: eaml,
        systemProgram: PublicKey.default,
      })
      .rpc();

    console.log(`✅ Success!\n   Tx: ${tx}\n`);
  } catch (err: any) {
    console.error(`❌ ${err.message}\n`);
    process.exit(1);
  }
}

main();
