/**
 * Verify Channel Vault Deployment State
 *
 * Comprehensive verification of all 16 channel vaults on mainnet:
 *   - Vault account exists & owned by vault program
 *   - vLOFI mint exists with correct authority
 *   - Metadata exists & readable
 *   - Admin authority matches expected (Squads vault)
 *   - TVL, exchange rate, paused state
 *   - Oracle position tracking
 *
 * Usage:
 *   RPC_URL="https://..." npx ts-node scripts/verify-mainnet-vaults.ts
 *   RPC_URL="https://..." npx ts-node scripts/verify-mainnet-vaults.ts --json
 */

import { Connection, PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Keypair } from "@solana/web3.js";

// Constants
const VAULT_PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const ORACLE_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const SQUADS_VAULT = new PublicKey("2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW");
const METADATA_PROGRAM_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

const VAULT_SEED = Buffer.from("vault");
const VLOFI_MINT_SEED = Buffer.from("vlofi");
const METADATA_SEED = Buffer.from("metadata");
const ORACLE_POSITION_SEED = Buffer.from("oracle_position");

// All 16 channel vaults
const VAULTS = [
  { label: "vLOFI Lofi 3h",     channelConfig: "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW" },
  { label: "vLOFI Lofi 6h",     channelConfig: "dJvatt5bga4ak64ghTLEtxs1jxfLX4TNoZuvfiDCcGy" },
  { label: "vLOFI Lofi 9h",     channelConfig: "2TWM1H1gHWrA6Ta6A9tH3E1TTTRbPpmSL2Xg7KdHwxCM" },
  { label: "vLOFI Lofi 12h",    channelConfig: "GZL7vAo9vxdNbsmrreVueVd1Xm9oWmatkQauFcxhq8qP" },
  { label: "vLOFI TWZRD 247",   channelConfig: "84SxXryEL2dFT5rno9F1SGBAFvvkEDyp3wNQZyxT3hQ9" },
  { label: "vLOFI TWZRD 1999",  channelConfig: "7g1qkWgZkbhZNFgbEzxxvYxCJHt4NMb3fwE2RHyrygDL" },
  { label: "vLOFI TWZRD 415",   channelConfig: "DqoM3QcGPbUD2Hic1fxsSLqZY1CaSDkiaNaas2ufZUpb" },
  { label: "vLOFI TWZRD 3121",  channelConfig: "EADvLuoe6ZXTfVBpVEKAMSfnFr1oZuHMxiButLVMnHuE" },
  { label: "vLOFI TWZRD 69",    channelConfig: "HEa4KgAyuvRZPyAsUPmVTRXiTRuxVEkkGbmtEeybzGB9" },
  { label: "vLOFI 999",         channelConfig: "9G1MvnVq3dX6UwGFvhTC9bDArNt9TyvS5UimffTL1BAJ" },
  { label: "vLOFI 212",         channelConfig: "Dg84d5BkSYxKSix9m6YgbLz1L7mEsSH81Svp24watxEC" },
  { label: "vLOFI 247",         channelConfig: "GdrV9DjKZFePZadxuQANKEBvVaB7rM8aEhMEzMHWrFJE" },
  { label: "vLOFI 1999",        channelConfig: "8LCSiL2a4FjTAveMMn8SjLVxrYecWSfFDH48sdhzdbv" },
  { label: "vLOFI 415",         channelConfig: "GxzK9iqyFJf3TRJG5XAQJD3eJtgKCivzkQtj7iPKrUsG" },
  { label: "vLOFI 3121",        channelConfig: "4JawzmsofxVCim7eDtFPCMwiP21NMcAQqsZRPT7k9uL1" },
  { label: "vLOFI 69",          channelConfig: "2uGQDJMsGy3undJCT9NazdJXjSoCcXd71vgkvYzMt3eR" },
];

interface VaultStatus {
  label: string;
  channelConfig: string;
  vault: string;
  vlofiMint: string;
  metadata: string;
  oraclePosition: string;
  exists: boolean;
  owner?: string;
  admin?: string;
  paused?: boolean;
  tvl?: string;
  exchangeRate?: string;
  vlofiSupply?: string;
  metadataExists?: boolean;
  metadataName?: string;
  metadataSymbol?: string;
  oraclePositionExists?: boolean;
  errors: string[];
}

function formatCCM(lamports: bigint): string {
  return (Number(lamports) / 1e9).toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });
}

