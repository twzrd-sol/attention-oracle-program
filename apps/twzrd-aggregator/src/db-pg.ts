import { Pool, PoolClient } from 'pg'
import fs from 'fs'
import { hashUser, ParticipationRow, SignalRow, WeightedParticipant } from './db-types.js'
import { canonicalUserHash } from './util/hashing.js'

export class TwzrdDBPostgres {
  private ingestPool: Pool // High-volume ingestion (recordParticipation, recordSignals)
  private maintenancePool: Pool // Publisher/sealing/maintenance (getUnpublishedRoots, getSealedParticipants, markRootAsPublished)
  private ensuredChannelPayoutsTable = false
  private schemaInfoLoaded = false
  private hasTokenGroup = false
  private hasCategory = false
  private ensuredWalletBindingTable = false
  private ensuredTwitchWalletBindingsTable = false
  private schemaInfoPromise: Promise<void> | null = null

  constructor(connectionString?: string) {
    const connString = connectionString || process.env.DATABASE_URL || 'postgresql://twzrd:twzrd_password_2025@localhost:5432/twzrd'

    // TLS Option C: CA-validated SSL for managed Postgres
    // Uses explicit CA file (default path provided by ops); override with PG_CA_CERT_PATH if needed.
    const caCertPath = process.env.PG_CA_CERT_PATH || '/home/twzrd/certs/do-managed-db-ca.crt'
    const sslConfig = {
      ca: fs.readFileSync(caCertPath, 'utf8'),
      rejectUnauthorized: true,
    }

    // Ingestion pool: higher capacity for high-volume participation/signal recording
    this.ingestPool = new Pool({
      connectionString: connString,
      max: Number(process.env.DB_POOL_INGEST_MAX || 40),
      idleTimeoutMillis: Number(process.env.DB_POOL_INGEST_IDLE || 30000),
      connectionTimeoutMillis: Number(process.env.DB_POOL_INGEST_TIMEOUT || 5000),
      ssl: sslConfig, // Option C: CA-validated TLS
    })

    // Maintenance pool: dedicated for publisher and other critical operations
    this.maintenancePool = new Pool({
      connectionString: connString,
      max: Number(process.env.DB_POOL_MAINT_MAX || 8),
      idleTimeoutMillis: Number(process.env.DB_POOL_MAINT_IDLE || 30000),
      connectionTimeoutMillis: Number(process.env.DB_POOL_MAINT_TIMEOUT || 10000),
      ssl: sslConfig, // Option C: CA-validated TLS
    })
  }

  // Backward-compatible getter: returns ingestPool for general queries
  get pool(): Pool {
    return this.ingestPool
  }

  private async loadSchemaInfo() {
    if (this.schemaInfoLoaded) return
    if (!this.schemaInfoPromise) {
      this.schemaInfoPromise = (async () => {
        const res = await this.maintenancePool.query(
          `SELECT column_name FROM information_schema.columns
           WHERE table_schema = 'public'
             AND table_name IN ('sealed_epochs', 'sealed_participants')
             AND column_name IN ('token_group', 'category')`
        )
        for (const row of res.rows) {
          if (row.column_name === 'token_group') this.hasTokenGroup = true
          if (row.column_name === 'category') this.hasCategory = true
        }
        this.schemaInfoLoaded = true
      })()
    }
    await this.schemaInfoPromise
    // Ensure wallet-binding table exists (idempotent)
    if (!this.ensuredWalletBindingTable) {
      await this.maintenancePool.query(`
        CREATE TABLE IF NOT EXISTS user_wallet_bindings (
          user_hash TEXT NOT NULL,
          username TEXT,
          wallet TEXT NOT NULL,
          verified BOOLEAN DEFAULT FALSE,
          source TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL,
          PRIMARY KEY (user_hash, wallet)
        );
        CREATE INDEX IF NOT EXISTS idx_uwb_user_hash ON user_wallet_bindings(user_hash);
        CREATE INDEX IF NOT EXISTS idx_uwb_wallet ON user_wallet_bindings(wallet);
      `)
      this.ensuredWalletBindingTable = true
    }

    if (!this.ensuredTwitchWalletBindingsTable) {
      await this.maintenancePool.query(`
        CREATE TABLE IF NOT EXISTS twitch_wallet_bindings (
          twitch_id TEXT PRIMARY KEY,
          login TEXT NOT NULL,
          wallet TEXT NOT NULL,
          created_at TIMESTAMPTZ DEFAULT NOW(),
          updated_at TIMESTAMPTZ DEFAULT NOW()
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_wallet_unique ON twitch_wallet_bindings (wallet);
      `)
      this.ensuredTwitchWalletBindingsTable = true
    }
  }


