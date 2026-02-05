import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const VAULT_PROGRAM = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const TARGET = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const BPF = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, "utf-8"))));
const member2 = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/oracle-authority.json`, "utf-8"))));
const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

const programInfo = await connection.getAccountInfo(VAULT_PROGRAM);
const programData = new PublicKey(programInfo.data.slice(4, 36));
const [vault] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });

console.log("Creating proposal to transfer Channel Vault authority...");
console.log("Program Data:", programData.toBase58());

// SetAuthority instruction
const data = Buffer.alloc(37);
data.writeUInt32LE(4, 0);
data.writeUInt8(1, 4);
TARGET.toBuffer().copy(data, 5);

const setAuthIx = {
  programId: BPF,
  keys: [
    { pubkey: programData, isSigner: false, isWritable: true },
    { pubkey: vault, isSigner: true, isWritable: false },
    { pubkey: TARGET, isSigner: false, isWritable: false },
  ],
  data,
};

const ms = await multisig.accounts.Multisig.fromAccountAddress(connection, MULTISIG);
const txIdx = BigInt(ms.transactionIndex) + 1n;
const { blockhash } = await connection.getLatestBlockhash();

const createIx = multisig.instructions.vaultTransactionCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIdx,
  creator: payer.publicKey,
  vaultIndex: 0,
  ephemeralSigners: 0,
  transactionMessage: new TransactionMessage({
    payerKey: vault,
    recentBlockhash: blockhash,
    instructions: [setAuthIx],
  }),
  memo: "Transfer Channel Vault upgrade authority to keypair (enables direct upgrades)",
});

const propIx = multisig.instructions.proposalCreate({
  multisigPda: MULTISIG,
  transactionIndex: txIdx,
  creator: payer.publicKey,
});

const tx = new VersionedTransaction(
  new TransactionMessage({
    payerKey: payer.publicKey,
    recentBlockhash: blockhash,
    instructions: [createIx, propIx],
  }).compileToV0Message()
);
tx.sign([payer]);

const sig = await connection.sendTransaction(tx, { skipPreflight: true });
await connection.confirmTransaction(sig, "confirmed");
console.log("✅ Created proposal #" + txIdx);

// Approve with both members
for (const [name, member] of [["Member1", payer], ["Member2", member2]]) {
  const { blockhash: bh } = await connection.getLatestBlockhash();
  const appIx = multisig.instructions.proposalApprove({
    multisigPda: MULTISIG,
    transactionIndex: txIdx,
    member: member.publicKey,
  });
  const appTx = new VersionedTransaction(
    new TransactionMessage({
      payerKey: member.publicKey,
      recentBlockhash: bh,
      instructions: [appIx],
    }).compileToV0Message()
  );
  appTx.sign([member]);
  await connection.sendTransaction(appTx, { skipPreflight: true });
  console.log(`✅ ${name} approved`);
}

console.log("\n📋 Proposal #" + txIdx + " ready - approve in UI to reach 3/3");
