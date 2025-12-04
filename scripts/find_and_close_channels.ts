import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";

const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const RPC_URL = "https://mainnet.helius-rpc.com/?api-key=4323eb4e-974c-49da-bbb9-ea11b1165a25";

// Known channel addresses from earlier investigation
const KNOWN_CHANNELS = [
  { name: "quin69", address: "53hi3PxBF3M6g55Z4y3ngW7o3CUug6nHDK4Y49jUAGKE" },
  { name: "kato_junichi0817", address: "5xZobYEX5fh45UdZZAf7qQUwNCxDqQ84ZGWMK7Rp7VJK" },
  { name: "eslcs", address: "24nzvGUbyDiqS7vuHE7CxD5872GN2bLZsB5454iBN22W" },
  { name: "jynxzi", address: "HJcbUr1Z1d3ogo1LReVPx1CMAVABLcjwD3FYXZ5SqMLr" },
  { name: "sasavot", address: "9MtoCxfb61H1jEcbph43MQwmb6io5VBjUA1K6ZP5CgQT" },
  { name: "xqc", address: "EM4vp9Wr6PeYkUmMnwHAqaTHJJeW8KQfSXwGuwBNyFnw" },
  { name: "jasontheween", address: "CoLAk24k6kpsLx7YC9AswPsR25S2zs8djLqG8zoVUJxq" },
  { name: "fps_shaka", address: "ahCnsmMqeXzqip2LnXt8s3uSSnBzx3kqypLonwPrkdq" },
  { name: "moonmoon", address: "BkAyn65NgLFpLf49kAqZhoUTs5B8YtYSiFzHbo2m82Bd" },
  { name: "summit1g", address: "D4hB6rVCJDWnYEMyR9pAmfJ53GxmqHV9dsLmwSuzcxLZ" },
  { name: "k4sen", address: "3QJQSkqeGsk5t8M6UFrgHxYgToYKCwzK8QjxyLo11SnW" },
  { name: "alanzoka", address: "7GUuDjiuJkPJUjNcWuZDQtwzw3E8zMKKpJ7wqcFVSFWe" },
  { name: "traytonlol", address: "GBRf6ihtTkFF5M4JSwheMEGon8Jwib1rT6v9hQcnQuZa" },
  { name: "stableronaldo", address: "CDN1bPkLEPFnySrDwyniCkC6g3RwnHS1sQ1UiX2CXtGL" },
  { name: "rubius", address: "UwTQnW44LDW91YfDAKdary2YfWBaDrHZmDKMDyzEBom" },
  { name: "zarbex", address: "B2huq8M1Vgrc3u8pYQXDD4xNjmTeRgRNJktNyyzJrXer" },
  { name: "yourragegaming", address: "6D1gpWV7gDfesXqrjPkiASbEZyFnX7QDxGPgRbbtwTBN" },
  { name: "hasanabi", address: "3MwwvGUY2vTg8Jv5Mr8tXYKZeHc7BD4L93KyyMiEwcFx" },
  { name: "lirik", address: "4j6QLRaDdpwaXQXLQj2zRuRpLqvVgXWyFbHWrjarBC66" },
  { name: "adapt", address: "1a4tbANHvtqPrmaN7DVP3HHCryNFNFcqmBucqPf1a3N" },
  { name: "lacy", address: "81RbwJX5F2rko4MsuSoBf2cN9fwgmL4PCWt2NyBHT6C9" },
  { name: "dakillzor", address: "3jMnVF2kV3LoC7oxn7CHDskX9zdWNbf6a64bph3Fq4FT" },
  { name: "caseoh_", address: "AEi6orE4DUtp6fUhS5khGdKjP4RamCwJnaSKVrUB6Wvj" },
  { name: "nmplol", address: "EJx5aFWDbknUCsog4sY2tBQd1vvZGaL4eZ7QWvU86acv" },
  { name: "emiru", address: "8or8F9QR1F4WqSY62xUhiY3mFHxQwGmqP2tARZTWn1Dc" },
  { name: "zackrawrr", address: "6mq6dGrdMEJaS6xymfdQcNRa7DT3t5KQz4Pm7BntuaVR" },
];

async function closeChannel(connection: Connection, admin: Keypair, name: string, address: string) {
  const channelState = new PublicKey(address);

  console.log(`Closing ${name}...`);
  console.log(`  Channel State: ${address}`);

  try {
    // No longer need to extract subject_id - instruction doesn't require it
    const accountInfo = await connection.getAccountInfo(channelState);
    if (!accountInfo) {
      console.log(`❌ Account doesn't exist: ${name}`);
      return null;
    }

    // Discriminator for "force_close_channel_state_legacy" - no parameters needed
    const discriminator = Buffer.from([0x8a, 0x5e, 0xfd, 0xff, 0x72, 0x7c, 0x27, 0x71]);

    const tx = new anchor.web3.Transaction().add(
      new anchor.web3.TransactionInstruction({
        programId: PROGRAM_ID,
        keys: [
          { pubkey: admin.publicKey, isSigner: true, isWritable: true },
          { pubkey: channelState, isSigner: false, isWritable: true },
        ],
        data: discriminator,
      })
    );

    const sig = await anchor.web3.sendAndConfirmTransaction(connection, tx, [admin], {
      commitment: "confirmed",
    });

    console.log(`✅ Closed ${name}: ${sig}`);
    return sig;
  } catch (err: any) {
    console.error(`❌ Failed to close ${name}: ${err.message}`);
    return null;
  }
}

async function main() {
  const connection = new Connection(RPC_URL, "confirmed");
  const adminKeypair = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync("/home/twzrd/.config/solana/amm-admin.json", "utf-8")))
  );

  console.log(`Admin: ${adminKeypair.publicKey.toBase58()}`);
  console.log(`Closing ${KNOWN_CHANNELS.length} Twitch channels...\n`);

  let closed = 0;
  for (const channel of KNOWN_CHANNELS) {
    const sig = await closeChannel(connection, adminKeypair, channel.name, channel.address);
    if (sig) closed++;
    await new Promise(r => setTimeout(r, 1000));
  }

  console.log(`\n✅ Closed ${closed}/${KNOWN_CHANNELS.length} channels`);
  console.log(`💰 Recovered ~${(closed * 0.04).toFixed(2)} SOL in rent`);
}

main().catch(console.error);
