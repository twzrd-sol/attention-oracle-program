import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const RPC_URL = "https://api.mainnet-beta.solana.com";
const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const PROGRAM_ID = new PublicKey("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop");
const BUFFER = new PublicKey("4XP54FJrgabhvNxE8bTpxqcJre5PSRzfniTPr2aBM6g8");
const PROGRAM_DATA = new PublicKey("5GyaaVmzRr2r9KcUuzt9SxBVq9ubTT5m3pH9Lzy3Kh4L");
const BPF_UPGRADEABLE = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

const keypairPath = `${process.env.HOME}/.config/solana/id.json`;
const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(keypairPath, "utf-8"))));

const connection = new Connection(RPC_URL, "confirmed");

// Get vault PDA
const [vaultPda] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });
console.log("Vault PDA:", vaultPda.toBase58());

// Get next transaction index
const msAccount = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const txIndex = BigInt(msAccount.transactionIndex) + 1n;
console.log("Transaction index:", txIndex.toString());

// BPF Loader Upgrade instruction (discriminator = 3)
const upgradeIxData = Buffer.alloc(4);
upgradeIxData.writeUInt32LE(3, 0);

const upgradeIx = {
  programId: BPF_UPGRADEABLE,
  keys: [
    { pubkey: PROGRAM_DATA, isSigner: false, isWritable: true },
    { pubkey: PROGRAM_ID, isSigner: false, isWritable: true },
    { pubkey: BUFFER, isSigner: false, isWritable: true },
    { pubkey: payer.publicKey, isSigner: false, isWritable: true },  // spill
    { pubkey: new PublicKey("11111111111111111111111111111111"), isSigner: false, isWritable: false },
    { pubkey: vaultPda, isSigner: true, isWritable: false },  // authority
  ],
  data: upgradeIxData,
};

const { blockhash } = await connection.getLatestBlockhash();

// Create vault transaction
const createTxIx = multisig.instructions.vaultTransactionCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  creator: payer.publicKey,
  vaultIndex: 0,
  ephemeralSigners: 0,
  transactionMessage: new TransactionMessage({
    payerKey: vaultPda,
    recentBlockhash: blockhash,
    instructions: [upgradeIx],
  }),
  memo: "Upgrade AO program: V3 proof expiry",
});

// Create proposal
const createProposalIx = multisig.instructions.proposalCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  creator: payer.publicKey,
});

// Approve
const approveIx = multisig.instructions.proposalApprove({
  multisigPda: MULTISIG,
  transactionIndex: txIndex,
  member: payer.publicKey,
});

const tx = new VersionedTransaction(
  new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [createTxIx, createProposalIx, approveIx],
  }).compileToV0Message()
);
tx.sign([payer]);

console.log("Sending...");
const sig = await connection.sendTransaction(tx, { skipPreflight: true });
console.log("Signature:", sig);
await connection.confirmTransaction(sig, "confirmed");
console.log("✅ Done! Check Squads UI for transaction", txIndex.toString());
