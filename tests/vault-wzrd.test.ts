/**
 * vLOFI WZRD Vault — Full Lifecycle Integration Test
 *
 * Self-contained Bankrun test that validates the vault program.
 * Oracle accounts are constructed manually (hardcoded admin blocks
 * Oracle init in test), then vault operations are tested via Anchor.
 *
 * Compound + complete-withdraw use Oracle CPI — tested on devnet.
 * This suite validates: init, deposit, pause/resume, sync, withdraw-request.
 */

import { describe, it, beforeAll, expect } from "vitest";
import { startAnchor, ProgramTestContext } from "solana-bankrun";
import { BankrunProvider } from "anchor-bankrun";
import { Program, BN, Idl } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createInitializeMintInstruction,
  createMintToInstruction,
  createAssociatedTokenAccountInstruction,
  getMintLen,
} from "@solana/spl-token";
import { readFileSync } from "fs";
import { keccak_256 } from "@noble/hashes/sha3";
import { createHash } from "crypto";
import path from "path";

// ---------------------------------------------------------------------------
// Program IDs
// ---------------------------------------------------------------------------
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");

// ---------------------------------------------------------------------------
// PDA Seeds
// ---------------------------------------------------------------------------
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const CHANNEL_USER_STAKE_SEED = Buffer.from("channel_user");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

const VAULT_SEED = Buffer.from("vault");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");
const USER_VAULT_STATE_SEED = Buffer.from("user_state");
const WITHDRAW_REQUEST_SEED = Buffer.from("withdraw");

// ---------------------------------------------------------------------------
// Test Constants
// ---------------------------------------------------------------------------
const CHANNEL_NAME = "wzrd";
const LOCK_DURATION_SLOTS = 54_000;
const WITHDRAW_QUEUE_SLOTS = 9_000;
const CCM_DECIMALS = 9;
const DEPOSIT_AMOUNT = new BN(10_000_000_000_000); // 10,000 CCM

// ---------------------------------------------------------------------------
// Account Discriminator Helper
// ---------------------------------------------------------------------------
function accountDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`account:${name}`).digest().subarray(0, 8);
}

// ---------------------------------------------------------------------------
// PDA Derivation
// ---------------------------------------------------------------------------
function deriveSubjectId(channel: string): Buffer {
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(channel.toLowerCase())]);
  return Buffer.from(keccak_256(input));
}

function deriveProtocolState(mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([PROTOCOL_SEED, mint.toBuffer()], ORACLE_PROGRAM_ID);
}

function deriveChannelConfig(mint: PublicKey, channel: string): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, mint.toBuffer(), deriveSubjectId(channel)],
    ORACLE_PROGRAM_ID
  );
}

function deriveStakePool(channelConfig: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CHANNEL_STAKE_POOL_SEED, channelConfig.toBuffer()], ORACLE_PROGRAM_ID);
}

function deriveStakeVault(stakePool: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([STAKE_VAULT_SEED, stakePool.toBuffer()], ORACLE_PROGRAM_ID);
}

function deriveUserStake(channelConfig: PublicKey, user: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CHANNEL_USER_STAKE_SEED, channelConfig.toBuffer(), user.toBuffer()], ORACLE_PROGRAM_ID);
}

function deriveVault(channelConfig: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VAULT_SEED, channelConfig.toBuffer()], VAULT_PROGRAM_ID);
}

function deriveVaultCcmBuffer(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VAULT_CCM_BUFFER_SEED, vault.toBuffer()], VAULT_PROGRAM_ID);
}

function deriveVlofiMint(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VLOFI_MINT_SEED, vault.toBuffer()], VAULT_PROGRAM_ID);
}

function deriveVaultOraclePosition(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([VAULT_ORACLE_POSITION_SEED, vault.toBuffer()], VAULT_PROGRAM_ID);
}

function deriveUserVaultState(vault: PublicKey, user: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([USER_VAULT_STATE_SEED, vault.toBuffer(), user.toBuffer()], VAULT_PROGRAM_ID);
}

