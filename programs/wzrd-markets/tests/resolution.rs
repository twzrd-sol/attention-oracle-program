#![cfg(feature = "localtest")]
//! LiteSVM integration coverage for `wzrd-markets` Phase 3 — resolution +
//! settlement (`publish_attention_root`, `resolve_market`, `extend_dispute_window`,
//! `settle`, `resolve_override`, `sweep_residual`, `close_market`, plus the
//! `add_publisher`/`remove_publisher` allow-list admin).
//!
//! THE THREE NON-NEGOTIABLE ACCEPTANCE GATES (scope §9/§12):
//!
//!   * GATE A — `gate_a_*`: the §4 merkle-rejection battery, run through the FULL
//!     `resolve_market` instruction (not the hash layer). A wrong-domain (node or
//!     leaf), overlong, tampered, wrong-market, or malformed proof is REJECTED;
//!     only the byte-correct proof against the H-01 create-time snapshot ACCEPTS.
//!     Cases 1-2 are the M-04 / CH-3 silent-failure kill switches: a proof built
//!     under the wrong domain can NEVER fold to the snapshot root, so it can never
//!     silently verify.
//!
//!   * GATE B — `gate_b_settle_solvency`: post-resolution `vault.amount >=
//!     winning_supply` holds across every partial settle, and the vault drains to
//!     exactly 0 on the final settle (audit MR-1 lockstep: burn 1 winning token,
//!     remove 1 USDC).
//!
//!   * GATE C — `gate_c_never_resolved_recovery`: a market that is NEVER resolved
//!     (deadline passed, no `resolve_market`) recovers full collateral 1:1 via
//!     `redeem_complete_set`. Replaces the old byte-poke `redeem_after_resolved_*`
//!     hack with a real lifecycle path.
//!
//! Run with (the `.so` must be built first via cargo-build-sbf):
//!   cargo-build-sbf --manifest-path programs/wzrd-markets/Cargo.toml
//!   cargo test -p wzrd-markets --features localtest --test resolution -- --nocapture
//!
//! Harness is the `complete_set.rs` litesvm boilerplate (address conversion +
//! program load (markets + Token-2022 + Associated-Token) + tx send), extended
//! with the Phase-3 instruction builders and a parameterized funded-market setup.
//! "USDC" is modeled as a fee-free Token-2022 mint (6 decimals) reached through
//! the token Interface — the same path mainnet USDC exercises, single token
//! program in the harness.

use anchor_lang::{
    error::ERROR_CODE_OFFSET, prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas,
};
use anchor_spl::associated_token::{
    get_associated_token_address_with_program_id, spl_associated_token_account,
    ID as ASSOCIATED_TOKEN_PROGRAM_ID,
};
use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_address::Address;
use solana_instruction::error::InstructionError;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_sdk::{
    instruction::Instruction as LegacyInstruction, keccak, program_pack::Pack,
    pubkey::Pubkey as LegacyPubkey, system_instruction, system_program,
};
use solana_signer::Signer;
use solana_transaction::{Transaction, TransactionError};
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::{Account as SplTokenAccount, Mint as SplMint};
use std::path::{Path, PathBuf};
use wzrd_markets::{
    accounts as markets_accounts, instruction as markets_ix,
    resolution::{
        self, compute_root_from_proof, markets_resolution_node_hash_v1, MarketsResolutionLeafV1,
        MARKETS_RESOLUTION_LEAF_SCHEMA_V1, MARKETS_RESOLUTION_NODE_V1_DOMAIN,
    },
    state::{
        AttentionRoot, AttentionRootConfig, Market, MarketMetric, MarketsConfig,
        ATTENTION_ROOT_SEED, MARKETS_CONFIG_SEED, MARKET_SEED, MINT_AUTH_SEED, NO_MINT_SEED,
        VAULT_SEED, YES_MINT_SEED,
    },
    MarketsError, ID as WZRD_MARKETS_PROGRAM_ID, MAX_MARKET_DURATION_SLOTS,
};

const USDC_DECIMALS: u8 = 6;
const MARKET_ID: u64 = 0;
/// 1_000 "USDC" at 6 decimals — the complete-set amount minted into each market
/// so the vault is funded for settlement / redemption.
const SET_AMOUNT: u64 = 1_000_000_000;
/// Depositor is funded with more than they deposit, to prove exact conservation.
const DEPOSITOR_USDC_FUNDING: u64 = 5_000_000_000;
/// Default resolution window committed in the test leaves.
const WINDOW_ID: u64 = 20_260_622;
/// Protocol-default dispute window stored on `MarketsConfig` at init. Must be
/// non-zero (the IX rejects 0). Never the in-force window on the test markets —
/// each market overrides it via `create_market`'s own `dispute_window_slots`.
const CONFIG_DEFAULT_DISPUTE_WINDOW: u64 = 54_000;
/// H-03: minimum dispute_window_slots enforced by create_market.
const MIN_DISPUTE_WINDOW: u64 = 150;

/// The rails NODE domain — DISTINCT from the markets node domain. A tree built
/// under this domain must be rejected by `resolve_market` (Gate A case 1). Kept
/// verbatim from `docs/cpmm-merkle-conventions-v1.md` so the test fails loudly if
/// the markets domain is ever "unified" with rails.
const RAILS_NODE_V1_DOMAIN: &[u8] = b"wzrd-rails:listen-payout-allocation-node:v1";
/// The rails LEAF domain — DISTINCT from the markets leaf domain (Gate A case 2).
const RAILS_LEAF_V1_DOMAIN: &[u8] = b"wzrd-rails:listen-payout-allocation-leaf:v1";

// ─── address conversion (legacy <-> modern) ──────────────────────────────────

fn address_from_legacy(pubkey: &LegacyPubkey) -> Address {
    Address::from(pubkey.to_bytes())
}

fn legacy_from_address(address: &Address) -> LegacyPubkey {
    LegacyPubkey::new_from_array(address.to_bytes())
}

fn legacy_from_signer(signer: &Keypair) -> LegacyPubkey {
    legacy_from_address(&signer.pubkey())
}

fn anchor_pubkey(pubkey: LegacyPubkey) -> Pubkey {
    Pubkey::new_from_array(pubkey.to_bytes())
}

fn convert_instruction(ix: &LegacyInstruction) -> solana_instruction::Instruction {
    solana_instruction::Instruction {
        program_id: address_from_legacy(&ix.program_id),
        accounts: ix
            .accounts
            .iter()
            .map(|meta| {
                let pubkey = address_from_legacy(&meta.pubkey);
                if meta.is_writable {
                    solana_instruction::AccountMeta::new(pubkey, meta.is_signer)
                } else {
                    solana_instruction::AccountMeta::new_readonly(pubkey, meta.is_signer)
                }
            })
            .collect(),
        data: ix.data.clone(),
    }
}

fn send_tx(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    instructions: &[LegacyInstruction],
) -> TransactionMetadata {
    let payer = signers.first().expect("at least one signer is required");
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());

    match svm.send_transaction(tx) {
        Ok(meta) => meta,
        Err(err) => {
            eprintln!("TX FAILED: {:?}", err.err);
            for log in &err.meta.logs {
                eprintln!("  LOG: {}", log);
            }
            panic!("transaction failed: {:?}", err.err);
        }
    }
}

fn try_send_tx(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    instructions: &[LegacyInstruction],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let payer = signers.first().expect("at least one signer is required");
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(tx)
}

// ─── program loading ──────────────────────────────────────────────────────────

fn load_wzrd_markets_program(svm: &mut LiteSVM) -> Result<(), String> {
    let program_path = Path::new("../../target/deploy/wzrd_markets.so");
    if !program_path.exists() {
        return Err(format!(
            "program binary not found at {} — run `cargo-build-sbf --manifest-path \
             programs/wzrd-markets/Cargo.toml` first",
            program_path.display()
        ));
    }
    let bytes = std::fs::read(program_path).map_err(|err| err.to_string())?;
    svm.add_program(address_from_legacy(&WZRD_MARKETS_PROGRAM_ID), &bytes)
        .map_err(|err| format!("{err:?}"))
}

