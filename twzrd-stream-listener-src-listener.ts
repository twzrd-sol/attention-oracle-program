import {
  Connection,
  PublicKey,
  Commitment,
  TransactionResponse,
  PartiallyDecodedInstruction,
  ParsedInstruction,
} from '@solana/web3.js';
import { Queue } from 'bullmq';
import { Logger } from 'pino';
import { appendFile } from 'fs/promises';
import { resolve } from 'path';

// ============================================================================
// Types
// ============================================================================

export interface StreamListenerConfig {
  connection: Connection;
  programId: PublicKey;
  logger: Logger;
  queue: Queue;
  logDir: string;
  commitment?: Commitment;
}

export interface StreamEvent {
  timestamp: string;
  slot: number;
  signature: string;
  blockTime?: number;
  instruction: {
    program: string;
    programId: string;
    action: string;
    data: Record<string, unknown>;
  };
  accounts?: string[];
  meta?: {
    fee: number;
    preTokenBalances: unknown[];
    postTokenBalances: unknown[];
  };
}

// ============================================================================
// Stream Listener Class
// ============================================================================

export class StreamListener {
  private config: StreamListenerConfig;
  private subscriptionId: number | null = null;
  private ndjsonLogPath: string;
  private isRunning = false;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectDelay = 3000; // ms

  constructor(config: StreamListenerConfig) {
    this.config = config;
    this.ndjsonLogPath = resolve(config.logDir, 'stream-events.ndjson');
  }

  /**
   * Start listening to program events
   */
  async start(): Promise<void> {
    if (this.isRunning) {
      this.config.logger.warn('Listener already running');
      return;
    }

    this.isRunning = true;

    try {
      // Subscribe to logs for this program
      this.subscriptionId = this.config.connection.onLogs(
        this.config.programId,
        (logs, context) => {
          this.handleLogs(logs, context).catch((err) => {
            this.config.logger.error(
              { err, logs, context },
              'Error handling logs'
            );
          });
        },
        this.config.commitment || 'confirmed'
      );

      this.config.logger.info(
        { subscriptionId: this.subscriptionId },
        'Subscription created'
      );

      // Also monitor via block subscription for full context
      this.setupBlockMonitor();
    } catch (err) {
      this.config.logger.error({ err }, 'Failed to start listener');
      this.isRunning = false;
      throw err;
    }
  }

  /**
   * Stop listening
   */
  async stop(): Promise<void> {
    if (!this.isRunning) {
      return;
    }

    this.isRunning = false;

    if (this.subscriptionId !== null) {
      try {
        await this.config.connection.removeOnLogsListener(this.subscriptionId);
        this.config.logger.info('Subscription removed');
      } catch (err) {
        this.config.logger.warn({ err }, 'Error removing subscription');
      }
    }
  }

  /**
   * Handle log notifications
   */
  private async handleLogs(
    logs: string[],
    context: { slot: number; signature?: string }
  ): Promise<void> {
    // Parse logs to find relevant events
    for (const log of logs) {
      if (log.includes('Program log:')) {
        // Extract data from program logs if available
        const message = log.replace('Program log: ', '');
        // Could parse structured logs here (JSON-encoded events)
      }
    }

    // Fetch full transaction for complete context
    if (context.signature) {
      await this.fetchAndProcessTransaction(context.signature, context.slot);
    }
  }

  /**
   * Setup block monitor for event detection
   */
  private setupBlockMonitor(): void {
    // Optional: Monitor blocks to detect program-related transactions
    // This provides another layer of event detection
    this.config.connection.onSlotChange((slotInfo) => {
      this.config.logger.debug({ slotInfo }, 'Slot changed');
    });
  }

  /**
   * Fetch transaction and process events
   */
  private async fetchAndProcessTransaction(
    signature: string,
    slot: number
  ): Promise<void> {
    try {
      // Fetch with commitment
      const tx = await this.config.connection.getTransaction(signature, {
        commitment: this.config.commitment || 'confirmed',
      });

      if (!tx) {
        this.config.logger.debug({ signature }, 'Transaction not found');
        return;
      }

      // Extract events from transaction
      const events = this.extractEvents(tx, signature, slot);

      for (const event of events) {
        // Queue for processing
        await this.config.queue.add('stream:event', event, {
          jobId: `${event.signature}-${event.instruction.action}`,
        });

        // Log to NDJSON
        await this.logEvent(event);

        this.config.logger.debug(
          {
            signature: event.signature,
            action: event.instruction.action,
            slot: event.slot,
          },
          'Event queued'
        );
      }
    } catch (err) {
      this.config.logger.error(
        { err, signature },
        'Error fetching transaction'
      );
      // Don't throw — continue processing other transactions
    }
  }

