/**
 * Centralized configuration for CCM-v3 scripts
 *
 * Architecture:
 * - INPUTS: Read from .env.ccm-v3 (mint, program IDs, external addresses)
 * - DERIVED: PDAs/ATAs computed from inputs (verified against env if present)
 * - SECRETS: RPC URLs, wallet paths (never exported to public config)
 *
 * Usage:
 *   import { CCM_V3_MINT, PROGRAM_ID, ... } from "./config.js";
 *   import { PUBLIC_CONFIG } from "./config.js"; // For cross-repo export
 */

import { PublicKey } from "@solana/web3.js";
import { getAssociatedTokenAddressSync, TOKEN_2022_PROGRAM_ID } from "@solana/spl-token";
import { config } from "dotenv";
import * as path from "path";
import * as url from "url";
import { requireScriptEnv } from "./script-guard.js";

// ESM __dirname shim
const __filename = url.fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Load .env.ccm-v3 from project root
config({ path: path.join(__dirname, "../.env.ccm-v3") });

// =============================================================================
// ENVIRONMENT HELPERS
// =============================================================================

function requireEnv(key: string): string {
  const value = process.env[key];
  if (!value) {
    throw new Error(`Missing required env var: ${key}. Check .env.ccm-v3`);
  }
  return value;
}

function optionalEnv(key: string, fallback: string): string {
  return process.env[key] || fallback;
}

// =============================================================================
// INPUTS (from environment - source of truth)
// =============================================================================

// Program IDs
export const PROGRAM_ID = new PublicKey(
  optionalEnv("AO_PROGRAM_ID", "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop")
);

// Token Mint (required - no fallback)
export const CCM_V3_MINT = new PublicKey(requireEnv("CCM_V3_MINT"));
export const DECIMALS = 9;

// External Addresses (Liquidity Venues)
export const METEORA_POOL = new PublicKey(
  optionalEnv("METEORA_POOL", "6FwqFJb345DvhNWJGdjnnKNefkxH1VaQznWwQgurssmm")
);

// Admin Authority (hardcoded in program constants.rs - immutable)
export const ADMIN_AUTHORITY = new PublicKey(
  "2pHjZLqsSqi35xuYHmZbZBM1xfYV6Ruv57r3eFPvZZaD"
);

// =============================================================================
// SEEDS (matching program constants.rs)
// =============================================================================

export const PROTOCOL_SEED = Buffer.from("protocol");
export const FEE_CONFIG_SEED = Buffer.from("fee_config");
export const STAKE_POOL_SEED = Buffer.from("stake_pool");
export const STAKE_VAULT_SEED = Buffer.from("stake_vault");
export const CHANNEL_STATE_SEED = Buffer.from("channel_state");

// =============================================================================
// DERIVED ADDRESSES (computed from inputs, verified against env)
// =============================================================================

// Derivation functions
export function deriveProtocolStatePda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveFeeConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [PROTOCOL_SEED, CCM_V3_MINT.toBuffer(), FEE_CONFIG_SEED],
    PROGRAM_ID
  );
}

export function deriveStakePoolPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [STAKE_POOL_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveStakeVaultPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [STAKE_VAULT_SEED, CCM_V3_MINT.toBuffer()],
    PROGRAM_ID
  );
}

export function deriveTreasuryAta(protocolStatePda: PublicKey): PublicKey {
  return getAssociatedTokenAddressSync(
    CCM_V3_MINT,
    protocolStatePda,
    true, // allowOwnerOffCurve = true for PDA
    TOKEN_2022_PROGRAM_ID
  );
}

// Derive and verify
const [_protocolStatePda, _protocolBump] = deriveProtocolStatePda();
const _treasuryAta = deriveTreasuryAta(_protocolStatePda);

// Verify against env if present (catches config drift)
function verifyDerivedAddress(name: string, derived: PublicKey, envKey: string): void {
  const envValue = process.env[envKey];
  if (envValue && envValue !== derived.toBase58()) {
    throw new Error(
      `Config drift detected: ${name}\n` +
      `  Derived: ${derived.toBase58()}\n` +
      `  Env (${envKey}): ${envValue}\n` +
      `  Fix: Update .env.ccm-v3 or check program/mint IDs`
    );
  }
}

verifyDerivedAddress("PROTOCOL_STATE_PDA", _protocolStatePda, "PROTOCOL_STATE_PDA");
verifyDerivedAddress("TREASURY_ATA", _treasuryAta, "TREASURY_ATA");

// Export verified derived addresses
export const PROTOCOL_STATE_PDA = _protocolStatePda;
export const PROTOCOL_STATE_BUMP = _protocolBump;
export const TREASURY_ATA = _treasuryAta;

// =============================================================================
// SECRETS (local only - never in PUBLIC_CONFIG)
// =============================================================================

export function getRpcUrl(): string {
  return requireScriptEnv().rpcUrl;
}

export function getWalletPath(): string {
  return requireScriptEnv().keypairPath;
}

// =============================================================================
// PUBLIC CONFIG (JSON-serializable, safe for cross-repo export)
// =============================================================================

export const PUBLIC_CONFIG = {
  // Schema metadata (for consumers to assert compatibility)
  schemaVersion: 1,
  cluster: "mainnet-beta",

  // Programs
  aoProgramId: PROGRAM_ID.toBase58(),

  // Token
  mint: CCM_V3_MINT.toBase58(),
  decimals: DECIMALS,

  // PDAs
  protocolStatePda: PROTOCOL_STATE_PDA.toBase58(),
  treasuryAta: TREASURY_ATA.toBase58(),

  // External
  meteoraPool: METEORA_POOL.toBase58(),

  // Links (for UI)
  links: {
    jupiter: `https://jup.ag/tokens/${CCM_V3_MINT.toBase58()}`,
    meteora: `https://www.meteora.ag/dammv2/${METEORA_POOL.toBase58()}`,
    solscan: `https://solscan.io/token/${CCM_V3_MINT.toBase58()}`,
  },
} as const;

// Type for PUBLIC_CONFIG (useful for consumers)
export type PublicConfig = typeof PUBLIC_CONFIG;

// =============================================================================
// DEBUG
// =============================================================================

export function printConfig(): void {
  console.log("=== Config ===");
  console.log("Program ID:", PROGRAM_ID.toBase58());
  console.log("Mint:", CCM_V3_MINT.toBase58());
  console.log("Meteora Pool:", METEORA_POOL.toBase58());
  console.log("Admin Authority:", ADMIN_AUTHORITY.toBase58());
  console.log("Protocol State PDA:", PROTOCOL_STATE_PDA.toBase58());
  console.log("Treasury ATA:", TREASURY_ATA.toBase58());
  console.log("RPC:", getRpcUrl().substring(0, 50) + "...");
}

// CLI: Generate public-config.json
if (process.argv[1]?.endsWith("config.ts") && process.argv.includes("--export")) {
  console.log(JSON.stringify(PUBLIC_CONFIG, null, 2));
}