/// Locate a litesvm-bundled SPL ELF (Token-2022 / Associated-Token) from the
/// cargo registry. Same lookup the complete-set harness uses.
fn find_litesvm_elf(prefix: &str) -> Option<Vec<u8>> {
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home).join(".cargo/registry/src");

    for index_entry in std::fs::read_dir(base).ok()?.flatten() {
        for crate_entry in std::fs::read_dir(index_entry.path()).ok()?.flatten() {
            let name = crate_entry.file_name();
            if !name
                .to_str()
                .is_some_and(|value| value.starts_with("litesvm-"))
            {
                continue;
            }
            let elf_dir = crate_entry.path().join("src/programs/elf");
            for elf_entry in std::fs::read_dir(elf_dir).ok()?.flatten() {
                let name = elf_entry.file_name();
                if name
                    .to_str()
                    .is_some_and(|value| value.starts_with(prefix) && value.ends_with(".so"))
                {
                    return std::fs::read(elf_entry.path()).ok();
                }
            }
        }
    }
    None
}

fn load_token_2022_program(svm: &mut LiteSVM) {
    let bytes =
        find_litesvm_elf("spl_token_2022").expect("Token-2022 ELF not found in cargo registry");
    svm.add_program(address_from_legacy(&spl_token_2022::id()), &bytes)
        .expect("add Token-2022 program");
}

fn load_associated_token_program(svm: &mut LiteSVM) {
    let bytes = find_litesvm_elf("spl_associated_token_account")
        .expect("Associated-Token ELF not found in cargo registry");
    svm.add_program(address_from_legacy(&ASSOCIATED_TOKEN_PROGRAM_ID), &bytes)
        .expect("add Associated-Token program");
}

// ─── PDA derivation ───────────────────────────────────────────────────────────

