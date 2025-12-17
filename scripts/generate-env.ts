#!/usr/bin/env tsx

/**
 * Generate environment files for other repos from canonical .env.ccm-v3
 *
 * Usage:
 *   pnpm v3:gen-env backend   # outputs backend .env.ccm-v3 (public addresses only)
 *   pnpm v3:gen-env frontend  # outputs frontend .env.local (VITE_ prefixed)
 *   pnpm v3:gen-env all       # outputs both to stdout (separated)
 *
 * Options:
 *   --out <path>  Write to file instead of stdout
 *   --json        Output as JSON (for programmatic consumption)
 */

import { PUBLIC_CONFIG } from "./config.js";
import * as fs from "fs";

type Target = "backend" | "frontend" | "all";

function generateBackendEnv(): string {
  const lines = [
    "# CCM-v3 Backend Configuration",
    "# Generated from attention-oracle-program config",
    "#",
    "# Copy this to your backend repo as .env.ccm-v3",
    "",
    "# =============================================================================",
    "# Schema",
    "# =============================================================================",
    `SCHEMA_VERSION=${PUBLIC_CONFIG.schemaVersion}`,
    `CLUSTER=${PUBLIC_CONFIG.cluster}`,
    "",
    "# =============================================================================",
    "# Token Configuration",
    "# =============================================================================",
    `CCM_V3_MINT=${PUBLIC_CONFIG.mint}`,
    `CCM_V3_DECIMALS=${PUBLIC_CONFIG.decimals}`,
    "",
    "# =============================================================================",
    "# Program IDs",
    "# =============================================================================",
    `AO_PROGRAM_ID=${PUBLIC_CONFIG.aoProgramId}`,
    `CCM_HOOK_PROGRAM_ID=${PUBLIC_CONFIG.ccmHookProgramId}`,
    "",
    "# =============================================================================",
    "# PDAs (derived from mint + program, verified at load)",
    "# =============================================================================",
    `PROTOCOL_STATE_PDA=${PUBLIC_CONFIG.protocolStatePda}`,
    `TREASURY_ATA=${PUBLIC_CONFIG.treasuryAta}`,
    "",
    "# =============================================================================",
    "# External Addresses (Liquidity Venues)",
    "# =============================================================================",
    `METEORA_POOL=${PUBLIC_CONFIG.meteoraPool}`,
    "",
    "# =============================================================================",
    "# SECRETS (add locally, DO NOT commit)",
    "# =============================================================================",
    "# DATABASE_URL=postgres://...",
    "# RPC_URL=https://...",
    "",
  ];
  return lines.join("\n");
}

function generateFrontendEnv(): string {
  const lines = [
    "# CCM-v3 Frontend Configuration (Vite)",
    "# Generated from attention-oracle-program config",
    "#",
    "# Copy this to your frontend repo as .env.local",
    "# Only VITE_* variables are exposed to client code",
    "",
    "# =============================================================================",
    "# Schema",
    "# =============================================================================",
    `VITE_SCHEMA_VERSION=${PUBLIC_CONFIG.schemaVersion}`,
    `VITE_CLUSTER=${PUBLIC_CONFIG.cluster}`,
    "",
    "# =============================================================================",
    "# Token Configuration",
    "# =============================================================================",
    `VITE_CCM_V3_MINT=${PUBLIC_CONFIG.mint}`,
    `VITE_CCM_V3_DECIMALS=${PUBLIC_CONFIG.decimals}`,
    "",
    "# =============================================================================",
    "# Program IDs",
    "# =============================================================================",
    `VITE_AO_PROGRAM_ID=${PUBLIC_CONFIG.aoProgramId}`,
    `VITE_CCM_HOOK_PROGRAM_ID=${PUBLIC_CONFIG.ccmHookProgramId}`,
    "",
    "# =============================================================================",
    "# PDAs",
    "# =============================================================================",
    `VITE_PROTOCOL_STATE_PDA=${PUBLIC_CONFIG.protocolStatePda}`,
    `VITE_TREASURY_ATA=${PUBLIC_CONFIG.treasuryAta}`,
    "",
    "# =============================================================================",
    "# External Links",
    "# =============================================================================",
    `VITE_METEORA_POOL=${PUBLIC_CONFIG.meteoraPool}`,
    `VITE_JUPITER_URL=${PUBLIC_CONFIG.links.jupiter}`,
    `VITE_METEORA_URL=${PUBLIC_CONFIG.links.meteora}`,
    `VITE_SOLSCAN_URL=${PUBLIC_CONFIG.links.solscan}`,
    "",
    "# =============================================================================",
    "# API Endpoints (add locally)",
    "# =============================================================================",
    "# VITE_API_URL=https://api.yourbackend.com",
    "",
  ];
  return lines.join("\n");
}

