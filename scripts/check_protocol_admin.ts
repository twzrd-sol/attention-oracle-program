import { Connection, PublicKey } from "@solana/web3.js";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const PROTOCOL_SEED = Buffer.from("protocol");

async function main() {
  const [protocolState] = PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_MINT.toBuffer()],
    PROGRAM_ID
  );

  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  const info = await connection.getAccountInfo(protocolState);

  if (!info) {
    console.log("ProtocolState not found!");
    return;
  }

  console.log("ProtocolState PDA: " + protocolState.toBase58());
  console.log("Data length: " + info.data.length);

  // Parse ProtocolState
  // Skip 8-byte discriminator
  let offset = 8;
  const version = info.data.readUInt8(offset); offset += 1;
  const bump = info.data.readUInt8(offset); offset += 1;
  const mint = new PublicKey(info.data.subarray(offset, offset + 32)); offset += 32;
  const admin = new PublicKey(info.data.subarray(offset, offset + 32)); offset += 32;
  const publisher = new PublicKey(info.data.subarray(offset, offset + 32)); offset += 32;

  console.log("\n--- ProtocolState ---");
  console.log("Version: " + version);
  console.log("Mint: " + mint.toBase58());
  console.log("Admin: " + admin.toBase58());
  console.log("Publisher: " + publisher.toBase58());
}

main().catch(console.error);