fn markets_config_pda() -> (LegacyPubkey, u8) {
    let (addr, bump) =
        Pubkey::find_program_address(&[MARKETS_CONFIG_SEED], &WZRD_MARKETS_PROGRAM_ID);
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn market_pda(market_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[MARKET_SEED, &market_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn yes_mint_pda(market_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[YES_MINT_SEED, &market_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn no_mint_pda(market_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[NO_MINT_SEED, &market_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn vault_pda(market_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[VAULT_SEED, &market_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn mint_auth_pda(market_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[MINT_AUTH_SEED, &market_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn attention_root_config_pda() -> (LegacyPubkey, u8) {
    let (addr, bump) =
        Pubkey::find_program_address(&[ATTENTION_ROOT_SEED], &WZRD_MARKETS_PROGRAM_ID);
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn attention_root_pda(window_id: u64) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[ATTENTION_ROOT_SEED, &window_id.to_le_bytes()],
        &WZRD_MARKETS_PROGRAM_ID,
    );
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

fn derive_ata(owner: &LegacyPubkey, mint: &LegacyPubkey) -> LegacyPubkey {
    get_associated_token_address_with_program_id(owner, mint, &spl_token_2022::id())
}

// ─── account reads ────────────────────────────────────────────────────────────

fn read_anchor_account<T: AccountDeserialize>(svm: &LiteSVM, address: &LegacyPubkey) -> T {
    let account = svm
        .get_account(&address_from_legacy(address))
        .unwrap_or_else(|| panic!("missing account: {address}"));
    let mut data = account.data.as_slice();
    T::try_deserialize(&mut data).expect("failed to deserialize anchor account")
}

fn read_token_balance(svm: &LiteSVM, address: &LegacyPubkey) -> u64 {
    let account = svm
        .get_account(&address_from_legacy(address))
        .unwrap_or_else(|| panic!("missing token account: {address}"));
    StateWithExtensions::<SplTokenAccount>::unpack(&account.data)
        .expect("failed to deserialize token account")
        .base
        .amount
}

fn read_mint_supply(svm: &LiteSVM, mint: &LegacyPubkey) -> u64 {
    let account = svm
        .get_account(&address_from_legacy(mint))
        .unwrap_or_else(|| panic!("missing mint: {mint}"));
    StateWithExtensions::<SplMint>::unpack(&account.data)
        .expect("failed to deserialize mint")
        .base
        .supply
}

// ─── token setup helpers (fee-free Token-2022) ────────────────────────────────

fn create_plain_token_2022_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    mint_authority: &LegacyPubkey,
) {
    let payer_pubkey = legacy_from_signer(payer);
    let mint_pubkey = legacy_from_signer(mint);
    let rent = svm.minimum_balance_for_rent_exemption(SplMint::LEN);
    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &mint_pubkey,
        rent,
        SplMint::LEN as u64,
        &spl_token_2022::id(),
    );
    let init_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        &mint_pubkey,
        mint_authority,
        None,
        USDC_DECIMALS,
    )
    .unwrap();
    send_tx(svm, &[payer, mint], &[create_ix, init_ix]);
}

fn create_ata(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &LegacyPubkey,
    mint: &LegacyPubkey,
) -> LegacyPubkey {
    let payer_pubkey = legacy_from_signer(payer);
    let ata = derive_ata(owner, mint);
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer_pubkey,
        owner,
        mint,
        &spl_token_2022::id(),
    );
    send_tx(svm, &[payer], &[ix]);
    ata
}

fn mint_token_2022(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    mint: &LegacyPubkey,
    destination: &LegacyPubkey,
    amount: u64,
) {
    let mint_authority_pubkey = legacy_from_signer(mint_authority);
    let mint_ix = spl_token_2022::instruction::mint_to(
        &spl_token_2022::id(),
        mint,
        destination,
        &mint_authority_pubkey,
        &[],
        amount,
    )
    .unwrap();
    send_tx(svm, &[mint_authority], &[mint_ix]);
}

// ─── instruction builders (Phase 0/1 reused + Phase 3) ─────────────────────────

fn build_initialize_markets_config_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    resolver_multisig: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::InitializeMarketsConfig {
            config,
            admin,
            usdc_mint,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializeMarketsConfig {
            resolver_multisig: anchor_pubkey(resolver_multisig),
            // Protocol-default dispute window applied to markets created WITHOUT
            // an explicit per-market override. Every market in this suite passes
            // its own `dispute_window_slots` to `create_market`, so this default
            // is never the value in force on the test markets — it only has to be
            // a valid non-zero window to satisfy the IX's ZeroDisputeWindow guard.
            default_dispute_window_slots: CONFIG_DEFAULT_DISPUTE_WINDOW,
            // Transparency metadata for the resolver multisig (1-of-N here; the
            // Squads vault enforces its own threshold). Must be 1..=MAX_PUBLISHERS.
            resolver_threshold: 1,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_create_market_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    market_id: u64,
    streamer_ref: [u8; 32],
    metric: u8,
    target: u64,
    resolution_root: [u8; 32],
    resolution_root_seq: u64,
    resolve_deadline_slot: u64,
    dispute_window_slots: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::CreateMarket {
            config,
            market,
            admin,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::CreateMarket {
            market_id,
            streamer_ref,
            metric,
            target,
            resolution_root,
            resolution_root_seq,
            resolve_deadline_slot,
            dispute_window_slots,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_initialize_market_tokens_ix(
    payer: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    mint_authority: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::InitializeMarketTokens {
            payer,
            config,
            market,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            mint_authority,
            outcome_token_program: spl_token_2022::id(),
            usdc_token_program: spl_token_2022::id(),
            system_program: system_program::ID,
            rent: solana_sdk::sysvar::rent::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializeMarketTokens {}.data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_mint_complete_set_ix(
    depositor: LegacyPubkey,
    market: LegacyPubkey,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    depositor_usdc: LegacyPubkey,
    depositor_yes: LegacyPubkey,
    depositor_no: LegacyPubkey,
    mint_authority: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::MintCompleteSet {
            depositor,
            market,
            config,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            depositor_usdc,
            depositor_yes,
            depositor_no,
            mint_authority,
            outcome_token_program: spl_token_2022::id(),
            usdc_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::MintCompleteSet { amount }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_redeem_complete_set_ix(
    redeemer: LegacyPubkey,
    market: LegacyPubkey,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    redeemer_usdc: LegacyPubkey,
    redeemer_yes: LegacyPubkey,
    redeemer_no: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::RedeemCompleteSet {
            redeemer,
            market,
            config,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            redeemer_usdc,
            redeemer_yes,
            redeemer_no,
            outcome_token_program: spl_token_2022::id(),
            usdc_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::RedeemCompleteSet { amount }.data(),
    }
}

fn build_initialize_attention_root_config_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    root_config: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::InitializeAttentionRootConfig {
            admin,
            config,
            root_config,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializeAttentionRootConfig {}.data(),
    }
}

fn build_add_publisher_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    publisher: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::AdminConfig { admin, config }.to_account_metas(None),
        data: markets_ix::AddPublisher {
            publisher: anchor_pubkey(publisher),
        }
        .data(),
    }
}

fn build_remove_publisher_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    publisher: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::AdminConfig { admin, config }.to_account_metas(None),
        data: markets_ix::RemovePublisher {
            publisher: anchor_pubkey(publisher),
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_publish_attention_root_ix(
    publisher: LegacyPubkey,
    config: LegacyPubkey,
    root_config: LegacyPubkey,
    attention_root: LegacyPubkey,
    window_id: u64,
    merkle_root: [u8; 32],
    leaf_count: u32,
    schema_version: u8,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::PublishAttentionRoot {
            publisher,
            config,
            root_config,
            attention_root,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::PublishAttentionRoot {
            window_id,
            merkle_root,
            leaf_count,
            schema_version,
        }
        .data(),
    }
}

fn build_resolve_market_ix(
    publisher: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    window_id: u64,
    observed_value: u64,
    outcome: u8,
    proof: Vec<[u8; 32]>,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::ResolveMarket {
            publisher,
            config,
            market,
        }
        .to_account_metas(None),
        data: markets_ix::ResolveMarket {
            window_id,
            observed_value,
            outcome,
            proof,
        }
        .data(),
    }
}

fn build_extend_dispute_window_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::ExtendDisputeWindow {
            admin,
            config,
            market,
        }
        .to_account_metas(None),
        data: markets_ix::ExtendDisputeWindow {}.data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_settle_ix(
    settler: LegacyPubkey,
    market: LegacyPubkey,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    settler_usdc: LegacyPubkey,
    settler_yes: LegacyPubkey,
    settler_no: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::Settle {
            settler,
            market,
            config,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            settler_usdc,
            settler_yes,
            settler_no,
            outcome_token_program: spl_token_2022::id(),
            usdc_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::Settle { amount }.data(),
    }
}

fn build_resolve_override_ix(
    resolver_multisig: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    new_outcome: u8,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::ResolveOverride {
            resolver_multisig,
            config,
            market,
        }
        .to_account_metas(None),
        data: markets_ix::ResolveOverride { new_outcome }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_sweep_residual_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    recipient: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::SweepResidual {
            admin,
            config,
            market,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            recipient,
            usdc_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::SweepResidual {}.data(),
    }
}

fn build_close_market_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    market: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    rent_recipient: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::CloseMarket {
            admin,
            config,
            market,
            yes_mint,
            no_mint,
            vault,
            rent_recipient,
        }
        .to_account_metas(None),
        data: markets_ix::CloseMarket {}.data(),
    }
}

fn markets_error_code(error: MarketsError) -> u32 {
    ERROR_CODE_OFFSET + error as u32
}

fn assert_markets_error(
    result: Result<TransactionMetadata, FailedTransactionMetadata>,
    error: MarketsError,
) {
    let failure = result.expect_err("expected transaction to fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(0, InstructionError::Custom(markets_error_code(error)),),
        "expected custom error {error:?} ({})",
        markets_error_code(error),
    );
}

// ─── merkle fixtures (conventions v1) ──────────────────────────────────────────

/// A market's resolution binding fields (baked into every leaf for that market).
const STREAMER_REF: [u8; 32] = [7u8; 32];
const METRIC: u8 = MarketMetric::AVG_VIEWERS; // 0
const OBSERVED_VALUE: u64 = 14_752;
/// The second leaf in the 2-leaf tree carries a DIFFERENT streamer/value (a
/// sibling market in the same window) — it only needs to be a distinct hash.
const SIBLING_STREAMER_REF: [u8; 32] = [0x99u8; 32];
const SIBLING_VALUE: u64 = 9_001;

/// Build the canonical 2-leaf resolution tree for `market_id` under the LOCKED
/// markets v1 convention. Leaf A is the market's own resolution leaf (outcome =
/// `outcome_a`); leaf B is a sibling leaf. Returns `(root, proof_for_a)` where
/// `proof_for_a == [hash(B)]`. The market's `resolution_root` is set to `root`,
/// so a `resolve_market(window_id, OBSERVED_VALUE, outcome_a, proof_for_a)`
/// folds back to the snapshot and ACCEPTS.
fn markets_two_leaf_tree(
    market_id: u64,
    window_id: u64,
    outcome_a: u8,
) -> ([u8; 32], Vec<[u8; 32]>) {
    let leaf_a = MarketsResolutionLeafV1::new(
        market_id,
        STREAMER_REF,
        window_id,
        METRIC,
        OBSERVED_VALUE,
        outcome_a,
    );
    let leaf_b = MarketsResolutionLeafV1::new(
        market_id,
        SIBLING_STREAMER_REF,
        window_id,
        METRIC,
        SIBLING_VALUE,
        resolution::outcome::NO,
    );
    let ha = leaf_a.hash();
    let hb = leaf_b.hash();
    let root = markets_resolution_node_hash_v1(&ha, &hb);
    // Cross-check: the convention's fold must reproduce `root` for proof [hb].
    debug_assert_eq!(compute_root_from_proof(ha, &[hb]), root);
    (root, vec![hb])
}

/// keccak(domain || canonical_bytes) — used to forge a leaf hash under the WRONG
/// (rails) leaf domain for Gate A case 2.
fn leaf_hash_with_domain(leaf: &MarketsResolutionLeafV1, domain: &[u8]) -> [u8; 32] {
    keccak::hashv(&[domain, leaf.canonical_bytes().as_ref()]).to_bytes()
}

/// Sorted-pair node hash under an ARBITRARY domain — used to forge a tree under
/// the WRONG (rails) node domain for Gate A case 1.
fn node_hash_with_domain(domain: &[u8], left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (a, b) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    keccak::hashv(&[domain, a.as_ref(), b.as_ref()]).to_bytes()
}

// ─── funded-market fixture (parameterized over root + dispute window) ──────────

struct Fixture {
    svm: LiteSVM,
    admin: Keypair,
    depositor: Keypair,
    publisher: Keypair,
    resolver_multisig: Keypair,
    config: LegacyPubkey,
    root_config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    market: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    depositor_usdc: LegacyPubkey,
    depositor_yes: LegacyPubkey,
    depositor_no: LegacyPubkey,
}

/// LiteSVM boots at slot 0; any large constant is comfortably in the future for
/// the `resolve_deadline_slot > current slot` guard.
fn future_deadline_slot() -> u64 {
    1_000_000
}

/// Boot the SVM, init config (with a distinct `resolver_multisig`), init the
/// attention-root-config singleton, allow-list a publisher, mint a fee-free
/// Token-2022 "USDC", fund the depositor, `create_market` (market_id 0) with the
/// caller-chosen `resolution_root` + `dispute_window_slots` + `deadline`, init
/// tokens, and mint a `SET_AMOUNT` complete set so the vault is funded.
///
/// Post-setup invariant: `vault == yes_supply == no_supply == SET_AMOUNT`,
/// depositor holds `SET_AMOUNT` YES + `SET_AMOUNT` NO and
/// `DEPOSITOR_USDC_FUNDING - SET_AMOUNT` USDC.
fn setup_funded(resolution_root: [u8; 32], dispute_window_slots: u64, deadline: u64) -> Fixture {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");
    load_token_2022_program(&mut svm);
    load_associated_token_program(&mut svm);

    let admin = Keypair::new();
    let depositor = Keypair::new();
    let publisher = Keypair::new();
    let resolver_multisig = Keypair::new();
    let usdc_mint_kp = Keypair::new();
    let usdc_mint_authority = Keypair::new();
    for kp in [
        &admin,
        &depositor,
        &publisher,
        &resolver_multisig,
        &usdc_mint_authority,
    ] {
        svm.airdrop(&kp.pubkey(), 100_000_000_000)
            .expect("airdrop signer");
    }

    let (config, _config_bump) = markets_config_pda();
    let (root_config, _rc_bump) = attention_root_config_pda();
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);

    // 1) config — resolver_multisig is DISTINCT from admin (resolve/override sep).
    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_markets_config_ix(
            legacy_from_signer(&admin),
            config,
            usdc_mint,
            legacy_from_signer(&resolver_multisig),
        )],
    );

    // 2) attention-root-config singleton + allow-list the publisher.
    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_attention_root_config_ix(
            legacy_from_signer(&admin),
            config,
            root_config,
        )],
    );
    send_tx(
        &mut svm,
        &[&admin],
        &[build_add_publisher_ix(
            legacy_from_signer(&admin),
            config,
            legacy_from_signer(&publisher),
        )],
    );

    // 3) "USDC" mint + fund depositor's USDC ATA.
    create_plain_token_2022_mint(
        &mut svm,
        &admin,
        &usdc_mint_kp,
        &legacy_from_signer(&usdc_mint_authority),
    );
    let depositor_usdc = create_ata(
        &mut svm,
        &depositor,
        &legacy_from_signer(&depositor),
        &usdc_mint,
    );
    mint_token_2022(
        &mut svm,
        &usdc_mint_authority,
        &usdc_mint,
        &depositor_usdc,
        DEPOSITOR_USDC_FUNDING,
    );

    // 4) create_market (market_id 0) with the caller's root + dispute window.
    let (market, _market_bump) = market_pda(MARKET_ID);
    send_tx(
        &mut svm,
        &[&admin],
        &[build_create_market_ix(
            legacy_from_signer(&admin),
            config,
            market,
            MARKET_ID,
            STREAMER_REF,
            METRIC,
            1_000,
            resolution_root,
            42,
            deadline,
            dispute_window_slots,
        )],
    );

    // 5) initialize_market_tokens.
    let (yes_mint, _) = yes_mint_pda(MARKET_ID);
    let (no_mint, _) = no_mint_pda(MARKET_ID);
    let (vault, _) = vault_pda(MARKET_ID);
    let (mint_authority, _) = mint_auth_pda(MARKET_ID);
    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_market_tokens_ix(
            legacy_from_signer(&admin),
            config,
            market,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            mint_authority,
        )],
    );

    // 6) depositor outcome ATAs + mint a complete set (funds the vault).
    let depositor_yes = create_ata(
        &mut svm,
        &depositor,
        &legacy_from_signer(&depositor),
        &yes_mint,
    );
    let depositor_no = create_ata(
        &mut svm,
        &depositor,
        &legacy_from_signer(&depositor),
        &no_mint,
    );
    send_tx(
        &mut svm,
        &[&depositor],
        &[build_mint_complete_set_ix(
            legacy_from_signer(&depositor),
            market,
            config,
            usdc_mint,
            yes_mint,
            no_mint,
            vault,
            depositor_usdc,
            depositor_yes,
            depositor_no,
            mint_authority,
            SET_AMOUNT,
        )],
    );

    // Post-setup solvency invariant (the Phase-1 rail, re-asserted as the base
    // state every resolution test builds on).
    assert_eq!(read_token_balance(&svm, &vault), SET_AMOUNT, "vault funded");
    assert_eq!(read_mint_supply(&svm, &yes_mint), SET_AMOUNT, "yes supply");
    assert_eq!(read_mint_supply(&svm, &no_mint), SET_AMOUNT, "no supply");

    Fixture {
        svm,
        admin,
        depositor,
        publisher,
        resolver_multisig,
        config,
        root_config,
        usdc_mint,
        market,
        yes_mint,
        no_mint,
        vault,
        depositor_usdc,
        depositor_yes,
        depositor_no,
    }
}

impl Fixture {
    /// Resolve the market YES against a freshly-built valid 2-leaf tree. Panics
    /// on failure (used by tests that need a resolved market as a precondition).
    /// `dispute_window_slots` was set at create-time; the caller warps the clock
    /// as needed before settling.
    fn resolve_yes(&mut self) {
        let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
        // Sanity: the market we resolve must have been created with this root.
        let market: Market = read_anchor_account(&self.svm, &self.market);
        assert_eq!(
            market.resolution_root, root,
            "test bug: market root != freshly-built tree root"
        );
        send_tx(
            &mut self.svm,
            &[&self.publisher],
            &[build_resolve_market_ix(
                legacy_from_signer(&self.publisher),
                self.config,
                self.market,
                WINDOW_ID,
                OBSERVED_VALUE,
                resolution::outcome::YES,
                proof,
            )],
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// GATE A — §4 merkle-rejection battery through `resolve_market`.
//
// Each case crafts a (root, proof) pair and runs the FULL instruction. Only the
// last case ACCEPTS; every malformed/wrong-domain/tampered proof is REJECTED
// with a typed error. Cases 1-2 are the M-04/CH-3 silent-failure kill switches.
// ════════════════════════════════════════════════════════════════════════════

/// GATE A — case 1: a tree built under the WRONG (rails) NODE domain is REJECTED.
///
/// The committed snapshot root is the rails-node-hash of the two markets leaves.
/// The on-chain verifier rebuilds the leaf correctly (markets leaf domain) but
/// folds with the markets NODE domain, producing a different root → the fold can
/// never equal the rails-domain snapshot → InvalidMerkleProof. A wrong-node-
/// domain tree can NEVER silently verify.
#[test]
fn gate_a_case1_wrong_node_domain_rejected() {
    // Build both leaves the correct (markets) way, but hash the NODE under rails.
    let leaf_a = MarketsResolutionLeafV1::new(
        MARKET_ID,
        STREAMER_REF,
        WINDOW_ID,
        METRIC,
        OBSERVED_VALUE,
        resolution::outcome::YES,
    );
    let leaf_b = MarketsResolutionLeafV1::new(
        MARKET_ID,
        SIBLING_STREAMER_REF,
        WINDOW_ID,
        METRIC,
        SIBLING_VALUE,
        resolution::outcome::NO,
    );
    let ha = leaf_a.hash();
    let hb = leaf_b.hash();
    // Snapshot root built with the WRONG node domain.
    let wrong_domain_root = node_hash_with_domain(RAILS_NODE_V1_DOMAIN, &ha, &hb);
    // Guard the premise: the wrong-domain root must differ from the correct one,
    // else the test proves nothing.
    assert_ne!(
        wrong_domain_root,
        markets_resolution_node_hash_v1(&ha, &hb),
        "rails and markets node domains must differ"
    );

    let mut f = setup_funded(
        wrong_domain_root,
        MIN_DISPUTE_WINDOW,
        future_deadline_slot(),
    );

    // Submit the honest sibling proof; on-chain fold uses the MARKETS node domain.
    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            vec![hb],
        )],
    );
    assert_markets_error(result, MarketsError::InvalidMerkleProof);

    // The market must remain UNRESOLVED (the kill switch held).
    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(!market.resolved, "wrong-node-domain proof must not resolve");
}

/// GATE A — case 2: a tree whose LEAF was hashed under the WRONG (rails) leaf
/// domain is REJECTED. The on-chain verifier rebuilds the leaf with the MARKETS
/// leaf domain, so its leaf hash differs from the one the snapshot was built on
/// → the fold misses the snapshot → InvalidMerkleProof.
#[test]
fn gate_a_case2_wrong_leaf_domain_rejected() {
    let leaf_a = MarketsResolutionLeafV1::new(
        MARKET_ID,
        STREAMER_REF,
        WINDOW_ID,
        METRIC,
        OBSERVED_VALUE,
        resolution::outcome::YES,
    );
    let leaf_b = MarketsResolutionLeafV1::new(
        MARKET_ID,
        SIBLING_STREAMER_REF,
        WINDOW_ID,
        METRIC,
        SIBLING_VALUE,
        resolution::outcome::NO,
    );
    // Forge BOTH leaf hashes under the rails LEAF domain, then node-hash them the
    // correct (markets) way — so only the leaf domain is wrong.
    let ha_wrong = leaf_hash_with_domain(&leaf_a, RAILS_LEAF_V1_DOMAIN);
    let hb_wrong = leaf_hash_with_domain(&leaf_b, RAILS_LEAF_V1_DOMAIN);
    assert_ne!(
        ha_wrong,
        leaf_a.hash(),
        "rails and markets leaf domains must differ"
    );
    let root = markets_resolution_node_hash_v1(&ha_wrong, &hb_wrong);

    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // The proof sibling is the rails-domain hash of leaf B. On-chain, the verifier
    // computes leaf A under the MARKETS domain (ha_correct != ha_wrong), folds with
    // hb_wrong → a root != snapshot.
    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            vec![hb_wrong],
        )],
    );
    assert_markets_error(result, MarketsError::InvalidMerkleProof);

    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(!market.resolved, "wrong-leaf-domain proof must not resolve");
}

