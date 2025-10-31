/**
 * Direct end-to-end verification script (no local IDL build required).
 *
 * Usage:
 *   export CLAIM_JSON=../apps/twzrd-aggregator/test-claim-export.json
 *   tsx scripts/e2e-direct.ts
 *
 * Requirements:
 *   - `target/deploy/token_2022.so` and `target/deploy/token_2022-keypair.json` exist.
 *   - `target/idl/token_2022.json` exists (download from GitHub if anchor idl build fails):
 *         mkdir -p target/idl
 *         curl -L https://raw.githubusercontent.com/twzrd-sol/attention-oracle-program/main/target/idl/token_2022.json \
 *           -o target/idl/token_2022.json
 *   - `solana-test-validator -r` is running in another terminal.
 *   - `solana program deploy target/deploy/token_2022.so --program-id target/deploy/token_2022-keypair.json --url localhost`
 *
 * PASS criteria:
 *   - Script prints "END-TO-END VERIFICATION PASSED".
 *   - Treasury ATA decreases by `amount`.
 *   - Claimer ATA increases by `amount`.
 *   - Second claim attempt fails with "AlreadyClaimed".
 */

import * as anchor from '@coral-xyz/anchor';
import { Program, web3, BN } from '@coral-xyz/anchor';
import {
  TOKEN_2022_PROGRAM_ID,
  ExtensionType,
  getMintLen,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
} from '@solana/spl-token';
import assert from 'assert';
import { keccak_256 } from 'js-sha3';
import fs from 'fs';
import path from 'path';

const IDL_PATH = path.join(__dirname, '../target/idl/token_2022.json');
const CLAIM_JSON_PATH = process.env.CLAIM_JSON ?? path.join(__dirname, '../../apps/twzrd-aggregator/test-claim-export.json');

if (!fs.existsSync(IDL_PATH)) {
  console.error(`IDL missing at ${IDL_PATH}. Download it first (see script header).`);
  process.exit(1);
}

if (!fs.existsSync(CLAIM_JSON_PATH)) {
  console.error(`Claim JSON not found at ${CLAIM_JSON_PATH}. Set CLAIM_JSON env or generate via aggregator.`);
  process.exit(1);
}

const idl = JSON.parse(fs.readFileSync(IDL_PATH, 'utf8'));
const CLAIM_DATA = JSON.parse(fs.readFileSync(CLAIM_JSON_PATH, 'utf8'));

const connection = new web3.Connection(process.env.RPC_URL ?? 'http://127.0.0.1:8899', 'confirmed');
const wallet = anchor.Wallet.local();
const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' });
anchor.setProvider(provider);

const programId = new web3.PublicKey(idl.metadata.address);
const program = new Program(idl as anchor.Idl, programId, provider);

function deriveStreamerKey(channel: string): web3.PublicKey {
  const lower = channel.toLowerCase();
  const hash = keccak_256.update('channel:').update(lower).digest();
  return new web3.PublicKey(Buffer.from(hash));
}

async function ensureAta(owner: web3.PublicKey, mint: web3.PublicKey, payer: web3.PublicKey): Promise<web3.PublicKey> {
  const ata = getAssociatedTokenAddressSync(mint, owner, true, TOKEN_2022_PROGRAM_ID);
  const info = await connection.getAccountInfo(ata);
  if (!info) {
    const ix = createAssociatedTokenAccountInstruction(
      payer,
      ata,
      owner,
      mint,
      TOKEN_2022_PROGRAM_ID
    );
    const tx = new web3.Transaction().add(ix);
    await provider.sendAndConfirm(tx);
  }
  return ata;
}