  /**
   * Record user participation (idempotent) with batched inserts for 10-20x throughput
   */
  async recordParticipation(rows: ParticipationRow[]) {
    if (rows.length === 0) return

    const BATCH_SIZE = 1000
    const MAX_RETRIES = 3
    const client = await this.ingestPool.connect()

    try {
      await client.query('BEGIN')

      for (let i = 0; i < rows.length; i += BATCH_SIZE) {
        const batch = rows.slice(i, i + BATCH_SIZE)
        let attempt = 0

        while (attempt < MAX_RETRIES) {
          try {
            // Build bulk INSERT with multiple VALUES
            const values: any[] = []
            const placeholders: string[] = []

            batch.forEach((row, idx) => {
              const base = idx * 6
              placeholders.push(`($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, $${base + 5}, $${base + 6})`)
              values.push(row.epoch, row.channel, row.user_hash, row.first_seen, row.token_group, row.category)
            })

            const query = `
              INSERT INTO channel_participation (epoch, channel, user_hash, first_seen, token_group, category)
              VALUES ${placeholders.join(', ')}
              ON CONFLICT (epoch, channel, user_hash) DO NOTHING
            `

            await client.query(query, values)
            break // Success

          } catch (err: any) {
            attempt++

            // Retry on specific errors
            if (
              err.code === '40001' || // serialization_failure
              err.code === '40P01' || // deadlock_detected
              err.message?.includes('too many clients') ||
              err.message?.includes('connection')
            ) {
              if (attempt >= MAX_RETRIES) throw err

              // Exponential backoff
              const delay = Math.min(100 * Math.pow(2, attempt), 2000)
              await new Promise(resolve => setTimeout(resolve, delay))
              continue
            }

            throw err // Non-retryable error
          }
        }
      }

      await client.query('COMMIT')

    } catch (err) {
      await client.query('ROLLBACK')
      throw err
    } finally {
      client.release()
    }
  }

  /**
   * Get participants for an epoch/channel
   */
  async getParticipants(epoch: number, channel: string): Promise<string[]> {
    const result = await this.maintenancePool.query(
      `SELECT user_hash FROM channel_participation
       WHERE epoch = $1 AND channel = $2
       ORDER BY first_seen ASC`,
      [epoch, channel]
    )
    return result.rows.map((r: any) => r.user_hash)
  }

  /**
   * Get sealed participants (frozen snapshot)
   */
  async getSealedParticipants(epoch: number, channel: string, tokenGroup = 'MILO', category = 'default'): Promise<string[] | null> {
    await this.loadSchemaInfo()

    // Query sealed_participants by epoch/channel only, don't filter by token_group
    // The token_group is determined by sealed_epochs, and there may be data inconsistency
    // where participants have different token_group than the epoch
    const conditions = ['epoch = $1', 'channel = $2']
    const values: any[] = [epoch, channel]

    const result = await this.maintenancePool.query(
      `SELECT user_hash FROM sealed_participants
       WHERE ${conditions.join(' AND ')}
       ORDER BY idx ASC`,
      values
    )
    if (result.rows.length === 0) return null
    return result.rows.map((r: any) => r.user_hash)
  }

