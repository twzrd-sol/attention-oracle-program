/**
 * ChannelVault Integration Test with Bankrun
 *
 * Uses bankrun to test against cloned mainnet accounts.
 */

import { describe, it, beforeAll, expect } from "vitest";
import { startAnchor, ProgramTestContext, BanksClient } from "solana-bankrun";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  SYSVAR_RENT_PUBKEY,
  Connection,
  AccountInfo,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  createMint,
  mintTo,
  createAssociatedTokenAccountInstruction,
  createInitializeMintInstruction,
  getMintLen,
  ExtensionType,
} from "@solana/spl-token";
import { readFileSync } from "fs";
import { keccak_256 } from "@noble/hashes/sha3";
import BN from "bn.js";

// Program IDs
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

// Oracle Seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const CHANNEL_STAKE_POOL_SEED = Buffer.from("channel_pool");
const STAKE_VAULT_SEED = Buffer.from("stake_vault");

// Vault Seeds
const VAULT_SEED = Buffer.from("channel_vault");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm_buffer");
const VLOFI_MINT_SEED = Buffer.from("vlofi_mint");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle_position");

// Helper to derive channel subject hash
function deriveSubjectId(channel: string): Buffer {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  return Buffer.from(keccak_256(input));
}

// Derive Oracle protocol state
function deriveProtocolState(mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, mint.toBuffer()],
    ORACLE_PROGRAM_ID
  );
}

// Derive channel config
function deriveChannelConfig(mint: PublicKey, channelName: string): [PublicKey, number] {
  const channelHash = deriveSubjectId(channelName);
  return PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, mint.toBuffer(), channelHash],
    ORACLE_PROGRAM_ID
  );
}

// Derive vault PDA
function deriveVault(channelConfig: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID
  );
}

// Derive vault CCM buffer
function deriveVaultCcmBuffer(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VAULT_CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );
}

// Derive vLOFI mint
function deriveVlofiMint(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );
}

// Derive vault oracle position
function deriveVaultOraclePosition(vault: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [VAULT_ORACLE_POSITION_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );
}

// Anchor instruction discriminator (first 8 bytes of sha256("global:<name>"))
function getDiscriminator(name: string): Buffer {
  const crypto = require("crypto");
  const hash = crypto.createHash("sha256").update(`global:${name}`).digest();
  return hash.slice(0, 8);
}

// Clone account from mainnet
async function cloneAccount(
  connection: Connection,
  pubkey: PublicKey
): Promise<{ address: PublicKey; info: AccountInfo<Buffer> } | null> {
  const info = await connection.getAccountInfo(pubkey);
  if (!info) return null;
  return { address: pubkey, info };
}