function deriveWithdrawRequest(vault: PublicKey, user: PublicKey, id: BN): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [WITHDRAW_REQUEST_SEED, vault.toBuffer(), user.toBuffer(), id.toArrayLike(Buffer, "le", 8)],
    VAULT_PROGRAM_ID
  );
}

// ---------------------------------------------------------------------------
// Manual Account Constructors (Oracle accounts, bypassing Oracle init)
// ---------------------------------------------------------------------------

function buildProtocolState(
  bump: number,
  admin: PublicKey,
  publisher: PublicKey,
  treasury: PublicKey,
  mint: PublicKey,
): Buffer {
  const disc = accountDiscriminator("ProtocolState");
  const buf = Buffer.alloc(8 + 1 + 1 + 32 * 4 + 1 + 1 + 1); // 141 bytes
  let offset = 0;
  disc.copy(buf, offset); offset += 8;
  buf.writeUInt8(1, offset); offset += 1; // is_initialized
  buf.writeUInt8(1, offset); offset += 1; // version
  admin.toBuffer().copy(buf, offset); offset += 32;
  publisher.toBuffer().copy(buf, offset); offset += 32;
  treasury.toBuffer().copy(buf, offset); offset += 32;
  mint.toBuffer().copy(buf, offset); offset += 32;
  buf.writeUInt8(0, offset); offset += 1; // paused = false
  buf.writeUInt8(0, offset); offset += 1; // require_receipt = false
  buf.writeUInt8(bump, offset); offset += 1;
  return buf;
}

function buildChannelConfigV2(
  bump: number,
  mint: PublicKey,
  subject: PublicKey,
  authority: PublicKey,
  creatorWallet: PublicKey,
): Buffer {
  const disc = accountDiscriminator("ChannelConfigV2");
  // 8 disc + 1 version + 1 bump + 32*3 pubkeys + 8 seq + 8 epoch + 32 creator + 2 fee + 6 pad + 320 roots = 482
  const buf = Buffer.alloc(482);
  let offset = 0;
  disc.copy(buf, offset); offset += 8;
  buf.writeUInt8(2, offset); offset += 1; // version
  buf.writeUInt8(bump, offset); offset += 1;
  mint.toBuffer().copy(buf, offset); offset += 32;
  subject.toBuffer().copy(buf, offset); offset += 32;
  authority.toBuffer().copy(buf, offset); offset += 32;
  // latest_root_seq = 0
  offset += 8;
  // cutover_epoch = 0
  offset += 8;
  creatorWallet.toBuffer().copy(buf, offset); offset += 32;
  // creator_fee_bps = 0
  offset += 2;
  // _padding = [0; 6]
  offset += 6;
  // roots = [RootEntry::default(); 4] — zeros
  return buf;
}

function buildChannelStakePool(
  bump: number,
  channel: PublicKey,
  mint: PublicKey,
  vault: PublicKey,
): Buffer {
  const disc = accountDiscriminator("ChannelStakePool");
  // 8 disc + 1 bump + 32*3 + 8*5 + 16 + 1 = 162
  const buf = Buffer.alloc(162);
  let offset = 0;
  disc.copy(buf, offset); offset += 8;
  buf.writeUInt8(bump, offset); offset += 1;
  channel.toBuffer().copy(buf, offset); offset += 32;
  mint.toBuffer().copy(buf, offset); offset += 32;
  vault.toBuffer().copy(buf, offset); offset += 32;
  // total_staked, total_weighted, staker_count, acc_reward_per_share,
  // last_reward_slot, reward_per_slot = 0
  // is_shutdown = false (already zeroed)
  return buf;
}

