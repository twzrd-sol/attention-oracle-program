/**
 * Test withdrawal queue flow
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";

const PROGRAM_ID = new PublicKey("5WH4UiSZ7fbPQbLrRCJyWxnTAoNyTZ3ZjcdgTuinCXmQ");
const VAULT = new PublicKey("7tjCgZcsK4sgV65wsNajUVRuGHQ7GZELWfTaWYbCBDTw");
const VLOFI_MINT = new PublicKey("E9Kt33axpCy3ve2PCY9BSrbPhcR9wdDsWQECAahzw2dS");

// Seeds
const USER_VAULT_STATE_SEED = Buffer.from("user_state");
const WITHDRAW_REQUEST_SEED = Buffer.from("withdraw");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const idl = await Program.fetchIdl(PROGRAM_ID, provider);
  if (!idl) throw new Error("IDL not found");
  const program = new Program(idl, provider);

  const user = provider.wallet.publicKey;

  // User's vLOFI ATA
  const userVlofi = getAssociatedTokenAddressSync(VLOFI_MINT, user, false, TOKEN_PROGRAM_ID);

  // Check vLOFI balance
  const balance = await provider.connection.getTokenAccountBalance(userVlofi);
  console.log("Current vLOFI balance:", balance.value.uiAmount);

  // Request withdraw of 100 vLOFI (keep some for other tests)
  const shares = new BN(100 * 1e9); // 100 vLOFI
  const minAmount = new BN(90 * 1e9); // Expect at least 90 CCM back (some slippage tolerance)

  // Derive user vault state PDA
  const [userVaultState] = PublicKey.findProgramAddressSync(
    [USER_VAULT_STATE_SEED, VAULT.toBuffer(), user.toBuffer()],
    PROGRAM_ID
  );

  // Get next request ID (0 if first time)
  let nextRequestId = new BN(0);
  try {
    const info = await provider.connection.getAccountInfo(userVaultState);
    if (info) {
      // Parse next_request_id from user vault state (offset: 8 disc + 1 bump + 32 user + 32 vault = 73)
      nextRequestId = new BN(info.data.readBigUInt64LE(73).toString());
    }
  } catch {}

  console.log("Next request ID:", nextRequestId.toString());

  // Derive withdraw request PDA
  const [withdrawRequest] = PublicKey.findProgramAddressSync(
    [
      WITHDRAW_REQUEST_SEED,
      VAULT.toBuffer(),
      user.toBuffer(),
      nextRequestId.toArrayLike(Buffer, "le", 8),
    ],
    PROGRAM_ID
  );

  console.log("\nRequesting withdrawal of 100 vLOFI...");
  console.log("User Vault State:", userVaultState.toBase58());
  console.log("Withdraw Request:", withdrawRequest.toBase58());

  try {
    const tx = await program.methods
      .requestWithdraw(shares, minAmount)
      .accounts({
        user: user,
        vault: VAULT,
        userVaultState: userVaultState,
        vlofiMint: VLOFI_MINT,
        userVlofi: userVlofi,
        withdrawRequest: withdrawRequest,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log("\n✅ Withdraw requested!");
    console.log("Signature:", tx);
    console.log("View: https://solscan.io/tx/" + tx);

    // Get current slot and calculate completion
    const slot = await provider.connection.getSlot();
    const completionSlot = slot + 27000; // 3 hours
    const hoursRemaining = 27000 / 9000;
    console.log(`\nQueue started at slot ${slot}`);
    console.log(`Completes at slot ${completionSlot} (~${hoursRemaining} hours)`);

  } catch (e: any) {
    console.error("❌ Failed:", e.message || e);
    if (e.logs) {
      console.log("\nLogs:");
      e.logs.slice(-10).forEach((l: string) => console.log("  ", l));
    }
  }
}

main();
