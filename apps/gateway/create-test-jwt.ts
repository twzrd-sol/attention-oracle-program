#!/usr/bin/env tsx
/**
 * Generate test JWT for end-to-end testing
 * Uses the same JWT configuration as the gateway
 */

import Fastify from 'fastify';
import jwt from '@fastify/jwt';
import cookie from '@fastify/cookie';
import { config } from './src/config.js';

async function generateTestJWT() {
  const server = Fastify({ logger: false });

  await server.register(cookie, {
    secret: config.COOKIE_SECRET
  });

  await server.register(jwt, {
    secret: config.JWT_SECRET,
    cookie: {
      cookieName: 'session',
      signed: true
    }
  });

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

  const token = server.jwt.sign(payload);
  const signedCookie = server.signCookie(token);

  console.log('🔐 Test JWT Generated for dizzybreezyy');
  console.log('');
  console.log('✅ Payload:');
  console.log(JSON.stringify(payload, null, 2));
  console.log('');
  console.log('🔑 Raw Token:');
  console.log(token);
  console.log('');
  console.log('🍪 Signed Cookie Value:');
  console.log(signedCookie);
  console.log('');
  console.log('📝 Export for testing:');
  console.log(`export TEST_JWT="${signedCookie}"`);
  console.log('');
  console.log('🧪 Test commands:');
  console.log('');
  console.log('# Test 1: Get available claims');
  console.log(`curl -X GET http://localhost:8082/api/claims/available -H "Cookie: session=${signedCookie}"`);
  console.log('');
  console.log('# Test 2: Get proof for epoch 1761753600');
  console.log(`curl -X GET http://localhost:8082/api/claims/proof/1761753600/jasontheween -H "Cookie: session=${signedCookie}"`);

  await server.close();
}

generateTestJWT().catch(console.error);
