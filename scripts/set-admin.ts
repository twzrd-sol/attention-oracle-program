/**
 * Safely rotate the wzrd-rails Config admin via set_admin.
 *
 * Modeled on scripts/set-reward-rate.ts: same env contract, same dry-run
 * default, same confirmation gate. Default mode derives accounts, verifies the
 * signer really is the current on-chain admin, signs, and simulates.
 *
 * set_admin is SINGLE STEP. There is no pending_admin on Config and no accept
 * leg to catch a mistake. Do not expect the repo's two-step set_payout_admin /
 * accept_payout_admin pair to save you: that pair is H-01 SOURCE that is not
 * deployed. The DEPLOYED binary has no accept_payout_admin at all, and its
 * set_payout_admin is itself single step. Once this lands, only NEW_ADMIN can
 * administer the config -- in the deployed binary that is nine admin-gated
 * instructions, including the init_payout_* bootstrap that has never been run
 * on mainnet -- and there is no recovery short of a program upgrade. Confirm
 * you control NEW_ADMIN before broadcasting.
 * See docs/playbooks/wzrd-rails-config-admin-rotation.md.
 *
 * NEW_ADMIN typo guard. Pubkeys carry no checksum, and every confirmation this
 * script prints is derived from the same parsed NEW_ADMIN, so a mistyped key is
 * only ever confirmed against itself. Preferred path: set NEW_ADMIN_KEYPAIR to
 * NEW_ADMIN's own keypair. It then pays the fee, so the promoting transaction
 * carries NEW_ADMIN's signature -- proof the key exists and is controlled. If
 * that keypair is not on this box, or NEW_ADMIN is an off-curve Squads V4 vault
 * PDA (the production shape per state.rs) that cannot sign at all, set
 * I_ACCEPT_UNPROVEN_NEW_ADMIN=1 to proceed unproven; the script then only warns,
 * reporting whether NEW_ADMIN is on curve and whether it has ever been seen on
 * this cluster. Dry runs always print those diagnostics; only broadcast is
 * refused without proof or the override.
 *
 * Env:
 *   CLUSTER, RPC_URL (or SOLANA_RPC_URL), KEYPAIR, NEW_ADMIN  required
 *   NEW_ADMIN_KEYPAIR  NEW_ADMIN's keypair; co-signs as fee payer (preferred)
 *   I_ACCEPT_UNPROVEN_NEW_ADMIN=1  proceed without that signature (loud)
 *   FEE_PAYER  funded payer when the admin key is drained; mutually exclusive
 *              with NEW_ADMIN_KEYPAIR, which is already the fee payer
 *   BROADCAST=1 or --send, CONFIRM_BROADCAST, I_UNDERSTAND_MAINNET
 *
 * Broadcast requires BROADCAST=1 or --send, plus ONE of:
 *   - CONFIRM_BROADCAST=<cluster>:<NEW_ADMIN>, which is sufficient on its own.
 *     It binds both the cluster and the exact target key, so it is strictly
 *     stronger than the content-free I_UNDERSTAND_MAINNET flag and deliberately
 *     short-circuits that check (same behavior as set-reward-rate.ts); or
 *   - the interactive typed phrase, which needs a TTY and, on mainnet-beta,
 *     additionally I_UNDERSTAND_MAINNET=1.
 *
 * After a confirmed send the Config is re-read at "confirmed" -- the same
 * commitment the send was confirmed at -- with a bounded poll. It must NOT be
 * read at "finalized": that lags confirmation by roughly 32 slots and would
 * report MISMATCH on a rotation that actually succeeded.
 *
 * Usage:
 *   CLUSTER=mainnet-beta RPC_URL="https://..." KEYPAIR=~/.config/solana/id.json \
 *     NEW_ADMIN=<pubkey> NEW_ADMIN_KEYPAIR=<path to NEW_ADMIN keypair> \
 *     npx tsx scripts/set-admin.ts
 *
 *   ... BROADCAST=1 I_UNDERSTAND_MAINNET=1 npx tsx scripts/set-admin.ts
 */

