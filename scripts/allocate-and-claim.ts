#!/usr/bin/env tsx
/**
 * allocate-and-claim.ts
 *
 * For a given wallet and epoch, this script:
 *  1. Reads allocation data from the L2 database (allocations table)
 *     - index  (leaf index in Merkle tree)
 *     - amount (token amount as BIGINT)
 *     - id     (leaf identifier string)
 *     - proof_json (JSON array of hex-encoded proof nodes)
 *  2. Calls the gateway `/api/claim-cls` endpoint with these parameters
 *  3. Receives an unsigned transaction, signs it with the wallet keypair
 *  4. Submits the transaction to Solana
 *  5. Marks the claim as confirmed in `cls_claims`
 *
 * Environment:
 *   DATABASE_URL   – Postgres connection URL (must contain `allocations` & `cls_claims`)
 *   SOLANA_RPC     – Solana RPC URL (default: mainnet-beta)
 *   GATEWAY_URL    – Gateway base URL (default: http://localhost:5000)
 *   KEYPAIR_PATH   – Path to signer keypair (default: ~/.config/solana/id.json)
 *
 * Usage (single wallet):
 *   npx tsx scripts/allocate-and-claim.ts --wallet <WALLET> --epochs 424243,424244
 *
 * Usage (CSV batch mode):
 *   npx tsx scripts/allocate-and-claim.ts --csv claims.csv
 *
 *   claims.csv columns:
 *     wallet,epochs,keypair_path
 *   Example:
 *     wallet,epochs,keypair_path
 *     DV8...,424243,424244,/home/twzrd/.config/solana/cls-claim-0001.json
 *
 * Notes:
 *   - The wallet public key MUST match the loaded keypair.
 *   - The `allocations` table is expected to have:
 *       epoch_id  INTEGER
 *       wallet    TEXT
 *       index     INTEGER
 *       amount    BIGINT
 *       id        TEXT
 *       proof_json JSON / TEXT (JSON array of hex strings, optionally 0x-prefixed)
 */

import { Connection, Keypair, PublicKey, Transaction } from '@solana/web3.js';
import { Pool } from 'pg';
import fs from 'fs';

type AllocationRow = {
  index: number;
  amount: string | number;
  id: string | null;
  proof_json: any;
};

type ClaimJob = {
  epochId: number;
  wallet: string;
};

type CsvRow = {
  wallet: string;
  epochs: number[];
  keypairPath: string;
};

function parseArgs(argv: string[]): { wallet?: string; epochs?: number[]; csvPath?: string } {
  const args = argv.slice(2);

  const getFlag = (name: string): string | undefined => {
    const idx = args.findIndex((a) => a === name);
    if (idx === -1 || idx + 1 >= args.length) return undefined;
    return args[idx + 1];
  };

  const wallet = getFlag('--wallet') || getFlag('-w');
  const epochsArg = getFlag('--epochs') || getFlag('--epoch');
  const csvPath = getFlag('--csv');

  if (csvPath) {
    return { csvPath };
  }

  if (!wallet) {
    throw new Error('Missing --wallet <WALLET> (or use --csv <file>)');
  }

  if (!epochsArg) {
    throw new Error('Missing --epochs <epoch1,epoch2,...> or --epoch <epoch>');
  }

  const epochs = epochsArg
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => {
      const n = Number(s);
      if (!Number.isInteger(n) || n < 0) {
        throw new Error(`Invalid epoch value: "${s}" (must be non-negative integer)`);
      }
      return n;
    });

  if (epochs.length === 0) {
    throw new Error('No valid epoch values provided');
  }

  return { wallet, epochs };
}

async function fetchAllocation(
  pool: Pool,
  epochId: number,
  wallet: string,
): Promise<AllocationRow | null> {
  const result = await pool.query<AllocationRow>(
    `
      SELECT index, amount, id, proof_json
      FROM allocations
      WHERE epoch_id = $1 AND wallet = $2
      LIMIT 1
    `,
    [epochId, wallet],
  );

  if (!result.rows || result.rows.length === 0) return null;
  return result.rows[0];
}