  /**
   * Get weighted participants with signal breakdown
   */
  async getWeightedParticipants(epoch: number, channel: string): Promise<WeightedParticipant[]> {
    const result = await this.maintenancePool.query(
      `SELECT
         user_hash,
         SUM(CASE WHEN signal_type = 'presence' THEN value ELSE 0 END) as presence,
         SUM(CASE WHEN signal_type = 'sub' THEN value ELSE 0 END) as sub,
         SUM(CASE WHEN signal_type = 'resub' THEN value ELSE 0 END) as resub,
         SUM(CASE WHEN signal_type = 'gift' THEN value ELSE 0 END) as gift,
         SUM(CASE WHEN signal_type = 'bits' THEN value ELSE 0 END) as bits,
         SUM(CASE WHEN signal_type = 'raid' THEN value ELSE 0 END) as raid
       FROM user_signals
       WHERE epoch = $1 AND channel = $2
       GROUP BY user_hash`,
      [epoch, channel]
    )

    return result.rows.map((row: any) => {
      const signals = {
        presence: parseFloat(row.presence) || 0,
        sub: parseFloat(row.sub) || 0,
        resub: parseFloat(row.resub) || 0,
        gift: parseFloat(row.gift) || 0,
        bits: parseFloat(row.bits) || 0,
        raid: parseFloat(row.raid) || 0,
      }
      // Weight formula: presence + 10*sub + 10*resub + 5*gift + 0.01*bits + 0.1*raid
      const weight =
        signals.presence +
        10 * signals.sub +
        10 * signals.resub +
        5 * signals.gift +
        0.01 * signals.bits +
        0.1 * signals.raid
      return {
        user_hash: row.user_hash,
        weight,
        signals,
      }
    })
  }

  /**
   * Get cached L2 tree
   */
  async getCachedL2Tree(epoch: number, channel: string): Promise<{
    root: string
    levels: Buffer[][]
    participantCount: number
    builtAt: number
  } | null> {
    const result = await this.maintenancePool.query(
      `SELECT root, levels_json, participant_count, built_at
       FROM l2_tree_cache
       WHERE epoch = $1 AND channel = $2`,
      [epoch, channel]
    )
    if (result.rows.length === 0) return null

    const row = result.rows[0]
    const levelsJson = JSON.parse(row.levels_json)
    const levels = levelsJson.map((level: string[]) => level.map((hex: string) => Buffer.from(hex, 'hex')))

    return {
      root: row.root,
      levels,
      participantCount: parseInt(row.participant_count),
      builtAt: parseInt(row.built_at),
    }
  }

  /**
   * Cache L2 tree
   */
  async cacheL2Tree(epoch: number, channel: string, root: string, levels: Buffer[][], participantCount: number) {
    const levelsJson = JSON.stringify(levels.map(level => level.map(buf => buf.toString('hex'))))
    const builtAt = Math.floor(Date.now() / 1000)

    await this.maintenancePool.query(
      `INSERT INTO l2_tree_cache (epoch, channel, root, levels_json, participant_count, built_at)
       VALUES ($1, $2, $3, $4, $5, $6)
       ON CONFLICT (epoch, channel) DO UPDATE
       SET root = EXCLUDED.root,
           levels_json = EXCLUDED.levels_json,
           participant_count = EXCLUDED.participant_count,
           built_at = EXCLUDED.built_at`,
      [epoch, channel, root, levelsJson, participantCount, builtAt]
    )
  }

  /**
   * Get active channels for an epoch
   */
  async getActiveChannels(epoch: number): Promise<string[]> {
    const result = await this.maintenancePool.query(
      `SELECT DISTINCT channel FROM channel_participation WHERE epoch = $1`,
      [epoch]
    )
    return result.rows.map((r: any) => r.channel)
  }

