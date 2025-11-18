import dotenv from 'dotenv';
import { z } from 'zod';
dotenv.config();
const schema = z.object({
    PROGRAM_ID: z.string().min(32, 'PROGRAM_ID is required'),
    RPC_URL: z.string().url('RPC_URL must be a valid URL'),
    RPC_URL_WS: z.string().url().optional(),
    STREAM_COMMITMENT: z
        .enum(['processed', 'confirmed', 'finalized'])
        .default('confirmed'),
    LOG_DIR: z.string().default('../logs'),
    GATEWAY_URL: z.string().url().optional(),
    INTERNAL_EVENT_TOKEN: z.string().optional(),
    MINT_PUBKEY: z.string().optional(),
    STREAM_CHANNELS: z
        .string()
        .transform((s) => s
        .split(',')
        .map((c) => c.trim())
        .filter(Boolean))
        .optional(),
});
export const env = schema.parse(process.env);
