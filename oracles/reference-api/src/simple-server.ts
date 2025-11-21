/**
 * Attention Oracle Reference API - Simplified Demo
 * 
 * Shows API patterns without complex Anchor dependencies
 * Integrators can adapt this to their own stack
 */

import http from 'http';
import { Connection, PublicKey } from '@solana/web3.js';
import keccak256 from 'keccak256';

// Config
const PORT = parseInt(process.env.PORT || '3000');
const RPC_URL = process.env.RPC_URL || 'https://api.mainnet-beta.solana.com';
const PROGRAM_ID = process.env.PROGRAM_ID || 'GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop';

const connection = new Connection(RPC_URL, 'confirmed');
const programId = new PublicKey(PROGRAM_ID);

// PDA Helpers
function getPassportPda(userHash: Buffer): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("passport_owner"), userHash],
    programId
  )[0];
}

function getStreamerKey(channel: string): PublicKey {
  const normalized = channel.toLowerCase();
  const preimage = Buffer.from(`channel:${normalized}`);
  const hash = keccak256(preimage);
  return new PublicKey(hash);
}

function getChannelStatePda(mint: PublicKey, streamerKey: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), mint.toBuffer(), streamerKey.toBuffer()],
    programId
  )[0];
}

// Merkle proof verification
function verifyMerkleProof(leaf: Buffer, proof: Buffer[], root: Buffer): boolean {
  let hash = keccak256(leaf);
  for (const p of proof) {
    hash = Buffer.compare(hash, p) <= 0
      ? keccak256(Buffer.concat([hash, p]))
      : keccak256(Buffer.concat([p, hash]));
  }
  return Buffer.compare(hash, root) === 0;
}

// HTTP Server
const server = http.createServer(async (req, res) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, X-Wallet-Pubkey');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    return res.end();
  }

  try {
    const url = new URL(req.url || '', `http://${req.headers.host}`);

    // 1. Passport Query
    if (url.pathname.startsWith('/passport/')) {
      const userHashHex = url.pathname.split('/')[2];
      if (!userHashHex || userHashHex.length !== 64) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Invalid user_hash (expected 64 hex chars)"
        }));
      }

      const userHash = Buffer.from(userHashHex, 'hex');
      const pda = getPassportPda(userHash);
      
      // Fetch raw account data
      const accountInfo = await connection.getAccountInfo(pda);
      
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({
        ok: true,
        user_hash: userHashHex,
        pda: pda.toBase58(),
        account: accountInfo ? {
          exists: true,
          data_length: accountInfo.data.length,
          owner: accountInfo.owner.toBase58(),
          note: "Use Anchor client for full deserialization"
        } : null
      }));
    }

    // 2. Channel State
    if (url.pathname.startsWith('/channel/')) {
      const parts = url.pathname.split('/');
      const mintStr = parts[2];
      const channelId = parts[3];

      if (!mintStr || !channelId) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Usage: /channel/:mint/:channel_id"
        }));
      }

      const mint = new PublicKey(mintStr);
      const streamerKey = getStreamerKey(channelId);
      const pda = getChannelStatePda(mint, streamerKey);
      const accountInfo = await connection.getAccountInfo(pda);

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({
        ok: true,
        channel: channelId,
        mint: mintStr,
        streamer_key: streamerKey.toBase58(),
        pda: pda.toBase58(),
        state: accountInfo ? "initialized" : "not_found",
        account_size: accountInfo?.data.length || 0
      }));
    }

    // 3. Proof Verification
    if (url.pathname === '/verify-proof' && req.method === 'POST') {
      let body = '';
      for await (const chunk of req) body += chunk;
      const json = JSON.parse(body);
      const { leaf_hex, proof_hex_array, root_hex } = json;

      if (!leaf_hex || !proof_hex_array || !root_hex) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Missing: leaf_hex, proof_hex_array, root_hex"
        }));
      }

      const leaf = Buffer.from(leaf_hex, 'hex');
      const proof = proof_hex_array.map((h: string) => Buffer.from(h, 'hex'));
      const root = Buffer.from(root_hex, 'hex');
      const isValid = verifyMerkleProof(leaf, proof, root);

      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({ ok: true, valid: isValid }));
    }

    // 4. x402 Premium (demo)
    if (url.pathname === '/premium') {
      res.writeHead(402, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({
        ok: false,
        code: 402,
        message: "Payment Required - Insufficient Reputation",
        note: "Check passport tier via /passport/:user_hash"
      }));
    }

    // Root
    if (url.pathname === '/' || url.pathname === '/help') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({
        ok: true,
        name: "Attention Oracle Reference API",
        version: "1.0.0",
        program_id: PROGRAM_ID,
        endpoints: {
          "GET /passport/:user_hash": "Query passport (32-byte hex)",
          "GET /channel/:mint/:channel": "Inspect channel state",
          "POST /verify-proof": "Verify Merkle proof",
          "GET /premium": "x402 demo (402 response)"
        },
        examples: {
          passport: `curl http://localhost:${PORT}/passport/deadbeef...`,
          channel: `curl http://localhost:${PORT}/channel/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/test`,
          verify: `curl -X POST -d '{"leaf_hex":"aa","proof_hex_array":[],"root_hex":"aa"}' http://localhost:${PORT}/verify-proof`
        }
      }));
    }

    res.writeHead(404, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({ ok: false, error: "Not found" }));

  } catch (e: any) {
    console.error(e);
    res.writeHead(500, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({ ok: false, error: e.message }));
  }
});

server.listen(PORT, () => {
  console.log(`🔮 Attention Oracle Reference API`);
  console.log(`   Port: ${PORT}`);
  console.log(`   Program: ${PROGRAM_ID}`);
  console.log(`   RPC: ${RPC_URL}`);
  console.log(``);
  console.log(`Ready! Test with: curl http://localhost:${PORT}/`);
});
