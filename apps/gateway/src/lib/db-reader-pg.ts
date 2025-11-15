/**
 * PostgreSQL DB Reader for Gateway
 * Reads aggregator PostgreSQL DB for sealed epochs and proofs
 * Migration from better-sqlite3 to native PostgreSQL adapter
 */

import { Pool, QueryResult } from 'pg';
import { makeParticipationLeaf, merkleRoot, generateProof, hex } from './participation-merkle';

const DATABASE_URL = process.env.GATEWAY_DATABASE_URL || process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd';

export class DbReaderPg {
  public pool: Pool;
  private schemaInfoLoaded = false;
  private hasTokenGroup = false;
  private hasCategory = false;
  private schemaInfoPromise: Promise<void> | null = null;

  constructor(connectionString: string = DATABASE_URL) {
    // Ensure SSL for managed Postgres (sslmode=require in URL may not be honored by all clients)
    const ssl = { rejectUnauthorized: false } as any;
    this.pool = new Pool({
      connectionString,
      ssl,
      max: 5,
      idleTimeoutMillis: 30000,
      connectionTimeoutMillis: 5000,
    });
  }

  private async loadSchemaInfo() {
    if (this.schemaInfoLoaded) return;
    if (!this.schemaInfoPromise) {
      this.schemaInfoPromise = (async () => {
        const res = await this.pool.query(
          `SELECT column_name FROM information_schema.columns
           WHERE table_schema = 'public'
             AND table_name IN ('sealed_epochs', 'sealed_participants')
             AND column_name IN ('token_group', 'category')`
        );
        for (const row of res.rows) {
          if (row.column_name === 'token_group') this.hasTokenGroup = true;
          if (row.column_name === 'category') this.hasCategory = true;
        }
        this.schemaInfoLoaded = true;
      })();
    }
    await this.schemaInfoPromise;
  }

  /**
   * Get sealed participants in frozen order
   * With optional token_group and category filtering for multi-dimensional gating
   */
  async getSealedParticipants(epoch: number, channel: string, tokenGroup: string = 'MILO', category: string = 'default'): Promise<string[] | null> {
    await this.loadSchemaInfo();

    const where: string[] = ['epoch = $1', 'channel = $2'];
    const params: any[] = [epoch, channel];

    if (this.hasTokenGroup) {
      where.push(`token_group = $${params.length + 1}`);
      params.push(tokenGroup);
    }
    if (this.hasCategory) {
      where.push(`category = $${params.length + 1}`);
      params.push(category);
    }

    const result: QueryResult = await this.pool.query(
      `SELECT user_hash FROM sealed_participants
       WHERE ${where.join(' AND ')}
       ORDER BY idx ASC`,
      params
    );

    if (!result.rows || result.rows.length === 0) return null;
    return result.rows.map((r) => r.user_hash);
  }

  /**
   * Get sealed root if exists
   * With optional token_group and category filtering for multi-dimensional gating
   */
  async getSealedRoot(epoch: number, channel: string, tokenGroup: string = 'MILO', category: string = 'default'): Promise<string | null> {
    await this.loadSchemaInfo();

    const where: string[] = ['epoch = $1', 'channel = $2'];
    const params: any[] = [epoch, channel];

    if (this.hasTokenGroup) {
      where.push(`token_group = $${params.length + 1}`);
      params.push(tokenGroup);
    }
    if (this.hasCategory) {
      where.push(`category = $${params.length + 1}`);
      params.push(category);
    }

    const result: QueryResult = await this.pool.query(
      `SELECT root FROM sealed_epochs
       WHERE ${where.join(' AND ')}
       LIMIT 1`,
      params
    );

    return result.rows[0]?.root || null;
  }

  /**
   * Check if epoch is sealed
   * With optional token_group and category filtering for multi-dimensional gating
   */
  async isEpochSealed(epoch: number, channel: string, tokenGroup: string = 'MILO', category: string = 'default'): Promise<boolean> {
    await this.loadSchemaInfo();

    const where: string[] = ['epoch = $1', 'channel = $2'];
    const params: any[] = [epoch, channel];

    if (this.hasTokenGroup) {
      where.push(`token_group = $${params.length + 1}`);
      params.push(tokenGroup);
    }
    if (this.hasCategory) {
      where.push(`category = $${params.length + 1}`);
      params.push(category);
    }

    const result: QueryResult = await this.pool.query(
      `SELECT 1 FROM sealed_epochs
       WHERE ${where.join(' AND ')}
       LIMIT 1`,
      params
    );

    return result.rows.length > 0;
  }

  /**
   * Generate proof for specific participant index
   * With optional token_group and category filtering for multi-dimensional gating
   * Returns { user_hash, proof, root } or null if invalid
   */
  async generateProof(epoch: number, channel: string, index: number, tokenGroup: string = 'MILO', category: string = 'default'): Promise<{
    user_hash: string;
    proof: string[];
    root: string;
  } | null> {
    const participants = await this.getSealedParticipants(epoch, channel, tokenGroup, category);
    if (!participants || index >= participants.length) return null;

    const user_hash = participants[index];
    const leaves = participants.map((u) => makeParticipationLeaf({ user_hash: u, channel, epoch }));
    const root = merkleRoot(leaves);
    const proof = generateProof(leaves, index);

    return {
      user_hash,
      proof: proof.map(hex),
      root: hex(root),
    };
  }

  /**
   * Get all sealed epochs for a channel
   */
  async getSealedEpochs(channel: string): Promise<number[]> {
    const result: QueryResult = await this.pool.query(
      `SELECT DISTINCT epoch FROM sealed_epochs
       WHERE channel = $1
       ORDER BY epoch DESC`,
      [channel]
    );

    return result.rows.map((r) => Number(r.epoch));
  }

  /**
   * Close database connection pool
   */
  async close(): Promise<void> {
    await this.pool.end();
  }
}

// Singleton instance for gateway
let dbReader: DbReaderPg | null = null;

export function getDbReader(): DbReaderPg {
  if (!dbReader) {
    dbReader = new DbReaderPg();
  }
  return dbReader;
}
