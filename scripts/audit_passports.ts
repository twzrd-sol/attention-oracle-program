import { Connection, PublicKey } from "@solana/web3.js";
import { keccak_256 } from "@noble/hashes/sha3";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const PASSPORT_SEED = Buffer.from("passport_owner");

// Known user_hashes from DB (platform:channel_id format)
const knownIdentities = [
  "youtube:UC-lHJZR3Gqxm24_Vd_AJ5Yw", // lofi girl
  "twitch:lofigirl",
  "spotify:zohaibmohd",
];

function deriveUserHash(identity: string): Buffer {
  return Buffer.from(keccak_256(Buffer.from(identity)));
}

async function main() {
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("=== Passport Registry Audit ===\n");

  // Check known identities
  console.log("Checking known identities:\n");

  for (const identity of knownIdentities) {
    const userHash = deriveUserHash(identity);
    const [passportPda] = PublicKey.findProgramAddressSync(
      [PASSPORT_SEED, userHash],
      PROGRAM_ID
    );

    const info = await connection.getAccountInfo(passportPda);
    if (info) {
      console.log(identity);
      console.log("  PDA: " + passportPda.toBase58());
      console.log("  Rent: " + (info.lamports / 1e9).toFixed(4) + " SOL");

      // Parse passport data (skip 8-byte discriminator)
      const data = info.data;
      let offset = 8;
      const version = data.readUInt8(offset); offset += 1;
      const bump = data.readUInt8(offset); offset += 1;
      offset += 32; // user_hash
      const owner = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
      const tier = data.readUInt8(offset); offset += 1;
      const score = data.readBigUInt64LE(offset);

      console.log("  Owner: " + owner.toBase58());
      console.log("  Tier: " + tier);
      console.log("  Score: " + score.toString());
      console.log("");
    } else {
      console.log(identity + ": NO PASSPORT");
    }
  }

  // Use getProgramAccounts to find all passports
  console.log("\n--- Enumerating All Passports ---");

  try {
    // Passport discriminator = first 8 bytes of sha256("account:PassportRegistry")
    // For Anchor, it's sha256("account:PassportRegistry")[0..8]
    const accounts = await connection.getProgramAccounts(PROGRAM_ID, {
      filters: [
        { dataSize: 213 }, // PassportRegistry::LEN
      ],
    });

    console.log("Total passport accounts found: " + accounts.length);

    if (accounts.length > 0) {
      let totalRent = 0;
      for (const { pubkey, account } of accounts) {
        totalRent += account.lamports;
        // Could parse each one but just count for now
      }
      console.log("Total rent in passports: " + (totalRent / 1e9).toFixed(4) + " SOL");
    }
  } catch (e: any) {
    console.log("Error enumerating: " + e.message);
  }
}

main().catch(console.error);
