import { Connection, PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { Program, Wallet, AnchorProvider } from "@coral-xyz/anchor";
import fs from "fs";

(async () => {
try {
const RPC_URL = process.env.RPC_URL!;
const KEYPAIR_PATH = process.env.KEYPAIR || "~/.config/solana/relayer.json";
const CHANNEL_CONFIG = new PublicKey("J3HAT4NbL6REyyNqbW1BDGF9BXXc3FYuQ1fr6NbCQaoW");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const AO_PROGRAM = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PROTOCOL_STATE = new PublicKey("596VBoVvzASAhe38CcBSJnv1LdVFPu4EdB8gw1Ko2nx3");

const connection = new Connection(RPC_URL);
const keyPath = KEYPAIR_PATH.replace("~", process.env.HOME!);
const keypairData = JSON.parse(fs.readFileSync(keyPath, "utf-8"));
const payer = Keypair.fromSecretKey(new Uint8Array(keypairData));

const wallet = new Wallet(payer);
const provider = new AnchorProvider(connection, wallet, {});

console.log("Fetching AO program IDL from chain...");
const idl = await Program.fetchIdl(AO_PROGRAM, provider);
if (!idl) throw new Error("IDL not found on-chain");

const program = new Program(idl, AO_PROGRAM, provider);

const [stakePool] = PublicKey.findProgramAddressSync([Buffer.from("channel_pool"), CHANNEL_CONFIG.toBuffer()], AO_PROGRAM);
const [vault] = PublicKey.findProgramAddressSync([Buffer.from("stake_vault"), stakePool.toBuffer()], AO_PROGRAM);
const TOKEN_2022 = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

console.log("Reinitializing stake pool...");
console.log("  Channel Config:", CHANNEL_CONFIG.toString());
console.log("  Stake Pool:", stakePool.toString());
console.log("  Vault:", vault.toString());

const tx = await program.methods
  .initializeStakePool()
  .accounts({
    payer: payer.publicKey,
    protocolState: PROTOCOL_STATE,
    channelConfig: CHANNEL_CONFIG,
    mint: CCM_MINT,
    stakePool,
    vault,
    tokenProgram: TOKEN_2022,
    systemProgram: SystemProgram.programId,
  })
  .rpc();

console.log("✅ Pool reinitialized!");
console.log("   Sig:", tx);
} catch (e) {
  console.error("❌ Error:", (e as any).message);
  process.exit(1);
}
})();
