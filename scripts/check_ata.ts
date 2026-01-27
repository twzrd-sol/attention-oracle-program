import { Connection, PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddressSync } from "@solana/spl-token";

const CCM_MINT = new PublicKey("Dxk8mAb3C7AM8JN6tAJfVuSja5yidhZM5sEKW3SRX2BM");
const TOKEN_2022 = new PublicKey("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const WALLET = new PublicKey("4MAxm5mcPLNc7iokTPFt76JXHEAhAMcrQ3NY5yGmAKgy");

async function main() {
  const ata = getAssociatedTokenAddressSync(CCM_MINT, WALLET, false, TOKEN_2022);
  console.log("ATA:", ata.toBase58());
  
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const conn = new Connection(rpcUrl);
  const info = await conn.getAccountInfo(ata);
  
  if (!info) {
    console.log("ATA not found");
    return;
  }
  
  // Token-2022 account: skip extension header if present
  // Standard layout: mint (32) + owner (32) + amount (8) + delegate_option (4) + ...
  const amount = info.data.readBigUInt64LE(64);
  console.log("Raw balance:", amount.toString());
  console.log("Display balance (9 decimals):", Number(amount) / 1e9);
}
main().catch(console.error);
