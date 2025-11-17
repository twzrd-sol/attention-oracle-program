/**
 * MILO Claim-Ring Endpoint (matches set_merkle_root_ring publisher)
 * Builds unsigned transaction for claim_with_ring instruction
 */

import { FastifyInstance, FastifyPluginAsync, FastifyRequest, FastifyReply } from 'fastify';
import {
  PublicKey,
  Transaction,
  TransactionInstruction,
  SystemProgram,
} from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from '@solana/spl-token';
import { keccak_256 } from 'js-sha3';
import { config } from '../config.js';

const AGGREGATOR_URL = process.env.AGGREGATOR_URL || 'http://localhost:3000';

interface ClaimRingRequest {
  wallet: string;
  channel: string;
  epoch: number;
  mint: string; // CCM mint address
  username?: string; // Twitch username or pseudonymous ID
}

interface AggregatorProof {
  epoch: number;
  channel: string;
  user: string;
  index: number;
  amount: number;
  id: string;
  proof: string[]; // hex strings
  root: string;
}

export const claimRingRoutes: FastifyPluginAsync = async (server: FastifyInstance) => {
  server.post<{ Body: ClaimRingRequest }>(
    '/claim-ring',
    {
      config: {
        rateLimit: {
          max: 5,
          timeWindow: '1 minute',
        },
      },
    },
    async (request: FastifyRequest<{ Body: ClaimRingRequest }>, reply: FastifyReply) => {
      try {
        const { wallet, channel, epoch, mint, username } = request.body;
        if (!wallet || !channel || !epoch || !mint) {
          return reply.code(400).send({ error: 'Missing wallet, channel, epoch, or mint' });
        }

        let walletPubkey: PublicKey;
        let mintPubkey: PublicKey;
        try {
          walletPubkey = new PublicKey(wallet);
          mintPubkey = new PublicKey(mint);
        } catch {
          return reply.code(400).send({ error: 'Invalid wallet or mint address' });
        }

        // Fetch proof from aggregator
        const userIdentifier = (username || wallet).trim();
        const proofUrl = `${AGGREGATOR_URL}/claim-proof?channel=${encodeURIComponent(channel)}&epoch=${epoch}&user=${encodeURIComponent(userIdentifier)}`;
        let proofData: AggregatorProof;
        try {
          const proofRes = await fetch(proofUrl);
          if (!proofRes.ok) {
            const errText = await proofRes.text();
            return reply.code(proofRes.status).send({
              error: 'Proof not found',
              details: errText,
              proofUrl,
              usedIdentifier: userIdentifier,
            });
          }
          proofData = await proofRes.json() as AggregatorProof;
        } catch (err: any) {
          return reply.code(500).send({ error: 'Failed to fetch proof from aggregator', message: err.message, proofUrl });
        }

        // PDAs for ring variant
        const PROTOCOL_SEED = Buffer.from('protocol');
        const CHANNEL_STATE_SEED = Buffer.from('channel_state');

        const [protocolState] = PublicKey.findProgramAddressSync(
          [PROTOCOL_SEED, mintPubkey.toBuffer()],
          new PublicKey(config.PROGRAM_ID)
        );

        const streamerKey = new PublicKey(Buffer.from(keccak_256.arrayBuffer(`twitch:${channel.toLowerCase()}`)).subarray(0,32));
        const [channelState] = PublicKey.findProgramAddressSync(
          [CHANNEL_STATE_SEED, mintPubkey.toBuffer(), streamerKey.toBuffer()],
          new PublicKey(config.PROGRAM_ID)
        );

        // ATAs
        const treasuryAta = getAssociatedTokenAddressSync(
          mintPubkey,
          protocolState,
          true,
          TOKEN_2022_PROGRAM_ID
        );
        const claimerAta = getAssociatedTokenAddressSync(
          mintPubkey,
          walletPubkey,
          false,
          TOKEN_2022_PROGRAM_ID
        );

        // Instruction data for claim_with_ring
        const disc = require('crypto').createHash('sha256').update('global:claim_with_ring').digest().subarray(0,8);

        const epochBuf = Buffer.alloc(8);
        epochBuf.writeBigUInt64LE(BigInt(epoch));

        const indexBuf = Buffer.alloc(4);
        indexBuf.writeUInt32LE(proofData.index);

        const amountBuf = Buffer.alloc(8);
        amountBuf.writeBigUInt64LE(BigInt(proofData.amount));

        const proofLen = Buffer.alloc(4);
        proofLen.writeUInt32LE(proofData.proof.length);
        const proofBytes = Buffer.concat(proofData.proof.map(p => Buffer.from(p, 'hex')));

        const idBytes = Buffer.from(proofData.id, 'utf8');
        const idLen = Buffer.alloc(4); idLen.writeUInt32LE(idBytes.length);

        const instructionData = Buffer.concat([
          disc,
          epochBuf,
          indexBuf,
          amountBuf,
          proofLen,
          proofBytes,
          idLen,
          idBytes,
          streamerKey.toBuffer(),
        ]);

        const accountMetas = [
          { pubkey: walletPubkey, isSigner: true, isWritable: true },
          { pubkey: protocolState, isSigner: false, isWritable: true },
          { pubkey: channelState, isSigner: false, isWritable: true },
          { pubkey: mintPubkey, isSigner: false, isWritable: false },
          { pubkey: treasuryAta, isSigner: false, isWritable: true },
          { pubkey: claimerAta, isSigner: false, isWritable: true },
          { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
          { pubkey: TOKEN_2022_PROGRAM_ID, isSigner: false, isWritable: false },
        ];

        const ix = new TransactionInstruction({
          programId: new PublicKey(config.PROGRAM_ID),
          keys: accountMetas,
          data: instructionData,
        });

        const { blockhash, lastValidBlockHeight } = await (await import('../lib/rpc.js')).withRpcFailover(
          (conn) => conn.getLatestBlockhash('finalized'),
          'finalized'
        );

        const tx = new Transaction({ feePayer: walletPubkey, blockhash, lastValidBlockHeight });
        tx.add(ix);

        const serialized = tx.serialize({ requireAllSignatures: false, verifySignatures: false });
        return reply.send({
          transaction: serialized.toString('base64'),
          blockhash,
          lastValidBlockHeight,
          proof: {
            index: proofData.index,
            amount: proofData.amount,
            root: proofData.root,
            id: proofData.id,
            user: proofData.user,
            identifierUsed: userIdentifier,
          },
          pdas: {
            protocolState: protocolState.toString(),
            channelState: channelState.toString(),
            streamerKey: streamerKey.toString(),
            treasuryAta: treasuryAta.toString(),
            claimerAta: claimerAta.toString(),
          },
        });
      } catch (error: any) {
        console.error('claim-ring error:', error);
        return reply.code(500).send({ error: 'Failed to build claim transaction (ring)', message: error.message });
      }
    }
  );
}

