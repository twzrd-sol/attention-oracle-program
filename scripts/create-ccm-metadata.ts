/**
 * Create on-chain Metaplex metadata for the CCM token mint.
 *
 * KNOWN LIMITATION (2026-02-01):
 *   The CCM mint (Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM) was created
 *   on Token-2022 with only the Transfer Fees extension.  The mint authority
 *   was revoked BEFORE on-chain metadata was created.
 *
 *   Both Metaplex instructions fail because the on-chain program requires the
 *   mint authority to sign metadata creation:
 *     - createV1 (disc. 42): error 0xa "InvalidMintAuthority"
 *     - createMetadataAccountV3 (disc. 33): error 0x99 (blocked for Token-2022)
 *
 *   Alternatives for token display:
 *     1. Jupiter Verify (V3) — https://jup.ag/verify
 *     2. Direct explorer registration (Solscan, SolanaFM)
 *     3. Off-chain metadata JSON hosted on Arweave/IPFS
 *     4. Future Metaplex program update for revoked-authority mints
 *
 *   This script is kept as a reference and will succeed if Metaplex ever
 *   updates the program to allow metadata creation for mints with
 *   COption::None mint authority.
 *
 * Usage:
 *   CLUSTER=mainnet-beta \
 *   RPC_URL=<your-rpc-endpoint> \
 *   KEYPAIR=~/.config/solana/id.json \
 *   I_UNDERSTAND_MAINNET=1 \
 *     npx tsx scripts/create-ccm-metadata.ts
 */

import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import {
  createV1,
  TokenStandard,
  createMetadataAccountV3,
  findMetadataPda,
} from "@metaplex-foundation/mpl-token-metadata";
import {
  publicKey,
  signerIdentity,
  percentAmount,
  none,
  some,
  createSignerFromKeypair,
} from "@metaplex-foundation/umi";
import {
  fromWeb3JsKeypair,
  fromWeb3JsPublicKey,
  toWeb3JsPublicKey,
} from "@metaplex-foundation/umi-web3js-adapters";
import { Keypair, PublicKey, Connection } from "@solana/web3.js";
import { readFileSync } from "fs";
import { requireScriptEnv } from "./script-guard.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const TOKEN_2022_PROGRAM_ID = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

const TOKEN_NAME = "CCM";
const TOKEN_SYMBOL = "CCM";
const TOKEN_URI = ""; // No logo URI yet – can be updated later via updateV1

const MIN_SOL_BALANCE = 0.01 * 1e9;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Load keypair from JSON file on disk. */
function loadKeypairFromFile(filePath: string): Keypair {
  const raw = readFileSync(filePath, "utf-8");
  const secretKey = Uint8Array.from(JSON.parse(raw));
  return Keypair.fromSecretKey(secretKey);
}

/** Check if a metadata PDA already exists on-chain. */
async function metadataExists(
  connection: Connection,
  address: PublicKey,
): Promise<boolean> {
  const info = await connection.getAccountInfo(address);
  if (!info) return false;
  console.log("[SKIP] Metadata account already exists!");
  console.log("  Owner:", info.owner.toBase58());
  console.log("  Size:", info.data.length, "bytes");
  return true;
}

/** Parse and display basic Metaplex metadata fields from raw account data. */
function parseMetadataFields(data: Buffer): void {
  if (data[0] !== 4) return; // MetadataV1 key discriminator

  // Layout: key(1) + update_authority(32) + mint(32) = offset 65
  const nameLen = data.readUInt32LE(65);
  const name = data
    .subarray(69, 69 + nameLen)
    .toString("utf8")
    .replace(/\0/g, "");

  const symbolOffset = 69 + nameLen;
  const symbolLen = data.readUInt32LE(symbolOffset);
  const symbol = data
    .subarray(symbolOffset + 4, symbolOffset + 4 + symbolLen)
    .toString("utf8")
    .replace(/\0/g, "");

  const uriOffset = symbolOffset + 4 + symbolLen;
  const uriLen = data.readUInt32LE(uriOffset);
  const uri = data
    .subarray(uriOffset + 4, uriOffset + 4 + uriLen)
    .toString("utf8")
    .replace(/\0/g, "");

  console.log("  Name:   ", JSON.stringify(name));
  console.log("  Symbol: ", JSON.stringify(symbol));
  console.log("  URI:    ", JSON.stringify(uri || "(empty)"));
}

/** Fetch and display metadata account details. */
async function verifyMetadata(
  connection: Connection,
  metadataAddress: PublicKey,
): Promise<void> {
  console.log("");
  console.log("--- Verification ---");
  const account = await connection.getAccountInfo(metadataAddress);
  if (!account) {
    console.error(
      "[WARN] Metadata account not found yet – may need a few seconds to propagate",
    );
    return;
  }

  console.log("  Account exists:  YES");
  console.log("  Owner:          ", account.owner.toBase58());
  console.log("  Size:           ", account.data.length, "bytes");
  console.log("  Lamports:       ", account.lamports);

  try {
    parseMetadataFields(account.data);
  } catch (e: any) {
    console.log("  (Could not parse metadata fields:", e.message, ")");
  }

  console.log("");
  console.log("Metadata PDA:", metadataAddress.toBase58());
  console.log("=== DONE ===");
}

/** Extract a base58 signature from a Umi transaction result. */
async function formatSignature(result: Uint8Array): Promise<string> {
  const { default: bs58 } = await import("bs58");
  return bs58.encode(result);
}