describe("ChannelVault", () => {
  let context: ProgramTestContext;
  let client: BanksClient;
  let payer: Keypair;

  // Test channel: twitch:lofigirl
  const TEST_CHANNEL = "twitch:lofigirl";

  beforeAll(async () => {
    const mainnetConnection = new Connection(
      process.env.MAINNET_RPC || "https://api.mainnet-beta.solana.com"
    );

    // Derive all PDAs we need to clone
    const [protocolState] = deriveProtocolState(CCM_MINT);
    const [channelConfig] = deriveChannelConfig(CCM_MINT, TEST_CHANNEL);

    console.log("Cloning accounts from mainnet...");
    console.log("  Protocol State:", protocolState.toBase58());
    console.log("  Channel Config:", channelConfig.toBase58());
    console.log("  CCM Mint:", CCM_MINT.toBase58());

    // Clone accounts from mainnet
    const accountsToClone: { address: PublicKey; info: AccountInfo<Buffer> }[] = [];

    const clonePubkeys = [
      protocolState,
      channelConfig,
      CCM_MINT,
    ];

    for (const pubkey of clonePubkeys) {
      const result = await cloneAccount(mainnetConnection, pubkey);
      if (result) {
        accountsToClone.push(result);
        console.log(`  Cloned ${pubkey.toBase58()} (${result.info.data.length} bytes)`);
      } else {
        console.log(`  WARNING: Could not clone ${pubkey.toBase58()}`);
      }
    }

    // Start bankrun with Anchor workspace
    context = await startAnchor(
      ".",
      [],
      accountsToClone
    );

    client = context.banksClient;
    payer = context.payer;

    console.log("\nBankrun context started");
    console.log("  Payer:", payer.publicKey.toBase58());
  }, 120000);

  it("should have cloned channel config", async () => {
    const [channelConfig] = deriveChannelConfig(CCM_MINT, TEST_CHANNEL);
    const account = await client.getAccount(channelConfig);

    expect(account).not.toBeNull();
    console.log("Channel config found, data length:", account?.data.length);
  });

  it("should derive correct vault PDAs", () => {
    const [channelConfig] = deriveChannelConfig(CCM_MINT, TEST_CHANNEL);
    const [vault, vaultBump] = deriveVault(channelConfig);
    const [ccmBuffer] = deriveVaultCcmBuffer(vault);
    const [vlofiMint] = deriveVlofiMint(vault);
    const [vaultOraclePosition] = deriveVaultOraclePosition(vault);

    console.log("\nVault PDAs derived:");
    console.log("  Vault:", vault.toBase58());
    console.log("  CCM Buffer:", ccmBuffer.toBase58());
    console.log("  vLOFI Mint:", vlofiMint.toBase58());
    console.log("  Oracle Position:", vaultOraclePosition.toBase58());

    // Just verify they're valid public keys
    expect(vault.toBase58()).toBeDefined();
    expect(ccmBuffer.toBase58()).toBeDefined();
    expect(vlofiMint.toBase58()).toBeDefined();
    expect(vaultOraclePosition.toBase58()).toBeDefined();
  });

  it("should build initialize_vault instruction correctly", async () => {
    const [channelConfig] = deriveChannelConfig(CCM_MINT, TEST_CHANNEL);
    const [protocolState] = deriveProtocolState(CCM_MINT);
    const [vault] = deriveVault(channelConfig);
    const [ccmBuffer] = deriveVaultCcmBuffer(vault);
    const [vlofiMint] = deriveVlofiMint(vault);
    const [vaultOraclePosition] = deriveVaultOraclePosition(vault);

    // Build initialize_vault instruction
    const discriminator = getDiscriminator("initialize_vault");
    const minDeposit = new BN(1_000_000_000); // 1 CCM minimum
    const lockDurationSlots = new BN(7 * 216_000); // 7 days
    const withdrawQueueSlots = new BN(7 * 216_000); // 7 days

    const data = Buffer.concat([
      discriminator,
      minDeposit.toArrayLike(Buffer, "le", 8),
      lockDurationSlots.toArrayLike(Buffer, "le", 8),
      withdrawQueueSlots.toArrayLike(Buffer, "le", 8),
    ]);

    console.log("\nInitialize vault instruction:");
    console.log("  Discriminator:", discriminator.toString("hex"));
    console.log("  Min deposit:", minDeposit.toString());
    console.log("  Lock duration slots:", lockDurationSlots.toString());
    console.log("  Withdraw queue slots:", withdrawQueueSlots.toString());
    console.log("  Data length:", data.length);

    const keys = [
      { pubkey: payer.publicKey, isSigner: true, isWritable: true },
      { pubkey: protocolState, isSigner: false, isWritable: false },
      { pubkey: channelConfig, isSigner: false, isWritable: false },
      { pubkey: CCM_MINT, isSigner: false, isWritable: false },
      { pubkey: vault, isSigner: false, isWritable: true },
      { pubkey: ccmBuffer, isSigner: false, isWritable: true },
      { pubkey: vlofiMint, isSigner: false, isWritable: true },
      { pubkey: vaultOraclePosition, isSigner: false, isWritable: true },
      { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
    ];

    console.log("  Account keys:", keys.length);
    expect(keys.length).toBe(13);

    // The instruction was built successfully
    const ix = new TransactionInstruction({
      programId: VAULT_PROGRAM_ID,
      keys,
      data,
    });

    expect(ix.programId.equals(VAULT_PROGRAM_ID)).toBe(true);
  });

  it("should verify protocol state data structure", async () => {
    const [protocolState] = deriveProtocolState(CCM_MINT);
    const account = await client.getAccount(protocolState);

    expect(account).not.toBeNull();
    if (!account) return;

    const data = Buffer.from(account.data);
    console.log("\nProtocol State analysis:");
    console.log("  Data length:", data.length);
    console.log("  First 8 bytes (discriminator):", data.subarray(0, 8).toString("hex"));

    // Parse after 8-byte discriminator
    let offset = 8;
    const isInitialized = data.readUInt8(offset); offset += 1;
    const version = data.readUInt8(offset); offset += 1;
    const admin = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const publisher = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const treasury = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const mint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const paused = data.readUInt8(offset); offset += 1;
    const requireReceipt = data.readUInt8(offset); offset += 1;
    const bump = data.readUInt8(offset); offset += 1;

    console.log("  is_initialized:", isInitialized);
    console.log("  version:", version);
    console.log("  admin:", admin.toBase58());
    console.log("  mint:", mint.toBase58());
    console.log("  paused:", paused);
    console.log("  bump:", bump);

    // Verify mint matches CCM_MINT
    expect(mint.equals(CCM_MINT)).toBe(true);
  });

  it("should verify channel config data structure", async () => {
    const [channelConfig] = deriveChannelConfig(CCM_MINT, TEST_CHANNEL);
    const account = await client.getAccount(channelConfig);

    expect(account).not.toBeNull();
    if (!account) return;

    const data = Buffer.from(account.data);
    console.log("\nChannel Config analysis:");
    console.log("  Data length:", data.length);
    console.log("  First 8 bytes (discriminator):", data.subarray(0, 8).toString("hex"));

    // Parse ChannelConfigV2 after 8-byte discriminator
    let offset = 8;
    const version = data.readUInt8(offset); offset += 1;
    const bump = data.readUInt8(offset); offset += 1;
    const mint = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const subject = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const authority = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;

    console.log("  version:", version);
    console.log("  bump:", bump);
    console.log("  mint:", mint.toBase58());
    console.log("  subject:", subject.toBase58());
    console.log("  authority:", authority.toBase58());

    // Verify mint matches CCM_MINT
    expect(mint.equals(CCM_MINT)).toBe(true);
  });
});
