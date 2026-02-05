import * as multisig from "@sqds/multisig";
import { Connection, PublicKey } from "@solana/web3.js";

const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const latestIdx = Number(msAccount.transactionIndex);

console.log("Checking proposals", latestIdx - 2, "to", latestIdx + 1);

for (let idx = latestIdx - 2; idx <= latestIdx + 1; idx++) {
  try {
    const [proposalPda] = multisig.getProposalPda({ multisigPda: MULTISIG, transactionIndex: BigInt(idx) });
    const proposal = await multisig.accounts.Proposal.fromAccountAddress(connection, proposalPda);
    const status = Object.keys(proposal.status)[0];
    const approved = proposal.approved?.length || 0;
    console.log(`#${idx}: ${status} (${approved}/${msAccount.threshold} approvals)`);
  } catch (e) {
    console.log(`#${idx}: not found`);
  }
}