/// GATE A — case 3: a proof longer than `MARKETS_MAX_PROOF_LEN` (16) is REJECTED
/// with ProofTooLong BEFORE the fold runs (conventions §3: cap FIRST).
#[test]
fn gate_a_case3_overlong_proof_rejected() {
    // Use a valid root so the ONLY thing wrong is proof length.
    let (root, _proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // 17 siblings (> 16). Content is irrelevant — the cap check trips first.
    let overlong: Vec<[u8; 32]> = (0..17u8).map(|i| [i; 32]).collect();
    assert_eq!(overlong.len(), resolution::MARKETS_MAX_PROOF_LEN + 1);

    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            overlong,
        )],
    );
    assert_markets_error(result, MarketsError::ProofTooLong);
}

/// GATE A — case 4: a tampered sibling (one byte flipped) is REJECTED. Valid
/// root, valid length, but the proof no longer folds to the snapshot →
/// InvalidMerkleProof.
#[test]
fn gate_a_case4_tampered_sibling_rejected() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    let mut tampered = proof.clone();
    tampered[0][0] ^= 0xFF; // flip a byte in the sibling
    assert_ne!(tampered[0], proof[0], "tamper must change the sibling");

    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            tampered,
        )],
    );
    assert_markets_error(result, MarketsError::InvalidMerkleProof);
}

