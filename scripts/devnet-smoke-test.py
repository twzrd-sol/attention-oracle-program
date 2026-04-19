#!/usr/bin/env python3
"""
wzrd-rails devnet smoke test — §7.2 single-agent fee-path assertion.

Critical validation gate: for a stake of 1_000_000_000 raw (1 UI CCM) on a
Token-2022 CCM mint with 50 bps TransferFeeConfig, the on-chain
UserStake.amount must be exactly 995_000_000 (post-fee) — NOT 1_000_000_000.

If the assertion fails, the program is reading `requested_amount` instead of
`actual_received = balance_after - balance_before`, and the TransferFee path
is broken. This is the bug we shipped the LiteSVM test for (stake_with_
transfer_fee_credits_actual_received) — §7.2 is the production equivalent.

Idempotent: re-running is safe. Agent keypair persists in /tmp so repeat runs
re-use it (attempt_stake will return status='already_staked' on 2nd call).

Usage:
    python3 scripts/devnet-smoke-test.py

Exit 0 on PASS, non-zero on FAIL.
"""

import asyncio
import base64
import json
import os
import subprocess
import sys
from pathlib import Path

# ── Devnet environment (pin BEFORE importing swarm.stake) ─────────────────
DEVNET_RPC = "https://api.devnet.solana.com"
DEVNET_CCM_MINT = "CZbfA62DHjJyndjRAdbTXMqCEV1uBJKjh3d3yVAZbJuj"
RAILS_PROGRAM_ID = "BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9"
DEVNET_POOL_PDA = "6oQDChd9wJv4CJdPT8zsBwPmYT2jUmogetVP9me6u5Vf"

os.environ["CCM_MINT"] = DEVNET_CCM_MINT
# RAILS_PROGRAM_ID env default already matches our deploy ID; no override needed.

# ── Admin (pays ATA rent + mints CCM to test agent) ──────────────────────
ADMIN_KEYPAIR_PATH = str(Path.home() / ".config" / "solana" / "id.json")

# ── Test agent (ephemeral — throwaway keypair, not a production identity) ─
TEST_AGENT_DIR = Path("/tmp/wzrd-rails-devnet-agents")
TEST_AGENT_PATH = TEST_AGENT_DIR / "agent-devnet-1.json"
TEST_AGENT_ID = "agent-devnet-1"

# ── Stake parameters ──────────────────────────────────────────────────────
STAKE_AMOUNT_RAW = 1_000_000_000   # 1 UI CCM at 9 decimals
FEE_BPS = 50
EXPECTED_POST_FEE = STAKE_AMOUNT_RAW * (10_000 - FEE_BPS) // 10_000  # 995_000_000

# ── swarm.stake import requires wzrd-final on sys.path ───────────────────
SWARM_RUNNER_PATH = "/home/twzrd/wzrd-final/agents/swarm-runner"
if SWARM_RUNNER_PATH not in sys.path:
    sys.path.insert(0, SWARM_RUNNER_PATH)

TOKEN_2022_PROGRAM_ID = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"


def run(cmd: list[str], check: bool = True, quiet: bool = False) -> subprocess.CompletedProcess:
    """Run a command, echo it, return CompletedProcess."""
    if not quiet:
        print(f"$ {' '.join(cmd)}")
    result = subprocess.run(cmd, capture_output=True, text=True)
    if not quiet and result.stdout:
        print(result.stdout, end="")
    if result.returncode != 0:
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)
        if check:
            sys.exit(f"Command failed (exit {result.returncode})")
    return result


def get_sol_balance(pubkey: str) -> float:
    r = run(["solana", "balance", "--url", DEVNET_RPC, pubkey],
            check=False, quiet=True)
    if r.returncode != 0:
        return 0.0
    try:
        return float(r.stdout.strip().split()[0])
    except (ValueError, IndexError):
        return 0.0


def ensure_agent_keypair() -> str:
    """Generate ephemeral agent keypair if missing. Return pubkey."""
    TEST_AGENT_DIR.mkdir(parents=True, exist_ok=True)
    if not TEST_AGENT_PATH.exists():
        print(f"Generating fresh agent keypair at {TEST_AGENT_PATH}")
        run(["solana-keygen", "new", "--no-bip39-passphrase",
             "--silent", "-o", str(TEST_AGENT_PATH)])
    r = run(["solana-keygen", "pubkey", str(TEST_AGENT_PATH)], quiet=True)
    return r.stdout.strip()


