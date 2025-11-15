/**
 * /api/claim endpoint
 *
 * Claim CHAT tokens for verified Twitch activity
 */

import { NextApiRequest, NextApiResponse } from 'next';
import { Client } from 'pg';
import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import {
    getOrCreateAssociatedTokenAccount,
    mintTo,
    TOKEN_2022_PROGRAM_ID
} from '@solana/spl-token';
import * as fs from 'fs';

const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025twzrd@localhost:5432/twzrd';
const RPC_URL = process.env.RPC_URL || 'https://api.devnet.solana.com';
const CHAT_MINT = process.env.CHAT_MINT!;
const CHAT_DECIMALS = parseInt(process.env.CHAT_DECIMALS || '6');
const MINT_AUTHORITY_PATH = process.env.MINT_AUTHORITY_PATH!;

// Rate limiting map (in production, use Redis)
const claimAttempts = new Map<string, { count: number; resetAt: number }>();

function checkRateLimit(identifier: string): boolean {
    const now = Date.now();
    const limit = claimAttempts.get(identifier);

    if (!limit || limit.resetAt < now) {
        claimAttempts.set(identifier, {
            count: 1,
            resetAt: now + 60000 // 1 minute window
        });
        return true;
    }

    if (limit.count >= 5) {
        return false; // Too many attempts
    }

    limit.count++;
    return true;
}

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
    if (req.method !== 'POST') {
        return res.status(405).json({ error: 'Method not allowed' });
    }

    // Get session data (would come from auth middleware)
    const username = req.body.username || req.session?.username;
    const wallet = req.body.wallet || req.session?.wallet;
    const epoch = req.body.epoch;
    const channel = req.body.channel;

    // Validate inputs
    if (!username || !wallet || !epoch) {
        return res.status(400).json({
            error: 'Missing required fields',
            required: ['username', 'wallet', 'epoch']
        });
    }

    // Rate limiting
    const rateLimitKey = `${username}:${epoch}`;
    if (!checkRateLimit(rateLimitKey)) {
        return res.status(429).json({
            error: 'Too many claim attempts. Please wait 1 minute.'
        });
    }

    const pgClient = new Client(DATABASE_URL);

    try {
        await pgClient.connect();

        // Begin transaction
        await pgClient.query('BEGIN');

        // Find allocation
        let query = `
            SELECT *
            FROM claimable_allocations
            WHERE username = $1
              AND epoch = $2
              AND redeemed_at IS NULL
        `;
        const params: any[] = [username.toLowerCase(), epoch];

        if (channel) {
            query += ' AND channel = $3';
            params.push(channel);
        }

        query += ' FOR UPDATE'; // Lock the row

        const allocResult = await pgClient.query(query, params);

        if (allocResult.rows.length === 0) {
            await pgClient.query('ROLLBACK');
            return res.status(400).json({
                error: 'No unclaimed allocation found',
                username,
                epoch,
                channel
            });
        }

        const allocation = allocResult.rows[0];
        const amount = BigInt(allocation.amount);

        // Setup Solana connection
        const connection = new Connection(RPC_URL, 'confirmed');

        // Load mint authority
        const mintAuthoritySecret = JSON.parse(fs.readFileSync(MINT_AUTHORITY_PATH, 'utf8'));
        const mintAuthority = Keypair.fromSecretKey(new Uint8Array(mintAuthoritySecret));

        // Parse user wallet
        let userWallet: PublicKey;
        try {
            userWallet = new PublicKey(wallet);
        } catch (e) {
            await pgClient.query('ROLLBACK');
            return res.status(400).json({ error: 'Invalid wallet address' });
        }

        // Get or create token account
        const mint = new PublicKey(CHAT_MINT);
        const tokenAccount = await getOrCreateAssociatedTokenAccount(
            connection,
            mintAuthority,
            mint,
            userWallet,
            false, // Don't allow off-curve
            'confirmed',
            { commitment: 'confirmed' },
            TOKEN_2022_PROGRAM_ID
        );

        // Mint tokens
        const mintAmount = amount * BigInt(10 ** CHAT_DECIMALS);
        const signature = await mintTo(
            connection,
            mintAuthority,
            mint,
            tokenAccount.address,
            mintAuthority,
            mintAmount,
            [],
            { commitment: 'confirmed' },
            TOKEN_2022_PROGRAM_ID
        );

        // Update database
        await pgClient.query(`
            UPDATE claimable_allocations
            SET
                redeemed_at = NOW(),
                wallet = $1,
                tx_signature = $2,
                updated_at = NOW()
            WHERE username = $3
              AND epoch = $4
              AND channel = $5
        `, [wallet, signature, allocation.username, allocation.epoch, allocation.channel]);

        await pgClient.query('COMMIT');

        // Return success with receipt
        return res.status(200).json({
            success: true,
            receipt: {
                username: allocation.username,
                channel: allocation.channel,
                epoch: allocation.epoch,
                amount: allocation.amount,
                wallet,
                token_account: tokenAccount.address.toBase58(),
                signature,
                leaf: allocation.leaf?.toString('hex'),
                proof: allocation.proof,
                claimed_at: new Date().toISOString()
            },
            message: `Successfully claimed ${allocation.amount} CHAT tokens!`,
            explorer_url: `https://solscan.io/tx/${signature}?cluster=devnet`
        });

    } catch (error) {
        console.error('Error processing claim:', error);
        await pgClient.query('ROLLBACK');

        // Check if it's a Solana error
        if (error instanceof Error && error.message.includes('insufficient')) {
            return res.status(503).json({
                error: 'Mint authority has insufficient SOL. Please contact support.'
            });
        }

        return res.status(500).json({ error: 'Failed to process claim' });
    } finally {
        await pgClient.end();
    }
}