/** Send a built Umi transaction and wait for confirmation. */
async function sendAndConfirm(
  umi: any,
  built: any,
  label: string,
): Promise<boolean> {
  console.log(`  Sending ${label} transaction...`);
  const result = await umi.rpc.sendTransaction(built, {
    skipPreflight: false,
    commitment: "confirmed",
  });

  const sig = await formatSignature(result);
  console.log("  [OK] Transaction sent!");
  console.log("  Signature:", sig);
  console.log("  Explorer: https://solscan.io/tx/" + sig);

  console.log("  Waiting for confirmation...");
  await umi.rpc.confirmTransaction(result, {
    strategy: {
      type: "blockhash",
      ...(await umi.rpc.getLatestBlockhash()),
    },
    commitment: "confirmed",
  });

  return true;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  const env = requireScriptEnv();
  console.log("=== CREATE CCM METADATA ===");
  console.log("Cluster:  ", env.cluster);
  console.log("RPC:      ", env.rpcUrl.substring(0, 60) + "...");

  // Load admin keypair
  const web3Keypair = loadKeypairFromFile(env.keypairPath);
  console.log("Admin:    ", web3Keypair.publicKey.toBase58());
  console.log("CCM Mint: ", CCM_MINT.toBase58());

  // Check SOL balance
  const connection = new Connection(env.rpcUrl, "confirmed");
  const balance = await connection.getBalance(web3Keypair.publicKey);
  console.log("Balance:  ", (balance / 1e9).toFixed(4), "SOL");
  if (balance < MIN_SOL_BALANCE) {
    throw new Error(
      "Insufficient SOL balance – need at least 0.01 SOL for metadata rent + fees",
    );
  }

  // Set up Umi
  const umi = createUmi(env.rpcUrl);
  const umiKeypair = fromWeb3JsKeypair(web3Keypair);
  const umiSigner = createSignerFromKeypair(umi, umiKeypair);
  umi.use(signerIdentity(umiSigner));

  const mintPk = fromWeb3JsPublicKey(CCM_MINT);
  const metadataPda = findMetadataPda(umi, { mint: mintPk });
  const metadataAddress = toWeb3JsPublicKey(metadataPda[0]);

  console.log("Metadata: ", metadataAddress.toBase58());
  console.log("");

  // Check if metadata already exists
  if (await metadataExists(connection, metadataAddress)) return;

  // -----------------------------------------------------------------------
  // Approach 1: createV1 (newer Metaplex instruction — supports Token-2022)
  // -----------------------------------------------------------------------
  console.log("[TRY] Approach 1: createV1 (discriminator 42)");
  console.log(
    "  name:",
    TOKEN_NAME,
    "| symbol:",
    TOKEN_SYMBOL,
    "| uri:",
    TOKEN_URI || "(empty)",
  );

  try {
    const tx = createV1(umi, {
      mint: mintPk,
      authority: umiSigner,
      updateAuthority: umiSigner,
      name: TOKEN_NAME,
      symbol: TOKEN_SYMBOL,
      uri: TOKEN_URI,
      sellerFeeBasisPoints: percentAmount(0),
      creators: none(),
      collection: none(),
      uses: none(),
      tokenStandard: TokenStandard.Fungible,
      isMutable: true,
      primarySaleHappened: false,
      collectionDetails: none(),
      decimals: some(9),
      printSupply: none(),
      ruleSet: none(),
      splTokenProgram: publicKey(TOKEN_2022_PROGRAM_ID.toBase58()),
    });

    console.log("  Simulating...");
    const built = await tx.buildAndSign(umi);
    await sendAndConfirm(umi, built, "createV1");

    console.log("");
    console.log("=== SUCCESS (createV1) ===");
    await verifyMetadata(connection, metadataAddress);
    return;
  } catch (err: any) {
    console.log(
      "  [FAIL] createV1 failed:",
      err.message?.substring(0, 200),
    );
    if (err.logs) {
      console.log("  Program logs:");
      err.logs.slice(-8).forEach((l: string) => console.log("    ", l));
    }
    console.log("");
  }

  // -----------------------------------------------------------------------
  // Approach 2: createMetadataAccountV3 (legacy — last resort)
  // -----------------------------------------------------------------------
  console.log("[TRY] Approach 2: createMetadataAccountV3 (discriminator 33)");
  try {
    const tx = createMetadataAccountV3(umi, {
      metadata: metadataPda,
      mint: mintPk,
      mintAuthority: umiSigner,
      payer: umiSigner,
      updateAuthority: umiSigner.publicKey,
      data: {
        name: TOKEN_NAME,
        symbol: TOKEN_SYMBOL,
        uri: TOKEN_URI,
        sellerFeeBasisPoints: 0,
        creators: none(),
        collection: none(),
        uses: none(),
      },
      isMutable: true,
      collectionDetails: none(),
    });

    console.log("  Simulating...");
    const built = await tx.buildAndSign(umi);
    await sendAndConfirm(umi, built, "createMetadataAccountV3");

    console.log("");
    console.log("=== SUCCESS (createMetadataAccountV3) ===");
    await verifyMetadata(connection, metadataAddress);
    return;
  } catch (err: any) {
    console.log(
      "  [FAIL] createMetadataAccountV3 failed:",
      err.message?.substring(0, 200),
    );
    if (err.logs) {
      console.log("  Program logs:");
      err.logs.slice(-8).forEach((l: string) => console.log("    ", l));
    }
    console.log("");
  }

  // -----------------------------------------------------------------------
  // Both failed
  // -----------------------------------------------------------------------
  console.error("=== BOTH APPROACHES FAILED ===");
  console.error(
    "The mint authority is revoked. On-chain metadata creation requires",
  );
  console.error("the mint authority to sign. Possible alternatives:");
  console.error("  1. Submit to the Jupiter Verified Token List");
  console.error("  2. Submit to Solana Token List (legacy-token-list)");
  console.error("  3. Contact Metaplex for a program exception/upgrade");
  process.exit(1);
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  if (err.logs)
    err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
  process.exit(1);
});
