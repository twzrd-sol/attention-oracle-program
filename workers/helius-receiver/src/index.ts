/**
 * Cloudflare Worker: Helius Webhook Receiver for pump.fun Activity
 *
 * Receives enhanced transaction data from Helius, aggregates per-user activity,
 * and stores epoch data in KV for later merkle tree generation.
 */

export interface Env {
  PUMP_DATA: KVNamespace;
  WEBHOOK_SECRET: string;
  PUMP_PROGRAM?: string;  // Optional: override default
  EPOCH_DURATION_MS?: string;  // Optional: override default 5min
}

// Constants
const DEFAULT_PUMP_PROGRAM = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const DEFAULT_EPOCH_DURATION_MS = 5 * 60 * 1000; // 5 minutes

interface UserActivity {
  wallet: string;
  volumeSol: number;
  tradeCount: number;
  uniqueTokens: string[];
  lastUpdate: number;
}

interface HeliusTransaction {
  signature: string;
  timestamp: number;
  nativeTransfers?: Array<{
    fromUserAccount: string;
    toUserAccount: string;
    amount: number;
  }>;
  tokenTransfers?: Array<{
    fromUserAccount: string;
    toUserAccount: string;
    mint: string;
    tokenAmount: number;
  }>;
  accountData?: any[];
  instructions?: any[];
  type?: string;
}

interface SwapEvent {
  wallet: string;
  tokenMint: string;
  solAmount: number;
  isBuy: boolean;
  timestamp: number;
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const url = new URL(request.url);

    // CORS headers
    const corsHeaders = {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type, x-webhook-secret',
    };

    if (request.method === 'OPTIONS') {
      return new Response(null, { headers: corsHeaders });
    }

    // Route: POST /webhook - Helius webhook receiver
    if (request.method === 'POST' && url.pathname === '/webhook') {
      return handleWebhook(request, env, corsHeaders);
    }

    // Route: GET /epoch/:epochNum - Fetch epoch data
    if (request.method === 'GET' && url.pathname.startsWith('/epoch/')) {
      const epochNum = url.pathname.split('/')[2];
      return getEpochData(epochNum, env, corsHeaders);
    }

    // Route: GET /current-epoch - Get current epoch number
    if (request.method === 'GET' && url.pathname === '/current-epoch') {
      const epochDuration = parseInt(env.EPOCH_DURATION_MS || String(DEFAULT_EPOCH_DURATION_MS));
      const currentEpoch = Math.floor(Date.now() / epochDuration);
      return new Response(JSON.stringify({ epoch: currentEpoch }), {
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      });
    }

    // Route: GET /health - Health check
    if (request.method === 'GET' && url.pathname === '/health') {
      return new Response(JSON.stringify({ status: 'ok' }), {
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      });
    }

    return new Response('Not found', { status: 404, headers: corsHeaders });
  },
};

async function handleWebhook(
  request: Request,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  // Verify webhook secret
  const secret = request.headers.get('x-webhook-secret');
  if (secret !== env.WEBHOOK_SECRET) {
    return new Response('Unauthorized', { status: 401, headers: corsHeaders });
  }

  try {
    const body = await request.json();
    const events: HeliusTransaction[] = Array.isArray(body) ? body : [body];

    const pumpProgram = env.PUMP_PROGRAM || DEFAULT_PUMP_PROGRAM;
    const epochDuration = parseInt(env.EPOCH_DURATION_MS || String(DEFAULT_EPOCH_DURATION_MS));
    const currentEpoch = Math.floor(Date.now() / epochDuration);

    let processedCount = 0;
    let errorCount = 0;

    for (const tx of events) {
      try {
        const swap = parsePumpSwap(tx, pumpProgram);
        if (!swap) continue;

        await aggregateSwap(swap, currentEpoch, env);
        processedCount++;
      } catch (err) {
        console.error('Error processing tx:', err);
        errorCount++;
      }
    }

    return new Response(
      JSON.stringify({
        received: events.length,
        processed: processedCount,
        errors: errorCount,
        epoch: currentEpoch,
      }),
      {
        status: 200,
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      }
    );
  } catch (err: any) {
    console.error('Webhook error:', err);
    return new Response(JSON.stringify({ error: err.message }), {
      status: 500,
      headers: { ...corsHeaders, 'Content-Type': 'application/json' },
    });
  }
}

