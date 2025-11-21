import 'dotenv/config';
import { z } from 'zod';

const schema = z.object({
  PORT: z.coerce.number().int().positive().default(3000),
  RPC_URL: z.string().url().default("https://api.mainnet-beta.solana.com"),
  // Deployed program ID (v1.1.0)
  PROGRAM_ID: z.string().default("GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"),
  // Minimum tier to access /premium endpoints (0-5)
  MIN_TIER_PREMIUM: z.coerce.number().int().min(0).max(5).default(2),
  // Minimum score for premium access
  MIN_SCORE_PREMIUM: z.coerce.number().default(1000),
});

export const env = schema.parse(process.env);
