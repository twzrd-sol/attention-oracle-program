#!/usr/bin/env ts-node
/**
 * Add + initialize Token-2022 TransferFeeConfig on an existing mint.
 *
 * This is the “safe first step” for fees:
 * - Reallocate the mint to include TransferFeeConfig (if missing)
 * - Initialize TransferFeeConfig with 0 bps / 0 max fee (unless overridden)
 * - Set withdraw_withheld_authority = protocol_state PDA (so AO program can sign via PDA)
 *
 * Usage:
 *   ts-node scripts/init-transfer-fee-config.ts <MINT> [--bps 0] [--max-fee 0]
 *
 * Env:
 * - AO_RPC_URL | SYNDICA_RPC | ANCHOR_PROVIDER_URL (RPC)
 * - ANCHOR_WALLET (payer keypair path)
 * - AO_PROGRAM_ID (defaults to GnGzNds...)
 */

import {
  ExtensionType,
  TOKEN_2022_PROGRAM_ID,
  createInitializeTransferFeeConfigInstruction,
  getExtensionTypes,
  getMintLen,
  unpackMint,
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

function loadKeypair(keypairPath: string): Keypair {
  const resolved = keypairPath.replace('~', process.env.HOME || '');
  return Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(resolved, 'utf8'))));
}

function parseArg(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  if (i === -1) return undefined;
  return process.argv[i + 1];
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error(
      'Usage: ts-node scripts/init-transfer-fee-config.ts <MINT> [--bps 0] [--max-fee 0]',
    );
    process.exit(1);
  }

  const mint = new PublicKey(args[0]);
  const bps = Number.parseInt(parseArg('--bps') || '0', 10);
  const maxFee = BigInt(parseArg('--max-fee') || '0');

  if (!Number.isFinite(bps) || bps < 0 || bps > 10_000) {
    throw new Error(`Invalid --bps ${bps} (expected 0..10000)`);
  }
  if (maxFee < 0n) {
    throw new Error(`Invalid --max-fee ${maxFee} (expected >= 0)`);
  }

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

  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    AO_PROGRAM_ID,
  );

  const mintAi = await connection.getAccountInfo(mint, 'confirmed');
  if (!mintAi) throw new Error(`Mint not found: ${mint.toBase58()}`);
  if (!mintAi.owner.equals(TOKEN_2022_PROGRAM_ID)) {
    throw new Error(
      `Mint owner is ${mintAi.owner.toBase58()} (expected Token-2022 ${TOKEN_2022_PROGRAM_ID.toBase58()})`,
    );
  }

  const mintState = unpackMint(mint, mintAi, TOKEN_2022_PROGRAM_ID);
  if (!mintState.mintAuthority) {
    throw new Error('Mint has no mintAuthority (unexpected for this workflow)');
  }
  if (!mintState.mintAuthority.equals(payer.publicKey)) {
    throw new Error(
      `Mint authority ${mintState.mintAuthority.toBase58()} != payer ${payer.publicKey.toBase58()}`,
    );
  }

  const existingExtTypes = getExtensionTypes(mintState.tlvData) as ExtensionType[];
  const hasTf = existingExtTypes.includes(ExtensionType.TransferFeeConfig);
  const neededLenIfAdded = getMintLen([...existingExtTypes, ExtensionType.TransferFeeConfig]);

  console.log('\n=== Init TransferFeeConfig (Token-2022) ===');
  console.log(`RPC:           ${rpc}`);
  console.log(`Mint:          ${mint.toBase58()}`);
  console.log(`Payer:         ${payer.publicKey.toBase58()}`);
  console.log(`ProtocolState: ${protocolState.toBase58()}`);
  console.log(`Decimals:      ${mintState.decimals}`);
  console.log(`Data len:      ${mintAi.data.length}`);
  console.log(`Existing ext:  ${existingExtTypes.join(', ') || '(none)'}`);
  console.log(`Target:        bps=${bps}, maxFee=${maxFee.toString()}`);

  if (!hasTf && mintAi.data.length < neededLenIfAdded) {
    throw new Error(
      [
        'Mint does not have TransferFeeConfig space allocated.',
        `Current data len: ${mintAi.data.length}`,
        `Needed data len:  ${neededLenIfAdded}`,
        '',
        'Token-2022 only supports reallocate for *token accounts* (not mints), so you cannot add new mint extensions post-creation.',
        'To use TransferFeeConfig withheld fees, you need a new mint created with TransferFeeConfig included from day 0 (mint migration).',
      ].join('\n'),
    );
  }

  if (hasTf) {
    console.log('ℹ TransferFeeConfig already present; nothing to do.');
    return;
  }

  const initTfIx = createInitializeTransferFeeConfigInstruction(
    mint,
    payer.publicKey, // transferFeeConfigAuthority (signs future updates)
    protocolState, // withdrawWithheldAuthority (AO PDA)
    bps,
    maxFee,
    TOKEN_2022_PROGRAM_ID,
  );

  const tx = new Transaction()
    .add(ComputeBudgetProgram.setComputeUnitLimit({ units: 250_000 }))
    .add(ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }))
    .add(initTfIx);

  const sig = await sendAndConfirmTransaction(connection, tx, [payer], {
    commitment: 'confirmed',
    skipPreflight: false,
  });

  console.log(`✅ Done. Signature: ${sig}`);
}

main().catch((err) => {
  console.error('\n❌ Error:', err);
  process.exit(1);
});