function parsePumpSwap(tx: HeliusTransaction, pumpProgram: string): SwapEvent | null {
  // pump.fun bonding curve swaps involve:
  // 1. SOL transfer (user → bonding curve OR bonding curve → user)
  // 2. Token transfer (bonding curve → user OR user → bonding curve)

  const solTransfers = tx.nativeTransfers || [];
  const tokenTransfers = tx.tokenTransfers || [];

  // Find SOL transfer involving pump program
  const solTransfer = solTransfers.find(
    (t) => t.fromUserAccount === pumpProgram || t.toUserAccount === pumpProgram
  );

  if (!solTransfer) return null;

  // Determine if buy or sell
  const isBuy = solTransfer.toUserAccount === pumpProgram;
  const wallet = isBuy ? solTransfer.fromUserAccount : solTransfer.toUserAccount;

  // Get token mint from token transfer
  const tokenTransfer = tokenTransfers.find(
    (t) => t.fromUserAccount === pumpProgram || t.toUserAccount === pumpProgram
  );

  if (!tokenTransfer) return null;

  return {
    wallet,
    tokenMint: tokenTransfer.mint,
    solAmount: solTransfer.amount, // lamports
    isBuy,
    timestamp: tx.timestamp || Date.now() / 1000,
  };
}

async function aggregateSwap(swap: SwapEvent, epoch: number, env: Env): Promise<void> {
  const key = `epoch:${epoch}:${swap.wallet}`;

  // Get existing data
  const existingData = await env.PUMP_DATA.get(key);
  const activity: UserActivity = existingData
    ? JSON.parse(existingData)
    : {
        wallet: swap.wallet,
        volumeSol: 0,
        tradeCount: 0,
        uniqueTokens: [],
        lastUpdate: 0,
      };

  // Update activity
  activity.volumeSol += swap.solAmount / 1e9; // lamports → SOL
  activity.tradeCount += 1;
  activity.lastUpdate = Date.now();

  // Add unique token
  if (!activity.uniqueTokens.includes(swap.tokenMint)) {
    activity.uniqueTokens.push(swap.tokenMint);
  }

  // Store back in KV
  // TTL: 7 days (604800 seconds) - adjust as needed
  await env.PUMP_DATA.put(key, JSON.stringify(activity), {
    expirationTtl: 604800,
  });

  // Also update epoch metadata (total users, total trades)
  const metaKey = `epoch:${epoch}:meta`;
  const metaData = await env.PUMP_DATA.get(metaKey);
  const meta = metaData ? JSON.parse(metaData) : { users: new Set(), totalTrades: 0 };

  if (Array.isArray(meta.users)) {
    meta.users = new Set(meta.users);
  }

  meta.users.add(swap.wallet);
  meta.totalTrades = (meta.totalTrades || 0) + 1;

  await env.PUMP_DATA.put(
    metaKey,
    JSON.stringify({
      users: Array.from(meta.users),
      totalTrades: meta.totalTrades,
      lastUpdate: Date.now(),
    }),
    { expirationTtl: 604800 }
  );
}

async function getEpochData(
  epochNum: string,
  env: Env,
  corsHeaders: Record<string, string>
): Promise<Response> {
  try {
    const epoch = parseInt(epochNum);
    if (isNaN(epoch)) {
      return new Response('Invalid epoch number', { status: 400, headers: corsHeaders });
    }

    // List all keys for this epoch
    const prefix = `epoch:${epoch}:`;
    const list = await env.PUMP_DATA.list({ prefix });

    const users: UserActivity[] = [];
    let metadata: any = null;

    for (const key of list.keys) {
      if (key.name.endsWith(':meta')) {
        const data = await env.PUMP_DATA.get(key.name);
        metadata = data ? JSON.parse(data) : null;
      } else {
        const data = await env.PUMP_DATA.get(key.name);
        if (data) {
          users.push(JSON.parse(data));
        }
      }
    }

    return new Response(
      JSON.stringify({
        epoch,
        users,
        metadata,
        userCount: users.length,
      }),
      {
        headers: { ...corsHeaders, 'Content-Type': 'application/json' },
      }
    );
  } catch (err: any) {
    return new Response(JSON.stringify({ error: err.message }), {
      status: 500,
      headers: { ...corsHeaders, 'Content-Type': 'application/json' },
    });
  }
}
