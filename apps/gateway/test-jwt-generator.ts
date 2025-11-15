#!/usr/bin/env tsx
/**
 * Generate test JWT for end-to-end testing
 */

import jwt from 'jsonwebtoken';

const secret = process.env.JWT_SECRET || 'dev_secret_key_replace_in_production';

const payload = {
  twitchId: 'mock_id_123',
  twitchLogin: 'dizzybreezyy',
  twitchDisplayName: 'DizzyBreezyy',
  profileImage: 'https://static-cdn.jtvnw.net/jtv_user_pictures/test.png',
  accessToken: 'mock_token_for_testing',
  nonce: 'test_nonce_' + Date.now(),
  iat: Math.floor(Date.now() / 1000),
  exp: Math.floor(Date.now() / 1000) + 3600
};

const token = jwt.sign(payload, secret);

console.log('🔐 Test JWT Generated for dizzybreezyy');
console.log('');
console.log('Token:');
console.log(token);
console.log('');
console.log('Payload:');
console.log(JSON.stringify(payload, null, 2));
console.log('');
console.log('Usage:');
console.log(`export TEST_JWT="${token}"`);
console.log('curl -X GET http://localhost:8082/api/claims/available -H "Cookie: session=s:${token}" --cookie "session=s:${token}"');
