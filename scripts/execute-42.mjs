import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, "utf-8"))));
const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

const txIndex = 42n;
console.log("Executing proposal #42...");

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

console.log("Sending...");
const sig = await connection.sendTransaction(tx, { skipPreflight: false });
console.log("Signature:", sig);
await connection.confirmTransaction(sig, "confirmed");
console.log("✅ Executed!");
