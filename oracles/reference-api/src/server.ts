/**
 * Attention Oracle Reference API
 *
 * Demonstrates:
 * - Passport/reputation queries (PassportRegistry)
 * - Channel state inspection (ChannelState ring buffer)
 * - Off-chain Merkle proof verification
 * - x402 gating via on-chain reputation tiers
 *
 * Public-safe, platform-agnostic reference implementation for integrators.
 */

import http from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { Connection, PublicKey } from '@solana/web3.js';
import { AnchorProvider, Program } from '@coral-xyz/anchor';
import keccak256 from 'keccak256';
import { env } from './env.ts';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const IDL_JSON = JSON.parse(readFileSync(join(__dirname, 'idl.json'), 'utf-8'));

// -------------------------------------------------------
// Setup
// -------------------------------------------------------

const connection = new Connection(env.RPC_URL, 'confirmed');
const provider = new AnchorProvider(connection, {} as any, { commitment: 'confirmed' });
const programId = new PublicKey(env.PROGRAM_ID);
const program = new Program(IDL_JSON as any, programId, provider);

// -------------------------------------------------------
// PDA Derivation Helpers
// -------------------------------------------------------

/**
 * Derive PassportRegistry PDA from user_hash
 */
function getPassportPda(userHash: Buffer): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("passport_owner"), userHash],
    programId
  )[0];
}

/**
 * Derive ChannelState PDA from mint + streamer_key
 */
function getChannelStatePda(mint: PublicKey, streamerKey: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("channel_state"), mint.toBuffer(), streamerKey.toBuffer()],
    programId
  )[0];
}

/**
 * Derive streamer_key from channel identifier (Keccak256)
 */
function getStreamerKey(channel: string): PublicKey {
  const normalized = channel.toLowerCase();
  const preimage = Buffer.from(`channel:${normalized}`);
  const hash = keccak256(preimage);
  return new PublicKey(hash);
}

// -------------------------------------------------------
// Merkle Proof Verification (Off-Chain)
// -------------------------------------------------------

/**
 * Verify a Merkle proof matches on-chain leaf format
 * Uses sorted Keccak256 pairs (same as Solana program)
 */
function verifyMerkleProof(leaf: Buffer, proof: Buffer[], root: Buffer): boolean {
  let hash = keccak256(leaf);

  for (const proofElement of proof) {
    // Sorted pairs (matches on-chain logic)
    if (Buffer.compare(hash, proofElement) <= 0) {
      hash = keccak256(Buffer.concat([hash, proofElement]));
    } else {
      hash = keccak256(Buffer.concat([proofElement, hash]));
    }
  }

  return Buffer.compare(hash, root) === 0;
}

// -------------------------------------------------------
// HTTP Server
// -------------------------------------------------------