  /**
   * Seal epoch (freeze participant snapshot)
   */
  async sealEpoch(epoch: number, channel: string, computeRoot: (users: string[]) => string, tokenGroup: string = 'OTHER', category: string = 'default') {
    await this.loadSchemaInfo()
    const client = await this.maintenancePool.connect()
    try {
      await client.query('BEGIN')

      // Check if already sealed - dynamic schema
      const sealedWhere: string[] = ['epoch = $1', 'channel = $2']
      const sealedParams: any[] = [epoch, channel]
      if (this.hasTokenGroup) {
        sealedWhere.push(`token_group = $${sealedParams.length + 1}`)
        sealedParams.push(tokenGroup)
      }
      if (this.hasCategory) {
        sealedWhere.push(`category = $${sealedParams.length + 1}`)
        sealedParams.push(category)
      }

      const existingSealed = await client.query(
        `SELECT 1 FROM sealed_epochs WHERE ${sealedWhere.join(' AND ')}`,
        sealedParams
      )
      if (existingSealed.rows.length > 0) {
        await client.query('ROLLBACK')
        return
      }

      // Get deterministic participant list - dynamic schema
      const partWhere: string[] = ['epoch = $1', 'channel = $2']
      const partParams: any[] = [epoch, channel]
      if (this.hasTokenGroup) {
        partWhere.push(`token_group = $${partParams.length + 1}`)
        partParams.push(tokenGroup)
      }
      if (this.hasCategory) {
        partWhere.push(`category = $${partParams.length + 1}`)
        partParams.push(category)
      }

      const participants = await client.query(
        `SELECT user_hash FROM channel_participation
         WHERE ${partWhere.join(' AND ')}
         ORDER BY first_seen ASC, user_hash ASC`,
        partParams
      )
      const userHashes = participants.rows.map((r: any) => r.user_hash)

      if (userHashes.length === 0) {
        await client.query('ROLLBACK')
        return
      }

      // Compute root
      const root = computeRoot(userHashes)
      const sealedAt = Math.floor(Date.now() / 1000)

      // Insert sealed epoch - dynamic schema
      const epochCols = ['epoch', 'channel', 'root', 'sealed_at']
      const epochVals: any[] = [epoch, channel, root, sealedAt]
      // NOTE: conflictCols must match the actual PK constraint (epoch, channel)
      // Do NOT add token_group/category here even if columns exist
      const conflictCols = ['epoch', 'channel']
      if (this.hasTokenGroup) {
        epochCols.push('token_group')
        epochVals.push(tokenGroup)
        // conflictCols.push('token_group') // ← REMOVED: not part of PK
      }
      if (this.hasCategory) {
        epochCols.push('category')
        epochVals.push(category)
        // conflictCols.push('category') // ← REMOVED: not part of PK
      }
      const epochPlaceholders = epochVals.map((_, i) => `$${i + 1}`).join(', ')

      await client.query(
        `INSERT INTO sealed_epochs (${epochCols.join(', ')})
         VALUES (${epochPlaceholders})
         ON CONFLICT (${conflictCols.join(', ')}) DO NOTHING`,
        epochVals
      )

      // Insert sealed participants with usernames - dynamic schema
      for (let idx = 0; idx < userHashes.length; idx++) {
        const userHash = userHashes[idx]
        const usernameRow = await client.query(
          `SELECT username FROM user_mapping WHERE user_hash = $1`,
          [userHash]
        )
        const username = usernameRow.rows[0]?.username || null

        const partCols = ['epoch', 'channel', 'idx', 'user_hash', 'username']
        const partVals: any[] = [epoch, channel, idx, userHash, username]
        if (this.hasTokenGroup) {
          partCols.push('token_group')
          partVals.push(tokenGroup)
        }
        if (this.hasCategory) {
          partCols.push('category')
          partVals.push(category)
        }
        const partPlaceholders = partVals.map((_, i) => `$${i + 1}`).join(', ')

        await client.query(
          `INSERT INTO sealed_participants (${partCols.join(', ')})
           VALUES (${partPlaceholders})
           ON CONFLICT (epoch, channel, idx) DO NOTHING`,
          partVals
        )
      }

      await client.query('COMMIT')
    } catch (err) {
      await client.query('ROLLBACK')
      throw err
    } finally {
      client.release()
    }
  }

