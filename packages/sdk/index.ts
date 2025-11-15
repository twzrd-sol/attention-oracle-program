/**
 * @twzrd/sdk - Official TypeScript SDK for TWZRD Attention Oracle
 *
 * Open-core Solana primitive for tokenized attention.
 * Presence → Proof → Tokens.
 *
 * @see https://twzrd.xyz
 * @see https://github.com/twzrd-sol/attention-oracle-program
 */

import { PublicKey, Connection } from '@solana/web3.js';

export const PROGRAM_ID = new PublicKey('YOUR_PROGRAM_ID_HERE');

/**
 * TWZRD SDK Client
 */
export class TwzrdClient {
  constructor(
    private connection: Connection,
    private programId: PublicKey = PROGRAM_ID
  ) {}

  /**
   * Get channel state for a given streamer
   */
  async getChannelState(streamer: PublicKey): Promise<any> {
    // TODO: Implement channel state fetching
    throw new Error('Not implemented');
  }

  /**
   * Claim attention tokens for a user
   */
  async claimTokens(
    user: PublicKey,
    channel: PublicKey
  ): Promise<string> {
    // TODO: Implement claim instruction
    throw new Error('Not implemented');
  }
}

export * from './types';
