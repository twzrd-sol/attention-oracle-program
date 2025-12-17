#!/usr/bin/env ts-node

/**
 * Set metadata for CCM-v3 token using Metaplex Token Metadata
 *
 * Creates on-chain metadata:
 * - name: "Creator Currency Merkle"
 * - symbol: "CCM"
 * - uri: JSON metadata (description + image)
 *
 * Usage:
 *   npx ts-node scripts/set-token-metadata-v3.ts [--dry-run]
 */

import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import {
  createMetadataAccountV3,
  mplTokenMetadata,
  findMetadataPda
} from "@metaplex-foundation/mpl-token-metadata";
import {
  keypairIdentity,
  publicKey,
  createSignerFromKeypair
} from "@metaplex-foundation/umi";
import { fromWeb3JsKeypair, fromWeb3JsPublicKey, toWeb3JsPublicKey } from "@metaplex-foundation/umi-web3js-adapters";
import { Keypair, PublicKey, Connection } from "@solana/web3.js";
import fs from "fs";

import { CCM_V3_MINT, getRpcUrl, getWalletPath } from "./config.js";

// Metadata
const TOKEN_NAME = "Creator Currency Merkle";
const TOKEN_SYMBOL = "CCM";
// Placeholder URI - replace with actual Arweave/IPFS link
const TOKEN_URI = "https://arweave.net/placeholder-ccm-v3-metadata";

async function main() {
  console.log("=== CCM-v3 Token Metadata ===\n");

  const dryRun = process.argv.includes("--dry-run");

  // Load admin wallet
  const web3jsKeypair = Keypair.fromSecretKey(
    new Uint8Array(JSON.parse(fs.readFileSync(getWalletPath(), "utf-8")))
  );
  console.log("Admin (Update Authority):", web3jsKeypair.publicKey.toBase58());

  // Setup connection and Umi
  const rpcUrl = getRpcUrl();
  console.log("RPC:", rpcUrl.substring(0, 50) + "...");
  console.log("Mint:", CCM_V3_MINT.toBase58());

  const umi = createUmi(rpcUrl)
    .use(mplTokenMetadata());

  // Convert web3.js keypair to Umi keypair and set as identity
  const umiKeypair = fromWeb3JsKeypair(web3jsKeypair);
  umi.use(keypairIdentity(umiKeypair));

  // Convert mint to Umi public key
  const mintPubkey = fromWeb3JsPublicKey(CCM_V3_MINT);

  // Derive metadata PDA
  const metadataPda = findMetadataPda(umi, { mint: mintPubkey });
  const metadataPdaWeb3 = toWeb3JsPublicKey(metadataPda[0]);

  console.log("\n=== Addresses ===");
  console.log("Mint:", CCM_V3_MINT.toBase58());
  console.log("Metadata PDA:", metadataPdaWeb3.toBase58());

  console.log("\n=== Metadata ===");
  console.log("Name:", TOKEN_NAME);
  console.log("Symbol:", TOKEN_SYMBOL);
  console.log("URI:", TOKEN_URI);

  // Check if metadata already exists
  const connection = new Connection(rpcUrl, "confirmed");
  const metadataInfo = await connection.getAccountInfo(metadataPdaWeb3);
  if (metadataInfo) {
    console.log("\n⚠️  Metadata already exists!");
    console.log("Use updateMetadataAccountV2 to modify.");
    process.exit(0);
  }

  if (dryRun) {
    console.log("\n=== DRY RUN ===");
    console.log("Transaction would create metadata account:");
    console.log("  Name:", TOKEN_NAME);
    console.log("  Symbol:", TOKEN_SYMBOL);
    console.log("  URI:", TOKEN_URI);
    console.log("\nTo execute, run without --dry-run flag");
    process.exit(0);
  }

  console.log("\n=== Executing ===");
  console.log("Creating metadata account...");

  try {
    // Create metadata using Metaplex SDK
    const result = await createMetadataAccountV3(umi, {
      metadata: metadataPda,
      mint: mintPubkey,
      mintAuthority: umi.identity,
      payer: umi.identity,
      updateAuthority: umi.identity.publicKey,
      data: {
        name: TOKEN_NAME,
        symbol: TOKEN_SYMBOL,
        uri: TOKEN_URI,
        sellerFeeBasisPoints: 0,
        creators: null,
        collection: null,
        uses: null,
      },
      isMutable: true,
      collectionDetails: null,
    }).sendAndConfirm(umi);

    const sig = Buffer.from(result.signature).toString('base64');

    console.log("\n✅ Metadata Created!");
    console.log("Signature:", sig);
    console.log(`Token: https://solscan.io/token/${CCM_V3_MINT.toBase58()}`);

    console.log("\n=== Next Steps ===");
    console.log("1. Upload actual metadata JSON to Arweave/IPFS");
    console.log("2. Update URI with updateMetadataAccountV2");
    console.log("3. Create Meteora pool");

  } catch (err: any) {
    console.error("\n❌ Error:", err.message || err);
    if (err.logs) {
      console.error("Logs:", err.logs.join("\n"));
    }
    process.exit(1);
  }
}

main().catch(console.error);