async function verifyVault(
  connection: Connection,
  program: Program,
  v: typeof VAULTS[0],
): Promise<VaultStatus> {
  const errors: string[] = [];
  const channelConfig = new PublicKey(v.channelConfig);

  // Derive PDAs
  const [vault] = PublicKey.findProgramAddressSync(
    [VAULT_SEED, channelConfig.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [vlofiMint] = PublicKey.findProgramAddressSync(
    [VLOFI_MINT_SEED, vault.toBuffer()],
    VAULT_PROGRAM_ID,
  );
  const [metadata] = PublicKey.findProgramAddressSync(
    [METADATA_SEED, METADATA_PROGRAM_ID.toBuffer(), vlofiMint.toBuffer()],
    METADATA_PROGRAM_ID,
  );
  const [oraclePosition] = PublicKey.findProgramAddressSync(
    [ORACLE_POSITION_SEED, vault.toBuffer()],
    ORACLE_PROGRAM_ID,
  );

  const status: VaultStatus = {
    label: v.label,
    channelConfig: v.channelConfig,
    vault: vault.toBase58(),
    vlofiMint: vlofiMint.toBase58(),
    metadata: metadata.toBase58(),
    oraclePosition: oraclePosition.toBase58(),
    exists: false,
    errors,
  };

  // Check vault account
  const vaultInfo = await connection.getAccountInfo(vault);
  if (!vaultInfo) {
    errors.push("Vault account not found");
    return status;
  }

  status.exists = true;
  status.owner = vaultInfo.owner.toBase58();

  if (!vaultInfo.owner.equals(VAULT_PROGRAM_ID)) {
    errors.push(`Wrong owner: ${vaultInfo.owner.toBase58()}`);
    return status;
  }

  // Fetch vault state via Anchor
  try {
    const vaultData = await program.account.channelVault.fetch(vault);
    status.admin = vaultData.admin.toBase58();
    status.paused = vaultData.paused;
    status.tvl = formatCCM(vaultData.totalStaked);

    // Exchange rate: total_shares / total_staked (scaled by 1e9)
    const totalStaked = BigInt(vaultData.totalStaked.toString());
    const totalShares = BigInt(vaultData.totalShares.toString());

    if (totalStaked > 0n) {
      const rate = (totalShares * 1_000_000_000n) / totalStaked;
      status.exchangeRate = (Number(rate) / 1e9).toFixed(6);
    } else {
      status.exchangeRate = "1.000000";
    }

    status.vlofiSupply = formatCCM(totalShares);

    // Verify admin
    if (!vaultData.admin.equals(SQUADS_VAULT)) {
      errors.push(`Admin mismatch: ${vaultData.admin.toBase58()} (expected ${SQUADS_VAULT.toBase58()})`);
    }

    // Verify paused
    if (vaultData.paused) {
      errors.push("Vault is paused");
    }

    // Verify CCM mint
    if (!vaultData.ccmMint.equals(CCM_MINT)) {
      errors.push(`CCM mint mismatch: ${vaultData.ccmMint.toBase58()}`);
    }

    // Verify vlofi mint
    if (!vaultData.vlofiMint.equals(vlofiMint)) {
      errors.push(`vLOFI mint mismatch: ${vaultData.vlofiMint.toBase58()}`);
    }
  } catch (err: any) {
    errors.push(`Failed to fetch vault state: ${err.message}`);
    return status;
  }

  // Check metadata
  const metadataInfo = await connection.getAccountInfo(metadata);
  status.metadataExists = metadataInfo !== null;

  if (metadataInfo) {
    try {
      // Parse Metaplex Metadata (simplified - just name + symbol)
      // Layout: [1] key, [32] updateAuthority, [32] mint, [4+N] name, [4+M] symbol, ...
      const data = metadataInfo.data;
      if (data[0] === 4) { // MetadataV1 key
        let offset = 1 + 32 + 32; // key + updateAuthority + mint

        // Name (u32 length prefix + UTF-8)
        const nameLen = data.readUInt32LE(offset);
        offset += 4;
        const nameBytes = data.slice(offset, offset + nameLen);
        status.metadataName = nameBytes.toString("utf8").replace(/\0/g, "");
        offset += nameLen;

        // Symbol (u32 length prefix + UTF-8)
        const symbolLen = data.readUInt32LE(offset);
        offset += 4;
        const symbolBytes = data.slice(offset, offset + symbolLen);
        status.metadataSymbol = symbolBytes.toString("utf8").replace(/\0/g, "");
      }
    } catch (err: any) {
      errors.push(`Failed to parse metadata: ${err.message}`);
    }
  } else {
    errors.push("Metadata account not found");
  }

  // Check oracle position (non-blocking — initialized lazily on first compound)
  const oraclePositionInfo = await connection.getAccountInfo(oraclePosition);
  status.oraclePositionExists = oraclePositionInfo !== null;

  // Oracle position not existing is expected for vaults that haven't been compounded yet
  // This doesn't block deposits — it's created via Oracle CPI on first compound

  // Check vLOFI mint
  const vlofiMintInfo = await connection.getAccountInfo(vlofiMint);
  if (!vlofiMintInfo) {
    errors.push("vLOFI mint not found");
  } else {
    // Verify mint authority = vault PDA
    // SPL Token mint layout (82 bytes base, NOT Token-2022):
    // [0..4]   mint_authority Option discriminator (0 = None, 1 = Some)
    // [4..36]  mint_authority Pubkey (if Some)
    // [36..40] supply (u64)
    // [40]     decimals (u8)
    // [41]     is_initialized (bool)
    // [42..46] freeze_authority Option discriminator
    // [46..78] freeze_authority Pubkey (if Some)

    const hasAuthority = vlofiMintInfo.data.readUInt32LE(0);
    if (hasAuthority !== 1) {
      errors.push("vLOFI mint has no authority");
    } else {
      const mintAuthority = new PublicKey(vlofiMintInfo.data.slice(4, 36));
      if (!mintAuthority.equals(vault)) {
        errors.push(`vLOFI mint authority mismatch: ${mintAuthority.toBase58()}`);
      }
    }
  }

  return status;
}

async function main() {
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: RPC_URL required");
    process.exit(1);
  }

  const jsonOutput = process.argv.includes("--json");

  const connection = new Connection(rpcUrl, "confirmed");

  // Dummy wallet for provider (read-only)
  const dummyKeypair = Keypair.generate();
  const wallet = new Wallet(dummyKeypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });

  // Load IDL from chain
  const idl = await Program.fetchIdl(VAULT_PROGRAM_ID, provider);
  if (!idl) {
    console.error("ERROR: Vault program IDL not found on-chain");
    process.exit(1);
  }

  const program = new Program(idl, provider);

  if (!jsonOutput) {
    console.log("=".repeat(80));
    console.log("  CHANNEL VAULT VERIFICATION");
    console.log("=".repeat(80));
    console.log(`  RPC:          ${rpcUrl.substring(0, 60)}...`);
    console.log(`  Vault Program: ${VAULT_PROGRAM_ID.toBase58()}`);
    console.log(`  Expected Admin: ${SQUADS_VAULT.toBase58()}`);
    console.log(`  Vaults:       ${VAULTS.length}`);
    console.log();
  }

  const results: VaultStatus[] = [];

  for (let i = 0; i < VAULTS.length; i++) {
    const v = VAULTS[i];
    const status = await verifyVault(connection, program, v);
    results.push(status);

    // Add delay between vaults to avoid rate limiting
    if (i < VAULTS.length - 1) {
      await new Promise(r => setTimeout(r, 1000));
    }

    if (!jsonOutput) {
      const icon = status.errors.length === 0 ? "✓" : "✗";
      console.log(`${icon} ${status.label.padEnd(20)}`);
      console.log(`   Vault:    ${status.vault}`);
      console.log(`   vLOFI:    ${status.vlofiMint}`);
      console.log(`   Exists:   ${status.exists}`);

      if (status.exists) {
        console.log(`   Admin:    ${status.admin} ${status.admin === SQUADS_VAULT.toBase58() ? "✓" : "✗"}`);
        console.log(`   Paused:   ${status.paused}`);
        console.log(`   TVL:      ${status.tvl} CCM`);
        console.log(`   Exchange: ${status.exchangeRate}`);
        console.log(`   vLOFI:    ${status.vlofiSupply} (supply)`);
        console.log(`   Metadata: ${status.metadataExists ? `✓ ${status.metadataName} (${status.metadataSymbol})` : "✗"}`);
        console.log(`   Oracle:   ${status.oraclePositionExists ? "✓" : "✗"}`);
      }

      if (status.errors.length > 0) {
        for (const err of status.errors) {
          console.log(`   ERROR: ${err}`);
        }
      }
      console.log();
    }
  }

  if (jsonOutput) {
    console.log(JSON.stringify(results, null, 2));
  } else {
    const totalOk = results.filter(r => r.errors.length === 0).length;
    const totalFailed = results.filter(r => r.errors.length > 0).length;

    console.log("=".repeat(80));
    console.log(`  SUMMARY: ${totalOk} OK, ${totalFailed} FAILED`);
    console.log("=".repeat(80));

    if (totalFailed > 0) {
      console.log("\nFailed vaults:");
      for (const r of results.filter(r => r.errors.length > 0)) {
        console.log(`  - ${r.label}: ${r.errors.join(", ")}`);
      }
      process.exit(1);
    }
  }
}

main().catch((err) => {
  console.error("Fatal error:", err.message || err);
  process.exit(1);
});
