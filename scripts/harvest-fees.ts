#!/usr/bin/env ts-node
/**
 * Harvest Token-2022 withheld transfer fees into the protocol treasury via AO program CPI.
 *
 * Usage:
 *   ts-node scripts/harvest-fees.ts <MINT> [--from-mint]
 *
 * Notes:
 * - Default mode enumerates all Token-2022 token accounts for <MINT> and calls
 *   `harvest_fees` in batches (<=255 accounts per call).
 * - `--from-mint` calls `harvest_fees` once with no remaining accounts, which
 *   withdraws fees already harvested to the mint (requires prior harvest_to_mint).
 */

import { AnchorProvider, Program, Wallet } from '@coral-xyz/anchor';
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  TOKEN_2022_PROGRAM_ID,
} from '@solana/spl-token';
import {
  ComputeBudgetProgram,
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import fs from 'fs';
import path from 'path';

const AO_PROGRAM_ID = new PublicKey(
  process.env.AO_PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop',
);

const PROTOCOL_SEED = Buffer.from('protocol');

function chunk<T>(items: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += size) out.push(items.slice(i, i + size));
  return out;
}

function loadKeypair(keypairPath: string): Keypair {
  const resolved = keypairPath.replace('~', process.env.HOME || '');
  return Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(resolved, 'utf8'))),
  );
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error('Usage: ts-node scripts/harvest-fees.ts <MINT> [--from-mint]');
    process.exit(1);
  }

  const mint = new PublicKey(args[0]);
  const fromMint = args.includes('--from-mint');

  const rpc =
    process.env.AO_RPC_URL ||
    process.env.SYNDICA_RPC ||
    process.env.ANCHOR_PROVIDER_URL ||
    'https://api.mainnet-beta.solana.com';
  const connection = new Connection(rpc, 'confirmed');

  const walletPath =
    process.env.ANCHOR_WALLET?.replace('~', process.env.HOME || '') ||
    path.join(process.env.HOME || '', '.config/solana/id.json');
  const payer = loadKeypair(walletPath);

  const wallet = new Wallet(payer);
  const provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' });

  const idlPath = path.join(process.cwd(), 'target/idl/token_2022.json');
  if (!fs.existsSync(idlPath)) {
    console.error(`❌ IDL not found at ${idlPath}. Run "anchor build" first.`);
    process.exit(1);
  }
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf8'));
  if (idl.accounts) {
    idl.accounts.forEach((acc: any) => {
      if (acc.size === null || acc.size === undefined) acc.size = 0;
    });
  }
  const program = new Program(idl, AO_PROGRAM_ID, provider);

  const [protocolState] = PublicKey.findProgramAddressSync([PROTOCOL_SEED, mint.toBuffer()], AO_PROGRAM_ID);
  const [feeConfig] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer(), Buffer.from('fee_config')],
    AO_PROGRAM_ID,
  );

  const treasuryAta = getAssociatedTokenAddressSync(
    mint,
    protocolState,
    true, // PDA owner
    TOKEN_2022_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  console.log('\n=== Harvest Withheld Fees ===');
  console.log(`RPC:           ${rpc}`);
  console.log(`AO Program:    ${AO_PROGRAM_ID.toBase58()}`);
  console.log(`Mint:          ${mint.toBase58()}`);
  console.log(`Payer:         ${payer.publicKey.toBase58()}`);
  console.log(`ProtocolState: ${protocolState.toBase58()}`);
  console.log(`FeeConfig:     ${feeConfig.toBase58()}`);
  console.log(`Treasury ATA:  ${treasuryAta.toBase58()}`);
  console.log(`Mode:          ${fromMint ? 'withdraw-from-mint' : 'withdraw-from-accounts'}`);

  if (fromMint) {
    const ix = await (program.methods as any)
      .harvestFees()
      .accounts({
        authority: payer.publicKey,
        protocolState,
        feeConfig,
        mint,
        treasury: treasuryAta,
        creatorPool: treasuryAta, // unused in 100% treasury mode
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .instruction();

    const tx = new Transaction()
      .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }))
      .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
      .add(ix);

    const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
      skipPreflight: false,
    });
    console.log(`✅ Harvested from mint. Signature: ${sig}`);
    return;
  }

  // Enumerate token accounts (Token-2022 program) for this mint.
  // Token account layout has mint at offset 0 even with extensions.
  const tokenAccounts = await connection.getProgramAccounts(TOKEN_2022_PROGRAM_ID, {
    commitment: 'confirmed',
    filters: [{ memcmp: { offset: 0, bytes: mint.toBase58() } }],
  });

  const sources = tokenAccounts
    .map((x) => x.pubkey)
    .filter((pk) => !pk.equals(treasuryAta));

  console.log(`Found ${sources.length} token accounts (excluding treasury).`);
  if (sources.length === 0) {
    console.log('Nothing to harvest.');
    return;
  }

  const batches = chunk(sources, 255);
  console.log(`Batches: ${batches.length} (max 255 accounts each)`);

  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    console.log(`\n--- Batch ${i + 1}/${batches.length} (${batch.length} accounts) ---`);

    const ix = await (program.methods as any)
      .harvestFees()
      .accounts({
        authority: payer.publicKey,
        protocolState,
        feeConfig,
        mint,
        treasury: treasuryAta,
        creatorPool: treasuryAta, // unused in 100% treasury mode
        tokenProgram: TOKEN_2022_PROGRAM_ID,
      })
      .remainingAccounts(
        batch.map((pubkey) => ({
          pubkey,
          isWritable: true,
          isSigner: false,
        })),
      )
      .instruction();

    const tx = new Transaction()
      .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_200_000 }))
      .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
      .add(ix);

    const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
      commitment: 'confirmed',
      skipPreflight: false,
    });
    console.log(`✅ Sent. Signature: ${sig}`);
  }

  console.log('\n✅ Done');
}

main().catch((err) => {
  console.error('\n❌ Error:', err);
  process.exit(1);
});

