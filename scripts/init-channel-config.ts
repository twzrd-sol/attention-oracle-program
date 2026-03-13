/**
 * Initialize ChannelConfigV2 for "lofi-vault-3h" on mainnet.
 *
 * This creates the PDA at J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW
 * which is required by the vault compound instruction.
 *
 * Signer: publisher (2pHjZ... = ~/.config/solana/id.json)
 * Program: GnGz... (mainnet AO)
 *
 * Usage:
 *   RPC_URL=<helius_url> npx tsx scripts/init-channel-config.ts [--execute]
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Connection,
} from "@solana/web3.js";
import { readFileSync } from "fs";
import { keccak_256 } from "@noble/hashes/sha3";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const AO_PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const CHANNEL_NAME = "lofi-vault-3h";
const KEYPAIR_PATH = process.env.KEYPAIR || `${process.env.HOME}/.config/solana/id.json`;

// ---------------------------------------------------------------------------
// PDA Derivation
// ---------------------------------------------------------------------------
function deriveSubjectId(channel: string): Buffer {
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(channel.toLowerCase())]);
  return Buffer.from(keccak_256(input));
}

function deriveProtocolState(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("protocol"), CCM_MINT.toBuffer()],
    AO_PROGRAM_ID
  );
}

function deriveChannelConfig(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), deriveSubjectId(CHANNEL_NAME)],
    AO_PROGRAM_ID
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  const execute = process.argv.includes("--execute");
  const rpcUrl = process.env.RPC_URL;
  if (!rpcUrl) {
    console.error("ERROR: RPC_URL env var required");
    process.exit(1);
  }

  console.log("=== INITIALIZE CHANNEL CONFIG V2 ===");
  console.log("Channel:", CHANNEL_NAME);
  console.log("Mode:", execute ? "EXECUTE" : "DRY RUN (add --execute to submit)");
  console.log("");

  // Load keypair
  const keypairData = JSON.parse(readFileSync(KEYPAIR_PATH, "utf-8"));
  const signer = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  console.log("Signer:", signer.publicKey.toBase58());

  // Setup provider
  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(signer);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  // Load IDL from chain
  const idl = await Program.fetchIdl(AO_PROGRAM_ID, provider);
  if (!idl) throw new Error("IDL not found on-chain");
  const program = new Program(idl, provider);

  // Derive PDAs
  const [protocolState] = deriveProtocolState();
  const [channelConfig] = deriveChannelConfig();

  console.log("Protocol State:", protocolState.toBase58());
  console.log("Channel Config:", channelConfig.toBase58());
  console.log("Expected J3HAT:", "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW");
  console.log("Match:", channelConfig.toBase58() === "J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW");
  console.log("");

  // Check if already exists
  const info = await connection.getAccountInfo(channelConfig);
  if (info) {
    console.log("Channel config ALREADY EXISTS!");
    console.log("  Owner:", info.owner.toBase58());
    console.log("  Lamports:", info.lamports);
    console.log("  Data length:", info.data.length, "bytes");
    return;
  }
  console.log("Channel config does NOT exist — will create.");

  // Build instruction
  // Args: channel (string), cutover_epoch (u64), creator_wallet (pubkey), creator_fee_bps (u16)
  const tx = await program.methods
    .initializeChannelCumulative(
      CHANNEL_NAME,
      new BN(0),               // cutover_epoch
      signer.publicKey,        // creator_wallet (admin for now)
      0                        // creator_fee_bps (0% — no creator fee)
    )
    .accounts({
      payer: signer.publicKey,
      protocolState,
      channelConfig,
      systemProgram: SystemProgram.programId,
    });

  if (!execute) {
    // Simulate
    console.log("Simulating...");
    try {
      const simTx = await tx.transaction();
      simTx.feePayer = signer.publicKey;
      const { blockhash } = await connection.getLatestBlockhash();
      simTx.recentBlockhash = blockhash;
      simTx.sign(signer);
      const sim = await connection.simulateTransaction(simTx);
      if (sim.value.err) {
        console.error("SIMULATION FAILED:", JSON.stringify(sim.value.err));
        sim.value.logs?.slice(-10).forEach((l) => console.error("  ", l));
      } else {
        console.log("SIMULATION SUCCESS!");
        console.log("  CU used:", sim.value.unitsConsumed);
        sim.value.logs?.forEach((l) => console.log("  ", l));
      }
    } catch (err: any) {
      console.error("SIMULATION ERROR:", err.message || err);
      if (err.logs) err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
    }
  } else {
    // Execute
    console.log("Submitting transaction...");
    try {
      const sig = await tx.rpc();
      console.log("SUCCESS! Tx:", sig);
      console.log(`  https://solscan.io/tx/${sig}`);

      // Verify
      const verifyInfo = await connection.getAccountInfo(channelConfig);
      if (verifyInfo) {
        console.log("  Verified: account exists with", verifyInfo.data.length, "bytes");
      }
    } catch (err: any) {
      console.error("TX FAILED:", err.message || err);
      if (err.logs) err.logs.slice(-10).forEach((l: string) => console.error("  ", l));
    }
  }
}

main().catch((err) => {
  console.error("FATAL:", err);
  process.exit(1);
});