function decodeProof(proofJson: any): string[] {
  if (Array.isArray(proofJson)) {
    return proofJson.map((p) => String(p));
  }

  if (typeof proofJson === 'string') {
    try {
      const parsed = JSON.parse(proofJson);
      if (Array.isArray(parsed)) {
        return parsed.map((p: any) => String(p));
      }
    } catch {
      // fall through
    }
  }

  // Last resort: attempt to JSON.parse the serialised value
  try {
    const parsed = JSON.parse(JSON.stringify(proofJson));
    if (Array.isArray(parsed)) {
      return parsed.map((p: any) => String(p));
    }
  } catch {
    // ignore
  }

  throw new Error('Unable to decode proof_json into string[]');
}

async function processWalletEpochs(
  pool: Pool,
  connection: Connection,
  gatewayUrl: string,
  wallet: string,
  epochs: number[],
  keypairPath: string,
) {
  console.log('🔐 Loading keypair from:', keypairPath);
  const keypairData = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
  const signer = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const signerPubkey = signer.publicKey.toBase58();
  if (signerPubkey !== wallet) {
    throw new Error(
      `Wallet mismatch: wallet=${wallet} but keypair pubkey=${signerPubkey}`,
    );
  }

  console.log('🔐 Wallet:', signerPubkey);
  console.log('📆 Epochs:', epochs.join(', '));
  console.log('');

  const jobs: ClaimJob[] = epochs.map((epochId) => ({ epochId, wallet }));

  for (const job of jobs) {
    const { epochId } = job;
    console.log(`===== Epoch ${epochId} / Wallet ${wallet} =====`);

    try {
      // 1) Fetch allocation row
      const allocation = await fetchAllocation(pool, epochId, wallet);
      if (!allocation) {
        console.log('  ⚠️  No allocation row found in allocations table, skipping.');
        continue;
      }

      const idx = Number(allocation.index);
      const amountStr =
        typeof allocation.amount === 'number'
          ? allocation.amount.toString()
          : String(allocation.amount);
      const id = allocation.id || `${process.env.CLS_CLAIM_ID_PREFIX || 'cls-epoch'}-${epochId}`;
      const proof = decodeProof(allocation.proof_json);

      console.log('  Allocation:');
      console.log('   • index  =', idx);
      console.log('   • amount =', amountStr);
      console.log('   • id     =', id);
      console.log('   • proof  =', proof.length, 'nodes');

      // 2) Request unsigned transaction from gateway
      console.log('  ➜ POST /api/claim-cls');

      const body = {
        wallet,
        epochId,
        index: idx,
        amount: amountStr,
        id,
        proof,
      };

      const resp = await fetch(gatewayUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });

      if (!resp.ok) {
        const text = await resp.text();
        console.error(
          `  ❌ Gateway responded with ${resp.status}: ${text}`,
        );
        continue;
      }

      const json: any = await resp.json();
      if (!json.transaction) {
        console.error('  ❌ No transaction field in gateway response:', json);
        continue;
      }

      console.log('  ✅ Received unsigned transaction');

      // 3) Decode and sign transaction
      const txBuffer = Buffer.from(json.transaction, 'base64');
      const tx = Transaction.from(txBuffer);
      tx.sign(signer);

      // 4) Submit to Solana
      console.log('  ➜ Submitting to Solana...');
      const sig = await connection.sendRawTransaction(tx.serialize());
      console.log('  ✅ Submitted. Signature:', sig);
      console.log(
        `     Explorer: https://explorer.solana.com/tx/${sig}`,
      );

      const confirmation = await connection.confirmTransaction(
        sig,
        'confirmed',
      );
      if (confirmation.value.err) {
        console.error('  ❌ Transaction failed:', confirmation.value.err);
        continue;
      }

      console.log('  ✅ Transaction confirmed on-chain');

      // 5) Mark claim as confirmed in cls_claims
      try {
        await pool.query(
          `
            UPDATE cls_claims
            SET amount = $3,
                tx_status = 'confirmed',
                tx_signature = $4,
                confirmed_at = NOW()
            WHERE wallet = $1 AND epoch_id = $2
          `,
          [wallet, epochId, amountStr, sig],
        );
        console.log('  📝 cls_claims updated to confirmed');
      } catch (dbErr: any) {
        console.warn(
          '  ⚠️  Failed to update cls_claims status:',
          dbErr?.message || dbErr,
        );
      }

      console.log('');
    } catch (err: any) {
      console.error(
        `  ❌ Error while processing epoch ${epochId}:`,
        err?.message || err,
      );
      console.log('');
    }
  }
}