  /**
   * Record user signals
   */
  async recordSignals(rows: SignalRow[]) {
    const client = await this.ingestPool.connect()
    try {
      await client.query('BEGIN')
      for (const row of rows) {
        await client.query(
          `INSERT INTO user_signals (epoch, channel, user_hash, signal_type, value, timestamp)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (epoch, channel, user_hash, signal_type, timestamp) DO NOTHING`,
          [row.epoch, row.channel, row.user_hash, row.signal_type, row.value, row.timestamp]
        )
      }
      await client.query('COMMIT')
    } catch (err) {
      await client.query('ROLLBACK')
      throw err
    } finally {
      client.release()
    }
  }

  /**
   * Add or update username mapping
   */
  async upsertUsernameMapping(userHash: string, username: string) {
    const firstSeen = Math.floor(Date.now() / 1000)
    await this.ingestPool.query(
      `INSERT INTO user_mapping (user_hash, username, first_seen)
       VALUES ($1, $2, $3)
       ON CONFLICT (user_hash) DO UPDATE
       SET username = EXCLUDED.username`,
      [userHash, username, firstSeen]
    )
  }

  private async ensureChannelPayoutsTable(client: PoolClient) {
    if (this.ensuredChannelPayoutsTable) return
    await client.query(`
      CREATE TABLE IF NOT EXISTS channel_payouts (
        epoch BIGINT NOT NULL,
        channel TEXT NOT NULL,
        participant_count INTEGER NOT NULL,
        total_weight DOUBLE PRECISION NOT NULL,
        viewer_amount BIGINT NOT NULL,
        streamer_amount BIGINT NOT NULL,
        viewer_ratio DOUBLE PRECISION NOT NULL,
        streamer_ratio DOUBLE PRECISION NOT NULL,
        updated_at BIGINT NOT NULL,
        PRIMARY KEY (epoch, channel)
      )
    `)
    this.ensuredChannelPayoutsTable = true
  }

  async recordChannelPayoutSnapshot(snapshot: {
    epoch: number
    channel: string
    participantCount: number
    totalWeight: number
    viewerAmount: number
    streamerAmount: number
    viewerRatio: number
    streamerRatio: number
  }): Promise<void> {
    const client = await this.maintenancePool.connect()
    try {
      await this.ensureChannelPayoutsTable(client)
      await client.query(
        `INSERT INTO channel_payouts (
           epoch,
           channel,
           participant_count,
           total_weight,
           viewer_amount,
           streamer_amount,
           viewer_ratio,
           streamer_ratio,
           updated_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (epoch, channel) DO UPDATE SET
           participant_count = EXCLUDED.participant_count,
           total_weight = EXCLUDED.total_weight,
           viewer_amount = EXCLUDED.viewer_amount,
           streamer_amount = EXCLUDED.streamer_amount,
           viewer_ratio = EXCLUDED.viewer_ratio,
           streamer_ratio = EXCLUDED.streamer_ratio,
           updated_at = EXCLUDED.updated_at`,
        [
          snapshot.epoch,
          snapshot.channel,
          snapshot.participantCount,
          snapshot.totalWeight,
          snapshot.viewerAmount,
          snapshot.streamerAmount,
          snapshot.viewerRatio,
          snapshot.streamerRatio,
          Math.floor(Date.now() / 1000)
        ]
      )
    } finally {
      client.release()
    }
  }

