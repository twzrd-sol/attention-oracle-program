import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");

const keypairPath = `${process.env.HOME}/.config/solana/id.json`;
const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, "utf-8"))));
const connection = new Connection(RPC_URL, "confirmed");

const txIndex = 40n;
console.log("Executing proposal #40...");

// Get vault PDA
const [vaultPda] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });

// Execute the vault transaction
const executeIx = await multisig.instructions.vaultTransactionExecute({
  connection,
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  member: payer.publicKey,
});

const { blockhash } = await connection.getLatestBlockhash();
const tx = new VersionedTransaction(
  new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [executeIx.instruction],
  }).compileToV0Message(executeIx.lookupTableAccounts)
);
tx.sign([payer]);

console.log("Sending execute transaction...");
const sig = await connection.sendTransaction(tx, { skipPreflight: true });
console.log("Signature:", sig);
await connection.confirmTransaction(sig, "confirmed");
console.log("✅ Executed!");
