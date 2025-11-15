/**
 * Authenticated Claims Routes
 * Provides proof generation for logged-in Twitch users
 * This is the CORRECT flow: user logs in via Twitch OAuth,
 * then gets proofs for their Twitch identity
 */

import type { FastifyPluginAsync } from 'fastify';
import { keccak_256 } from '@noble/hashes/sha3.js';
import { getDbReader } from '../lib/db-reader-pg.js';
import type { SessionPayload } from '../types/session.js';

export const claimsAuthenticatedRoutes: FastifyPluginAsync = async (server) => {
  const dbReader = getDbReader();

  /**
   * Get all available claims for the authenticated user
   * Returns list of epochs/channels where user has unclaimed tokens
   */
  server.get('/api/claims/available', {
    config: {
      rateLimit: {
        max: 30,
        timeWindow: '1 minute'
      }
    },
    preHandler: async (request, reply) => {
      try {
        await request.jwtVerify<SessionPayload>();
      } catch {
        return reply.code(401).send({ error: 'unauthorized' });
      }
    }
  }, async (request, reply) => {
    try {
      const session = request.user as SessionPayload;

      // Compute user_hash from Twitch Login (username)
      // CRITICAL: Aggregator hashes the username (twitchLogin), not numeric ID
      const twitchLogin = session.twitchLogin.toLowerCase();
      const user_hash = Buffer.from(
        keccak_256(Buffer.from(twitchLogin, 'utf8'))
      ).toString('hex');

      request.log.info({
        twitchLogin: session.twitchLogin,
        user_hash: user_hash.substring(0, 16) + '...'
      }, 'Looking up claims for authenticated user');

      // Query database for all sealed_participants entries matching this user_hash
      const result = await dbReader.pool.query(`
        SELECT DISTINCT
          sp.epoch,
          sp.channel,
          sp.idx as index,
          se.root,
          se.sealed_at,
          se.published
        FROM sealed_participants sp
        JOIN sealed_epochs se
          ON sp.epoch = se.epoch
          AND sp.channel = se.channel
        WHERE sp.user_hash = $1
        ORDER BY sp.epoch DESC, sp.channel ASC
        LIMIT 100
      `, [user_hash]);

      if (result.rows.length === 0) {
        return reply.send({
          claims: [],
          message: 'No claims found. Make sure you watched streams during active epochs.',
          twitchLogin: session.twitchLogin
        });
      }

      const claims = result.rows.map(row => ({
        epoch: Number(row.epoch),
        channel: row.channel,
        index: row.index,
        root: row.root,
        sealedAt: Number(row.sealed_at),
        published: row.published === 1 || row.published === true,
        // TODO: Add amount/weight from viewer_activity or separate table
        estimatedAmount: '1024000000000', // Placeholder - 1024 MILO
      }));

      return reply.send({
        twitchLogin: session.twitchLogin,
        twitchDisplayName: session.twitchDisplayName,
        user_hash: user_hash.substring(0, 16) + '...',
        claims,
        totalClaimable: claims.length
      });

    } catch (error: any) {
      request.log.error({ error }, 'Failed to fetch available claims');
      return reply.code(500).send({ error: 'claims_fetch_failed' });
    }
  });

  /**
   * Get merkle proof for a specific epoch/channel for authenticated user
   * This proof can be used to claim tokens with ANY Solana wallet
   */
  server.get<{
    Params: { epoch: string; channel: string }
  }>('/api/claims/proof/:epoch/:channel', {
    config: {
      rateLimit: {
        max: 60,
        timeWindow: '1 minute'
      }
    },
    preHandler: async (request, reply) => {
      try {
        await request.jwtVerify<SessionPayload>();
      } catch {
        return reply.code(401).send({ error: 'unauthorized' });
      }
    }
  }, async (request, reply) => {
    try {
      const session = request.user as SessionPayload;
      const { epoch, channel } = request.params;

      const epochNum = parseInt(epoch, 10);
      if (!Number.isFinite(epochNum) || epochNum <= 0) {
        return reply.code(400).send({ error: 'invalid_epoch' });
      }

      // Compute user_hash from Twitch Login (username)
      // CRITICAL: Aggregator hashes the username (twitchLogin), not numeric ID
      const twitchLogin = session.twitchLogin.toLowerCase();
      const user_hash = Buffer.from(
        keccak_256(Buffer.from(twitchLogin, 'utf8'))
      ).toString('hex');

      request.log.info({
        twitchLogin: session.twitchLogin,
        epoch: epochNum,
        channel
      }, 'Generating proof for authenticated user');

      // Get participants list to find user's index
      const participants = await dbReader.getSealedParticipants(
        epochNum,
        channel,
        'MILO',  // Default token group
        'default' // Default category
      );

      if (!participants || participants.length === 0) {
        return reply.code(404).send({
          error: 'epoch_not_sealed',
          message: `No participants found for epoch ${epochNum}, channel ${channel}`
        });
      }

      // Find user's index in the participants list
      const userIndex = participants.indexOf(user_hash);
      if (userIndex === -1) {
        return reply.code(404).send({
          error: 'not_participant',
          message: `You were not a participant in ${channel} epoch ${epochNum}`,
          hint: 'Make sure you watched the stream during this epoch'
        });
      }

      // Generate merkle proof
      const proofData = await dbReader.generateProof(
        epochNum,
        channel,
        userIndex,
        'MILO',  // Default token group
        'default' // Default category
      );

      if (!proofData) {
        return reply.code(500).send({ error: 'proof_generation_failed' });
      }

      // Return proof in format ready for on-chain submission
      return reply.send({
        // User info
        twitchLogin: session.twitchLogin,
        twitchDisplayName: session.twitchDisplayName,

        // Claim data
        channel,
        epoch: epochNum,
        index: userIndex,

        // Merkle proof
        root: proofData.root.startsWith('0x') ? proofData.root : `0x${proofData.root}`,
        proof: proofData.proof.map(p => p.startsWith('0x') ? p : `0x${p}`),

        // Metadata
        participantCount: participants.length,
        user_hash: user_hash.substring(0, 16) + '...',

        // Instructions for frontend
        instructions: {
          step1: 'Connect your Solana wallet (Phantom, Backpack, etc.)',
          step2: 'Submit this proof with a claim transaction',
          step3: 'Sign the transaction with your wallet',
          note: 'You can claim to ANY wallet - it does not need to match your Twitch account'
        }
      });

    } catch (error: any) {
      request.log.error({ error }, 'Failed to generate proof');
      return reply.code(500).send({ error: 'proof_generation_failed' });
    }
  });

  /**
   * Convenience endpoint: Get proof for most recent claimable epoch
   */
  server.get<{
    Querystring: { channel?: string }
  }>('/api/claims/proof/latest', {
    config: {
      rateLimit: {
        max: 30,
        timeWindow: '1 minute'
      }
    },
    preHandler: async (request, reply) => {
      try {
        await request.jwtVerify<SessionPayload>();
      } catch {
        return reply.code(401).send({ error: 'unauthorized' });
      }
    }
  }, async (request, reply) => {
    try {
      const session = request.user as SessionPayload;
      const channel = request.query.channel;

      if (!channel) {
        return reply.code(400).send({
          error: 'missing_channel',
          hint: 'Provide ?channel=marlon (or other streamer)'
        });
      }

      // Compute user_hash from Twitch Login (username)
      // CRITICAL: Aggregator hashes the username (twitchLogin), not numeric ID
      const twitchLogin = session.twitchLogin.toLowerCase();
      const user_hash = Buffer.from(
        keccak_256(Buffer.from(twitchLogin, 'utf8'))
      ).toString('hex');

      // Find most recent epoch for this channel where user participated
      const result = await dbReader.pool.query(`
        SELECT sp.epoch, sp.idx as index
        FROM sealed_participants sp
        JOIN sealed_epochs se
          ON sp.epoch = se.epoch
          AND sp.channel = se.channel
        WHERE sp.user_hash = $1
          AND sp.channel = $2
        ORDER BY sp.epoch DESC
        LIMIT 1
      `, [user_hash, channel]);

      if (result.rows.length === 0) {
        return reply.code(404).send({
          error: 'no_claims_found',
          message: `No claims found for ${channel}`,
          hint: 'Watch the stream to become eligible for future epochs'
        });
      }

      const latestEpoch = Number(result.rows[0].epoch);

      // Redirect to the specific epoch proof endpoint
      return reply.redirect(`/api/claims/proof/${latestEpoch}/${channel}`);

    } catch (error: any) {
      request.log.error({ error }, 'Failed to find latest claim');
      return reply.code(500).send({ error: 'latest_claim_lookup_failed' });
    }
  });
};
