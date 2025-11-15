/**
 * /api/eligibility endpoint
 *
 * Check if a user is eligible to claim CHAT tokens
 */

import { NextApiRequest, NextApiResponse } from 'next';
import { Client } from 'pg';

const DATABASE_URL = process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025twzrd@localhost:5432/twzrd';

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
    if (req.method !== 'GET') {
        return res.status(405).json({ error: 'Method not allowed' });
    }

    // Get session (would come from auth middleware)
    const username = req.query.username as string || req.session?.username;
    const epoch = req.query.epoch ? parseInt(req.query.epoch as string) : null;
    const channel = req.query.channel as string;

    if (!username) {
        return res.status(400).json({ error: 'Username required' });
    }

    const client = new Client(DATABASE_URL);

    try {
        await client.connect();

        // If no epoch specified, use most recent
        let targetEpoch = epoch;
        if (!targetEpoch) {
            const epochResult = await client.query(`
                SELECT MAX(epoch) as latest_epoch
                FROM claimable_allocations
                WHERE username = $1
            `, [username.toLowerCase()]);

            targetEpoch = epochResult.rows[0]?.latest_epoch;

            if (!targetEpoch) {
                return res.status(200).json({
                    eligible: false,
                    username,
                    message: 'No allocations found for this user'
                });
            }
        }

        // Build query
        let query = `
            SELECT
                ca.epoch,
                ca.channel,
                ca.username,
                ca.amount,
                ca.redeemed_at,
                ca.wallet,
                ca.tx_signature,
                es.messages,
                es.unique_minutes,
                es.gifts,
                es.new_subs,
                es.bits
            FROM claimable_allocations ca
            LEFT JOIN twitch_epoch_stats es
                ON ca.epoch = es.epoch
                AND ca.channel = es.channel
                AND ca.username = es.username
            WHERE ca.username = $1
              AND ca.epoch = $2
        `;

        const params: any[] = [username.toLowerCase(), targetEpoch];

        if (channel) {
            query += ' AND ca.channel = $3';
            params.push(channel);
        }

        const result = await client.query(query, params);

        if (result.rows.length === 0) {
            return res.status(200).json({
                eligible: false,
                username,
                epoch: targetEpoch,
                channel,
                message: 'No allocation found for this epoch/channel'
            });
        }

        // Aggregate if multiple channels
        const allocations = result.rows;
        const totalAmount = allocations.reduce((sum, a) => sum + parseInt(a.amount), 0);
        const claimed = allocations.filter(a => a.redeemed_at).length;
        const unclaimed = allocations.filter(a => !a.redeemed_at);

        // Return eligibility
        return res.status(200).json({
            eligible: unclaimed.length > 0,
            username,
            epoch: targetEpoch,
            epochTime: new Date(targetEpoch * 1000).toISOString(),
            summary: {
                total_amount: totalAmount,
                claimed_amount: allocations
                    .filter(a => a.redeemed_at)
                    .reduce((sum, a) => sum + parseInt(a.amount), 0),
                unclaimed_amount: unclaimed.reduce((sum, a) => sum + parseInt(a.amount), 0),
                channels_eligible: allocations.length,
                channels_claimed: claimed
            },
            allocations: allocations.map(a => ({
                channel: a.channel,
                amount: parseInt(a.amount),
                messages: a.messages || 0,
                unique_minutes: a.unique_minutes || 0,
                gifts: a.gifts || 0,
                new_subs: a.new_subs || 0,
                bits: a.bits || 0,
                claimed: !!a.redeemed_at,
                claimed_at: a.redeemed_at,
                wallet: a.wallet,
                tx_signature: a.tx_signature
            }))
        });

    } catch (error) {
        console.error('Error checking eligibility:', error);
        return res.status(500).json({ error: 'Internal server error' });
    } finally {
        await client.end();
    }
}