def ensure_agent_sol(agent_pubkey: str) -> None:
    bal = get_sol_balance(agent_pubkey)
    print(f"Agent SOL balance: {bal} SOL")
    if bal >= 0.05:
        return
    print("Trying devnet airdrop of 0.5 SOL...")
    airdrop = run(["solana", "airdrop", "0.5", agent_pubkey,
                   "--url", DEVNET_RPC], check=False)
    if airdrop.returncode == 0:
        return
    print("Airdrop rate-limited — falling back to admin transfer...")
    run(["solana", "transfer", agent_pubkey, "0.2",
         "--from", ADMIN_KEYPAIR_PATH,
         "--keypair", ADMIN_KEYPAIR_PATH,
         "--fee-payer", ADMIN_KEYPAIR_PATH,
         "--allow-unfunded-recipient",
         "--url", DEVNET_RPC])


def get_agent_ccm_ata(agent_pubkey: str) -> str:
    """Derive agent's Token-2022 CCM ATA."""
    r = run(["spl-token", "--url", DEVNET_RPC,
             "--program-id", TOKEN_2022_PROGRAM_ID,
             "address", "--token", DEVNET_CCM_MINT,
             "--owner", agent_pubkey, "--verbose"], quiet=True)
    for line in r.stdout.split("\n"):
        if "Associated token address" in line:
            return line.split(":", 1)[1].strip()
    sys.exit("Failed to derive agent's CCM ATA")


def ensure_agent_ccm(agent_pubkey: str, agent_ata: str) -> None:
    # Check current balance first
    r = run(["spl-token", "--url", DEVNET_RPC,
             "--program-id", TOKEN_2022_PROGRAM_ID,
             "balance", "--address", agent_ata],
            check=False, quiet=True)
    if r.returncode == 0:
        try:
            current_ui = float(r.stdout.strip())
            print(f"Agent CCM balance: {current_ui} UI CCM")
            if current_ui >= 1.0:
                print("Sufficient for 1 CCM stake; skipping mint")
                return
        except ValueError:
            pass

    # ATA missing or empty — create (idempotent via admin) and mint
    print("Creating agent's CCM ATA (if missing) and minting 10 UI CCM...")
    create_res = run(["spl-token", "--url", DEVNET_RPC,
                      "--program-id", TOKEN_2022_PROGRAM_ID,
                      "create-account", DEVNET_CCM_MINT,
                      "--owner", agent_pubkey,
                      "--fee-payer", ADMIN_KEYPAIR_PATH],
                     check=False)
    # "already in use" is fine; anything else is a real error
    if create_res.returncode != 0 and "already in use" not in (create_res.stderr or ""):
        print(create_res.stderr or "", file=sys.stderr)
        if "already" not in (create_res.stdout or "") + (create_res.stderr or ""):
            sys.exit(f"create-account failed (exit {create_res.returncode})")

    run(["spl-token", "--url", DEVNET_RPC,
         "--program-id", TOKEN_2022_PROGRAM_ID,
         "mint", DEVNET_CCM_MINT, "10",
         "--recipient-owner", agent_pubkey])


def fetch_user_stake_pda_data(user_stake_pda: str) -> bytes:
    """Return raw account data for the UserStake PDA."""
    r = run(["solana", "account", user_stake_pda, "--url", DEVNET_RPC,
             "--output", "json"], quiet=True)
    acct = json.loads(r.stdout)
    data_field = acct["account"]["data"]
    # Solana CLI json format: [base64_encoded_data, "base64"]
    if isinstance(data_field, list) and len(data_field) >= 1:
        return base64.b64decode(data_field[0])
    raise ValueError(f"Unexpected data field shape: {data_field!r}")


def parse_user_stake_amount(data: bytes) -> int:
    """Parse UserStake.amount from raw account bytes.

    Layout (Anchor, little-endian):
      bytes  0- 7: discriminator
      bytes  8-39: user Pubkey (32)
      bytes 40-71: pool Pubkey (32)
      bytes 72-79: amount u64  ← THIS
      bytes 80-95: reward_debt u128
      bytes 96-103: pending_rewards u64
      bytes 104-111: lock_end_slot u64
      bytes 112: bump u8
      Total: 113 bytes
    """
    if len(data) < 80:
        raise ValueError(f"UserStake data too short: {len(data)} bytes (expected 113)")
    return int.from_bytes(data[72:80], "little")


