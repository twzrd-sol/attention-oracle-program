/**
 * Initialize 4 trial vaults with different lock durations
 * lofi-vault-3h, lofi-vault-6h, lofi-vault-9h, lofi-vault-12h
 */

import {
  Connection,
  PublicKey,
  Keypair,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { keccak_256 } from "@noble/hashes/sha3";
import { readFileSync } from "fs";
import { createHash } from "crypto";
import BN from "bn.js";

// Program IDs
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");

// Seeds
const PROTOCOL_SEED = Buffer.from("protocol");
const CHANNEL_CONFIG_V2_SEED = Buffer.from("channel_cfg_v2");
const VAULT_SEED = Buffer.from("vault");
const VAULT_CCM_BUFFER_SEED = Buffer.from("vault_ccm");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const VAULT_ORACLE_POSITION_SEED = Buffer.from("vault_oracle");

// Trial configurations
const SLOTS_PER_HOUR = 9_000;
const MIN_DEPOSIT = new BN(10_000_000_000); // 10 CCM

const TRIAL_CONFIGS = [
  { channel: "lofi-vault-3h", hours: 3 },
  { channel: "lofi-vault-6h", hours: 6 },
  { channel: "lofi-vault-9h", hours: 9 },
  { channel: "lofi-vault-12h", hours: 12 },
];

function deriveSubjectId(channel: string): Buffer {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  return Buffer.from(keccak_256(input));
}

function getDiscriminator(name: string): Buffer {
  const hash = createHash("sha256").update(`global:${name}`).digest();
  return hash.slice(0, 8);
}

async function initializeTrialVault(
  connection: Connection,
  admin: Keypair,
  protocolState: PublicKey,
  channel: string,
  lockSlots: number
) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`🏦 Initializing: ${channel} (${lockSlots / SLOTS_PER_HOUR}h lock)`);
  console.log(`${"=".repeat(60)}`);

  // Derive PDAs
  const subjectId = deriveSubjectId(channel);
  const [channelConfig] = PublicKey.findProgramAddressSync(
    [CHANNEL_CONFIG_V2_SEED, CCM_MINT.toBuffer(), subjectId],
    ORACLE_PROGRAM_ID
  );

  const [vault] = PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID
  );

  const [ccmBuffer] = PublicKey.findProgramAddressSync(
    [VAULT_CCM_BUFFER_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );

  const [vlofiMint] = PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );

  const [vaultOraclePosition] = PublicKey.findProgramAddressSync(
    [VAULT_ORACLE_POSITION_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID
  );

  console.log("  Channel Config:", channelConfig.toBase58());
  console.log("  Vault:", vault.toBase58());
  console.log("  vLOFI Mint:", vlofiMint.toBase58());

  // Step 1: Create channel if needed
  const channelExists = await connection.getAccountInfo(channelConfig);
  if (channelExists) {
    console.log("  📡 Channel already exists, skipping...");
  } else {
    console.log("  📡 Creating channel on Oracle...");

    const initChannelDiscriminator = getDiscriminator("initialize_channel_cumulative");
    const channelBytes = Buffer.from(channel, "utf-8");
    const channelLenBuf = Buffer.alloc(4);
    channelLenBuf.writeUInt32LE(channelBytes.length);

    const cutoverEpoch = new BN(0);
    const creatorWallet = admin.publicKey;
    const creatorFeeBps = 0;

    const initChannelData = Buffer.concat([
      initChannelDiscriminator,
      channelLenBuf,
      channelBytes,
      cutoverEpoch.toArrayLike(Buffer, "le", 8),
      creatorWallet.toBuffer(),
      Buffer.from([creatorFeeBps & 0xff, (creatorFeeBps >> 8) & 0xff]),
    ]);

    const initChannelIx = new TransactionInstruction({
      programId: ORACLE_PROGRAM_ID,
      keys: [
        { pubkey: admin.publicKey, isSigner: true, isWritable: true },
        { pubkey: protocolState, isSigner: false, isWritable: false },
        { pubkey: channelConfig, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: initChannelData,
    });

    const tx1 = new Transaction().add(initChannelIx);
    const sig1 = await sendAndConfirmTransaction(connection, tx1, [admin], {
      commitment: "confirmed",
    });
    console.log("  ✅ Channel created:", sig1);
  }

  // Step 2: Create vault if needed
  const vaultExists = await connection.getAccountInfo(vault);
  if (vaultExists) {
    console.log("  🏦 Vault already exists, skipping...");
    return { channel, channelConfig, vault, vlofiMint, status: "exists" };
  }

  console.log("  🏦 Creating ChannelVault...");

  const initVaultDiscriminator = getDiscriminator("initialize_vault");
  const lockDurationSlots = new BN(lockSlots);
  const withdrawQueueSlots = new BN(lockSlots); // Same as lock duration

  const initVaultData = Buffer.concat([
    initVaultDiscriminator,
    MIN_DEPOSIT.toArrayLike(Buffer, "le", 8),
    lockDurationSlots.toArrayLike(Buffer, "le", 8),
    withdrawQueueSlots.toArrayLike(Buffer, "le", 8),
  ]);

  const initVaultIx = new TransactionInstruction({
    programId: VAULT_PROGRAM_ID,
    keys: [
      { pubkey: admin.publicKey, isSigner: true, isWritable: true },
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
    ],
    data: initVaultData,
  });

  const tx2 = new Transaction().add(initVaultIx);
  const sig2 = await sendAndConfirmTransaction(connection, tx2, [admin], {
    commitment: "confirmed",
  });
  console.log("  ✅ Vault created:", sig2);

  return { channel, channelConfig, vault, vlofiMint, status: "created", sig: sig2 };
}

async function main() {
  const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

  // Load admin wallet
  const walletPath = process.env.HOME + "/.config/solana/id.json";
  const secretKey = JSON.parse(readFileSync(walletPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(secretKey));

  console.log("🎵 LOFI VAULT TRIAL INITIALIZATION 🎵");
  console.log("=====================================");
  console.log("Admin:", admin.publicKey.toBase58());
  console.log("Min Deposit: 10 CCM");
  console.log("Vaults to create:", TRIAL_CONFIGS.length);

  // Derive protocol state
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    ORACLE_PROGRAM_ID
  );
  console.log("Protocol State:", protocolState.toBase58());

  // Initialize each trial vault
  const results = [];
  for (const config of TRIAL_CONFIGS) {
    const lockSlots = config.hours * SLOTS_PER_HOUR;
    try {
      const result = await initializeTrialVault(
        connection,
        admin,
        protocolState,
        config.channel,
        lockSlots
      );
      results.push(result);
    } catch (err: any) {
      console.error(`  ❌ Failed: ${err.message}`);
      results.push({ channel: config.channel, status: "failed", error: err.message });
    }
  }

  // Summary
  console.log("\n");
  console.log("=".repeat(60));
  console.log("🎉 TRIAL VAULTS SUMMARY 🎉");
  console.log("=".repeat(60));
  console.log("");

  for (const r of results) {
    const statusIcon = r.status === "created" ? "✅" : r.status === "exists" ? "📌" : "❌";
    console.log(`${statusIcon} ${r.channel}`);
    if (r.vault) console.log(`   Vault: ${r.vault.toBase58()}`);
    if (r.vlofiMint) console.log(`   vLOFI: ${r.vlofiMint.toBase58()}`);
    console.log("");
  }

  console.log("Next steps:");
  console.log("  1. Fund each vault with initial deposits");
  console.log("  2. Test deposit/redeem flows");
  console.log("  3. Monitor lock/queue behavior across durations");
}

main().catch(console.error);
