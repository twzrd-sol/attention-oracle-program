/**
 * Example: Claim tokens using merkle proof
 *
 * This example shows how to claim tokens from a channel using a merkle proof.
 */

import {
  Connection,
  Keypair,
  PublicKey,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import { AttentionOracleClient, MerkleProof, ClaimBuilder } from '@attention-oracle/sdk';

async function main() {
  // Setup connection
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');

  // User's wallet (load from environment or file)
  const userKeypair = Keypair.generate(); // Replace with actual keypair

  // Channel to claim from
  const channelId = 'kaicenat';

  // Merkle proof (obtained from API or off-chain service)
  const proof: MerkleProof = {
    claimer: userKeypair.publicKey,
    index: 42,
    amount: BigInt(1000_000_000), // 1 token (9 decimals)
    id: 'claim_2025_11_18_001',
    proof: [
      // Array of 32-byte proof hashes
      Buffer.from('abcd...', 'hex'),
      Buffer.from('efgh...', 'hex'),
    ],
    epochIndex: 12345,
  };

  // Build claim transaction
  const claimTx = new ClaimBuilder()
    .addClaim(userKeypair.publicKey, channelId, proof)
    .build();

  // Sign and send
  claimTx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  claimTx.feePayer = userKeypair.publicKey;

  const signature = await sendAndConfirmTransaction(connection, claimTx, [userKeypair]);

  console.log('✅ Claimed tokens!');
  console.log('   Signature:', signature);
  console.log('   Amount:', proof.amount.toString());
  console.log('   Explorer:', `https://solscan.io/tx/${signature}`);
}

main().catch(console.error);