function generateJson(target: Target): string {
  if (target === "backend") {
    return JSON.stringify({
      schemaVersion: PUBLIC_CONFIG.schemaVersion,
      cluster: PUBLIC_CONFIG.cluster,
      mint: PUBLIC_CONFIG.mint,
      decimals: PUBLIC_CONFIG.decimals,
      aoProgramId: PUBLIC_CONFIG.aoProgramId,
      ccmHookProgramId: PUBLIC_CONFIG.ccmHookProgramId,
      protocolStatePda: PUBLIC_CONFIG.protocolStatePda,
      treasuryAta: PUBLIC_CONFIG.treasuryAta,
      meteoraPool: PUBLIC_CONFIG.meteoraPool,
    }, null, 2);
  } else if (target === "frontend") {
    return JSON.stringify({
      schemaVersion: PUBLIC_CONFIG.schemaVersion,
      cluster: PUBLIC_CONFIG.cluster,
      VITE_CCM_V3_MINT: PUBLIC_CONFIG.mint,
      VITE_CCM_V3_DECIMALS: PUBLIC_CONFIG.decimals,
      VITE_AO_PROGRAM_ID: PUBLIC_CONFIG.aoProgramId,
      VITE_CCM_HOOK_PROGRAM_ID: PUBLIC_CONFIG.ccmHookProgramId,
      VITE_PROTOCOL_STATE_PDA: PUBLIC_CONFIG.protocolStatePda,
      VITE_TREASURY_ATA: PUBLIC_CONFIG.treasuryAta,
      VITE_METEORA_POOL: PUBLIC_CONFIG.meteoraPool,
      VITE_JUPITER_URL: PUBLIC_CONFIG.links.jupiter,
      VITE_METEORA_URL: PUBLIC_CONFIG.links.meteora,
      VITE_SOLSCAN_URL: PUBLIC_CONFIG.links.solscan,
    }, null, 2);
  } else {
    return JSON.stringify(PUBLIC_CONFIG, null, 2);
  }
}

function main() {
  const args = process.argv.slice(2);
  const target = (args.find(a => !a.startsWith("--")) || "all") as Target;
  const jsonMode = args.includes("--json");
  const outIndex = args.indexOf("--out");
  const outPath = outIndex !== -1 ? args[outIndex + 1] : null;

  if (!["backend", "frontend", "all"].includes(target)) {
    console.error(`Unknown target: ${target}`);
    console.error("Usage: pnpm v3:gen-env [backend|frontend|all] [--json] [--out <path>]");
    process.exit(1);
  }

  let output: string;

  if (jsonMode) {
    output = generateJson(target);
  } else if (target === "backend") {
    output = generateBackendEnv();
  } else if (target === "frontend") {
    output = generateFrontendEnv();
  } else {
    // all
    output = [
      "# =========================================",
      "# BACKEND (.env.ccm-v3)",
      "# =========================================",
      generateBackendEnv(),
      "",
      "# =========================================",
      "# FRONTEND (.env.local)",
      "# =========================================",
      generateFrontendEnv(),
    ].join("\n");
  }

  if (outPath) {
    fs.writeFileSync(outPath, output);
    console.log(`Written to ${outPath}`);
  } else {
    console.log(output);
  }
}

main();
