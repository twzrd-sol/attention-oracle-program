#!/usr/bin/env python3
"""Initialize wzrd-rails program state on a target Solana cluster.

One-time-per-cluster setup (devnet or mainnet):
  1. initialize_config — creates the global Config PDA bound to (ccm_mint, treasury_ata)
  2. initialize_pool(pool_id=0, lock_duration_slots=1_512_000) — Day 1 global pool, 7-day lock
  3. (optional) set_reward_rate(pool_id=0, rate) — starts 0 (emissions off); turn on later

Idempotent: each step is skipped when the target PDA already exists on-chain, so the
script can be re-run after partial failures without tripping Anchor `init` constraints.

Usage (devnet):
  scripts/devnet-init-rails.py \
      --rpc https://api.devnet.solana.com \
      --admin ~/.config/solana/devnet-deployer.json \
      --ccm-mint <devnet CCM mint pubkey>

Mainnet deploy re-uses this script with --rpc and --admin flipped to the Squads
vault + mainnet RPC. Do NOT hardcode cluster-specific values; the script is
cluster-agnostic because the declared program ID and keypair are the same
everywhere.

Dependencies: solders, aiohttp (already in swarm-runner's Python env).
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import hashlib
import json
import struct
import sys
from pathlib import Path

import aiohttp
from solders.keypair import Keypair
from solders.pubkey import Pubkey
from solders.instruction import Instruction, AccountMeta
from solders.hash import Hash as SolHash
from solders.message import MessageV0
from solders.transaction import VersionedTransaction


# ── Constants ───────────────────────────────────────────────────────────

RAILS_PROGRAM_ID_DEFAULT = "BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9"

# PDA seeds — MUST match programs/wzrd-rails/src/state.rs
CONFIG_SEED = b"config"
POOL_SEED = b"pool"
STAKE_VAULT_SEED = b"stake_vault"
REWARD_VAULT_SEED = b"reward_vault"

TOKEN_2022_PROGRAM = Pubkey.from_string("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
ATA_PROGRAM = Pubkey.from_string("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")
SYSTEM_PROGRAM = Pubkey.from_string("11111111111111111111111111111111")
RENT_SYSVAR = Pubkey.from_string("SysvarRent111111111111111111111111111111111")

# 7 days * 24h * 60m * 60s / 0.4s-per-slot = 1_512_000
DEFAULT_LOCK_SLOTS = 1_512_000


# ── Helpers ─────────────────────────────────────────────────────────────

def anchor_disc(ix_name: str) -> bytes:
    """Anchor instruction discriminator: sha256('global:<name>')[..8]."""
    return hashlib.sha256(f"global:{ix_name}".encode()).digest()[:8]


def find_pda(seeds: list[bytes], program: Pubkey) -> Pubkey:
    return Pubkey.find_program_address(seeds, program)[0]


def get_ata(owner: Pubkey, mint: Pubkey) -> Pubkey:
    """Associated Token Account for Token-2022."""
    return Pubkey.find_program_address(
        [bytes(owner), bytes(TOKEN_2022_PROGRAM), bytes(mint)],
        ATA_PROGRAM,
    )[0]


def load_keypair(path: str) -> Keypair:
    raw = Path(path).expanduser().read_text().strip()
    arr = json.loads(raw)
    return Keypair.from_bytes(bytes(arr))


async def get_account(http: aiohttp.ClientSession, rpc_url: str, pubkey: Pubkey) -> dict | None:
    body = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getAccountInfo",
        "params": [str(pubkey), {"encoding": "base64"}],
    }
    async with http.post(rpc_url, json=body, timeout=aiohttp.ClientTimeout(total=10)) as resp:
        data = await resp.json()
    return data.get("result", {}).get("value")


async def get_latest_blockhash(http: aiohttp.ClientSession, rpc_url: str) -> SolHash:
    body = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getLatestBlockhash",
        "params": [{"commitment": "confirmed"}],
    }
    async with http.post(rpc_url, json=body, timeout=aiohttp.ClientTimeout(total=10)) as resp:
        data = await resp.json()
    return SolHash.from_string(data["result"]["value"]["blockhash"])


async def submit_tx(
    http: aiohttp.ClientSession,
    rpc_url: str,
    payer: Keypair,
    ixs: list[Instruction],
) -> str:
    blockhash = await get_latest_blockhash(http, rpc_url)
    msg = MessageV0.try_compile(payer.pubkey(), ixs, [], blockhash)
    tx = VersionedTransaction(msg, [payer])
    body = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "sendTransaction",
        "params": [
            base64.b64encode(bytes(tx)).decode(),
            {
                "encoding": "base64",
                "skipPreflight": False,
                "preflightCommitment": "confirmed",
            },
        ],
    }
    async with http.post(rpc_url, json=body, timeout=aiohttp.ClientTimeout(total=30)) as resp:
        data = await resp.json()
    if "error" in data:
        raise RuntimeError(f"tx failed: {data['error']}")
    return data["result"]


async def wait_for_confirmation(
    http: aiohttp.ClientSession,
    rpc_url: str,
    sig: str,
    max_polls: int = 12,
    poll_interval: float = 2.5,
) -> bool:
    for _ in range(max_polls):
        await asyncio.sleep(poll_interval)
        body = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignatureStatuses",
            "params": [[sig], {"searchTransactionHistory": False}],
        }
        async with http.post(rpc_url, json=body, timeout=aiohttp.ClientTimeout(total=10)) as resp:
            data = await resp.json()
        statuses = data.get("result", {}).get("value", [])
        if statuses and statuses[0] and statuses[0].get("confirmationStatus") in ("confirmed", "finalized"):
            return True
    return False


# ── Instruction builders ────────────────────────────────────────────────

def build_init_config_ix(
    program: Pubkey,
    admin: Pubkey,
    ccm_mint: Pubkey,
    treasury_ata: Pubkey,
) -> Instruction:
    """initialize_config(ccm_mint: Pubkey, treasury_ccm_ata: Pubkey).

    Data: [disc(8)] + [ccm_mint(32)] + [treasury_ata(32)] = 72 bytes.
    Accounts: config (init PDA mut) + signer (admin, mut+signer) + system_program.
    """
    config = find_pda([CONFIG_SEED], program)
    data = anchor_disc("initialize_config") + bytes(ccm_mint) + bytes(treasury_ata)
    accounts = [
        AccountMeta(config, is_signer=False, is_writable=True),
        AccountMeta(admin, is_signer=True, is_writable=True),
        AccountMeta(SYSTEM_PROGRAM, is_signer=False, is_writable=False),
    ]
    return Instruction(program, data, accounts)


def build_init_pool_ix(
    program: Pubkey,
    admin: Pubkey,
    ccm_mint: Pubkey,
    pool_id: int,
    lock_duration_slots: int,
) -> Instruction:
    """initialize_pool(pool_id: u32, lock_duration_slots: u64).

    Data: [disc(8)] + [pool_id u32 LE (4)] + [lock u64 LE (8)] = 20 bytes.
    Accounts: 9 (config mut, pool init, ccm_mint, stake_vault init, reward_vault init,
                 admin signer+mut, token_2022_program, system_program, rent).
    """
    config = find_pda([CONFIG_SEED], program)
    pool = find_pda([POOL_SEED, pool_id.to_bytes(4, "little")], program)
    stake_vault = find_pda([STAKE_VAULT_SEED, bytes(pool)], program)
    reward_vault = find_pda([REWARD_VAULT_SEED, bytes(pool)], program)

    data = anchor_disc("initialize_pool") + struct.pack("<IQ", pool_id, lock_duration_slots)

    accounts = [
        AccountMeta(config, is_signer=False, is_writable=True),            # 0: config (mut, has_one admin+ccm_mint)
        AccountMeta(pool, is_signer=False, is_writable=True),              # 1: pool (init)
        AccountMeta(ccm_mint, is_signer=False, is_writable=False),         # 2: ccm_mint
        AccountMeta(stake_vault, is_signer=False, is_writable=True),       # 3: stake_vault (init)
        AccountMeta(reward_vault, is_signer=False, is_writable=True),      # 4: reward_vault (init)
        AccountMeta(admin, is_signer=True, is_writable=True),              # 5: admin (signer+mut)
        AccountMeta(TOKEN_2022_PROGRAM, is_signer=False, is_writable=False),  # 6
        AccountMeta(SYSTEM_PROGRAM, is_signer=False, is_writable=False),   # 7
        AccountMeta(RENT_SYSVAR, is_signer=False, is_writable=False),      # 8
    ]
    return Instruction(program, data, accounts)


def build_set_reward_rate_ix(
    program: Pubkey,
    admin: Pubkey,
    pool_id: int,
    new_rate: int,
) -> Instruction:
    """set_reward_rate(pool_id: u32, new_rate: u64). Admin-only, accrues before apply."""
    config = find_pda([CONFIG_SEED], program)
    pool = find_pda([POOL_SEED, pool_id.to_bytes(4, "little")], program)
    data = anchor_disc("set_reward_rate") + struct.pack("<IQ", pool_id, new_rate)
    accounts = [
        AccountMeta(config, is_signer=False, is_writable=False),
        AccountMeta(pool, is_signer=False, is_writable=True),
        AccountMeta(admin, is_signer=True, is_writable=False),
    ]
    return Instruction(program, data, accounts)


# ── Main ────────────────────────────────────────────────────────────────

async def run(args: argparse.Namespace) -> int:
    program = Pubkey.from_string(args.program)
    ccm_mint = Pubkey.from_string(args.ccm_mint)
    admin = load_keypair(args.admin)
    treasury = (
        Pubkey.from_string(args.treasury)
        if args.treasury
        else get_ata(admin.pubkey(), ccm_mint)
    )

    config_pda = find_pda([CONFIG_SEED], program)
    pool_pda = find_pda([POOL_SEED, args.pool_id.to_bytes(4, "little")], program)
    stake_vault = find_pda([STAKE_VAULT_SEED, bytes(pool_pda)], program)
    reward_vault = find_pda([REWARD_VAULT_SEED, bytes(pool_pda)], program)
    lock_days = args.lock_slots * 0.4 / 86400

    print("═══ wzrd-rails init plan ═══")
    print(f"  RPC:              {args.rpc}")
    print(f"  Program:          {program}")
    print(f"  Admin:            {admin.pubkey()}")
    print(f"  CCM mint:         {ccm_mint}")
    print(f"  Treasury CCM ATA: {treasury}")
    print(f"  Pool ID:          {args.pool_id}")
    print(f"  Lock slots:       {args.lock_slots:,} (~{lock_days:.1f} days at 400ms/slot)")
    print(f"  Reward rate:      {args.reward_rate} units/slot", end="")
    print(" (emissions OFF)" if args.reward_rate == 0 else "")
    print("  Derived PDAs:")
    print(f"    Config:         {config_pda}")
    print(f"    Pool:           {pool_pda}")
    print(f"    StakeVault:     {stake_vault}")
    print(f"    RewardVault:    {reward_vault}")
    print()

    if args.dry_run:
        print("DRY RUN — no transactions submitted")
        return 0

    async with aiohttp.ClientSession() as http:
        # Step 1: initialize_config (skip if Config PDA already exists)
        cfg_account = await get_account(http, args.rpc, config_pda)
        if cfg_account is not None:
            print("✓ Config PDA already exists, skipping initialize_config")
        else:
            print("→ initialize_config...")
            ix = build_init_config_ix(program, admin.pubkey(), ccm_mint, treasury)
            sig = await submit_tx(http, args.rpc, admin, [ix])
            print(f"  tx: {sig}")
            confirmed = await wait_for_confirmation(http, args.rpc, sig)
            print(f"  {'✓ confirmed' if confirmed else '⚠ not confirmed in 30s (may still land)'}")

        # Step 2: initialize_pool (skip if Pool PDA already exists)
        pool_account = await get_account(http, args.rpc, pool_pda)
        if pool_account is not None:
            print(f"✓ Pool PDA already exists, skipping initialize_pool(pool_id={args.pool_id})")
        else:
            print(f"→ initialize_pool(pool_id={args.pool_id}, lock_slots={args.lock_slots:,})...")
            ix = build_init_pool_ix(
                program, admin.pubkey(), ccm_mint, args.pool_id, args.lock_slots
            )
            sig = await submit_tx(http, args.rpc, admin, [ix])
            print(f"  tx: {sig}")
            confirmed = await wait_for_confirmation(http, args.rpc, sig)
            print(f"  {'✓ confirmed' if confirmed else '⚠ not confirmed in 30s (may still land)'}")

        # Step 3: set_reward_rate (only if --reward-rate > 0)
        if args.reward_rate > 0:
            print(f"→ set_reward_rate(pool_id={args.pool_id}, rate={args.reward_rate})...")
            ix = build_set_reward_rate_ix(program, admin.pubkey(), args.pool_id, args.reward_rate)
            sig = await submit_tx(http, args.rpc, admin, [ix])
            print(f"  tx: {sig}")
            confirmed = await wait_for_confirmation(http, args.rpc, sig)
            print(f"  {'✓ confirmed' if confirmed else '⚠ not confirmed in 30s (may still land)'}")
        else:
            print("✓ reward_rate=0 (emissions off); call set_reward_rate later when ready")

        print()
        print("✓ wzrd-rails init complete")
        return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Initialize wzrd-rails program state")
    ap.add_argument("--rpc", required=True, help="RPC URL (e.g., https://api.devnet.solana.com)")
    ap.add_argument("--admin", required=True, help="Path to admin keypair JSON (solana-keygen format)")
    ap.add_argument("--ccm-mint", required=True, help="CCM mint pubkey on this cluster")
    ap.add_argument(
        "--program",
        default=RAILS_PROGRAM_ID_DEFAULT,
        help=f"wzrd-rails program ID (default: {RAILS_PROGRAM_ID_DEFAULT})",
    )
    ap.add_argument(
        "--treasury",
        help="Treasury CCM ATA (defaults to admin's CCM ATA derived from --ccm-mint)",
    )
    ap.add_argument(
        "--pool-id",
        type=int,
        default=0,
        help="Pool ID to initialize (default: 0 for the global pool)",
    )
    ap.add_argument(
        "--lock-slots",
        type=int,
        default=DEFAULT_LOCK_SLOTS,
        help=f"Lock duration in slots (default: {DEFAULT_LOCK_SLOTS:,} = ~7 days)",
    )
    ap.add_argument(
        "--reward-rate",
        type=int,
        default=0,
        help="Initial reward rate per slot (default: 0 = emissions off; turn on later)",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the plan and derived PDAs without submitting any transactions",
    )
    args = ap.parse_args()
    return asyncio.run(run(args))


if __name__ == "__main__":
    sys.exit(main())