  /**
   * Extract events from transaction
   */
  private extractEvents(
    tx: TransactionResponse,
    signature: string,
    slot: number
  ): StreamEvent[] {
    const events: StreamEvent[] = [];

    if (!tx.transaction.message.instructions) {
      return events;
    }

    for (const instruction of tx.transaction.message.instructions) {
      // Check if instruction is for our program
      if (!this.isOurProgram(instruction, tx)) {
        continue;
      }

      const event: StreamEvent = {
        timestamp: new Date().toISOString(),
        slot,
        signature,
        blockTime: tx.blockTime,
        instruction: {
          program: 'Attention Oracle',
          programId: this.config.programId.toBase58(),
          action: this.extractAction(instruction),
          data: this.extractData(instruction),
        },
        accounts: this.extractAccounts(instruction, tx),
        meta: {
          fee: tx.transaction.message.header ? 0 : 0, // Could parse fee
          preTokenBalances: tx.meta?.preTokenBalances || [],
          postTokenBalances: tx.meta?.postTokenBalances || [],
        },
      };

      events.push(event);
    }

    return events;
  }

  /**
   * Check if instruction is for our program
   */
  private isOurProgram(
    instruction: PartiallyDecodedInstruction | ParsedInstruction,
    tx: TransactionResponse
  ): boolean {
    const programIdIndex = instruction.programIdIndex;
    const keys = tx.transaction.message.accountKeys;
    if (programIdIndex >= keys.length) {
      return false;
    }

    const programId = keys[programIdIndex];
    return programId.equals(this.config.programId);
  }

  /**
   * Extract action name from instruction
   */
  private extractAction(
    instruction: PartiallyDecodedInstruction | ParsedInstruction
  ): string {
    if ('parsed' in instruction && instruction.parsed) {
      return instruction.parsed.type || 'unknown';
    }

    if ('data' in instruction) {
      // Decode first byte to determine action
      const data = instruction.data as string | Buffer;
      const buffer = typeof data === 'string' ? Buffer.from(data, 'base64') : data;
      const discriminator = buffer.slice(0, 8).toString('hex');

      // Map discriminators to action names (would need SDK constants)
      const actionMap: Record<string, string> = {
        // Add discriminator → action mappings here
        // e.g., '6a6e5b7c3a2d1e4f' → 'finalize_epoch'
      };

      return actionMap[discriminator] || `unknown_${discriminator}`;
    }

    return 'unknown';
  }

  /**
   * Extract instruction data
   */
  private extractData(
    instruction: PartiallyDecodedInstruction | ParsedInstruction
  ): Record<string, unknown> {
    if ('parsed' in instruction && instruction.parsed) {
      return instruction.parsed.info || {};
    }

    // For non-parsed instructions, would need SDK to decode
    return {
      rawData:
        'data' in instruction
          ? (instruction.data as string | Buffer)
          : undefined,
    };
  }

  /**
   * Extract account keys
   */
  private extractAccounts(
    instruction: PartiallyDecodedInstruction | ParsedInstruction,
    tx: TransactionResponse
  ): string[] {
    const accounts: string[] = [];

    if ('accounts' in instruction && instruction.accounts) {
      for (const account of instruction.accounts) {
        accounts.push(account.toBase58());
      }
    }

    return accounts;
  }

  /**
   * Log event to NDJSON file
   */
  private async logEvent(event: StreamEvent): Promise<void> {
    try {
      const line = JSON.stringify(event) + '\n';
      await appendFile(this.ndjsonLogPath, line);
    } catch (err) {
      this.config.logger.error({ err }, 'Failed to write event log');
    }
  }

  /**
   * Get current status
   */
  getStatus() {
    return {
      isRunning: this.isRunning,
      subscriptionId: this.subscriptionId,
      reconnectAttempts: this.reconnectAttempts,
    };
  }
}