  /**
   * Get unpublished sealed epoch roots for publishing
   */
  async getUnpublishedRoots(currentEpoch: number, limit: number): Promise<Array<{ epoch: number; channel: string; root: string; token_group: string; category: string }>> {
    await this.loadSchemaInfo()

    // Build dynamic filters
    const where: string[] = [
      'epoch < $1',
      '(published IS NULL OR published = 0)'
    ]
    const params: any[] = [currentEpoch]

    // Focus on CLS token_group
    if (this.hasTokenGroup) {
      where.push("token_group = 'CLS'")
    }

    // Ghost-user filter: require at least one participant not in suppression_list
    // Includes token_group/category matching when columns exist.
    const existsParts: string[] = [
      'sp.epoch = sealed_epochs.epoch',
      'sp.channel = sealed_epochs.channel'
    ]
    if (this.hasTokenGroup) {
      existsParts.push("COALESCE(sp.token_group, 'MILO') = COALESCE(sealed_epochs.token_group, 'MILO')")
    }
    if (this.hasCategory) {
      existsParts.push("COALESCE(sp.category, 'default') = COALESCE(sealed_epochs.category, 'default')")
    }
    const existsClause = `EXISTS (
      SELECT 1
        FROM sealed_participants sp
        LEFT JOIN suppression_list sl ON sl.user_hash = sp.user_hash
       WHERE ${existsParts.join(' AND ')}
         AND sp.username IS NOT NULL
         AND sl.user_hash IS NULL
    )`
    where.push(existsClause)

    console.log('[DEBUG getUnpublishedRoots] hasTokenGroup:', this.hasTokenGroup, 'hasCategory:', this.hasCategory, 'whereClause:', where.join(' AND '), 'params:', params, 'limit:', limit);

    if (this.hasTokenGroup || this.hasCategory) {
      const sql = `SELECT epoch, channel, root,
                COALESCE(token_group, 'MILO') AS token_group,
                COALESCE(category, 'default') AS category
         FROM sealed_epochs
         WHERE ${where.join(' AND ')}
         ORDER BY epoch ASC
         LIMIT $${params.length + 1}`;
      console.log('[DEBUG getUnpublishedRoots] Running SQL:', sql);
      const result = await this.maintenancePool.query(sql, [...params, limit]);
      console.log('[DEBUG getUnpublishedRoots] Result count:', result.rows.length, 'First row:', result.rows[0]);
      return result.rows
    }

    const sql = `SELECT epoch, channel, root
       FROM sealed_epochs
       WHERE ${where.join(' AND ')}
       ORDER BY epoch ASC
       LIMIT $${params.length + 1}`;
    console.log('[DEBUG getUnpublishedRoots] Running SQL (no token_group):', sql);
    const result = await this.maintenancePool.query(sql, [...params, limit]);
    console.log('[DEBUG getUnpublishedRoots] Result count:', result.rows.length, 'First row:', result.rows[0]);
    return result.rows.map((row: any) => ({
      epoch: row.epoch,
      channel: row.channel,
      root: row.root,
      token_group: 'MILO',
      category: 'default',
    }))
  }

  /**
   * Mark a sealed epoch root as published
   */
  async markRootAsPublished(epoch: number, channel: string, tokenGroup = 'MILO', category = 'default'): Promise<void> {
    await this.loadSchemaInfo()

    const conditions = ['epoch = $1', 'channel = $2']
    const values: any[] = [epoch, channel]
    if (this.hasTokenGroup) {
      conditions.push('token_group = $' + (values.length + 1))
      values.push(tokenGroup)
    }
    if (this.hasCategory) {
      conditions.push('category = $' + (values.length + 1))
      values.push(category)
    }

    await this.maintenancePool.query(
      `UPDATE sealed_epochs
       SET published = 1, published_at = NOW()
       WHERE ${conditions.join(' AND ')}`,
      values
    )
  }

  /**
   * Get count of unpublished sealed epochs (for metrics)
   */
  async getBacklogCount(): Promise<number> {
    const result = await this.maintenancePool.query(
      `SELECT COUNT(*) AS c FROM sealed_epochs WHERE published IS NULL OR published = 0`
    )
    return parseInt(result.rows[0]?.c || '0')
  }

