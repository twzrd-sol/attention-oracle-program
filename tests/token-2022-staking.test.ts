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
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountInstruction,
  createInitializeMintInstruction,
  createInitializeTransferFeeConfigInstruction,
  createMintToInstruction,
  ExtensionType,
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

// ---------------------------------------------------------------------------
// PDA Seeds
// ---------------------------------------------------------------------------
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const CHANNEL_USER_STAKE_SEED = Buffer.from("channel_user");
const STAKE_NFT_MINT_SEED = Buffer.from("stake_nft");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

// ---------------------------------------------------------------------------
// Test constants
// ---------------------------------------------------------------------------
const CCM_DECIMALS = 9;
const FEE_BPS = 50; // 0.5%
const MAX_FEE = 1_000_000_000_000n;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function accountDiscriminator(name: string): Buffer {
  return createHash("sha256").update(`account:${name}`).digest().subarray(0, 8);
}

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

function deriveNftMint(stakePool: PublicKey, user: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([STAKE_NFT_MINT_SEED, stakePool.toBuffer(), user.toBuffer()], ORACLE_PROGRAM_ID);
}

function buildProtocolState(
  bump: number,
  admin: PublicKey,
  publisher: PublicKey,
  treasury: PublicKey,
  mint: PublicKey
): Buffer {
  const buf = Buffer.alloc(8 + 1 + 1 + 32 * 4 + 1 + 1 + 1); // 141 bytes
  let offset = 0;
  accountDiscriminator("ProtocolState").copy(buf, offset); offset += 8;
  buf.writeUInt8(1, offset); offset += 1; // is_initialized
  buf.writeUInt8(1, offset); offset += 1; // version
  admin.toBuffer().copy(buf, offset); offset += 32;
  publisher.toBuffer().copy(buf, offset); offset += 32;
  treasury.toBuffer().copy(buf, offset); offset += 32;
  mint.toBuffer().copy(buf, offset); offset += 32;
  buf.writeUInt8(0, offset); offset += 1; // paused
  buf.writeUInt8(0, offset); offset += 1; // require_receipt (legacy)
  buf.writeUInt8(bump, offset); offset += 1;
  return buf;
}

function buildChannelConfigV2(
  bump: number,
  mint: PublicKey,
  subject: PublicKey,
  authority: PublicKey,
  creatorWallet: PublicKey
): Buffer {
  const buf = Buffer.alloc(482);
  let offset = 0;
  accountDiscriminator("ChannelConfigV2").copy(buf, offset); offset += 8;
  buf.writeUInt8(2, offset); offset += 1; // version
  buf.writeUInt8(bump, offset); offset += 1;
  mint.toBuffer().copy(buf, offset); offset += 32;
  subject.toBuffer().copy(buf, offset); offset += 32;
  authority.toBuffer().copy(buf, offset); offset += 32;
  offset += 8; // latest_root_seq
  offset += 8; // cutover_epoch
  creatorWallet.toBuffer().copy(buf, offset); offset += 32;
  offset += 2; // creator_fee_bps
  offset += 6; // padding
  // roots array left zeroed
  return buf;
}

function loadIdl(name: string): Idl {
  return JSON.parse(readFileSync(path.join(__dirname, `../target/idl/${name}.json`), "utf-8"));
}

// ---------------------------------------------------------------------------
// Test Suite
// ---------------------------------------------------------------------------