async def run_stake_and_assert(agent_pubkey: str) -> bool:
    """Call attempt_stake, fetch UserStake PDA, assert amount == EXPECTED_POST_FEE."""
    import aiohttp
    from solders.keypair import Keypair
    from solders.pubkey import Pubkey
    from swarm import stake

    kp_bytes = bytes(json.loads(TEST_AGENT_PATH.read_text()))
    kp = Keypair.from_bytes(kp_bytes)

    print(f"\n--- Calling attempt_stake({STAKE_AMOUNT_RAW}) via swarm.stake ---")
    async with aiohttp.ClientSession() as http:
        result = await stake.attempt_stake(
            http, DEVNET_RPC, kp, STAKE_AMOUNT_RAW, TEST_AGENT_ID
        )
        print(f"StakeResult.status = {result.status}")
        print(f"StakeResult.tx_sig = {result.tx_sig}")
        if result.error:
            print(f"StakeResult.error  = {result.error}")

        if result.status not in ("staked", "already_staked", "lock_active"):
            print(f"\nFAIL: unexpected status {result.status!r}")
            return False

        if result.status != "staked":
            print(f"(Agent already had a {result.status!r} position from prior run — "
                  f"asserting against existing UserStake state)")

    # Derive UserStake PDA
    program_id = Pubkey.from_string(RAILS_PROGRAM_ID)
    pool_pda = Pubkey.from_string(DEVNET_POOL_PDA)
    user_stake_pda, _ = Pubkey.find_program_address(
        [b"user_stake", bytes(pool_pda), bytes(kp.pubkey())], program_id
    )
    print(f"\n--- Fetching UserStake PDA {user_stake_pda} ---")

    data = fetch_user_stake_pda_data(str(user_stake_pda))
    print(f"Account size: {len(data)} bytes (expected 113)")

    amount = parse_user_stake_amount(data)
    print(f"\n=== FEE-PATH ASSERTION ===")
    print(f"  Stake requested     : {STAKE_AMOUNT_RAW:>13,}")
    print(f"  Expected post-fee   : {EXPECTED_POST_FEE:>13,}")
    print(f"  UserStake.amount    : {amount:>13,}")

    if amount == EXPECTED_POST_FEE:
        print(f"\n  ✓ PASS: amount matches post-fee expectation")
        return True
    if amount == STAKE_AMOUNT_RAW:
        print(f"\n  ✗ FAIL: amount is pre-fee ({amount}). TransferFee path is BROKEN —")
        print(f"  program is reading requested_amount instead of actual_received.")
        return False
    print(f"\n  ✗ FAIL: amount is {amount} (neither pre-fee nor post-fee)")
    return False


def main() -> int:
    print("=" * 60)
    print("wzrd-rails devnet smoke test — §7.2 single-agent")
    print("=" * 60)
    print(f"RPC:         {DEVNET_RPC}")
    print(f"Program:     {RAILS_PROGRAM_ID}")
    print(f"CCM Mint:    {DEVNET_CCM_MINT}")
    print(f"Pool PDA:    {DEVNET_POOL_PDA}")
    print(f"Agent key:   {TEST_AGENT_PATH}")

    print("\n--- Phase A: setup test agent (idempotent) ---")
    agent_pubkey = ensure_agent_keypair()
    print(f"Agent pubkey: {agent_pubkey}")
    ensure_agent_sol(agent_pubkey)
    agent_ata = get_agent_ccm_ata(agent_pubkey)
    print(f"Agent CCM ATA: {agent_ata}")
    ensure_agent_ccm(agent_pubkey, agent_ata)

    print("\n--- Phase B/C: stake + fee-path assertion ---")
    success = asyncio.run(run_stake_and_assert(agent_pubkey))

    print("\n" + "=" * 60)
    print(f"RESULT: {'✓ §7.2 PASS' if success else '✗ §7.2 FAIL'}")
    print("=" * 60)
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())
