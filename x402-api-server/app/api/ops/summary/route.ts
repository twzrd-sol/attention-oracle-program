import { NextResponse } from 'next/server';
import { Pool } from 'pg';

function getPool() {
  const conn = process.env.DATABASE_URL;
  if (!conn) return null;
  const sslEnv = process.env.DB_SSL_INSECURE === '1' ? { rejectUnauthorized: false } : undefined;
  const ssl = process.env.PGSSL_DISABLE === '1' ? false : (sslEnv || { rejectUnauthorized: false });
  return new Pool({ connectionString: conn, ssl });
}

export async function GET() {
  const pool = getPool();
  if (!pool) {
    return NextResponse.json({ ok: false, error: 'DATABASE_URL not set on server' }, { status: 200 });
  }
  try {
    const client = await pool.connect();
    try {
      const q1 = await client.query(
        `SELECT MAX(epoch) AS latest_epoch,
                MAX(sealed_at) AS latest_sealed_at_epoch,
                to_timestamp(MAX(sealed_at)) AS latest_sealed_at_ts,
                COUNT(*) FILTER (WHERE sealed_at >= extract(epoch from now()) - 86400) AS sealed_24h
         FROM sealed_epochs`
      );
      const q2 = await client.query(
        `SELECT COUNT(*) AS events_last_hour
         FROM channel_participation
         WHERE epoch >= extract(epoch from now()) - 3600`
      );
      const q3 = await client.query(
        `SELECT COUNT(*) AS sp_total,
                SUM(CASE WHEN username IS NULL THEN 1 ELSE 0 END) AS sp_null
         FROM sealed_participants`
      );
      const q4 = await client.query(
        `SELECT COUNT(DISTINCT channel) AS channels_24h
         FROM sealed_participants
         WHERE epoch >= extract(epoch from now()) - 86400`
      );

      const sealed = q1.rows[0] || {};
      const events = q2.rows[0] || {};
      const participants = q3.rows[0] || {};
      const channels_24h = q4.rows[0] || {};

      return NextResponse.json({ ok: true, sealed, events, participants, channels_24h, ts: Date.now() });
    } finally {
      pool.end();
    }
  } catch (e: any) {
    return NextResponse.json({ ok: false, error: e?.message || String(e) }, { status: 200 });
  }
}

