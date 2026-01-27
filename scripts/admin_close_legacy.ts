import pkg from "@coral-xyz/anchor";
const { Program, AnchorProvider, Wallet } = pkg;
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_STATE = new PublicKey("596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3");

// Legacy channels to close
const LEGACY_CHANNELS = [
  "spotify:oracle",
  // These don't exist on-chain but include for completeness
  // "youtube",
  // "twitch",
  // "youtube_lofi",
  // "lofi_girl",
];

function deriveChannelPda(channel: string): PublicKey {
  const lower = channel.toLowerCase();
  const input = Buffer.concat([Buffer.from("channel:"), Buffer.from(lower)]);
  const hash = Buffer.from(keccak_256(input));

  return PublicKey.findProgramAddressSync(
    [Buffer.from("channel_cfg_v2"), CCM_MINT.toBuffer(), hash],
    PROGRAM_ID
  )[0];
}

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  // Use id.json (2pHjZLqs...) which is the protocol admin
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.join(process.env.HOME!, ".config/solana/id.json"), "utf-8")))
  );
  const wallet = new Wallet(adminKeypair);
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });

  const idl = JSON.parse(fs.readFileSync(path.join(__dirname, "../target/idl/token_2022.json"), "utf-8"));
  const program = new Program(idl, provider);

  console.log("=== Closing Legacy Channels ===");
  console.log("Admin: " + adminKeypair.publicKey.toBase58());
  console.log("Expected admin: 2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
  console.log("");

  for (const channel of LEGACY_CHANNELS) {
    console.log("Processing: " + channel);

    try {
      const pda = deriveChannelPda(channel);
      console.log("  PDA: " + pda.toBase58());

      // Check if account exists
      const info = await connection.getAccountInfo(pda);
      if (!info) {
        console.log("  Status: Already closed or doesn't exist");
        continue;
      }

      console.log("  Rent: " + (info.lamports / 1e9).toFixed(4) + " SOL");
      console.log("  Closing...");

      const tx = await program.methods
        .closeChannel(channel)
        .accounts({
          admin: adminKeypair.publicKey,
          protocolState: PROTOCOL_STATE,
          channelConfig: pda,
        })
        .rpc();

      console.log("  ✅ Closed. Tx: https://solscan.io/tx/" + tx);
    } catch (e: any) {
      console.error("  ❌ Error: " + (e.message || e));
      if (e.logs) {
        console.error("  Logs:");
        e.logs.slice(-5).forEach((log: string) => console.error("    " + log));
      }
    }
    console.log("");
  }
}

main().catch(console.error);