/// GATE A — case 5: a snapshot built from a leaf carrying the WRONG market_id is
/// REJECTED. The on-chain verifier ALWAYS rebuilds the leaf from the market's OWN
/// market_id/streamer_ref/metric, so a snapshot committed over a foreign-market
/// leaf can never be reproduced → InvalidMerkleProof (the binding failure surfaces
/// as a fold miss before the explicit Leaf*Mismatch asserts are reached, since the
/// leaf is built from the market's own fields).
#[test]
fn gate_a_case5_wrong_market_leaf_rejected() {
    let foreign_market_id: u64 = 999;
    // Build a tree whose leaf A binds the WRONG market_id.
    let (root, proof) =
        markets_two_leaf_tree(foreign_market_id, WINDOW_ID, resolution::outcome::YES);
    // Create the REAL market (id 0) with that foreign-market root.
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    assert_markets_error(result, MarketsError::InvalidMerkleProof);

    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(!market.resolved, "wrong-market-leaf proof must not resolve");
}

/// GATE A — case 6: a malformed proof (garbage sibling against a valid root) is
/// REJECTED. Sorted-pair hashing makes ordering irrelevant, so "malformed" here
/// is a junk sibling that simply does not belong to the tree → the fold misses
/// the snapshot → InvalidMerkleProof.
#[test]
fn gate_a_case6_malformed_proof_rejected() {
    let (root, _proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // A junk sibling that is not hash(leafB).
    let junk = vec![[0xABu8; 32]];

    let result = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            junk,
        )],
    );
    assert_markets_error(result, MarketsError::InvalidMerkleProof);
}

/// GATE A — case 7: the byte-correct proof against the H-01 snapshot ACCEPTS.
/// The market resolves YES, the dispute window starts, and the outcome is fixed.
#[test]
fn gate_a_case7_valid_proof_accepted() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(market.resolved, "valid proof must resolve");
    assert_eq!(
        market.outcome,
        resolution::outcome::YES,
        "outcome fixed to YES"
    );
    // resolved_at_slot is 0 (litesvm boots at slot 0); settle_unlock is +window.
    assert_eq!(
        market.settle_unlock_slot,
        market.resolved_at_slot + MIN_DISPUTE_WINDOW,
        "dispute window started"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// GATE B — post-resolution solvency across settle.
//
// vault.amount >= winning_supply holds after every partial settle, and the vault
// drains to EXACTLY 0 on the final settle (audit MR-1 lockstep).
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn gate_b_settle_solvency() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Resolve YES (the depositor's YES is now the winning side).
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    // H-03: warp past the MIN_DISPUTE_WINDOW before settling.
    let market: Market = read_anchor_account(&f.svm, &f.market);
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);

    // Baseline: vault == winning(yes) supply == SET_AMOUNT.
    assert_eq!(read_token_balance(&f.svm, &f.vault), SET_AMOUNT);
    assert_eq!(read_mint_supply(&f.svm, &f.yes_mint), SET_AMOUNT);

    let usdc_before = read_token_balance(&f.svm, &f.depositor_usdc);

    // Partial settle #1: 400 USDC.
    let part1: u64 = 400_000_000;
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            part1,
        )],
    );
    let vault_after1 = read_token_balance(&f.svm, &f.vault);
    let yes_after1 = read_mint_supply(&f.svm, &f.yes_mint);
    assert_eq!(vault_after1, SET_AMOUNT - part1, "vault debited by settle");
    assert_eq!(yes_after1, SET_AMOUNT - part1, "winning supply burned 1:1");
    assert!(
        vault_after1 >= yes_after1,
        "MR-1: vault >= winning_supply after partial settle #1 ({vault_after1} >= {yes_after1})"
    );

    // Partial settle #2: 350 USDC.
    let part2: u64 = 350_000_000;
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            part2,
        )],
    );
    let vault_after2 = read_token_balance(&f.svm, &f.vault);
    let yes_after2 = read_mint_supply(&f.svm, &f.yes_mint);
    assert_eq!(vault_after2, SET_AMOUNT - part1 - part2);
    assert_eq!(yes_after2, SET_AMOUNT - part1 - part2);
    assert!(
        vault_after2 >= yes_after2,
        "MR-1: vault >= winning_supply after partial settle #2 ({vault_after2} >= {yes_after2})"
    );

    // Final settle: the remaining 250 USDC. Vault must drain to EXACTLY 0.
    let remaining = SET_AMOUNT - part1 - part2;
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            remaining,
        )],
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        0,
        "vault drains to exactly 0 on final settle"
    );
    assert_eq!(
        read_mint_supply(&f.svm, &f.yes_mint),
        0,
        "all winning supply burned"
    );
    // The depositor recovered the full SET_AMOUNT in USDC across the settles.
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_usdc),
        usdc_before + SET_AMOUNT,
        "settler made whole 1:1"
    );

    // The losing (NO) supply is untouched and has no claim on the (empty) vault.
    assert_eq!(
        read_mint_supply(&f.svm, &f.no_mint),
        SET_AMOUNT,
        "losing supply not burned by settle"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// GATE C — never-resolved recovery.
//
// A market that is never resolved (deadline passed, no resolve_market) recovers
// full collateral 1:1 via redeem_complete_set. Replaces the byte-poke hack.
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn gate_c_never_resolved_recovery() {
    // Deadline = 500 so we can warp past it. Root is the sentinel (never used —
    // we never resolve). dispute_window irrelevant.
    let mut f = setup_funded([9u8; 32], MIN_DISPUTE_WINDOW, 500);

    // Warp PAST the resolution deadline: the market is now in never-resolved
    // recovery. A late resolve would be rejected (ResolutionDeadlinePassed); the
    // recovery path is redeem_complete_set, which only needs !resolved + balances.
    f.svm.warp_to_slot(600);

    let usdc_before = read_token_balance(&f.svm, &f.depositor_usdc);
    assert_eq!(read_token_balance(&f.svm, &f.vault), SET_AMOUNT);

    // Redeem the full complete set 1:1.
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_redeem_complete_set_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );

    // Full collateral recovered; vault drained; both supplies back to 0.
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_usdc),
        usdc_before + SET_AMOUNT,
        "redeemer made whole 1:1 on never-resolved recovery"
    );
    assert_eq!(read_token_balance(&f.svm, &f.vault), 0, "vault drained");
    assert_eq!(read_mint_supply(&f.svm, &f.yes_mint), 0, "yes burned");
    assert_eq!(read_mint_supply(&f.svm, &f.no_mint), 0, "no burned");

    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(!market.resolved, "market was never resolved");
}

