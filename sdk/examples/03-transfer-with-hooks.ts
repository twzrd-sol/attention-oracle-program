/**
 * Example: Transfer tokens with dynamic fee calculation
 *
 * Shows how Token-2022 transfer hooks automatically calculate fees based on passport tier.
 */

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import {
  TOKEN_2022_PROGRAM_ID,
  createTransferCheckedInstruction,
  getAssociatedTokenAddress,
} from '@solana/spl-token';
import { AttentionOracleClient } from '@attention-oracle/sdk';

async function main() {
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
  const client = new AttentionOracleClient(connection);

  // Wallet keypairs
  const sender = Keypair.generate(); // Replace with actual
  const recipient = new PublicKey('RECIPIENT_WALLET');

  // Token mint
  const mint = new PublicKey('AAHd7u22jCMgmbF7ATkiY3BhkifD4MN3Vbsy4eYQGWN5');

  // Get token accounts
  const senderAta = await getAssociatedTokenAddress(mint, sender.publicKey, false, TOKEN_2022_PROGRAM_ID);
  const recipientAta = await getAssociatedTokenAddress(mint, recipient, false, TOKEN_2022_PROGRAM_ID);

  // Check sender's passport tier (optional - just for display)
  const passport = await client.getPassport(sender.publicKey);
  console.log('📤 Transfer Details:');
  console.log('   From:', sender.publicKey.toBase58());
  console.log('   To:', recipient.toBase58());
  console.log('   Sender Tier:', passport ? passport.tier : 'No passport');

  // Create transfer instruction
  // NOTE: Transfer hook will automatically:
  // 1. Look up sender's passport tier
  // 2. Calculate dynamic fee (treasury 0.05% + creator 0.05% * tier_multiplier)
  // 3. Emit TransferFeeEvent for off-chain tracking
  const transferIx = createTransferCheckedInstruction(
    senderAta,
    mint,
    recipientAta,
    sender.publicKey,
    1_000_000_000, // 1 token (9 decimals)
    9, // decimals
    [],
    TOKEN_2022_PROGRAM_ID
  );

  // Build and send transaction
  const tx = new Transaction().add(transferIx);
  tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
  tx.feePayer = sender.publicKey;

  const signature = await sendAndConfirmTransaction(connection, tx, [sender]);

  console.log('✅ Transfer complete!');
  console.log('   Signature:', signature);
  console.log('   Note: Fees calculated automatically based on passport tier');
  console.log('   Explorer:', `https://solscan.io/tx/${signature}`);
}

main().catch(console.error);
