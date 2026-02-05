import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");

const keypairPath = `${process.env.HOME}/.config/solana/id.json`;
const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, "utf-8"))));
const connection = new Connection(RPC_URL, "confirmed");

// Check latest transaction index
const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
console.log("Multisig threshold:", msAccount.threshold);
console.log("Latest tx index:", msAccount.transactionIndex.toString());

// Check proposals 38 and 39
for (const idx of [38n, 39n]) {
  try {
    const [proposalPda] = multisig.getProposalPda({ multisigPda: MULTISIG, transactionIndex: idx });
    const proposal = await multisig.accounts.Proposal.fromAccountAddress(connection, proposalPda);
    console.log(`\nProposal ${idx}:`);
    console.log("  Status:", Object.keys(proposal.status)[0]);
    console.log("  Approved:", proposal.approved?.length || 0);
    console.log("  Rejected:", proposal.rejected?.length || 0);
    
    // If approved >= threshold, try to execute
    if ((proposal.approved?.length || 0) >= msAccount.threshold) {
      console.log("  -> Ready to execute!");
    }
  } catch (e) {
    console.log(`\nProposal ${idx}: Not found or error -`, e.message?.slice(0, 50));
  }
}