// ════════════════════════════════════════════════════════════════════════════
// FUNCTIONAL §9.1-§9.8
// ════════════════════════════════════════════════════════════════════════════

/// §9.1 — publish_attention_root happy path + one-root-per-window idempotency.
#[test]
fn func_publish_root_happy_and_idempotent() {
    let (root, _) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());
    let (attention_root, _) = attention_root_pda(WINDOW_ID);

    // Publish a (separate, discoverability-only) root for the window.
    let published_root = [0x42u8; 32];
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_publish_attention_root_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.root_config,
            attention_root,
            WINDOW_ID,
            published_root,
            2,
            MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
        )],
    );

    let stored: AttentionRoot = read_anchor_account(&f.svm, &attention_root);
    assert_eq!(stored.window_id, WINDOW_ID);
    assert_eq!(stored.merkle_root, published_root);
    assert_eq!(stored.leaf_count, 2);
    assert_eq!(stored.schema_version, MARKETS_RESOLUTION_LEAF_SCHEMA_V1);
    assert_eq!(
        stored.publisher.to_bytes(),
        f.publisher.pubkey().to_bytes(),
        "publisher recorded"
    );

    let rc: AttentionRootConfig = read_anchor_account(&f.svm, &f.root_config);
    assert_eq!(rc.last_published_seq, 1, "publish seq bumped");

    // Re-publishing the SAME window must fail (init on existing PDA). Expire the
    // blockhash so the second tx is distinct.
    f.svm.expire_blockhash();
    let dup = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_publish_attention_root_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.root_config,
            attention_root,
            WINDOW_ID,
            [0x43u8; 32],
            2,
            MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
        )],
    );
    assert!(dup.is_err(), "second publish for same window must fail");
}

/// §9.1b — publish_attention_root rejects a bad schema version and a zero root.
#[test]
fn func_publish_root_validation() {
    let (root, _) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());
    let (attention_root, _) = attention_root_pda(WINDOW_ID);

    // Zero root → ZeroResolutionRoot.
    let zero = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_publish_attention_root_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.root_config,
            attention_root,
            WINDOW_ID,
            [0u8; 32],
            1,
            MARKETS_RESOLUTION_LEAF_SCHEMA_V1,
        )],
    );
    assert_markets_error(zero, MarketsError::ZeroResolutionRoot);

    // Wrong schema version → InvalidLeafSchemaVersion.
    f.svm.expire_blockhash();
    let bad_schema = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_publish_attention_root_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.root_config,
            attention_root,
            WINDOW_ID,
            [0x42u8; 32],
            1,
            MARKETS_RESOLUTION_LEAF_SCHEMA_V1 + 7,
        )],
    );
    assert_markets_error(bad_schema, MarketsError::InvalidLeafSchemaVersion);
}

/// §9.2 — settle is refused while the dispute window is still open
/// (DisputeWindowOpen), then succeeds once the window closes.
#[test]
fn func_settle_dispute_window_enforced() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    // Non-zero window so settle is initially illegal at slot 0.
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert_eq!(
        market.settle_unlock_slot,
        market.resolved_at_slot + MIN_DISPUTE_WINDOW
    );

    // Window still open (slot 0 < unlock 100) → settle rejected.
    let early = try_send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            100_000_000,
        )],
    );
    assert_markets_error(early, MarketsError::DisputeWindowOpen);

    // Warp past the unlock slot → settle now legal.
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            100_000_000,
        )],
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        SET_AMOUNT - 100_000_000
    );
}

/// §9.3 — an INVALID resolution refuses `settle` (MarketInvalidUseRedeem) and
/// routes both sides to `redeem_complete_set` instead. INVALID is set via the
/// multisig override (resolve_market commits NO/YES from the proof; INVALID is
/// the override escape hatch).
#[test]
fn func_invalid_routes_to_redeem() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Resolve YES first...
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    // ...then override to INVALID (pre-settle, multisig signer != admin).
    send_tx(
        &mut f.svm,
        &[&f.resolver_multisig],
        &[build_resolve_override_ix(
            legacy_from_signer(&f.resolver_multisig),
            f.config,
            f.market,
            resolution::outcome::INVALID,
        )],
    );
    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert_eq!(market.outcome, resolution::outcome::INVALID);

    // Warp past the restarted re-dispute window so the settle guard reaches the
    // INVALID check (otherwise DisputeWindowOpen trips first).
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);
    f.svm.expire_blockhash();

    // settle is refused for INVALID.
    let bad = try_send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            100_000_000,
        )],
    );
    assert_markets_error(bad, MarketsError::MarketInvalidUseRedeem);

    // redeem_complete_set recovers both sides 1:1 (note: redeem requires
    // !resolved; an INVALID market IS resolved, so the recovery path here is the
    // INVALID-specific one — confirm the program's intended INVALID recovery).
    let usdc_before = read_token_balance(&f.svm, &f.depositor_usdc);
    let redeem = try_send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_redeem_complete_set_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );
    // Audit C-01 fix: redeem_complete_set STAYS OPEN for an INVALID market, so
    // the depositor recovers collateral 1:1. (Pre-fix this was refused with
    // MarketResolved, and since settle also refused INVALID, 100% of collateral
    // was permanently locked — the C-01 Critical.) This is now a HARD assertion:
    // redeem MUST succeed and the depositor MUST be made whole.
    redeem.expect("C-01: INVALID redeem must succeed (collateral recoverable 1:1)");
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_usdc),
        usdc_before + SET_AMOUNT,
        "C-01: INVALID redeem made whole 1:1"
    );
}

/// Audit C-03 PoC: `resolve_override` is FORBIDDEN once any settlement has
/// occurred. Pre-fix, an override could flip the outcome AFTER winners settled
/// (draining the vault) without resetting `settled_supply`, so the new winners
/// would settle against an already-drained vault. The fix gates override on
/// `settled_supply == 0`.
#[test]
fn func_c03_override_after_settle_forbidden() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Resolve YES.
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    // Warp strictly past settle_unlock_slot so settle can proceed (DC-3: settle
    // requires clock > settle_unlock_slot; the exact boundary belongs to override).
    let market: Market = read_anchor_account(&f.svm, &f.market);
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );
    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert!(market.settled_supply > 0, "a settle occurred");

    // Re-open the override window by extending the dispute window (still on first
    // extension; now settle_unlock_slot > current slot so override is available again).
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_extend_dispute_window_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
        )],
    );

    // Override must be REFUSED because settled_supply > 0 (C-03 guard), even though
    // the override window is technically open again after the extension.
    let bad = try_send_tx(
        &mut f.svm,
        &[&f.resolver_multisig],
        &[build_resolve_override_ix(
            legacy_from_signer(&f.resolver_multisig),
            f.config,
            f.market,
            resolution::outcome::NO,
        )],
    );
    assert_markets_error(bad, MarketsError::OverrideAfterSettle);
}

