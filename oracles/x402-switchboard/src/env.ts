import 'dotenv/config';
import { z } from 'zod';

const schema = z.object({
  PORT: z.coerce.number().int().positive().default(3000),
  SB_CLUSTER: z
    .enum(['devnet', 'mainnet-beta', 'testnet'])
    .default('devnet'),
  SB_FEED: z.string().min(1, 'SB_FEED (Switchboard aggregator pubkey) is required'),
});

export const env = schema.parse(process.env);

