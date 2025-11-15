/**
 * Actions Auth Routes - Verify Twitch tokens for UI
 */

import { FastifyPluginAsync } from 'fastify';
import { z } from 'zod';
import { config } from '../config.js';
import { logger } from '../utils/logger.js';

const TWITCH_API_URL = 'https://api.twitch.tv/helix';

export const actionsAuthRoutes: FastifyPluginAsync = async (server) => {
  /**
   * Verify Twitch access token
   * Used by the claim UI to verify a user's Twitch login
   */
  server.get<{ Querystring: { access_token: string } }>('/verify-token', {
    config: {
      rateLimit: {
        max: 100,
        timeWindow: '1 minute'
      }
    }
  }, async (request, reply) => {
    try {
      const { access_token } = request.query;

      if (!access_token) {
        return reply.code(400).send({ error: 'Missing access_token' });
      }

      // Parse the token format: username:token_id
      const parts = access_token.split(':');
      if (parts.length !== 2) {
        return reply.code(400).send({ error: 'Invalid token format' });
      }

      const [username, tokenId] = parts;

      // Validate token with Twitch API
      const validationResponse = await fetch('https://id.twitch.tv/oauth2/validate', {
        headers: {
          'Authorization': `OAuth ${access_token}`
        }
      });

      if (!validationResponse.ok) {
        // Token is invalid or expired
        logger.warn({ username, tokenId }, 'Invalid Twitch token');
        return reply.code(401).send({
          error: 'Invalid or expired token',
          authenticated: false
        });
      }

      const tokenData = await validationResponse.json() as any;

      // Get user info from Twitch
      const userResponse = await fetch(`${TWITCH_API_URL}/users?login=${username}`, {
        headers: {
          'Authorization': `Bearer ${access_token}`,
          'Client-Id': config.TWITCH_CLIENT_ID
        }
      });

      if (!userResponse.ok) {
        // For now, just return basic validation success
        return {
          authenticated: true,
          username: username,
          twitchId: tokenData.user_id || null,
          twitchLogin: username,
          expiresIn: tokenData.expires_in || 0
        };
      }

      const { data: users } = await userResponse.json() as any;
      const user = users && users[0];

      if (!user) {
        // User not found, but token is valid
        return {
          authenticated: true,
          username: username,
          twitchId: tokenData.user_id || null,
          twitchLogin: username,
          expiresIn: tokenData.expires_in || 0
        };
      }

      // Return user data
      return {
        authenticated: true,
        twitchId: user.id,
        twitchLogin: user.login,
        twitchDisplayName: user.display_name,
        profileImage: user.profile_image_url,
        email: user.email || null,
        createdAt: user.created_at,
        expiresIn: tokenData.expires_in || 0
      };

    } catch (error: any) {
      logger.error({ error: error.message }, 'Error verifying Twitch token');

      // For development/testing, allow any token in the format user:token
      if (config.NODE_ENV === 'development') {
        const { access_token } = request.query;
        const parts = (access_token || '').split(':');
        if (parts.length === 2) {
          return {
            authenticated: true,
            username: parts[0],
            twitchLogin: parts[0],
            twitchDisplayName: parts[0],
            development: true
          };
        }
      }

      return reply.code(500).send({
        error: 'Failed to verify token',
        authenticated: false
      });
    }
  });

  /**
   * Get user eligibility/participation data
   * Can be extended to show which channels they've watched
   */
  server.get<{ Querystring: { username: string } }>('/user-data', {
    config: {
      rateLimit: {
        max: 60,
        timeWindow: '1 minute'
      }
    }
  }, async (request, reply) => {
    try {
      const { username } = request.query;

      if (!username) {
        return reply.code(400).send({ error: 'Missing username' });
      }

      // For now, return basic structure
      // This could be extended to query the aggregator for participation data
      return {
        username: username,
        channels: [
          'yourragegaming',
          'jasontheween',
          'stableronaldo',
          'lacy',
          'adapt',
          'silky',
          'kaysan'
        ],
        totalEpochs: 0,
        claimableAmount: 0,
        lastSeen: null
      };

    } catch (error: any) {
      logger.error({ error: error.message }, 'Error fetching user data');
      return reply.code(500).send({ error: 'Failed to fetch user data' });
    }
  });
};