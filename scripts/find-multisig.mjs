import { Connection, PublicKey } from "@solana/web3.js";
import * as multisig from "@sqds/multisig";

const VAULT = new PublicKey("2v9jrkuJM99uf4xDFwfyxuzoNmqfggqbuW34mad2n6kW");
const connection = new Connection("https://api.mainnet-beta.solana.com", "confirmed");

// The vault PDA is derived as: seeds = ["squad", multisig.toBytes(), [vault_index], "vault"]
// We need to find what multisig produces this vault

// Try the known multisig
const knownMultisig = new PublicKey("BX2fRy4Jfko3cMttDmn2n6CaHfa9iAqT69YgAKZis9EQ");
const [derivedVault] = multisig.getVaultPda({ multisigPda: knownMultisig, index: 0 });

console.log("Known multisig:", knownMultisig.toBase58());
console.log("Derived vault:", derivedVault.toBase58());
console.log("Target vault:", VAULT.toBase58());
console.log("Match:", derivedVault.equals(VAULT));

// Check if vault itself could be a multisig
try {
  const account = await multisig.accounts.Multisig.fromAccountAddress(connection, VAULT);
  console.log("\nVault IS a multisig! Members:", account.members.length);
} catch (e) {
  console.log("\nVault is NOT a multisig account");
}

// Check known multisig
try {
  const account = await multisig.accounts.Multisig.fromAccountAddress(connection, knownMultisig);
  console.log("\nBX2f IS a multisig! Threshold:", account.threshold, "Members:", account.members.length);
  console.log("Transaction index:", account.transactionIndex.toString());
} catch (e) {
  console.log("\nBX2f is NOT a valid multisig:", e.message);
}