import {
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { createHash } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import * as readline from "node:readline/promises";

const RAILS_PROGRAM = new PublicKey("BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9");
const CONFIG_SEED = Buffer.from("config");
const ALLOWED_CLUSTERS = new Set(["localnet", "devnet", "testnet", "mainnet-beta"]);
const UNPROVEN_OVERRIDE = "I_ACCEPT_UNPROVEN_NEW_ADMIN";
const READBACK_ATTEMPTS = 10;
const READBACK_DELAY_MS = 1500;

function fail(message: string): never {
  console.error(`ERROR: ${message}`);
  process.exit(2);
}

function env(name: string): string {
  return process.env[name]?.trim() ?? "";
}

function requireEnv(name: string): string {
  const value = env(name);
  if (!value) fail(`Missing ${name}`);
  return value;
}

function normalizeCluster(raw: string): string {
  const cluster = raw === "mainnet" ? "mainnet-beta" : raw;
  if (!ALLOWED_CLUSTERS.has(cluster)) {
    fail(`Invalid CLUSTER=${raw}. Use localnet, devnet, testnet, or mainnet-beta`);
  }
  return cluster;
}

function expandHome(inputPath: string): string {
  if (inputPath === "~") return process.env.HOME ?? "";
  if (inputPath.startsWith("~/")) return path.join(process.env.HOME ?? "", inputPath.slice(2));
  return inputPath;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function loadKeypair(keypairPath: string, envName = "KEYPAIR"): Keypair {
  const expanded = expandHome(keypairPath);
  if (!expanded || !fs.existsSync(expanded)) fail(`${envName} not found: ${expanded}`);

  const raw = JSON.parse(fs.readFileSync(expanded, "utf8"));
  if (!Array.isArray(raw)) fail(`${envName} must be a Solana secret-key JSON array: ${expanded}`);
  return Keypair.fromSecretKey(new Uint8Array(raw));
}

function parseNewAdmin(): PublicKey {
  const raw = requireEnv("NEW_ADMIN");
  let key: PublicKey;
  try {
    key = new PublicKey(raw);
  } catch {
    return fail(`NEW_ADMIN is not a valid pubkey: ${raw}`);
  }
  // Mirrors the on-chain guard (audit M-3 / EZ-7): all-zeros permanently
  // retires the admin role and is recoverable only via program upgrade.
  if (key.equals(PublicKey.default)) fail("NEW_ADMIN is the all-zeros pubkey; refusing");
  return key;
}

/**
 * Proof-of-custody / typo guard for NEW_ADMIN.
 *
 * Base58 pubkeys carry no checksum, so one mistyped character is a different,
 * valid, un-signable key. Every other confirmation here -- the printed line,
 * the CONFIRM_BROADCAST token, the typed phrase -- is derived from the same
 * parsed NEW_ADMIN, so those only ever confirm a typo against itself. set_admin
 * has no accept leg, so a wrong NEW_ADMIN is unrecoverable short of a program
 * upgrade.
 *
 * Strong proof: NEW_ADMIN signs the very transaction that promotes it, which it
 * does by paying the fee (see the fee-payer resolution in main). Weak signal:
 * NEW_ADMIN already exists on this cluster. Neither is available for an
 * off-curve target such as a Squads V4 vault PDA -- state.rs names that the
 * production shape, and a PDA has no keypair to sign with -- so off curve is a
 * warning only, and the unproven path stays open behind an explicit override
 * rather than being rejected outright.
 */
async function guardNewAdmin(
  connection: Connection,
  cluster: string,
  newAdmin: PublicKey,
  signedByNewAdmin: boolean,
  broadcast: boolean,
): Promise<void> {
  if (signedByNewAdmin) {
    console.log("  NEW_ADMIN custody: PROVEN (NEW_ADMIN co-signs this transaction as fee payer)");
    return;
  }

  const onCurve = PublicKey.isOnCurve(newAdmin.toBytes());
  const acct = await connection.getAccountInfo(newAdmin, "confirmed");

  console.log("  NEW_ADMIN custody: UNPROVEN (no NEW_ADMIN_KEYPAIR, so NEW_ADMIN does not sign)");
  console.log(
    `    On curve: ${onCurve ? "yes" : "no -- PDA-shaped; a Squads V4 vault looks like this and cannot ever sign"}`,
  );
  // A closed account reads exactly like one that never existed, so this is a
  // typo signal in one direction only. Presence is reassuring; absence is not
  // proof of a typo, and neither is proof of custody.
  console.log(
    acct
      ? `    On chain: yes -- ${acct.lamports} lamports, owner ${acct.owner.toBase58()}`
      : `    On chain: NO ACCOUNT on ${cluster} -- strong typo signal`,
  );

  if (env(UNPROVEN_OVERRIDE) !== "1") {
    if (broadcast) {
      fail(
        "NEW_ADMIN custody is unproven, and set_admin is irreversible.\n" +
          "  Preferred: NEW_ADMIN_KEYPAIR=<path to NEW_ADMIN's keypair>, so NEW_ADMIN pays the\n" +
          "  fee and signs this transaction.\n" +
          "  If that keypair is not on this box, or NEW_ADMIN is a vault PDA that cannot sign:\n" +
          "  re-check the base58 character by character against an independent source, then set\n" +
          `  ${UNPROVEN_OVERRIDE}=1 to broadcast without proof.`,
      );
    }
    console.log(
      `    Broadcast will be REFUSED until NEW_ADMIN_KEYPAIR is supplied or ${UNPROVEN_OVERRIDE}=1 is set.`,
    );
    return;
  }

  console.log("");
  console.log(`  !! ${UNPROVEN_OVERRIDE}=1 -- proceeding WITHOUT proof that NEW_ADMIN is signable !!`);
  console.log("  If NEW_ADMIN is wrong by one character, the deployed binary's nine admin-gated");
  console.log("  instructions -- set_admin, set_reward_rate, initialize_pool,");
  console.log("  compensate_external_stakers, realloc_stake_pool, register_verified_moment,");
  console.log("  init_payout_authority_config, init_payout_cap_config, init_payout_vault_config");
  console.log("  -- are bricked permanently, with no accept leg and no way back.");
  if (!acct) {
    console.log(`  NEW_ADMIN has never been seen on ${cluster}. Re-verify it before continuing.`);
  }
}

function anchorDiscriminator(ixName: string): Buffer {
  return createHash("sha256").update(`global:${ixName}`).digest().subarray(0, 8);
}

function readConfigAdmin(data: Buffer): PublicKey {
  const adminOffset = 8;
  if (data.length < adminOffset + 32) fail(`Config account is too short: ${data.length} bytes`);
  return new PublicKey(data.subarray(adminOffset, adminOffset + 32));
}

/**
 * Re-read Config.admin at "confirmed" -- the same commitment sendTransaction was
 * confirmed at -- retrying briefly until it matches.
 *
 * Reading at "finalized" here would be a deterministic false alarm: finalization
 * lags confirmation by roughly 32 slots, so the finalized view is still the
 * pre-transaction state and a rotation that actually landed would be reported as
 * MISMATCH on an operation that cannot be retried. The retry loop only absorbs
 * the small lag before an RPC node's account view catches up to the slot it
 * already confirmed.
 */
async function pollConfigAdmin(
  connection: Connection,
  configPda: PublicKey,
  expected: PublicKey,
): Promise<PublicKey> {
  let observed: PublicKey | null = null;
  for (let attempt = 1; attempt <= READBACK_ATTEMPTS; attempt++) {
    const acct = await connection.getAccountInfo(configPda, "confirmed");
    if (!acct) fail("Config account missing after confirmed transaction");
    observed = readConfigAdmin(acct.data);
    if (observed.equals(expected)) return observed;
    if (attempt < READBACK_ATTEMPTS) {
      console.log(`  Readback ${attempt}/${READBACK_ATTEMPTS}: still ${observed.toBase58()}, retrying...`);
      await sleep(READBACK_DELAY_MS);
    }
  }
  if (!observed) fail("Config readback produced no result");
  return observed;
}

function shouldBroadcast(): boolean {
  return env("BROADCAST") === "1" || process.argv.includes("--send");
}

async function confirmBroadcast(cluster: string, newAdmin: PublicKey): Promise<void> {
  const confirmationToken = `${cluster}:${newAdmin.toBase58()}`;
  if (env("CONFIRM_BROADCAST") === confirmationToken) return;

  if (cluster === "mainnet-beta" && env("I_UNDERSTAND_MAINNET") !== "1") {
    fail("Refusing mainnet broadcast without I_UNDERSTAND_MAINNET=1");
  }
  if (!process.stdin.isTTY) {
    fail(`Non-interactive broadcast requires CONFIRM_BROADCAST=${confirmationToken}`);
  }

  const phrase = `set ${confirmationToken}`;
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await rl.question(`Type "${phrase}" to broadcast: `);
    if (answer.trim() !== phrase) fail("Confirmation phrase did not match; aborting");
  } finally {
    rl.close();
  }
}

async function main(): Promise<void> {
  const cluster = normalizeCluster(requireEnv("CLUSTER"));
  const rpcUrl = env("RPC_URL") || env("SOLANA_RPC_URL");
  if (!rpcUrl) fail("Missing RPC_URL or SOLANA_RPC_URL");

  const keypairPath = requireEnv("KEYPAIR");
  const newAdmin = parseNewAdmin();
  const broadcast = shouldBroadcast();

  const connection = new Connection(rpcUrl, "confirmed");
  const admin = loadKeypair(keypairPath);

  // Proof of custody for NEW_ADMIN. When NEW_ADMIN_KEYPAIR is supplied it must
  // be NEW_ADMIN itself, and it becomes the fee payer: the fee-payer slot is the
  // only signer this transaction can gain WITHOUT altering the set_admin account
  // list that was already simulated clean against the deployed binary.
  const newAdminKeypairPath = env("NEW_ADMIN_KEYPAIR");
  let newAdminSigner: Keypair | null = null;
  if (newAdminKeypairPath) {
    newAdminSigner = loadKeypair(newAdminKeypairPath, "NEW_ADMIN_KEYPAIR");
    if (!newAdminSigner.publicKey.equals(newAdmin)) {
      fail(
        `NEW_ADMIN_KEYPAIR is ${newAdminSigner.publicKey.toBase58()} but NEW_ADMIN is ` +
          `${newAdmin.toBase58()}. One of the two is wrong; resolve before broadcasting.`,
      );
    }
  }

  // A retired admin is often drained, and a zero-balance signer cannot pay its
  // own fee: simulation fails with AccountNotFound before the program is
  // reached. FEE_PAYER lets a funded key pay while the admin still signs, so
  // the rotation does not require re-funding a key that was deliberately
  // emptied. Defaults to the admin when unset.
  const feePayerPath = env("FEE_PAYER");
  if (newAdminSigner && feePayerPath) {
    fail(
      "Set NEW_ADMIN_KEYPAIR or FEE_PAYER, not both. NEW_ADMIN_KEYPAIR already pays the fee, " +
        "and that signature is the proof of custody. To rotate to a key you cannot sign with, " +
        `drop NEW_ADMIN_KEYPAIR and set ${UNPROVEN_OVERRIDE}=1.`,
    );
  }
  const feePayer = newAdminSigner ?? (feePayerPath ? loadKeypair(feePayerPath, "FEE_PAYER") : admin);
  const separatePayer = !feePayer.publicKey.equals(admin.publicKey);

  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], RAILS_PROGRAM);

  console.log("\nwzrd-rails set_admin (SINGLE STEP - no accept leg)");
  console.log(`  Mode      : ${broadcast ? "broadcast" : "dry-run simulation only"}`);
  console.log(`  Cluster   : ${cluster}`);
  console.log(`  Program   : ${RAILS_PROGRAM.toBase58()}`);
  console.log(`  Config    : ${configPda.toBase58()}`);
  console.log(`  Signer    : ${admin.publicKey.toBase58()}`);
  const payerRole = newAdminSigner
    ? " (NEW_ADMIN, proves custody)"
    : separatePayer
      ? " (separate)"
      : " (same as signer)";
  console.log(`  Fee payer : ${feePayer.publicKey.toBase58()}${payerRole}`);
  console.log(`  New admin : ${newAdmin.toBase58()}`);

  await guardNewAdmin(connection, cluster, newAdmin, newAdminSigner !== null, broadcast);

  const payerBalance = await connection.getBalance(feePayer.publicKey, "confirmed");
  console.log(`  Fee payer balance: ${payerBalance} lamports`);
  if (payerBalance === 0) {
    fail(
      `Fee payer ${feePayer.publicKey.toBase58()} has 0 lamports and cannot pay. ` +
        (newAdminSigner
          ? "On the proven path NEW_ADMIN pays: fund NEW_ADMIN, or drop NEW_ADMIN_KEYPAIR and " +
            `set ${UNPROVEN_OVERRIDE}=1 to pay from FEE_PAYER instead.`
          : "Set FEE_PAYER to a funded keypair, or fund this one."),
    );
  }

  const configAcct = await connection.getAccountInfo(configPda, "confirmed");
  if (!configAcct) fail("Config account not found on-chain");
  if (!configAcct.owner.equals(RAILS_PROGRAM)) {
    fail(`Config owner mismatch: ${configAcct.owner.toBase58()}`);
  }

  const configuredAdmin = readConfigAdmin(configAcct.data);
  console.log(`\n  Current on-chain admin: ${configuredAdmin.toBase58()}`);
  if (!configuredAdmin.equals(admin.publicKey)) {
    fail(`KEYPAIR pubkey is not config admin. Expected ${configuredAdmin.toBase58()}`);
  }
  if (configuredAdmin.equals(newAdmin)) {
    console.log("  Admin already equals target. No transaction needed.");
    return;
  }

  const ixData = Buffer.concat([anchorDiscriminator("set_admin"), newAdmin.toBuffer()]);
  const ix = new TransactionInstruction({
    programId: RAILS_PROGRAM,
    keys: [
      { pubkey: configPda, isSigner: false, isWritable: true },
      { pubkey: admin.publicKey, isSigner: true, isWritable: false },
    ],
    data: ixData,
  });

  const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash("confirmed");
  const message = new TransactionMessage({
    payerKey: feePayer.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();

  const tx = new VersionedTransaction(message);
  tx.sign(separatePayer ? [feePayer, admin] : [admin]);

  console.log("\nSimulating transaction...");
  const simulation = await connection.simulateTransaction(tx, {
    commitment: "confirmed",
    sigVerify: true,
  });
  if (simulation.value.logs?.length) {
    for (const line of simulation.value.logs) console.log(`  ${line}`);
  }
  if (simulation.value.err) {
    console.error("Simulation failed:", JSON.stringify(simulation.value.err));
    process.exit(1);
  }
  console.log("  Simulation passed.");

  if (!broadcast) {
    console.log("\nDRY RUN ONLY. No transaction sent.");
    console.log(`To broadcast, rerun with BROADCAST=1 and confirm token ${cluster}:${newAdmin.toBase58()}`);
    return;
  }

  await confirmBroadcast(cluster, newAdmin);

  console.log("\nSending transaction...");
  const signature = await connection.sendTransaction(tx, { skipPreflight: false });
  console.log(`  Signature: ${signature}`);

  const confirmed = await connection.confirmTransaction(
    { signature, blockhash, lastValidBlockHeight },
    "confirmed",
  );
  if (confirmed.value.err) {
    console.error("Transaction failed:", JSON.stringify(confirmed.value.err));
    process.exit(1);
  }

  const adminAfter = await pollConfigAdmin(connection, configPda, newAdmin);
  console.log(`\n  On-chain admin after: ${adminAfter.toBase58()}`);
  if (!adminAfter.equals(newAdmin)) {
    console.error(`MISMATCH: expected ${newAdmin.toBase58()}, got ${adminAfter.toBase58()}`);
    console.error("  The transaction confirmed WITHOUT error, so the rotation may well have landed");
    console.error("  and this readback may just be a stale RPC view. Do NOT re-run this script");
    console.error("  until you have checked on chain -- set_admin is irreversible and has no");
    console.error("  accept leg:");
    console.error(`    solana confirm -v ${signature}`);
    console.error(`    solana account ${configPda.toBase58()}`);
    console.error("  If it did land, the old admin is no longer the admin and a re-run would fail");
    console.error("  the signer check anyway.");
    process.exit(1);
  }

  console.log("  SUCCESS: config admin rotated.");
  console.log(`\nExplorer: https://solscan.io/tx/${signature}${cluster === "mainnet-beta" ? "" : `?cluster=${cluster}`}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