function buildUserChannelStake(
  bump: number,
  user: PublicKey,
  channel: PublicKey,
  amount: BN,
  lockEndSlot: BN,
  nftMint: PublicKey,
): Buffer {
  const disc = accountDiscriminator("UserChannelStake");
  // 8 + 1 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 16 + 8 = 161
  const buf = Buffer.alloc(161);
  let offset = 0;
  disc.copy(buf, offset); offset += 8;
  buf.writeUInt8(bump, offset); offset += 1;
  user.toBuffer().copy(buf, offset); offset += 32;
  channel.toBuffer().copy(buf, offset); offset += 32;
  amount.toArrayLike(Buffer, "le", 8).copy(buf, offset); offset += 8;
  // start_slot = 0
  offset += 8;
  lockEndSlot.toArrayLike(Buffer, "le", 8).copy(buf, offset); offset += 8;
  // multiplier_bps = 10000 (1.0x)
  new BN(10000).toArrayLike(Buffer, "le", 8).copy(buf, offset); offset += 8;
  nftMint.toBuffer().copy(buf, offset); offset += 32;
  // reward_debt (u128) = 0
  offset += 16;
  // pending_rewards = 0
  return buf;
}

// ---------------------------------------------------------------------------
// Vault State Parser
// ---------------------------------------------------------------------------
function parseVaultState(data: Buffer) {
  let offset = 8; // skip discriminator
  const bump = data.readUInt8(offset); offset += 1;
  const version = data.readUInt8(offset); offset += 1;
  const channelConfig = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const ccmMint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const vlofiMint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const ccmBuffer = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const totalStaked = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const totalShares = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const pendingDeposits = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const pendingWithdrawals = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const lastCompoundSlot = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const compoundCount = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const admin = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const minDeposit = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const paused = data.readUInt8(offset) === 1; offset += 1;
  const emergencyReserve = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const lockDurationSlots = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const withdrawQueueSlots = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  return {
    bump, version, channelConfig, ccmMint, vlofiMint, ccmBuffer,
    totalStaked, totalShares, pendingDeposits, pendingWithdrawals,
    lastCompoundSlot, compoundCount, admin, minDeposit, paused,
    emergencyReserve, lockDurationSlots, withdrawQueueSlots,
  };
}

function parseVaultOraclePosition(data: Buffer) {
  let offset = 8;
  const bump = data.readUInt8(offset); offset += 1;
  const vault = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const oracleUserStake = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const oracleNftMint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const oracleNftAta = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const isActive = data.readUInt8(offset) === 1; offset += 1;
  const stakeAmount = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  const lockEndSlot = new BN(data.subarray(offset, offset + 8), "le"); offset += 8;
  return { bump, vault, oracleUserStake, oracleNftMint, oracleNftAta, isActive, stakeAmount, lockEndSlot };
}

// ---------------------------------------------------------------------------
// IDL Loader
// ---------------------------------------------------------------------------
function loadIdl(name: string): Idl {
  return JSON.parse(readFileSync(path.join(__dirname, `../target/idl/${name}.json`), "utf-8"));
}

