import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const VAULT_PROGRAM = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const BUFFER = new PublicKey("HRWjZAU5d4Pb9FQ2mTMdPffWq5ykRUwgpnkkgGhN76az");
const BPF_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

// Load both member keypairs
const member1 = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, "utf-8"))));
const member2 = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/oracle-authority.json`, "utf-8"))));

console.log("Member 1:", member1.publicKey.toBase58());
console.log("Member 2:", member2.publicKey.toBase58());

const connection = new Connection(RPC_URL, "confirmed");

// Get vault PDA and program data
const [vaultPda] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });
console.log("Vault PDA:", vaultPda.toBase58());

// Get program data address for vault
const programInfo = await connection.getAccountInfo(VAULT_PROGRAM);
const programDataAddress = new PublicKey(programInfo.data.slice(4, 36));
console.log("Program Data:", programDataAddress.toBase58());

// Get next transaction index
const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const txIndex = BigInt(msAccount.transactionIndex) + 1n;
console.log("Transaction Index:", txIndex.toString());

// BPF Loader Upgrade instruction
const upgradeIxData = Buffer.alloc(4);
upgradeIxData.writeUInt32LE(3, 0);

const upgradeIx = {
  programId: BPF_UPGRADEABLE,
  keys: [
    { pubkey: programDataAddress, isSigner: false, isWritable: true },
    { pubkey: VAULT_PROGRAM, isSigner: false, isWritable: true },
    { pubkey: BUFFER, isSigner: false, isWritable: true },
    { pubkey: member1.publicKey, isSigner: false, isWritable: true },
    { pubkey: new PublicKey("11111111111111111111111111111111"), isSigner: false, isWritable: false },
    { pubkey: vaultPda, isSigner: true, isWritable: false },
  ],
  data: upgradeIxData,
};

const { blockhash } = await connection.getLatestBlockhash();

// Create vault transaction
const createTxIx = multisig.instructions.vaultTransactionCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  creator: member1.publicKey,
  vaultIndex: 0,
  ephemeralSigners: 0,
  transactionMessage: new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [upgradeIx],
  }),
  memo: "Upgrade Channel Vault: slippage + bounty",
});

// Create proposal
const createProposalIx = multisig.instructions.proposalCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  creator: member1.publicKey,
});

// Approve with member 1
const approve1Ix = multisig.instructions.proposalApprove({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  member: member1.publicKey,
});

// Build tx with member1
const tx1 = new VersionedTransaction(
  new TransactionMessage({
    payerKey: member1.publicKey,
    recentBlockhash: blockhash,
    instructions: [createTxIx, createProposalIx, approve1Ix],
  }).compileToV0Message()
);
tx1.sign([member1]);

console.log("\nCreating proposal and approving with Member 1...");
const sig1 = await connection.sendTransaction(tx1, { skipPreflight: true });
console.log("Sig:", sig1);
await connection.confirmTransaction(sig1, "confirmed");
console.log("✅ Created and approved by Member 1");

// Now approve with member 2
const { blockhash: bh2 } = await connection.getLatestBlockhash();
const approve2Ix = multisig.instructions.proposalApprove({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  member: member2.publicKey,
});

const tx2 = new VersionedTransaction(
  new TransactionMessage({
    payerKey: member2.publicKey,
    recentBlockhash: bh2,
    instructions: [approve2Ix],
  }).compileToV0Message()
);
tx2.sign([member2]);

console.log("\nApproving with Member 2...");
const sig2 = await connection.sendTransaction(tx2, { skipPreflight: true });
console.log("Sig:", sig2);
await connection.confirmTransaction(sig2, "confirmed");
console.log("✅ Approved by Member 2");

console.log("\n🎯 Proposal #" + txIndex + " has 2/3 approvals!");
console.log("You just need to approve in UI to reach 3/3, then execute.");
