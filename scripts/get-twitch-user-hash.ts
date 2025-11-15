#!/usr/bin/env tsx
/**
 * Get Twitch User ID and Hash for ZoWzrd
 * Uses the hashUser function from aggregator to match database lookups
 */

import { createHash } from 'crypto';

// Hash function from aggregator (db-types.ts)
function hashUser(userId: string): string {
  return createHash('sha256').update(userId.toLowerCase()).digest('hex');
}

async function getTwitchUserDetails(username: string) {
  const clientId = process.env.TWITCH_CLIENT_ID;
  const clientSecret = process.env.TWITCH_CLIENT_SECRET;

  if (!clientId || !clientSecret) {
    throw new Error('Missing TWITCH_CLIENT_ID or TWITCH_CLIENT_SECRET in environment');
  }

  // Get OAuth token
  const tokenResp = await fetch('https://id.twitch.tv/oauth2/token', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: `client_id=${clientId}&client_secret=${clientSecret}&grant_type=client_credentials`
  });

  if (!tokenResp.ok) {
    throw new Error(`Failed to get OAuth token: ${tokenResp.statusText}`);
  }

  const { access_token } = await tokenResp.json();

  // Get user info
  const userResp = await fetch(`https://api.twitch.tv/helix/users?login=${username}`, {
    headers: {
      'Authorization': `Bearer ${access_token}`,
      'Client-Id': clientId
    }
  });

  if (!userResp.ok) {
    throw new Error(`Failed to get user info: ${userResp.statusText}`);
  }

  const userData = await userResp.json();

  if (!userData.data || userData.data.length === 0) {
    console.log(`❌ Twitch user "${username}" not found`);
    console.log('\nPlease verify:');
    console.log('  1. Username spelling is correct');
    console.log('  2. Account exists and is not banned/suspended');
    console.log('  3. You can access https://www.twitch.tv/' + username);
    return null;
  }

  const user = userData.data[0];
  const user_id = user.id;
  const user_hash = hashUser(user_id);

  console.log('✅ Twitch User Found!\n');
  console.log('User Details:');
  console.log(`  Username: ${user.login}`);
  console.log(`  Display Name: ${user.display_name}`);
  console.log(`  User ID: ${user_id}`);
  console.log(`  User Hash: ${user_hash}`);
  console.log(`  Profile: https://www.twitch.tv/${user.login}`);

  return {
    username: user.login,
    displayName: user.display_name,
    userId: user_id,
    userHash: user_hash
  };
}

const username = process.argv[2] || 'zowzrd';
console.log(`🔍 Looking up Twitch user: ${username}\n`);

getTwitchUserDetails(username)
  .then(result => {
    if (result) {
      console.log('\n📋 Copy these values for database queries:');
      console.log(`  USER_ID="${result.userId}"`);
      console.log(`  USER_HASH="${result.userHash}"`);
    }
  })
  .catch(err => {
    console.error('❌ Error:', err.message);
    process.exit(1);
  });
