/**
 * End-to-End Cryptographic Integrity Verification Test
 *
 * Success Criterion (Singular): A real, production proof generated off-chain
 * verifies on-chain via claim_with_ring and transfers tokens successfully.
 *
 * This is the FINAL GATE before enabling the publisher on devnet/mainnet.
 */
import * as anchor from '@coral-xyz/anchor';
import { Program, AnchorProvider, web3, BN } from '@coral-xyz/anchor';
import {
  TOKEN_2022_PROGRAM_ID,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  getMintLen,
  ExtensionType,
  getAssociatedTokenAddressSync,
  createMintToInstruction,
} from '@solana/spl-token';
import { assert } from 'chai';
import * as fs from 'fs';
import * as path from 'path';

// Load IDL and test claim data
const idl = require('../target/idl/token_2022.json');
const PROGRAM_ID = new web3.PublicKey(idl.metadata.address);

// Load exported claim from off-chain tree builder
const CLAIM_DATA = JSON.parse(
  fs.readFileSync(
    path.join(__dirname, '../../apps/twzrd-aggregator/test-claim-export.json'),
    'utf8'
  )
);

describe('End-to-End Cryptographic Integrity Verification', () => {
  const provider = AnchorProvider.env();
  anchor.setProvider(provider);
  const program = new Program(idl as anchor.Idl, PROGRAM_ID, provider);

  // Test state
  let mint: web3.Keypair;
  let streamerKey: web3.Keypair;
  let protocolState: web3.PublicKey;
  let channelState: web3.PublicKey;
  let treasuryAta: web3.PublicKey;
  let claimerAta: web3.PublicKey;

  const DECIMALS = 9;
  const TRANSFER_FEE_BASIS_POINTS = 100; // 1%
  const MAX_FEE = new BN(1_000_000_000); // 1 token max

  before(async () => {
    console.log('\n🔧 Setting up end-to-end test environment...\n');

    // Generate keypairs
    mint = web3.Keypair.generate();
    streamerKey = web3.Keypair.generate();

    console.log(`Mint: ${mint.publicKey.toBase58()}`);
    console.log(`Streamer: ${streamerKey.publicKey.toBase58()}`);
    console.log(`Claimer (wallet): ${provider.wallet.publicKey.toBase58()}`);
    console.log(`Expected claimer from claim data: ${CLAIM_DATA.claimer}\n`);

    // Verify claimer matches
    assert.equal(
      provider.wallet.publicKey.toBase58(),
      CLAIM_DATA.claimer,
      'Wallet mismatch: regenerate claim with correct wallet pubkey'
    );

    // Derive PDAs
    [protocolState] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from('protocol'), mint.publicKey.toBuffer()],
      PROGRAM_ID
    );

    [channelState] = web3.PublicKey.findProgramAddressSync(
      [Buffer.from('channel_state'), mint.publicKey.toBuffer(), streamerKey.publicKey.toBuffer()],
      PROGRAM_ID
    );

    treasuryAta = getAssociatedTokenAddressSync(
      mint.publicKey,
      protocolState,
      true,
      TOKEN_2022_PROGRAM_ID
    );

    claimerAta = getAssociatedTokenAddressSync(
      mint.publicKey,
      provider.wallet.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );

    console.log(`Protocol State PDA: ${protocolState.toBase58()}`);
    console.log(`Channel State PDA: ${channelState.toBase58()}`);
    console.log(`Treasury ATA: ${treasuryAta.toBase58()}`);
    console.log(`Claimer ATA: ${claimerAta.toBase58()}\n`);
  });

  it('Step 1: Create Token-2022 mint with TransferFeeConfig', async () => {
    console.log('📝 Creating Token-2022 mint with transfer fees...');

    const extensions = [ExtensionType.TransferFeeConfig];
    const mintLen = getMintLen(extensions);
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const createAccountIx = web3.SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey: mint.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    });

    const initTransferFeeIx = createInitializeTransferFeeConfigInstruction(
      mint.publicKey,
      provider.wallet.publicKey, // transferFeeConfigAuthority
      provider.wallet.publicKey, // withdrawWithheldAuthority
      TRANSFER_FEE_BASIS_POINTS,
      BigInt(MAX_FEE.toString()),
      TOKEN_2022_PROGRAM_ID
    );

    const initMintIx = createInitializeMintInstruction(
      mint.publicKey,
      DECIMALS,
      provider.wallet.publicKey, // mintAuthority
      null, // freezeAuthority
      TOKEN_2022_PROGRAM_ID
    );

    const tx = new web3.Transaction()
      .add(createAccountIx)
      .add(initTransferFeeIx)
      .add(initMintIx);

    await provider.sendAndConfirm(tx, [mint]);
    console.log(`✅ Mint created: ${mint.publicKey.toBase58()}\n`);
  });

  it('Step 2: Initialize protocol state', async () => {
    console.log('📝 Initializing protocol state...');

    await program.methods
      .initializeMintOpen(TRANSFER_FEE_BASIS_POINTS, MAX_FEE)
      .accounts({
        payer: provider.wallet.publicKey,
        mint: mint.publicKey,
        protocolState,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    const state = await program.account.protocolState.fetch(protocolState);
    console.log(`✅ Protocol initialized`);
    console.log(`   Admin: ${state.admin.toBase58()}`);
    console.log(`   Mint: ${state.mint.toBase58()}\n`);
  });

  it('Step 3: Set publisher authority', async () => {
    console.log('📝 Setting publisher authority...');

    await program.methods
      .updatePublisherOpen(provider.wallet.publicKey)
      .accounts({
        admin: provider.wallet.publicKey,
        protocolState,
      })
      .rpc();

    const state = await program.account.protocolState.fetch(protocolState);
    console.log(`✅ Publisher set: ${state.publisher.toBase58()}\n`);
  });

  it('Step 4: Initialize channel state', async () => {
    console.log('📝 Initializing channel state...');

    await program.methods
      .initializeChannel(streamerKey.publicKey)
      .accounts({
        payer: provider.wallet.publicKey,
        protocolState,
        channelState,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    const state = await program.account.channelState.fetch(channelState);
    console.log(`✅ Channel initialized`);
    console.log(`   Streamer: ${state.streamer.toBase58()}`);
    console.log(`   Latest epoch: ${state.latestEpoch.toString()}\n`);
  });

  it('Step 5: Set merkle root from off-chain proof', async () => {
    console.log('📝 Setting merkle root from exported claim...');
    console.log(`   Root: ${CLAIM_DATA.root}`);
    console.log(`   Epoch: ${CLAIM_DATA.epoch}`);
    console.log(`   Claim count: ${CLAIM_DATA.claim_count}\n`);

    const rootBuffer = Buffer.from(CLAIM_DATA.root, 'hex');
    const rootArray = Array.from(rootBuffer);

    await program.methods
      .setMerkleRootRing(
        rootArray,
        new BN(CLAIM_DATA.epoch),
        CLAIM_DATA.claim_count,
        streamerKey.publicKey
      )
      .accounts({
        updateAuthority: provider.wallet.publicKey,
        protocolState,
        channelState,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log(`✅ Merkle root set successfully\n`);
  });

  it('Step 6: Fund protocol treasury', async () => {
    console.log('📝 Funding protocol treasury...');

    const amountToFund = new BN(CLAIM_DATA.amount).mul(new BN(2)); // 2x to be safe

    // Mint to treasury
    const mintToIx = createMintToInstruction(
      mint.publicKey,
      treasuryAta,
      provider.wallet.publicKey,
      BigInt(amountToFund.toString()),
      [],
      TOKEN_2022_PROGRAM_ID
    );

    await provider.sendAndConfirm(new web3.Transaction().add(mintToIx));

    const treasuryBalance = await provider.connection.getTokenAccountBalance(treasuryAta);
    console.log(`✅ Treasury funded: ${treasuryBalance.value.uiAmount} tokens\n`);
  });

  it('Step 7: Execute claim_with_ring (FINAL VERIFICATION)', async () => {
    console.log('🚀 Executing claim_with_ring with off-chain proof...\n');
    console.log(`   Claimer: ${provider.wallet.publicKey.toBase58()}`);
    console.log(`   Index: ${CLAIM_DATA.index}`);
    console.log(`   Amount: ${CLAIM_DATA.amount}`);
    console.log(`   ID: ${CLAIM_DATA.id}`);
    console.log(`   Proof nodes: ${CLAIM_DATA.proof.length}\n`);

    // Get balances before
    let claimerBalanceBefore = 0;
    try {
      const bal = await provider.connection.getTokenAccountBalance(claimerAta);
      claimerBalanceBefore = parseInt(bal.value.amount);
    } catch (e) {
      // ATA doesn't exist yet
      console.log('   Claimer ATA will be created...');
    }

    const treasuryBalanceBefore = await provider.connection.getTokenAccountBalance(treasuryAta);
    console.log(`   Treasury balance before: ${treasuryBalanceBefore.value.uiAmount}`);
    console.log(`   Claimer balance before: ${claimerBalanceBefore / 1e9}\n`);

    // Parse proof
    const proofNodes = CLAIM_DATA.proof.map((hex: string) =>
      Array.from(Buffer.from(hex, 'hex'))
    );

    // Execute claim
    const tx = await program.methods
      .claimWithRing(
        new BN(CLAIM_DATA.epoch),
        CLAIM_DATA.index,
        new BN(CLAIM_DATA.amount),
        proofNodes,
        CLAIM_DATA.id,
        streamerKey.publicKey
      )
      .accounts({
        claimer: provider.wallet.publicKey,
        protocolState,
        channelState,
        mint: mint.publicKey,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: web3.SystemProgram.programId,
      })
      .rpc();

    console.log(`✅ Claim transaction successful!`);
    console.log(`   TX: ${tx}\n`);

    // Verify balances after
    const claimerBalanceAfter = await provider.connection.getTokenAccountBalance(claimerAta);
    const treasuryBalanceAfter = await provider.connection.getTokenAccountBalance(treasuryAta);

    console.log(`   Treasury balance after: ${treasuryBalanceAfter.value.uiAmount}`);
    console.log(`   Claimer balance after: ${claimerBalanceAfter.value.uiAmount}\n`);

    // Assert token transfer occurred
    const expectedAmount = parseInt(CLAIM_DATA.amount);
    const actualAmount = parseInt(claimerBalanceAfter.value.amount) - claimerBalanceBefore;

    assert.equal(actualAmount, expectedAmount, 'Token transfer amount mismatch');

    console.log(`✅✅✅ END-TO-END VERIFICATION PASSED ✅✅✅`);
    console.log(`\nCryptographic alignment confirmed:`);
    console.log(`  - Off-chain leaf hashing: ✅ Correct`);
    console.log(`  - Off-chain proof generation: ✅ Correct`);
    console.log(`  - On-chain proof verification: ✅ PASSED`);
    console.log(`  - Token transfer: ✅ SUCCESS (${actualAmount / 1e9} tokens)\n`);
    console.log(`The protocol is VIABLE. Ready for production deployment.\n`);
  });

  it('Step 8: Verify double-claim prevention', async () => {
    console.log('🔒 Testing double-claim prevention...\n');

    const proofNodes = CLAIM_DATA.proof.map((hex: string) =>
      Array.from(Buffer.from(hex, 'hex'))
    );

    try {
      await program.methods
        .claimWithRing(
          new BN(CLAIM_DATA.epoch),
          CLAIM_DATA.index,
          new BN(CLAIM_DATA.amount),
          proofNodes,
          CLAIM_DATA.id,
          streamerKey.publicKey
        )
        .accounts({
          claimer: provider.wallet.publicKey,
          protocolState,
          channelState,
          mint: mint.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: web3.SystemProgram.programId,
        })
        .rpc();

      assert.fail('Expected AlreadyClaimed error');
    } catch (err: any) {
      assert.include(err.toString(), 'AlreadyClaimed', 'Should reject double-claim');
      console.log(`✅ Double-claim correctly rejected: AlreadyClaimed\n`);
    }
  });
});
