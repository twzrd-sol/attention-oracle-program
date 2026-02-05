import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const AO_PROGRAM = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const TARGET_AUTHORITY = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const BPF_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, "utf-8"))));
const connection = new Connection(RPC_URL, "confirmed");

// Get program data address
const programInfo = await connection.getAccountInfo(AO_PROGRAM);
const programDataAddress = new PublicKey(programInfo.data.slice(4, 36));
console.log("Program Data:", programDataAddress.toBase58());

const [vaultPda] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });
console.log("Current authority (vault):", vaultPda.toBase58());
console.log("Target authority:", TARGET_AUTHORITY.toBase58());

// SetAuthority instruction (discriminator = 4)
// Layout: [4u8, Option<Pubkey>] where Option is [1u8, Pubkey] for Some
const setAuthorityData = Buffer.alloc(37);
setAuthorityData.writeUInt32LE(4, 0); // SetAuthority discriminator
setAuthorityData.writeUInt8(1, 4);    // Some
TARGET_AUTHORITY.toBuffer().copy(setAuthorityData, 5);

const setAuthorityIx = {
  programId: BPF_UPGRADEABLE,
  keys: [
    { pubkey: programDataAddress, isSigner: false, isWritable: true },
    { pubkey: vaultPda, isSigner: true, isWritable: false }, // current authority
    { pubkey: TARGET_AUTHORITY, isSigner: false, isWritable: false }, // new authority (optional for close)
  ],
  data: setAuthorityData,
};

// Simulate first
console.log("\nSimulating SetAuthority via Squads vault...");
const { blockhash } = await connection.getLatestBlockhash();

const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const txIndex = BigInt(msAccount.transactionIndex) + 1n;

const createTxIx = multisig.instructions.vaultTransactionCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  creator: payer.publicKey,
  vaultIndex: 0,
  ephemeralSigners: 0,
  transactionMessage: new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [setAuthorityIx],
  }),
  memo: "TEST: Transfer AO upgrade authority to keypair",
});

// Just simulate the create (don't actually send)
const simTx = new VersionedTransaction(
  new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [createTxIx],
  }).compileToV0Message()
);

const simResult = await connection.simulateTransaction(simTx);
if (simResult.value.err) {
  console.log("❌ Create simulation failed:", simResult.value.err);
  console.log("Logs:", simResult.value.logs?.slice(-5));
} else {
  console.log("✅ Create simulation passed - SetAuthority proposal CAN be created");
  console.log("\nWant me to actually create this proposal? (It would transfer AO upgrade authority to your keypair)");
}