describe("Token-2022 staking invariants", () => {
  let context: ProgramTestContext;
  let provider: BankrunProvider;
  let program: Program;
  let payer: Keypair;
  let ccmMint: Keypair;
  let userCcmAta: PublicKey;
  let protocolState: PublicKey;

  beforeAll(async () => {
    context = await startAnchor(".", [], []);
    provider = new BankrunProvider(context);
    program = new Program(loadIdl("token_2022"), provider);
    payer = context.payer;

    // --- Create Token-2022 mint with TransferFeeConfig ---
    ccmMint = Keypair.generate();
    const mintLen = getMintLen([ExtensionType.TransferFeeConfig]);
    const mintRent = await provider.connection.getMinimumBalanceForRentExemption(mintLen);

    const createMintTx = new Transaction().add(
      SystemProgram.createAccount({
        fromPubkey: payer.publicKey,
        newAccountPubkey: ccmMint.publicKey,
        space: mintLen,
        lamports: mintRent,
        programId: TOKEN_2022_PROGRAM_ID,
      }),
      createInitializeTransferFeeConfigInstruction(
        ccmMint.publicKey,
        payer.publicKey,
        payer.publicKey,
        FEE_BPS,
        Number(MAX_FEE),
        TOKEN_2022_PROGRAM_ID
      ),
      createInitializeMintInstruction(
        ccmMint.publicKey,
        CCM_DECIMALS,
        payer.publicKey,
        payer.publicKey,
        TOKEN_2022_PROGRAM_ID
      )
    );
    createMintTx.recentBlockhash = context.lastBlockhash;
    createMintTx.sign(payer, ccmMint);
    await context.banksClient.processTransaction(createMintTx);

    // --- User token account ---
    userCcmAta = getAssociatedTokenAddressSync(
      ccmMint.publicKey,
      payer.publicKey,
      false,
      TOKEN_2022_PROGRAM_ID
    );
    const createAtaTx = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        userCcmAta,
        payer.publicKey,
        ccmMint.publicKey,
        TOKEN_2022_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID
      ),
      createMintToInstruction(
        ccmMint.publicKey,
        userCcmAta,
        payer.publicKey,
        1_000_000_000_000n,
        [],
        TOKEN_2022_PROGRAM_ID
      )
    );
    createAtaTx.recentBlockhash = context.lastBlockhash;
    createAtaTx.sign(payer);
    await context.banksClient.processTransaction(createAtaTx);

    // --- Protocol state ---
    const [ps, psBump] = deriveProtocolState(ccmMint.publicKey);
    protocolState = ps;
    const protocolData = buildProtocolState(
      psBump,
      payer.publicKey,
      payer.publicKey,
      payer.publicKey,
      ccmMint.publicKey
    );
    context.setAccount(protocolState, {
      lamports: 10_000_000,
      data: protocolData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });
  }, 120_000);

  async function initChannelPool(channelName: string) {
    const [channelConfig, ccBump] = deriveChannelConfig(ccmMint.publicKey, channelName);
    const subject = new PublicKey(deriveSubjectId(channelName));
    const channelData = buildChannelConfigV2(
      ccBump,
      ccmMint.publicKey,
      subject,
      payer.publicKey,
      payer.publicKey
    );
    context.setAccount(channelConfig, {
      lamports: 10_000_000,
      data: channelData,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });

    const [stakePool] = deriveStakePool(channelConfig);
    const [vault] = deriveStakeVault(stakePool);

    await program.methods
      .initializeStakePool()
      .accounts({
        payer: payer.publicKey,
        protocolState,
        channelConfig,
        mint: ccmMint.publicKey,
        stakePool,
        vault,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    return { channelConfig, stakePool, vault };
  }

  async function stakeToChannel(channelConfig: PublicKey, stakePool: PublicKey, vault: PublicKey, amount: bigint) {
    const [userStake] = deriveUserStake(channelConfig, payer.publicKey);
    const [nftMint] = deriveNftMint(stakePool, payer.publicKey);
    const nftAta = getAssociatedTokenAddressSync(nftMint, payer.publicKey, false, TOKEN_2022_PROGRAM_ID);

    await program.methods
      .stakeChannel(new BN(amount.toString()), new BN(0))
      .accounts({
        user: payer.publicKey,
        payer: payer.publicKey,
        protocolState,
        channelConfig,
        mint: ccmMint.publicKey,
        stakePool,
        userStake,
        vault,
        userTokenAccount: userCcmAta,
        nftMint,
        nftAta,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    return { userStake, nftMint, nftAta };
  }

  async function mintToVault(vault: PublicKey, amount: bigint) {
    const tx = new Transaction().add(
      createMintToInstruction(
        ccmMint.publicKey,
        vault,
        payer.publicKey,
        amount,
        [],
        TOKEN_2022_PROGRAM_ID
      )
    );
    tx.recentBlockhash = context.lastBlockhash;
    tx.sign(payer);
    await context.banksClient.processTransaction(tx);
  }

  async function setUserPendingRewards(userStake: PublicKey, pending: bigint) {
    const decoded = await program.account.userChannelStake.fetch(userStake);
    const acctInfo = await provider.connection.getAccountInfo(userStake);
    if (!acctInfo) throw new Error("User stake account missing");
    decoded.pendingRewards = new BN(pending.toString());
    decoded.rewardDebt = new BN(0);
    const data = Buffer.from(await program.coder.accounts.encode("userChannelStake", decoded));
    context.setAccount(userStake, {
      lamports: acctInfo.lamports,
      data,
      owner: ORACLE_PROGRAM_ID,
      executable: false,
    });
  }

  it("uses actual_received (post-fee) for total_staked and user_stake.amount", async () => {
    const { channelConfig, stakePool, vault } = await initChannelPool("wzrd-fee-test");
    const stakeAmount = 10_000_000_000n; // 10 CCM

    const { userStake } = await stakeToChannel(channelConfig, stakePool, vault, stakeAmount);

    const pool = await program.account.channelStakePool.fetch(stakePool);
    const userStakeAcct = await program.account.userChannelStake.fetch(userStake);

    const fee = (stakeAmount * BigInt(FEE_BPS)) / 10_000n;
    const expected = stakeAmount - fee;

    expect(pool.totalStaked.toString()).toBe(expected.toString());
    expect(userStakeAcct.amount.toString()).toBe(expected.toString());
  });

  it("blocks unstake when pending rewards are claimable", async () => {
    const { channelConfig, stakePool, vault } = await initChannelPool("wzrd-pending-block");
    const stakeAmount = 10_000_000_000n;
    const pending = 1_000_000_000n; // 1 CCM

    const { userStake, nftMint, nftAta } = await stakeToChannel(channelConfig, stakePool, vault, stakeAmount);
    await setUserPendingRewards(userStake, pending);
    await mintToVault(vault, pending); // create excess >= pending

    await expect(
      program.methods
        .unstakeChannel()
        .accounts({
          user: payer.publicKey,
          channelConfig,
          mint: ccmMint.publicKey,
          stakePool,
          userStake,
          vault,
          userTokenAccount: userCcmAta,
          nftMint,
          nftAta,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .rpc()
    ).rejects.toThrow(/PendingRewardsOnUnstake/);
  });

  it("allows unstake with forfeit when rewards are underfunded", async () => {
    const { channelConfig, stakePool, vault } = await initChannelPool("wzrd-underfunded");
    const stakeAmount = 10_000_000_000n;
    const pending = 1_000_000_000n;

    const { userStake, nftMint, nftAta } = await stakeToChannel(channelConfig, stakePool, vault, stakeAmount);
    await setUserPendingRewards(userStake, pending);

    await program.methods
      .unstakeChannel()
      .accounts({
        user: payer.publicKey,
        channelConfig,
        mint: ccmMint.publicKey,
        stakePool,
        userStake,
        vault,
        userTokenAccount: userCcmAta,
        nftMint,
        nftAta,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    const userStakeAcct = await context.banksClient.getAccount(userStake);
    expect(userStakeAcct).toBeNull();

    const pool = await program.account.channelStakePool.fetch(stakePool);
    expect(pool.totalStaked.toString()).toBe("0");
  });

  it("prevents claims that exceed available rewards (principal protection)", async () => {
    const { channelConfig, stakePool, vault } = await initChannelPool("wzrd-claim-exceed");
    const stakeAmount = 10_000_000_000n;
    const pending = 1_000_000_000n;

    const { userStake } = await stakeToChannel(channelConfig, stakePool, vault, stakeAmount);
    await setUserPendingRewards(userStake, pending);

    await expect(
      program.methods
        .claimChannelRewards()
        .accounts({
          user: payer.publicKey,
          channelConfig,
          mint: ccmMint.publicKey,
          stakePool,
          userStake,
          vault,
          userTokenAccount: userCcmAta,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
        })
        .rpc()
    ).rejects.toThrow(/ClaimExceedsAvailableRewards/);
  });
});