const server = http.createServer(async (req, res) => {
  // CORS headers
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type, X-Wallet-Pubkey');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    return res.end();
  }

  try {
    const url = new URL(req.url || '', `http://${req.headers.host}`);

    // -------------------------------------------------------
    // 1. PASSPORT API: Query wallet reputation
    // -------------------------------------------------------
    // GET /passport/:user_hash_hex
    // Returns: owner, tier, score, epoch_count, badges, etc.
    if (url.pathname.startsWith('/passport/')) {
      const userHashHex = url.pathname.split('/')[2];

      if (!userHashHex || userHashHex.length !== 64) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Invalid user_hash (expected 64 hex chars)"
        }));
      }

      try {
        const userHash = Buffer.from(userHashHex, 'hex');
        const pda = getPassportPda(userHash);
        const account: any = await program.account.passportRegistry.fetchNullable(pda);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: true,
          user_hash: userHashHex,
          pda: pda.toBase58(),
          passport: account ? {
            owner: account.owner.toBase58(),
            tier: account.tier,
            score: account.score.toString(),
            epoch_count: account.epochCount,
            weighted_presence: account.weightedPresence.toString(),
            badges: account.badges,
            updated_at: new Date(Number(account.updatedAt) * 1000).toISOString(),
          } : null
        }));
      } catch (e: any) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ ok: false, error: e.message }));
      }
    }

    // -------------------------------------------------------
    // 2. CHANNEL STATE INSPECTOR: Query ring buffer
    // -------------------------------------------------------
    // GET /channel/:mint/:channel_identifier
    // Returns: latest_epoch, ring buffer slots, streamer_key
    if (url.pathname.startsWith('/channel/')) {
      const parts = url.pathname.split('/');
      const mintStr = parts[2];
      const channelId = parts[3];

      if (!mintStr || !channelId) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Usage: /channel/:mint/:channel_identifier"
        }));
      }

      try {
        const mint = new PublicKey(mintStr);
        const streamerKey = getStreamerKey(channelId);
        const pda = getChannelStatePda(mint, streamerKey);

        // ChannelState is zero-copy, so we fetch raw account data
        const accountInfo = await connection.getAccountInfo(pda);

        if (!accountInfo) {
          res.writeHead(200, { 'Content-Type': 'application/json' });
          return res.end(JSON.stringify({
            ok: true,
            channel: channelId,
            mint: mintStr,
            streamer_key: streamerKey.toBase58(),
            pda: pda.toBase58(),
            state: null,
            message: "Channel not yet initialized"
          }));
        }

        // Parse zero-copy data (simplified - real impl needs proper deserialization)
        // For demo, just show account exists
        res.writeHead(200, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: true,
          channel: channelId,
          mint: mintStr,
          streamer_key: streamerKey.toBase58(),
          pda: pda.toBase58(),
          state: "initialized",
          account_size: accountInfo.data.length,
          note: "Zero-copy account - use Anchor client for full deserialization"
        }));
      } catch (e: any) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ ok: false, error: e.message }));
      }
    }

    // -------------------------------------------------------
    // 3. MERKLE PROOF VERIFICATION: Off-chain validation
    // -------------------------------------------------------
    // POST /verify-proof
    // Body: { leaf_hex, proof_hex_array, root_hex }
    // Returns: { valid: boolean }
    if (url.pathname === '/verify-proof' && req.method === 'POST') {
      let body = '';
      for await (const chunk of req) body += chunk;

      try {
        const json = JSON.parse(body);
        const { leaf_hex, proof_hex_array, root_hex } = json;

        if (!leaf_hex || !proof_hex_array || !root_hex) {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          return res.end(JSON.stringify({
            ok: false,
            error: "Missing required fields: leaf_hex, proof_hex_array, root_hex"
          }));
        }

        const leaf = Buffer.from(leaf_hex, 'hex');
        const proof = proof_hex_array.map((h: string) => Buffer.from(h, 'hex'));
        const root = Buffer.from(root_hex, 'hex');

        const isValid = verifyMerkleProof(leaf, proof, root);

        res.writeHead(200, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: true,
          valid: isValid,
          leaf_hex,
          root_hex
        }));
      } catch (e: any) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ ok: false, error: e.message }));
      }
    }

    // -------------------------------------------------------
    // 4. x402 PREMIUM ENDPOINT: Reputation-gated access
    // -------------------------------------------------------
    // GET /premium
    // Header: X-Wallet-Pubkey (in production, verify signature)
    // Requires: tier >= MIN_TIER_PREMIUM OR score >= MIN_SCORE_PREMIUM
    if (url.pathname === '/premium') {
      const walletHeader = req.headers['x-wallet-pubkey'] as string;

      if (!walletHeader) {
        res.writeHead(401, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          error: "Missing X-Wallet-Pubkey header",
          hint: "Include X-Wallet-Pubkey: <base58-pubkey>"
        }));
      }

      try {
        // In production: verify signed message to prove wallet ownership
        // For this demo, we trust the header
        const wallet = new PublicKey(walletHeader);

        // Lookup passport by wallet (requires user_hash)
        // For demo, we'll show the gating logic - in production you'd
        // maintain a wallet->user_hash mapping or require the user_hash as param

        res.writeHead(402, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({
          ok: false,
          code: 402,
          message: "Payment Required - Insufficient Reputation",
          requirement: `Tier >= ${env.MIN_TIER_PREMIUM} OR Score >= ${env.MIN_SCORE_PREMIUM}`,
          hint: "Earn reputation by engaging with the protocol",
          upgrade_url: "https://example.com/earn-reputation",
          note_demo: "This endpoint requires user_hash lookup - see /passport/:user_hash for reputation check"
        }));
      } catch (e: any) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        return res.end(JSON.stringify({ ok: false, error: e.message }));
      }
    }

    // -------------------------------------------------------
    // Root / Help
    // -------------------------------------------------------
    if (url.pathname === '/' || url.pathname === '/help') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      return res.end(JSON.stringify({
        ok: true,
        name: "Attention Oracle Reference API",
        version: "1.0.0",
        program_id: env.PROGRAM_ID,
        endpoints: {
          "GET /passport/:user_hash": "Query passport reputation by user_hash (32-byte hex)",
          "GET /channel/:mint/:channel_id": "Inspect channel state ring buffer",
          "POST /verify-proof": "Verify Merkle proof off-chain (saves gas)",
          "GET /premium": "x402-gated premium endpoint (requires reputation)"
        },
        examples: {
          passport: `curl http://localhost:${env.PORT}/passport/deadbeef...`,
          channel: `curl http://localhost:${env.PORT}/channel/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/example_channel`,
          verify: `curl -X POST -H "Content-Type: application/json" -d '{"leaf_hex":"...", "proof_hex_array":["..."], "root_hex":"..."}' http://localhost:${env.PORT}/verify-proof`,
          premium: `curl -H "X-Wallet-Pubkey: YourWalletBase58" http://localhost:${env.PORT}/premium`
        }
      }));
    }

    // 404 Not Found
    res.writeHead(404, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({
      ok: false,
      error: "Not found",
      hint: "GET / for API documentation"
    }));

  } catch (e: any) {
    console.error('[ERROR]', e);
    res.writeHead(500, { 'Content-Type': 'application/json' });
    return res.end(JSON.stringify({ ok: false, error: e.message }));
  }
});

// -------------------------------------------------------
// Start Server
// -------------------------------------------------------

server.listen(env.PORT, () => {
  console.log(`🔮 Attention Oracle Reference API`);
  console.log(`   Port: ${env.PORT}`);
  console.log(`   Program: ${env.PROGRAM_ID}`);
  console.log(`   RPC: ${env.RPC_URL}`);
  console.log(``);
  console.log(`Endpoints:`);
  console.log(`   GET  /                           API documentation`);
  console.log(`   GET  /passport/:user_hash        Query reputation`);
  console.log(`   GET  /channel/:mint/:channel     Channel state`);
  console.log(`   POST /verify-proof               Off-chain Merkle verification`);
  console.log(`   GET  /premium                    x402 gated content`);
  console.log(``);
  console.log(`Ready for integrators! 🪄`);
});