// ---------------------------------------------------------------------------
// Test Suite
// ---------------------------------------------------------------------------
describe("vLOFI WZRD Vault Lifecycle", () => {
  let context: ProgramTestContext;
  let provider: BankrunProvider;
  let vaultProgram: Program;
  let payer: Keypair;
  let ccmMint: Keypair;

  let protocolState: PublicKey;
  let channelConfig: PublicKey;
  let stakePool: PublicKey;
  let stakeVaultAddr: PublicKey;
  let vault: PublicKey;
  let vlofiMint: PublicKey;
  let ccmBuffer: PublicKey;
  let vaultOraclePosition: PublicKey;
  let userCcmAta: PublicKey;
  let userVlofiAta: PublicKey;

  // ---------------------------------------------------------------------------
  // Setup
  // ---------------------------------------------------------------------------
  beforeAll(async () => {
    context = await startAnchor(".", [], []);
    provider = new BankrunProvider(context);
    payer = context.payer;
    vaultProgram = new Program(loadIdl("channel_vault"), provider);

    // --- Create Token-2022 mint ---
    ccmMint = Keypair.generate();
    const mintLen = getMintLen([]);
    const mintRent = await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const createMintTx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: ccmMint.publicKey,
        space: mintLen,
        lamports: mintRent,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeMintInstruction(ccmMint.publicKey, CCM_DECIMALS, payer.publicKey, payer.publicKey, TOKEN_2022_PROGRAM_ID)
    );
    createMintTx.recentBlockhash = context.lastBlockhash;
    createMintTx.sign(payer, ccmMint);
    await context.banksClient.processTransaction(createMintTx);
    console.log("CCM mint created:", ccmMint.publicKey.toBase58());

    // --- Derive PDAs ---
    const [ps, psBump] = deriveProtocolState(ccmMint.publicKey);
    protocolState = ps;
    const [cc, ccBump] = deriveChannelConfig(ccmMint.publicKey, CHANNEL_NAME);
    channelConfig = cc;
    const [sp, spBump] = deriveStakePool(channelConfig);
    stakePool = sp;
    const [sv] = deriveStakeVault(stakePool);
    stakeVaultAddr = sv;
    [vault] = deriveVault(channelConfig);
    [vlofiMint] = deriveVlofiMint(vault);
    [ccmBuffer] = deriveVaultCcmBuffer(vault);
    [vaultOraclePosition] = deriveVaultOraclePosition(vault);

    // --- Manually construct Oracle accounts ---
    const subjectId = new PublicKey(deriveSubjectId(CHANNEL_NAME));

    const protocolData = buildProtocolState(psBump, payer.publicKey, payer.publicKey, payer.publicKey, ccmMint.publicKey);
    context.setAccount(protocolState, {
      lamports: 10_000_000,
      data: protocolData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });
    console.log("Protocol state set:", protocolState.toBase58());

    const channelData = buildChannelConfigV2(ccBump, ccmMint.publicKey, subjectId, payer.publicKey, payer.publicKey);
    context.setAccount(channelConfig, {
      lamports: 10_000_000,
      data: channelData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });
    console.log("Channel config set:", channelConfig.toBase58());

    // Create stake vault token account (Token-2022 ATA owned by stakePool PDA)
    // For init, just need the stake pool account to exist
    const stakePoolData = buildChannelStakePool(spBump, channelConfig, ccmMint.publicKey, stakeVaultAddr);
    context.setAccount(stakePool, {
      lamports: 10_000_000,
      data: stakePoolData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });
    console.log("Stake pool set:", stakePool.toBase58());

    // --- Fund user with CCM ---
    userCcmAta = getAssociatedTokenAddressSync(ccmMint.publicKey, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);
    const fundTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(payer.publicKey, userCcmAta, payer.publicKey, ccmMint.publicKey, TOKEN_2022_PROGRAM_ID),
      createMintToInstruction(ccmMint.publicKey, userCcmAta, payer.publicKey, BigInt(DEPOSIT_AMOUNT.mul(new BN(10)).toString()), [], TOKEN_2022_PROGRAM_ID)
    );
    fundTx.recentBlockhash = context.lastBlockhash;
    fundTx.sign(payer);
    await context.banksClient.processTransaction(fundTx);
    console.log("User funded with CCM:", userCcmAta.toBase58());

    userVlofiAta = getAssociatedTokenAddressSync(vlofiMint, payer.publicKey, false, TOKEN_PROGRAM_ID);
    console.log("\n--- Setup complete ---");
  }, 120_000);

  // ---------------------------------------------------------------------------
  // Test 1: Initialize Vault
  // ---------------------------------------------------------------------------
  it("should initialize the wzrd vault", async () => {
    await vaultProgram.methods
      .initializeVault(new BN(1_000_000_000), new BN(LOCK_DURATION_SLOTS), new BN(WITHDRAW_QUEUE_SLOTS))
      .accounts({
        admin: payer.publicKey,
        oracleProtocol: protocolState,
        oracleChannelConfig: channelConfig,
        ccmMint: ccmMint.publicKey,
        vault: vault,
        ccmBuffer: ccmBuffer,
        vlofiMint: vlofiMint,
        vaultOraclePosition: vaultOraclePosition,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const acct = await context.banksClient.getAccount(vault);
    expect(acct).not.toBeNull();
    const state = parseVaultState(Buffer.from(acct!.data));
    expect(state.channelConfig.equals(channelConfig)).toBe(true);
    expect(state.admin.equals(payer.publicKey)).toBe(true);
    expect(state.totalStaked.toNumber()).toBe(0);
    expect(state.totalShares.toNumber()).toBe(0);
    expect(state.lockDurationSlots.toNumber()).toBe(LOCK_DURATION_SLOTS);
    expect(state.withdrawQueueSlots.toNumber()).toBe(WITHDRAW_QUEUE_SLOTS);
    console.log("Vault initialized:", vault.toBase58());
  });

  // ---------------------------------------------------------------------------
  // Test 2: Deposit CCM → vLOFI
  // ---------------------------------------------------------------------------
  it("should deposit CCM and mint vLOFI shares", async () => {
    await vaultProgram.methods
      .deposit(DEPOSIT_AMOUNT, new BN(1))
      .accounts({
        user: payer.publicKey,
        vault: vault,
        ccmMint: ccmMint.publicKey,
        vlofiMint: vlofiMint,
        userCcm: userCcmAta,
        vaultCcmBuffer: ccmBuffer,
        userVlofi: userVlofiAta,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const acct = await context.banksClient.getAccount(vault);
    const state = parseVaultState(Buffer.from(acct!.data));
    expect(state.pendingDeposits.gt(new BN(0))).toBe(true);
    expect(state.totalShares.gt(new BN(0))).toBe(true);
    console.log("Deposited:", DEPOSIT_AMOUNT.toString(), "CCM → shares:", state.totalShares.toString());
  });

  // ---------------------------------------------------------------------------
  // Test 3: Pause / Resume
  // ---------------------------------------------------------------------------
  it("should pause and resume the vault", async () => {
    await vaultProgram.methods.pause().accounts({ admin: payer.publicKey, vault }).rpc();
    let acct = await context.banksClient.getAccount(vault);
    expect(parseVaultState(Buffer.from(acct!.data)).paused).toBe(true);

    await vaultProgram.methods.resume().accounts({ admin: payer.publicKey, vault }).rpc();
    acct = await context.banksClient.getAccount(vault);
    expect(parseVaultState(Buffer.from(acct!.data)).paused).toBe(false);
    console.log("Pause/resume verified");
  });

  // ---------------------------------------------------------------------------
  // Test 4: Sync Oracle Position
  // ---------------------------------------------------------------------------
  it("should sync oracle position from a mock UserChannelStake", async () => {
    // Create a mock UserChannelStake account with 995 CCM staked
    const [vaultUserStake, usBump] = deriveUserStake(channelConfig, vault);
    const mockNftMint = Keypair.generate().publicKey;
    const mockAmount = new BN(995_000_000_000); // 995 CCM
    const mockLockEnd = new BN(100_000);

    const userStakeData = buildUserChannelStake(usBump, vault, channelConfig, mockAmount, mockLockEnd, mockNftMint);
    context.setAccount(vaultUserStake, {
      lamports: 10_000_000,
      data: userStakeData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });

    await vaultProgram.methods
      .syncOraclePosition()
      .accounts({
        admin: payer.publicKey,
        vault: vault,
        vaultOraclePosition: vaultOraclePosition,
        oracleUserStake: vaultUserStake,
      })
      .rpc();

    const posAcct = await context.banksClient.getAccount(vaultOraclePosition);
    const pos = parseVaultOraclePosition(Buffer.from(posAcct!.data));

    expect(pos.isActive).toBe(true);
    expect(pos.stakeAmount.eq(mockAmount)).toBe(true);
    expect(pos.lockEndSlot.eq(mockLockEnd)).toBe(true);
    expect(pos.oracleNftMint.equals(mockNftMint)).toBe(true);
    console.log("Oracle position synced: active=true, amount=995 CCM, lockEnd=100000");
  });

  // ---------------------------------------------------------------------------
  // Test 5: Second deposit (validates exchange rate)
  // ---------------------------------------------------------------------------
  it("should accept a second deposit", async () => {
    const secondDeposit = new BN(5_000_000_000_000); // 5,000 CCM

    const beforeAcct = await context.banksClient.getAccount(vault);
    const beforeState = parseVaultState(Buffer.from(beforeAcct!.data));

    await vaultProgram.methods
      .deposit(secondDeposit, new BN(1))
      .accounts({
        user: payer.publicKey,
        vault: vault,
        ccmMint: ccmMint.publicKey,
        vlofiMint: vlofiMint,
        userCcm: userCcmAta,
        vaultCcmBuffer: ccmBuffer,
        userVlofi: userVlofiAta,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const afterAcct = await context.banksClient.getAccount(vault);
    const afterState = parseVaultState(Buffer.from(afterAcct!.data));

    expect(afterState.totalShares.gt(beforeState.totalShares)).toBe(true);
    expect(afterState.pendingDeposits.gt(beforeState.pendingDeposits)).toBe(true);
    console.log("Second deposit: shares before=", beforeState.totalShares.toString(), "after=", afterState.totalShares.toString());
  });

  // ---------------------------------------------------------------------------
  // Test 6: Exchange Rate Calculation
  // ---------------------------------------------------------------------------
  it("should calculate valid exchange rate", async () => {
    const acct = await context.banksClient.getAccount(vault);
    const state = parseVaultState(Buffer.from(acct!.data));

    const VIRTUAL_SHARES = new BN(1_000_000_000);
    const VIRTUAL_ASSETS = new BN(1_000_000_000);

    const netAssets = state.totalStaked
      .add(state.pendingDeposits)
      .add(state.emergencyReserve)
      .sub(state.pendingWithdrawals);

    const rate = netAssets.add(VIRTUAL_ASSETS).mul(new BN(1e9)).div(state.totalShares.add(VIRTUAL_SHARES));

    console.log("Exchange rate:", (rate.toNumber() / 1e9).toFixed(6), "CCM/vLOFI");
    console.log("  Net assets:", netAssets.toString());
    console.log("  Total shares:", state.totalShares.toString());

    expect(rate.gte(new BN(1e9))).toBe(true);
  });

  // ---------------------------------------------------------------------------
  // Test 7: Request Withdrawal
  // ---------------------------------------------------------------------------
  it("should request withdrawal (burn vLOFI, queue CCM)", async () => {
    const acct = await context.banksClient.getAccount(vault);
    const state = parseVaultState(Buffer.from(acct!.data));

    const sharesToBurn = state.totalShares.div(new BN(4)); // withdraw 25%
    const [userVaultState] = deriveUserVaultState(vault, payer.publicKey);
    const [withdrawRequest] = deriveWithdrawRequest(vault, payer.publicKey, new BN(0));

    await vaultProgram.methods
      .requestWithdraw(sharesToBurn, new BN(1))
      .accounts({
        user: payer.publicKey,
        vault: vault,
        userVaultState: userVaultState,
        vlofiMint: vlofiMint,
        userVlofi: userVlofiAta,
        withdrawRequest: withdrawRequest,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const updated = await context.banksClient.getAccount(vault);
    const updatedState = parseVaultState(Buffer.from(updated!.data));

    expect(updatedState.pendingWithdrawals.gt(new BN(0))).toBe(true);
    expect(updatedState.totalShares.lt(state.totalShares)).toBe(true);
    console.log("Withdraw requested:", sharesToBurn.toString(), "shares → pending:", updatedState.pendingWithdrawals.toString());
  });

  // ---------------------------------------------------------------------------
  // Test 8: Update Admin
  // ---------------------------------------------------------------------------
  it("should update admin authority", async () => {
    const newAdmin = Keypair.generate().publicKey;

    await vaultProgram.methods
      .updateAdmin(newAdmin)
      .accounts({ admin: payer.publicKey, vault })
      .rpc();

    const acct = await context.banksClient.getAccount(vault);
    const state = parseVaultState(Buffer.from(acct!.data));
    expect(state.admin.equals(newAdmin)).toBe(true);

    // Restore original admin for subsequent tests
    // (We can't do this — the new admin is a random key we don't control)
    // This intentionally tests that admin transfer is permanent
    console.log("Admin updated to:", newAdmin.toBase58());
  });
});