function parseCsvClaims(csvPath: string): CsvRow[] {
  const raw = fs.readFileSync(csvPath, 'utf-8');
  const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0);
  if (lines.length === 0) {
    throw new Error(`CSV file ${csvPath} is empty`);
  }

  const header = lines[0].split(',').map((h) => h.trim().toLowerCase());
  const walletIdx = header.indexOf('wallet');
  const epochsIdx = header.indexOf('epochs');
  const keypairIdx = header.indexOf('keypair_path');

  if (walletIdx === -1 || epochsIdx === -1 || keypairIdx === -1) {
    throw new Error(
      `CSV header must contain wallet,epochs,keypair_path (found: ${header.join(',')})`,
    );
  }

  const rows: CsvRow[] = [];
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line || line.startsWith('#')) continue;
    const parts = line.split(',').map((p) => p.trim());
    if (parts.length < header.length) continue;

    const wallet = parts[walletIdx];
    const epochsRaw = parts[epochsIdx];
    const keypairPath = parts[keypairIdx];

    if (!wallet || !epochsRaw || !keypairPath) continue;

    const epochs = epochsRaw
      .split(/[;|,]/)
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) => {
        const n = Number(s);
        if (!Number.isInteger(n) || n < 0) {
          throw new Error(
            `Invalid epoch value in CSV row ${i + 1}: "${s}" (must be non-negative integer)`,
          );
        }
        return n;
      });

    if (epochs.length === 0) continue;

    rows.push({ wallet, epochs, keypairPath });
  }

  return rows;
}

async function main() {
  const { wallet, epochs, csvPath } = parseArgs(process.argv);

  const DATABASE_URL =
    process.env.DATABASE_URL || process.env.L2_DATABASE_URL;
  if (!DATABASE_URL) {
    throw new Error('DATABASE_URL (or L2_DATABASE_URL) must be set');
  }

  const SOLANA_RPC =
    process.env.SOLANA_RPC || 'https://api.mainnet-beta.solana.com';
  const GATEWAY_BASE =
    process.env.GATEWAY_URL || 'http://localhost:5000';
  const DEFAULT_KEYPAIR_PATH =
    process.env.KEYPAIR_PATH || `${process.env.HOME}/.config/solana/id.json`;

  const gatewayUrl = GATEWAY_BASE.endsWith('/api/claim-cls')
    ? GATEWAY_BASE
    : `${GATEWAY_BASE.replace(/\/$/, '')}/api/claim-cls`;

  console.log('🌐 RPC:', SOLANA_RPC);
  console.log('🌐 Gateway:', gatewayUrl);
  console.log('🗃  Database:', DATABASE_URL);

  const pool = new Pool({
    connectionString: DATABASE_URL,
    ssl: { rejectUnauthorized: false } as any,
  });
  const connection = new Connection(SOLANA_RPC, 'confirmed');

  if (csvPath) {
    console.log('📄 CSV mode:', csvPath);
    const rows = parseCsvClaims(csvPath);
    console.log(`Found ${rows.length} claim rows in CSV\n`);

    for (const row of rows) {
      try {
        console.log(
          `>>> Processing wallet ${row.wallet} (epochs: ${row.epochs.join(
            ',',
          )})`,
        );
        await processWalletEpochs(
          pool,
          connection,
          gatewayUrl,
          row.wallet,
          row.epochs,
          row.keypairPath || DEFAULT_KEYPAIR_PATH,
        );
      } catch (err: any) {
        console.error(
          `❌ Error processing wallet ${row.wallet}:`,
          err?.message || err,
        );
      }
      console.log('');
    }
  } else {
    if (!wallet || !epochs) {
      throw new Error('Single-wallet mode requires --wallet and --epochs');
    }

    await processWalletEpochs(
      pool,
      connection,
      gatewayUrl,
      wallet,
      epochs,
      DEFAULT_KEYPAIR_PATH,
    );
  }

  await pool.end();
}

main().catch((err) => {
  console.error('Fatal error in allocate-and-claim.ts:', err);
  process.exit(1);
});
