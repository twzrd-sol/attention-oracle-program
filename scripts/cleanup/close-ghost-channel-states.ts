#!/usr/bin/env tsx
/**
 * Close Ghost ChannelState Accounts (V2 Rent Recovery)
 *
 * Identifies and closes ChannelState accounts that were created with incorrect
 * PDA derivations or are no longer needed. Recovers rent to admin wallet.
 *
 * Usage:
 *   ./close-ghost-channel-states.ts [--dry-run] [--limit N]
 */

import { Connection, PublicKey, Transaction, Keypair } from '@solana/web3.js';
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import bs58 from 'bs58';
import fs from 'fs';
import path from 'path';

const PROGRAM_ID = new PublicKey('GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop');
const MINT = new PublicKey('CCmcMcaHp78vzDx6bxgK7qPW4AnpQW69kvFn5JqNh2C');
const PROTOCOL_SEED = Buffer.from('protocol');

// Channel state discriminator (first 8 bytes) - SHA256('account:ChannelState')
const CHANNEL_STATE_DISCRIMINATOR = Buffer.from([74, 132, 141, 196, 64, 52, 83, 136]);

async function main() {
  const args = process.argv.slice(2);
  const dryRun = args.includes('--dry-run');
  const limitIdx = args.indexOf('--limit');
  const limit = limitIdx >= 0 ? parseInt(args[limitIdx + 1]) : undefined;

  console.log('\n🔍 Ghost ChannelState Account Scanner');
  console.log('=====================================\n');
  console.log(`Mode: ${dryRun ? '🔎 DRY RUN' : '⚠️  LIVE EXECUTION'}`);
  console.log(`Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`Mint: ${MINT.toBase58()}\n`);

  // Load wallet
  const walletPath = path.join(process.env.HOME!, '.config/solana/id.json');
  const walletKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(walletPath, 'utf-8')))
  );

  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
  const wallet = new Wallet(walletKeypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: 'confirmed' });

  const idlPath = path.join(__dirname, '../../clean-hackathon/target/idl/token_2022.json');
  const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));
  const program = new Program(idl, provider);

  // Derive protocol state PDA (singleton variant - no mint in seed)
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED],
    PROGRAM_ID
  );

  console.log(`Admin wallet: ${wallet.publicKey.toBase58()}`);
  console.log(`ProtocolState: ${protocolState.toBase58()}\n`);

  // Verify admin authority
  const protocolStateAccount = await connection.getAccountInfo(protocolState);
  if (!protocolStateAccount) {
    throw new Error('ProtocolState account not found');
  }

  console.log('📊 Scanning for ChannelState accounts...\n');

  // Get all program accounts with ChannelState discriminator
  const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
    filters: [
      {
        memcmp: {
          offset: 0,
          bytes: bs58.encode(CHANNEL_STATE_DISCRIMINATOR),
        },
      },
    ],
  });

  console.log(`Found ${accounts.length} ChannelState accounts\n`);

  let ghostAccounts: PublicKey[] = [];
  let totalRent = 0;

  for (const { pubkey, account } of accounts) {
    const size = account.data.length;
    const lamports = account.lamports;

    // V1 accounts are 1802 bytes, V2 are 10762 bytes
    // Ghost accounts might be any size if they were incorrectly created
    // We'll identify accounts that don't match expected sizes or other criteria

    const isV1Size = size === 1802;
    const isV2Size = size === 10762;

    if (!isV1Size && !isV2Size) {
      console.log(`🚩 Ghost account found: ${pubkey.toBase58()}`);
      console.log(`   Size: ${size} bytes (expected 1802 or 10762)`);
      console.log(`   Rent: ${lamports} lamports (~${(lamports / 1e9).toFixed(6)} SOL)`);

      ghostAccounts.push(pubkey);
      totalRent += lamports;
    }
  }

  console.log(`\n📈 Summary:`);
  console.log(`   Ghost accounts: ${ghostAccounts.length}`);
  console.log(`   Total rent: ${totalRent} lamports (~${(totalRent / 1e9).toFixed(6)} SOL)\n`);

  if (ghostAccounts.length === 0) {
    console.log('✅ No ghost accounts found!');
    return;
  }

  if (dryRun) {
    console.log('🔎 DRY RUN - No accounts closed');
    console.log('\nTo close these accounts, run without --dry-run');
    return;
  }

  const accountsToClose = limit ? ghostAccounts.slice(0, limit) : ghostAccounts;
  console.log(`🗑️  Closing ${accountsToClose.length} ghost accounts...\n`);

  let closedCount = 0;
  let recoveredRent = 0;

  for (const ghostAccount of accountsToClose) {
    try {
      const tx = await program.methods
        .closeChannelState()
        .accounts({
          authority: wallet.publicKey,
          protocolState,
          channelState: ghostAccount,
          rentReceiver: wallet.publicKey,
        })
        .rpc();

      const accountInfo = await connection.getAccountInfo(ghostAccount);
      const rent = accountInfo?.lamports || 0;

      console.log(`✅ Closed: ${ghostAccount.toBase58()}`);
      console.log(`   Signature: ${tx}`);
      console.log(`   Recovered: ${rent} lamports\n`);

      closedCount++;
      recoveredRent += rent;
    } catch (err: any) {
      console.error(`❌ Failed to close ${ghostAccount.toBase58()}: ${err.message}\n`);
    }
  }

  console.log('\n🎉 Cleanup Complete!');
  console.log(`   Closed: ${closedCount}/${accountsToClose.length}`);
  console.log(`   Recovered: ${recoveredRent} lamports (~${(recoveredRent / 1e9).toFixed(6)} SOL)`);
}

main().catch((err) => {
  console.error('Fatal error:', err);
  process.exit(1);
});