  async getBacklogCountsByGroup(): Promise<Array<{ group: string; count: number }>> {
    const result = await this.maintenancePool.query(
      `SELECT COALESCE(token_group, 'UNKNOWN') AS grp, COUNT(*) AS c
         FROM sealed_epochs
         WHERE published IS NULL OR published = 0
         GROUP BY COALESCE(token_group, 'UNKNOWN')`
    )
    return result.rows.map((row: any) => ({
      group: String(row.grp || 'UNKNOWN').toLowerCase(),
      count: parseInt(row.c || '0')
    }))
  }

  /**
   * Get last sealed epoch (for metrics)
   */
  async getLastSealedEpoch(): Promise<{ epoch: number; sealed_at: number } | null> {
    const result = await this.maintenancePool.query(
      `SELECT epoch, sealed_at FROM sealed_epochs ORDER BY epoch DESC LIMIT 1`
    )
    if (!result.rows[0]) return null
    return {
      epoch: parseInt(result.rows[0].epoch),
      sealed_at: parseInt(result.rows[0].sealed_at)
    }
  }

  /**
   * Get recent sealed epochs (for /stats)
   */
  async getRecentSealedEpochs(limit: number): Promise<number[]> {
    const result = await this.maintenancePool.query(
      `SELECT epoch FROM sealed_epochs GROUP BY epoch ORDER BY epoch DESC LIMIT $1`,
      [limit]
    )
    return result.rows.map(r => r.epoch)
  }

  /**
   * Get sealed participant counts by channel for an epoch (for /stats)
   */
  async getSealedParticipantCountsByChannel(epoch: number): Promise<Array<{ channel: string; cnt: number }>> {
    const result = await this.maintenancePool.query(
      `SELECT channel, COUNT(*) AS cnt FROM sealed_participants WHERE epoch = $1 GROUP BY channel`,
      [epoch]
    )
    return result.rows.map(r => ({ channel: r.channel, cnt: parseInt(r.cnt) }))
  }

  /**
   * Get username mapping for a user hash (for /claim-root)
   */
  async getUsernameMapping(userHash: string): Promise<string | null> {
    const result = await this.maintenancePool.query(
      `SELECT username FROM user_mapping WHERE user_hash = $1`,
      [userHash]
    )
    return result.rows[0]?.username || null
  }

  /**
   * Get distinct sealed channels for an epoch (for /claim-root category mode)
   */
  async getSealedChannels(epoch: number): Promise<string[]> {
    const result = await this.maintenancePool.query(
      `SELECT DISTINCT channel FROM sealed_participants WHERE epoch = $1`,
      [epoch]
    )
    return result.rows.map(r => r.channel)
  }

  async cleanupBefore(epochCutoff: number): Promise<void> {
    const tables = ['sealed_participants', 'user_signals', 'channel_participation', 'sealed_epochs']
    const client = await this.maintenancePool.connect()
    try {
      await client.query('BEGIN')
      for (const t of tables) {
        await client.query(`DELETE FROM ${t} WHERE epoch < $1`, [epochCutoff])
      }
      await client.query('COMMIT')
    } catch (err) {
      await client.query('ROLLBACK')
      throw err
    } finally {
      client.release()
    }
  }

  /**
   * Wallet binding: attach a claimer wallet to a user_hash for ring claims.
   * - Upserts (user_hash, wallet) with latest timestamps; verified flag optional
   */
  async bindWallet(params: { userId?: string; username?: string; wallet: string; verified?: boolean; source?: string }): Promise<void> {
    const user_hash = canonicalUserHash({ userId: params.userId, user: params.username })
    const username = params.username || null
    const wallet = params.wallet
    const verified = params.verified === true
    const source = params.source || null
    const ts = Math.floor(Date.now() / 1000)
    await this.maintenancePool.query(
      `INSERT INTO user_wallet_bindings (user_hash, username, wallet, verified, source, created_at, updated_at)
       VALUES ($1, $2, $3, $4, $5, $6, $6)
       ON CONFLICT (user_hash, wallet) DO UPDATE SET
         username = COALESCE(EXCLUDED.username, user_wallet_bindings.username),
         verified = user_wallet_bindings.verified OR EXCLUDED.verified,
         source = COALESCE(EXCLUDED.source, user_wallet_bindings.source),
         updated_at = EXCLUDED.updated_at`,
      [user_hash, username, wallet, verified, source, ts]
    )
  }