/// §9.4 — resolve_override authorization: wrong signer is rejected
/// (MultisigThresholdNotMet); the admin acting as the multisig is rejected
/// (MultisigMemberIsAdmin); the configured multisig succeeds and changes outcome.
#[test]
fn func_override_authorization() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    // (a) a random signer (not the configured multisig) → MultisigThresholdNotMet.
    let imposter = Keypair::new();
    f.svm
        .airdrop(&imposter.pubkey(), 10_000_000_000)
        .expect("airdrop imposter");
    let wrong = try_send_tx(
        &mut f.svm,
        &[&imposter],
        &[build_resolve_override_ix(
            legacy_from_signer(&imposter),
            f.config,
            f.market,
            resolution::outcome::NO,
        )],
    );
    assert_markets_error(wrong, MarketsError::MultisigThresholdNotMet);

    // (b) the admin acting as the multisig → MultisigMemberIsAdmin. (Only reachable
    // if admin == resolver_multisig, which our config forbids — so we cannot send
    // this directly. Instead, assert the config separation that makes it
    // impossible: admin != resolver_multisig.)
    let cfg: MarketsConfig = read_anchor_account(&f.svm, &f.config);
    assert_ne!(
        cfg.admin.to_bytes(),
        cfg.resolver_multisig.to_bytes(),
        "resolve/override separation: admin must differ from resolver_multisig"
    );

    // (c) the configured multisig succeeds and changes the outcome YES → NO.
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.resolver_multisig],
        &[build_resolve_override_ix(
            legacy_from_signer(&f.resolver_multisig),
            f.config,
            f.market,
            resolution::outcome::NO,
        )],
    );
    let market: Market = read_anchor_account(&f.svm, &f.market);
    assert_eq!(
        market.outcome,
        resolution::outcome::NO,
        "override flipped outcome"
    );
}

/// §9.5 — extend_dispute_window is one-shot: the first extension succeeds, the
/// second is refused (DisputeAlreadyExtended).
#[test]
fn func_extend_dispute_window_once() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    let before: Market = read_anchor_account(&f.svm, &f.market);

    // First extension: +dispute_window_slots.
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_extend_dispute_window_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
        )],
    );
    let after: Market = read_anchor_account(&f.svm, &f.market);
    assert_eq!(
        after.settle_unlock_slot,
        before.settle_unlock_slot + before.dispute_window_slots,
        "first extension pushes unlock out by one window"
    );
    assert!(after.dispute_extended, "dispute_extended flag set");

    // Second extension: refused.
    f.svm.expire_blockhash();
    let again = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_extend_dispute_window_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
        )],
    );
    assert_markets_error(again, MarketsError::DisputeAlreadyExtended);
}

/// §9.6 — sweep_residual supply guard: refused while winning supply is live
/// (SupplyNotZero); succeeds once the winning side is fully settled (sweeps the
/// remaining vault dust to a treasury recipient).
#[test]
fn func_sweep_residual_supply_guard() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    // Treasury recipient ATA (owned by admin).
    let treasury = create_ata(
        &mut f.svm,
        &f.admin,
        &legacy_from_signer(&f.admin),
        &f.usdc_mint,
    );

    // Winning (YES) supply is live → sweep refused (supply guard, not window).
    let early = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_sweep_residual_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            treasury,
        )],
    );
    assert_markets_error(early, MarketsError::SupplyNotZero);

    // H-03: warp past the dispute window before settling.
    let market: Market = read_anchor_account(&f.svm, &f.market);
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);

    // Settle the FULL winning side (burns all YES, drains vault to 0).
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );
    assert_eq!(read_mint_supply(&f.svm, &f.yes_mint), 0);
    assert_eq!(read_token_balance(&f.svm, &f.vault), 0);

    // Sweep now succeeds (vault is 0, so dust transfer is a no-op but the guard
    // passes and the event emits).
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_sweep_residual_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            treasury,
        )],
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        0,
        "vault remains drained after sweep"
    );
}

/// §9.7 — publisher allow-list is enforced by resolve_market: a signer NOT on the
/// allow-list is rejected (UnauthorizedPublisher), and remove_publisher revokes a
/// previously-allowed publisher.
#[test]
fn func_publisher_allowlist_enforced() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // A non-allowlisted signer cannot resolve.
    let outsider = Keypair::new();
    f.svm
        .airdrop(&outsider.pubkey(), 10_000_000_000)
        .expect("airdrop outsider");
    let denied = try_send_tx(
        &mut f.svm,
        &[&outsider],
        &[build_resolve_market_ix(
            legacy_from_signer(&outsider),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof.clone(),
        )],
    );
    assert_markets_error(denied, MarketsError::UnauthorizedPublisher);

    // Remove the allow-listed publisher; now even it is denied.
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_remove_publisher_ix(
            legacy_from_signer(&f.admin),
            f.config,
            legacy_from_signer(&f.publisher),
        )],
    );
    let cfg: MarketsConfig = read_anchor_account(&f.svm, &f.config);
    assert!(
        cfg.publisher_allowlist.is_empty(),
        "publisher removed from allow-list"
    );

    f.svm.expire_blockhash();
    let revoked = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    assert_markets_error(revoked, MarketsError::UnauthorizedPublisher);
}

/// §9.8 — allow-list management guards: duplicate add (PublisherAlreadyPresent),
/// remove-absent (PublisherNotFound), zero pubkey (InvalidPubkey).
#[test]
fn func_allowlist_management_guards() {
    let (root, _) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Duplicate add (publisher already added in setup) → PublisherAlreadyPresent.
    // Expire the blockhash first: `setup_funded` already sent a byte-identical
    // add_publisher tx, so without a fresh blockhash this duplicate has the same
    // signature and the runtime rejects it as AlreadyProcessed BEFORE the program
    // runs — masking the on-chain guard we are actually testing.
    f.svm.expire_blockhash();
    let dup = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_add_publisher_ix(
            legacy_from_signer(&f.admin),
            f.config,
            legacy_from_signer(&f.publisher),
        )],
    );
    assert_markets_error(dup, MarketsError::PublisherAlreadyPresent);

    // Remove a publisher that is not present → PublisherNotFound.
    f.svm.expire_blockhash();
    let absent = Keypair::new();
    let not_found = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_remove_publisher_ix(
            legacy_from_signer(&f.admin),
            f.config,
            legacy_from_signer(&absent),
        )],
    );
    assert_markets_error(not_found, MarketsError::PublisherNotFound);

    // Add the zero pubkey → InvalidPubkey.
    f.svm.expire_blockhash();
    let zero = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_add_publisher_ix(
            legacy_from_signer(&f.admin),
            f.config,
            LegacyPubkey::default(),
        )],
    );
    assert_markets_error(zero, MarketsError::InvalidPubkey);
}

/// Regression: resolve cannot run twice (MarketAlreadyResolved) and cannot run
/// after the deadline (ResolutionDeadlinePassed).
#[test]
fn func_resolve_lifecycle_guards() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Resolve once.
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof.clone(),
        )],
    );

    // Second resolve → MarketAlreadyResolved.
    f.svm.expire_blockhash();
    let again = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    assert_markets_error(again, MarketsError::MarketAlreadyResolved);
}

/// Regression: a market created with a deadline we can warp past rejects a late
/// resolve with ResolutionDeadlinePassed.
#[test]
fn func_resolve_after_deadline_rejected() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    // Deadline = 500 so we can warp to 600.
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, 500);
    f.svm.warp_to_slot(600);

    let late = try_send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    assert_markets_error(late, MarketsError::ResolutionDeadlinePassed);
}

