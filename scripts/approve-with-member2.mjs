import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");

// Load Member 2 keypair (oracle-authority)
const keypairPath = `${process.env.HOME}/.config/solana/oracle-authority.json`;
const member2 = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, "utf-8"))));
console.log("Member 2:", member2.publicKey.toBase58());

const connection = new Connection(RPC_URL, "confirmed");

// Get latest transaction index
const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const latestIdx = Number(msAccount.transactionIndex);
console.log("Latest tx index:", latestIdx);

// Check recent proposals for active ones
console.log("\nChecking proposals...");
for (let idx = latestIdx; idx >= Math.max(latestIdx - 5, 1); idx--) {
  try {
    const [proposalPda] = multisig.getProposalPda({ multisigPda: MULTISIG, transactionIndex: BigInt(idx) });
    const proposal = await multisig.accounts.Proposal.fromAccountAddress(connection, proposalPda);
    const status = Object.keys(proposal.status)[0];
    const approved = proposal.approved?.length || 0;
    const approvers = proposal.approved?.map(m => m.toBase58().slice(0,4)) || [];
    
    console.log(`#${idx}: ${status} (${approved}/3) - approvers: [${approvers.join(', ')}]`);
    
    // If active and member2 hasn't approved, approve it
    if (status === 'active' || status === 'Active') {
      const alreadyApproved = proposal.approved?.some(m => m.equals(member2.publicKey));
      if (!alreadyApproved) {
        console.log(`  -> Approving #${idx} with Member 2...`);
        
        const approveIx = multisig.instructions.proposalApprove({
          multisigPda: MULTISIG,
          transactionIndex: BigInt(idx),
          member: member2.publicKey,
        });
        
        const { blockhash } = await connection.getLatestBlockhash();
        const tx = new VersionedTransaction(
          new TransactionMessage({
            payerKey: member2.publicKey,
            recentBlockhash: blockhash,
            instructions: [approveIx],
          }).compileToV0Message()
        );
        tx.sign([member2]);
        
        const sig = await connection.sendTransaction(tx, { skipPreflight: true });
        console.log(`  ✅ Approved! Sig: ${sig}`);
        await connection.confirmTransaction(sig, "confirmed");
      } else {
        console.log(`  -> Already approved by Member 2`);
      }
    }
  } catch (e) {
    if (!e.message?.includes('Account does not exist')) {
      console.log(`#${idx}: error - ${e.message?.slice(0, 50)}`);
    }
  }
}