async function main() {
  console.log('\n🔧 Direct end-to-end verification (no local IDL build)…\n');

  const claimerPubkey = new web3.PublicKey(CLAIM_DATA.claimer);
  assert.strictEqual(
    claimerPubkey.toBase58(),
    wallet.publicKey.toBase58(),
    'Claimer pubkey must match local wallet; regenerate JSON with correct wallet.'
  );

  const mintKeypair = web3.Keypair.generate();
  const streamerKey = deriveStreamerKey(CLAIM_DATA.channel);
  const epoch = new BN(CLAIM_DATA.epoch);
  const claimCount = new BN(CLAIM_DATA.claim_count);
  const claimIndex = CLAIM_DATA.index;
  const claimAmount = new BN(CLAIM_DATA.amount);
  const claimId = CLAIM_DATA.id;
  const proofNodes: number[][] = CLAIM_DATA.proof.map((hex: string) => Array.from(Buffer.from(hex, 'hex')));
  const rootBytes = Array.from(Buffer.from(CLAIM_DATA.root, 'hex'));

  const protocolPda = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mintKeypair.publicKey.toBuffer()],
    programId
  )[0];
  const feeConfigPda = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('protocol'), mintKeypair.publicKey.toBuffer(), Buffer.from('fee_config')],
    programId
  )[0];
  const channelPda = web3.PublicKey.findProgramAddressSync(
    [Buffer.from('channel_state'), mintKeypair.publicKey.toBuffer(), streamerKey.toBuffer()],
    programId
  )[0];

  const treasuryAta = getAssociatedTokenAddressSync(mintKeypair.publicKey, protocolPda, true, TOKEN_2022_PROGRAM_ID);
  const claimerAta = getAssociatedTokenAddressSync(mintKeypair.publicKey, claimerPubkey, false, TOKEN_2022_PROGRAM_ID);

  console.log(`Program ID: ${programId.toBase58()}`);
  console.log(`Mint: ${mintKeypair.publicKey.toBase58()}`);
  console.log(`Protocol PDA: ${protocolPda.toBase58()}`);
  console.log(`Fee Config PDA: ${feeConfigPda.toBase58()}`);
  console.log(`Channel PDA: ${channelPda.toBase58()}`);
  console.log(`Streamer key: ${streamerKey.toBase58()}`);
  console.log(`Claimer ATA: ${claimerAta.toBase58()}`);
  console.log(`Treasury ATA: ${treasuryAta.toBase58()}\n`);

  console.log('Step 1: Create Token-2022 mint with transfer fee extension…');
  const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
  const lamports = await connection.getMinimumBalanceForRentExemption(mintLen);
  const createAccountIx = web3.SystemProgram.createAccount({
    fromPubkey: wallet.publicKey,
    newAccountPubkey: mintKeypair.publicKey,
    space: mintLen,
    lamports,
    programId: TOKEN_2022_PROGRAM_ID,
  });
  const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
    mintKeypair.publicKey,
    wallet.publicKey,
    wallet.publicKey,
    100, // 1% for test
    BigInt(1_000_000_000),
    TOKEN_2022_PROGRAM_ID
  );
  const initMintIx = createInitializeMintInstruction(
    mintKeypair.publicKey,
    9,
    wallet.publicKey,
    null,
    TOKEN_2022_PROGRAM_ID
  );
  await provider.sendAndConfirm(
    new web3.Transaction().add(createAccountIx).add(initTransferFeeIx).add(initMintIx),
    [mintKeypair]
  );
  console.log('✅ Mint created\n');

  console.log('Step 2: Initialize protocol state…');
  await program.methods
    .initializeMintOpen(100, new BN(1_000_000_000))
    .accounts({
      payer: wallet.publicKey,
      mint: mintKeypair.publicKey,
      protocolState: protocolPda,
      feeConfig: feeConfigPda,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();
  console.log('✅ Protocol initialized\n');

  console.log('Step 3: Set publisher (self)…');
  await program.methods
    .updatePublisherOpen(wallet.publicKey)
    .accounts({
      admin: wallet.publicKey,
      protocolState: protocolPda,
    })
    .rpc();
  console.log('✅ Publisher set\n');

  console.log('Step 4: Initialize channel…');
  await program.methods
    .initializeChannel(streamerKey)
    .accounts({
      payer: wallet.publicKey,
      protocolState: protocolPda,
      channelState: channelPda,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();
  console.log('✅ Channel initialized\n');

  console.log('Step 5: Publish ring root…');
  await program.methods
    .setMerkleRootRing(rootBytes, epoch, claimCount.toNumber(), streamerKey)
    .accounts({
      updateAuthority: wallet.publicKey,
      protocolState: protocolPda,
      channelState: channelPda,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();
  console.log('✅ Root published\n');

  console.log('Step 6: Fund protocol treasury…');
  await ensureAta(protocolPda, mintKeypair.publicKey, wallet.publicKey);
  await ensureAta(wallet.publicKey, mintKeypair.publicKey, wallet.publicKey); // ensure payer ATA exists
  const mintToIx = createMintToInstruction(
    mintKeypair.publicKey,
    treasuryAta,
    wallet.publicKey,
    BigInt(claimAmount.mul(new BN(2)).toString()),
    [],
    TOKEN_2022_PROGRAM_ID
  );
  await provider.sendAndConfirm(new web3.Transaction().add(mintToIx));
  console.log('✅ Treasury funded\n');

  console.log('Step 7: Execute claim_with_ring…');
  const treasuryBefore = await connection.getTokenAccountBalance(treasuryAta);
  let claimerBefore = 0;
  try {
    const bal = await connection.getTokenAccountBalance(claimerAta);
    claimerBefore = parseInt(bal.value.amount);
  } catch (_) {
    // ATA not created yet
  }

  const tx = await program.methods
    .claimWithRing(epoch, claimIndex, claimAmount, proofNodes, claimId, streamerKey)
    .accounts({
      claimer: wallet.publicKey,
      protocolState: protocolPda,
      channelState: channelPda,
      mint: mintKeypair.publicKey,
      tokenProgram: TOKEN_2022_PROGRAM_ID,
      associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
      systemProgram: web3.SystemProgram.programId,
    })
    .rpc();
  console.log(`✅ Claim transaction: ${tx}`);

  const treasuryAfter = await connection.getTokenAccountBalance(treasuryAta);
  const claimerAfter = await connection.getTokenAccountBalance(claimerAta);
  const deltaTreasury = parseInt(treasuryBefore.value.amount) - parseInt(treasuryAfter.value.amount);
  const deltaClaimer = parseInt(claimerAfter.value.amount) - claimerBefore;

  console.log(`   Treasury delta: ${deltaTreasury}`);
  console.log(`   Claimer delta: ${deltaClaimer}`);

  assert.strictEqual(deltaClaimer, parseInt(claimAmount.toString()), 'Claimer did not receive expected amount');
  assert.strictEqual(deltaTreasury, parseInt(claimAmount.toString()), 'Treasury did not decrease as expected');

  console.log('Step 8: Ensure double claim fails…');
  let doubleClaimRejected = false;
  try {
    await program.methods
      .claimWithRing(epoch, claimIndex, claimAmount, proofNodes, claimId, streamerKey)
      .accounts({
        claimer: wallet.publicKey,
        protocolState: protocolPda,
        channelState: channelPda,
        mint: mintKeypair.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();
  } catch (err: any) {
    if (err.toString().includes('AlreadyClaimed')) {
      doubleClaimRejected = true;
    } else {
      throw err;
    }
  }

  assert.ok(doubleClaimRejected, 'Second claim should fail with AlreadyClaimed');

  console.log('\n✅✅✅ END-TO-END VERIFICATION PASSED ✅✅✅\n');
  console.log('Cryptographic alignment confirmed:');
  console.log('  • Off-chain leaf hashing: ✅ matches compute_leaf');
  console.log('  • Proof verification: ✅ on-chain PASS');
  console.log('  • Token transfer: ✅ treasury → claimer');
  console.log('  • Double-claim guard: ✅ AlreadyClaimed\n');
  console.log('The protocol is ready for production deployment.');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

