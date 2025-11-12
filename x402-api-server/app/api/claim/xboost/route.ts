import { NextRequest, NextResponse } from 'next/server';
import { getPool } from '@/lib/db';
import { Connection, Keypair, PublicKey, SystemProgram, Transaction, sendAndConfirmTransaction } from '@solana/web3.js';
import bs58 from 'bs58';
import fs from 'fs';
import path from 'path';

const AMOUNT = Number(process.env.BOOST_LAMPORTS || 100000);
const RATE_LIMIT_WINDOW = 24 * 3600;
const LOG_PATH = '/home/twzrd/milo-token/clean-hackathon/vox-xboost.log';

async function ensureTable(client: any) {
  await client.query(`
    CREATE TABLE IF NOT EXISTS xboost_claims (
      wallet TEXT PRIMARY KEY,
      last_claim TIMESTAMPTZ NOT NULL
    )
  `);
}

async function sendLamports(wallet: string) {
  const secret = process.env.BOOST_TREASURY_SECRET;
  const rpc = process.env.SOLANA_RPC_URL || 'https://api.devnet.solana.com';
  if (!secret) return null;
  const treasury = Keypair.fromSecretKey(bs58.decode(secret));
  const connection = new Connection(rpc, 'confirmed');
  const tx = new Transaction().add(
    SystemProgram.transfer({
      fromPubkey: treasury.publicKey,
      toPubkey: new PublicKey(wallet),
      lamports: AMOUNT,
    })
  );
  const sig = await sendAndConfirmTransaction(connection, tx, [treasury], { commitment: 'confirmed' });
  return sig;
}

async function queueOffline(wallet: string) {
  const entry = `${new Date().toISOString()} wallet=${wallet} amount=${AMOUNT}\n`;
  await fs.promises.mkdir(path.dirname(LOG_PATH), { recursive: true }).catch(() => {});
  await fs.promises.appendFile(LOG_PATH, entry);
  return null;
}

export async function POST(req: NextRequest) {
  const pool = getPool();
  if (!pool) {
    return NextResponse.json({ ok: false, error: 'DATABASE_URL missing' });
  }
  const body = await req.json().catch(() => ({}));
  const wallet = (body.wallet || '').trim();
  if (!wallet) return NextResponse.json({ ok: false, error: 'wallet required' });

  const client = await pool.connect();
  try {
    await ensureTable(client);
    const { rows } = await client.query('SELECT last_claim FROM xboost_claims WHERE wallet=$1', [wallet]);
    const now = Math.floor(Date.now() / 1000);
    if (rows.length > 0) {
      const last = Math.floor(new Date(rows[0].last_claim).getTime() / 1000);
      if (now - last < RATE_LIMIT_WINDOW) {
        return NextResponse.json({ ok: false, error: 'Only one boost per 24h' });
      }
    }
    await client.query(
      `INSERT INTO xboost_claims (wallet, last_claim) VALUES ($1, to_timestamp($2))
       ON CONFLICT (wallet) DO UPDATE SET last_claim = to_timestamp($2)`,
      [wallet, now]
    );
  } finally {
    client.release();
  }

  try {
    const sig = await sendLamports(wallet);
    if (sig) return NextResponse.json({ ok: true, method: 'lamports', signature: sig });
    await queueOffline(wallet);
    return NextResponse.json({ ok: true, method: 'offline', note: 'queued for manual mint' });
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || 'mint failed' });
  }
}

