/**
 * Example: Check user's passport tier
 *
 * Demonstrates how to fetch and display passport information.
 */

import { Connection, PublicKey } from '@solana/web3.js';
import { AttentionOracleClient, PassportTier } from '@attention-oracle/sdk';

async function main() {
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
  const client = new AttentionOracleClient(connection);

  // User to check
  const userPubkey = new PublicKey('YOUR_WALLET_HERE');

  // Fetch passport
  const passport = await client.getPassport(userPubkey);

  if (!passport) {
    console.log('❌ No passport found for user');
    return;
  }

  console.log('🎫 Passport Information:');
  console.log('   Tier:', PassportTier[passport.tier]);
  console.log('   Points:', passport.points);
  console.log('   Last Update:', new Date(passport.lastUpdate * 1000).toLocaleString());

  // Calculate creator fee based on tier
  const tierMultipliers = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.0];
  const baseCreatorFee = 5; // 0.05%
  const effectiveFee = baseCreatorFee * tierMultipliers[passport.tier];

  console.log('   Creator Fee:', `${effectiveFee / 100}%`);
}

main().catch(console.error);