/// Regression: settle with amount 0 → ZeroAmount.
#[test]
fn func_settle_zero_amount_rejected() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    // H-03: warp past the dispute window so the ZeroAmount check is reached.
    let market: Market = read_anchor_account(&f.svm, &f.market);
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);
    let zero = try_send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            0,
        )],
    );
    assert_markets_error(zero, MarketsError::ZeroAmount);
}

/// Sanity: the markets node domain constant is what the convention locks (a
/// drift here would silently break cross-repo proof verification). Mirrors the
/// in-crate golden check but at the integration boundary.
#[test]
fn func_node_domain_locked() {
    assert_eq!(
        MARKETS_RESOLUTION_NODE_V1_DOMAIN,
        b"wzrd-markets:attention-resolution-node:v1"
    );
}

// ─── Audit Phase 4 Low + DC-3 fixes ──────────────────────────────────────────

/// L-01: create_market rejects a deadline > now + MAX_MARKET_DURATION_SLOTS.
/// Uses market_id=1 (setup_funded consumed market_id=0, so config.next_market_id=1).
#[test]
fn func_l01_deadline_too_far_rejected() {
    let (root, _proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());
    let (market1, _) = market_pda(1);

    // u64::MAX is always beyond clock + MAX_MARKET_DURATION_SLOTS.
    f.svm.expire_blockhash();
    let bad = try_send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_create_market_ix(
            legacy_from_signer(&f.admin),
            f.config,
            market1,
            1,
            STREAMER_REF,
            METRIC,
            1_000,
            [0xab; 32],
            1,
            u64::MAX,
            MIN_DISPUTE_WINDOW,
        )],
    );
    assert_markets_error(bad, MarketsError::DeadlineTooFar);

    // A near-term deadline (1_000_000 << MAX_MARKET_DURATION_SLOTS) is accepted.
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_create_market_ix(
            legacy_from_signer(&f.admin),
            f.config,
            market1,
            1,
            STREAMER_REF,
            METRIC,
            1_000,
            [0xab; 32],
            1,
            future_deadline_slot(),
            MIN_DISPUTE_WINDOW,
        )],
    );
    let mkt1: Market = read_anchor_account(&f.svm, &market1);
    assert_eq!(mkt1.resolve_deadline_slot, future_deadline_slot());
}

/// L-02: resolve_market on a market whose tokens have not been initialized is
/// rejected with TokensNotInitialized (check fires before the merkle proof step).
/// Uses a minimal fresh SVM: config + publisher allowlist + create_market only
/// (no initialize_market_tokens, no USDC mint creation needed).
#[test]
fn func_l02_resolve_requires_tokens_initialized() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut svm2 = LiteSVM::new();
    load_wzrd_markets_program(&mut svm2).expect("load wzrd-markets program");
    load_token_2022_program(&mut svm2);
    load_associated_token_program(&mut svm2);

    let admin2 = Keypair::new();
    let resolver2 = Keypair::new();
    let publisher2 = Keypair::new();
    for kp in [&admin2, &resolver2, &publisher2] {
        svm2.airdrop(&kp.pubkey(), 100_000_000_000).unwrap();
    }

    let (config2, _) = markets_config_pda();
    let (market2, _) = market_pda(0);
    // InitializeMarketsConfig uses UncheckedAccount for usdc_mint and only checks
    // Token-2022 extensions when data.len() > 82; a non-existent account (0 bytes)
    // skips that check entirely. No mint creation needed for this test.
    let fake_usdc = LegacyPubkey::new_unique();

    send_tx(
        &mut svm2,
        &[&admin2],
        &[build_initialize_markets_config_ix(
            legacy_from_signer(&admin2),
            config2,
            fake_usdc,
            legacy_from_signer(&resolver2),
        )],
    );
    send_tx(
        &mut svm2,
        &[&admin2],
        &[build_add_publisher_ix(
            legacy_from_signer(&admin2),
            config2,
            legacy_from_signer(&publisher2),
        )],
    );
    // Create market but skip initialize_market_tokens — tokens_initialized stays false.
    send_tx(
        &mut svm2,
        &[&admin2],
        &[build_create_market_ix(
            legacy_from_signer(&admin2),
            config2,
            market2,
            0,
            STREAMER_REF,
            METRIC,
            OBSERVED_VALUE,
            root,
            1,
            future_deadline_slot(),
            MIN_DISPUTE_WINDOW,
        )],
    );
    let mkt2: Market = read_anchor_account(&svm2, &market2);
    assert!(!mkt2.tokens_initialized, "tokens not yet initialized");

    // resolve_market fires the L-02 guard (tokens_initialized) before the merkle
    // proof check, so even a valid proof returns TokensNotInitialized.
    let bad = try_send_tx(
        &mut svm2,
        &[&publisher2],
        &[build_resolve_market_ix(
            legacy_from_signer(&publisher2),
            config2,
            market2,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    assert_markets_error(bad, MarketsError::TokensNotInitialized);
}

/// DC-3: settle at exactly settle_unlock_slot is now rejected (must be strictly
/// after). The boundary slot belongs to resolve_override.
#[test]
fn func_dc3_settle_boundary_slot_rejected() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );
    let market: Market = read_anchor_account(&f.svm, &f.market);

    // Warp to EXACTLY settle_unlock_slot — settle must be refused (boundary slot
    // belongs to override, not settle).
    f.svm.warp_to_slot(market.settle_unlock_slot);
    f.svm.expire_blockhash();
    let at_boundary = try_send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );
    assert_markets_error(at_boundary, MarketsError::DisputeWindowOpen);

    // One slot later settle succeeds.
    f.svm.warp_to_slot(market.settle_unlock_slot + 1);
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.depositor],
        &[build_settle_ix(
            legacy_from_signer(&f.depositor),
            f.market,
            f.config,
            f.usdc_mint,
            f.yes_mint,
            f.no_mint,
            f.vault,
            f.depositor_usdc,
            f.depositor_yes,
            f.depositor_no,
            SET_AMOUNT,
        )],
    );
}

/// L-05: resolve_override resets dispute_extended so the new post-override
/// window can be extended once. Before the fix, the flag was sticky and the
/// single-extension allowance was consumed by any prior call to
/// extend_dispute_window, leaving the post-override window permanently locked.
#[test]
fn func_l05_override_resets_dispute_extended() {
    let (root, proof) = markets_two_leaf_tree(MARKET_ID, WINDOW_ID, resolution::outcome::YES);
    let mut f = setup_funded(root, MIN_DISPUTE_WINDOW, future_deadline_slot());

    // Resolve YES.
    send_tx(
        &mut f.svm,
        &[&f.publisher],
        &[build_resolve_market_ix(
            legacy_from_signer(&f.publisher),
            f.config,
            f.market,
            WINDOW_ID,
            OBSERVED_VALUE,
            resolution::outcome::YES,
            proof,
        )],
    );

    // Consume the extension allowance on the initial window.
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_extend_dispute_window_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
        )],
    );
    let after_extend: Market = read_anchor_account(&f.svm, &f.market);
    assert!(
        after_extend.dispute_extended,
        "flag set after first extension"
    );

    // Override (still within the window): dispute_extended MUST be cleared.
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.resolver_multisig],
        &[build_resolve_override_ix(
            legacy_from_signer(&f.resolver_multisig),
            f.config,
            f.market,
            resolution::outcome::NO,
        )],
    );
    let after_override: Market = read_anchor_account(&f.svm, &f.market);
    assert!(
        !after_override.dispute_extended,
        "L-05: override must clear dispute_extended"
    );

    // The new post-override window can now be extended once.
    f.svm.expire_blockhash();
    send_tx(
        &mut f.svm,
        &[&f.admin],
        &[build_extend_dispute_window_ix(
            legacy_from_signer(&f.admin),
            f.config,
            f.market,
        )],
    );
    let final_mkt: Market = read_anchor_account(&f.svm, &f.market);
    assert!(
        final_mkt.dispute_extended,
        "post-override extension consumed the allowance"
    );
}
