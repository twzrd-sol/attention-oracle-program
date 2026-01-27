import fs from "fs";
import path from "path";

const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);

function normalizeCluster(input: string): string {
  const trimmed = input.trim();
  if (!trimmed) return trimmed;
  if (trimmed === "mainnet") return "mainnet-beta";
  return trimmed;
}

function expandHome(p: string): string {
  if (!p) return p;
  if (p.startsWith("~")) {
    const home = process.env.HOME || "";
    return path.join(home, p.slice(1));
  }
  return p;
}

export type ScriptEnv = {
  cluster: string;
  rpcUrl: string;
  keypairPath: string;
};

export function requireScriptEnv(): ScriptEnv {
  const rawCluster = process.env.CLUSTER || "";
  if (!rawCluster.trim()) {
    console.error("❌ Missing CLUSTER. Set CLUSTER=localnet|devnet|testnet|mainnet-beta");
    process.exit(2);
  }
  const cluster = normalizeCluster(rawCluster);
  if (!ALLOWED_CLUSTERS.has(cluster)) {
    console.error(`❌ Invalid CLUSTER: ${cluster}. Use localnet|devnet|testnet|mainnet-beta`);
    process.exit(2);
  }
  if (cluster === "mainnet-beta" && process.env.I_UNDERSTAND_MAINNET !== "1") {
    console.error("❌ Refusing mainnet without I_UNDERSTAND_MAINNET=1");
    process.exit(2);
  }

  const rawKeypair = process.env.KEYPAIR || process.env.ANCHOR_WALLET || "";
  if (!rawKeypair.trim()) {
    console.error("❌ Missing KEYPAIR. Set KEYPAIR=/path/to/keypair.json");
    process.exit(2);
  }
  const keypairPath = expandHome(rawKeypair.trim());
  if (!fs.existsSync(keypairPath)) {
    console.error(`❌ Keypair not found: ${keypairPath}`);
    process.exit(2);
  }

  const rpcUrl =
    process.env.RPC_URL ||
    process.env.ANCHOR_PROVIDER_URL ||
    process.env.AO_RPC_URL ||
    process.env.SYNDICA_RPC ||
    process.env.SOLANA_RPC ||
    process.env.SOLANA_URL ||
    "";

  if (!rpcUrl.trim()) {
    console.error("❌ Missing RPC URL. Set RPC_URL or ANCHOR_PROVIDER_URL");
    process.exit(2);
  }

  return { cluster, rpcUrl: rpcUrl.trim(), keypairPath };
}