  /** Preferred wallet for a user_hash: verified first, else most-recent */
  async getWalletForUserHash(user_hash: string): Promise<string | null> {
    const q = await this.maintenancePool.query(
      `SELECT wallet FROM user_wallet_bindings
       WHERE user_hash = $1
       ORDER BY verified DESC, updated_at DESC
       LIMIT 1`,
      [user_hash]
    )
    return q.rows[0]?.wallet || null
  }

  /** Lookup user_hash by wallet (latest binding) */
  async getUserHashForWallet(wallet: string): Promise<string | null> {
    const q = await this.maintenancePool.query(
      `SELECT user_hash FROM user_wallet_bindings
       WHERE wallet = $1
       ORDER BY verified DESC, updated_at DESC
       LIMIT 1`,
      [wallet]
    )
    return q.rows[0]?.user_hash || null
  }

  /**
   * Check if an epoch overlaps a live window for a channel.
   */
  async hasLiveOverlap(epoch: number, channel: string, epochSeconds: number = Number(process.env.EPOCH_SECONDS || 3600)): Promise<boolean> {
    const start = epoch;
    const end = epoch + epochSeconds;
    const result = await this.maintenancePool.query(
      `SELECT 1
         FROM live_windows
        WHERE lower(channel) = lower($1)
          AND COALESCE(end_ts, $3) > $2
          AND start_ts < $3
        LIMIT 1`,
      [channel, start, end]
    );
    return result.rows.length > 0;
  }

  /**
   * Close connection pool
   */
  async close() {
    await this.ingestPool.end()
    await this.maintenancePool.end()
  }

  /**
   * Check if a user is suppressed (opted out)
   */
  async isSuppressed(userHash: string): Promise<boolean> {
    const result = await this.maintenancePool.query(
      `SELECT 1 FROM suppression_list WHERE user_hash = $1 LIMIT 1`,
      [userHash]
    )
    return result.rows.length > 0
  }

  /**
   * Add user to suppression list (opt-out)
   */
  async addSuppression(userHash: string, username: string, reason?: string, ipHash?: string): Promise<void> {
    const now = Math.floor(Date.now() / 1000)
    const client = await this.maintenancePool.connect()
    try {
      await client.query('BEGIN')

      // Add to suppression list
      await client.query(
        `INSERT INTO suppression_list (user_hash, username, requested_at, reason, ip_hash)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_hash) DO UPDATE SET
           requested_at = EXCLUDED.requested_at,
           reason = EXCLUDED.reason`,
        [userHash, username, now, reason || null, ipHash || null]
      )

      // Log the action
      await client.query(
        `INSERT INTO suppression_log (user_hash, username, action, requested_at, ip_hash)
         VALUES ($1, $2, 'opted_out', $3, $4)`,
        [userHash, username, now, ipHash || null]
      )

      await client.query('COMMIT')
    } catch (err) {
      await client.query('ROLLBACK')
      throw err
    } finally {
      client.release()
    }
  }

  /**
   * Get suppression status for a username
   */
  async getSuppressionStatus(username: string): Promise<{ suppressed: boolean; requested_at?: number } | null> {
    const userHash = hashUser(username)
    const result = await this.maintenancePool.query(
      `SELECT requested_at FROM suppression_list WHERE user_hash = $1`,
      [userHash]
    )
    if (result.rows.length === 0) {
      return { suppressed: false }
    }
    return {
      suppressed: true,
      requested_at: result.rows[0].requested_at
    }
  }
}
