import * as multisig from "@sqds/multisig";
import { Connection, Keypair, PublicKey, TransactionMessage, VersionedTransaction } from "@solana/web3.js";
import fs from "fs";

const MULTISIG = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const TARGET = new PublicKey("2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD");
const BPF = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

const BUFFERS = [
  { address: "HRWjZAU5d4Pb9FQ2mTMdPffWq5ykRUwgpnkkgGhN76az", name: "Vault buffer", sol: "5.03" },
  { address: "AJREAuqzkev8hZDopUabUqJ3bozdBbftaJnV67MXjn1m", name: "AO buffer", sol: "6.38" },
];

const payer = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/id.json`, "utf-8"))));
const member2 = Keypair.fromSecretKey(new Uint8Array(JSON.parse(fs.readFileSync(`${process.env.HOME}/.config/solana/oracle-authority.json`, "utf-8"))));
const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");
const [vault] = multisig.getVaultPda({ multisigPda: MULTISIG, index: 0 });

for (const buf of BUFFERS) {
  const bufferPubkey = new PublicKey(buf.address);
  console.log(`\nCreating proposal for ${buf.name} (${buf.sol} SOL)...`);

  // SetBufferAuthority instruction (same as SetAuthority but for buffers)
  // For buffers, the account layout is: [buffer_account, current_authority, new_authority]
  const data = Buffer.alloc(37);
  data.writeUInt32LE(4, 0); // SetAuthority = 4
  data.writeUInt8(1, 4);    // Some(new_authority)
  TARGET.toBuffer().copy(data, 5);

  const setAuthIx = {
    programId: BPF,
    keys: [
      { pubkey: bufferPubkey, isSigner: false, isWritable: true },
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
    memo: `Transfer ${buf.name} authority to keypair (recover ${buf.sol} SOL)`,
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
  console.log(`✅ Created proposal #${txIdx}`);

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
  }
  console.log(`✅ 2/3 approved`);
}

console.log("\n📋 All buffer authority proposals ready - approve in UI to reach 3/3");
