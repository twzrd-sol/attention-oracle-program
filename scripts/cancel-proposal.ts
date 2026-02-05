import * as multisig from "@sqds/multisig";
import { Connection, Keypair, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import * as fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG_PUBKEY = new multisig.publicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");

async function main() {
  const keypairPath = `${process.env.HOME}/.config/solana/id.json`;
  const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
  const payer = Keypair.fromSecretKey(new Uint8Array(keypairData));
  const connection = new Connection(RPC_URL, "confirmed");

  // Cancel proposal 38
  const transactionIndex = 38n;
  
  const cancelIx = multisig.instructions.proposalCancel({
    multisigPda: MULTISIG_PUBKEY,
    transactionIndex,
    member: payer.publicKey,
  });

  const { blockhash } = await connection.getLatestBlockhash();
  const tx = new VersionedTransaction(
    new TransactionMessage({
      payerKey: payer.publicKey,
      recentBlockhash: blockhash,
      instructions: [cancelIx],
    }).compileToV0Message()
  );
  tx.sign([payer]);
  
  const sig = await connection.sendTransaction(tx);
  console.log("Cancelled proposal, sig:", sig);
}
main().catch(console.error);
