import { Pool } from 'pg';

let pool: Pool | null = null;

export function getPool() {
  if (pool) return pool;
  const conn = process.env.DATABASE_URL;
  if (!conn) return null;
  const sslDisabled = process.env.PGSSL_DISABLE === '1';
  pool = new Pool({ connectionString: conn, ssl: sslDisabled ? false : { rejectUnauthorized: false } });
  return pool;
}
