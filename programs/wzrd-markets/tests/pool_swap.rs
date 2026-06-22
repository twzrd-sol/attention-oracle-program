#![cfg(feature = "localtest")]
//! LiteSVM integration coverage for `wzrd-markets` Phase 2 — the CPMM pool
//! (`initialize_pool`), liquidity (`add_liquidity` / `remove_liquidity`), and the
//! moving-odds primitive (`swap`).
//!
//! THE ACCEPTANCE GATE is `arb_coherence_no_free_usdc`: a trader mints a complete
//! set for N USDC, round-trips it through the pool (swap one side, swap it back),
//! redeems back to USDC, and we assert the trader ends with `<= N` USDC (no free
//! money — only slippage/dust loss) AND the pool's constant-product `k =
//! yes_reserve * no_reserve` never DECREASES across any swap.
//!
//! THE PHANTOM-PAYOUT GUARD is `bounding_phase_solvency`: a swap that the virtual
//! floor would "price" as payable but that exceeds the pool's real output reserve
//! MUST revert with `InsufficientPoolLiquidity` — the pool can never pay tokens it
//! does not hold; the virtual floor shifts price, never payout solvency.
//!
//! Run with (the `.so` must be built first via cargo-build-sbf):
//!   cargo-build-sbf --manifest-path programs/wzrd-markets/Cargo.toml
//!   cargo test -p wzrd-markets --features localtest --test pool_swap -- --nocapture
//!
//! Reuses the Phase 1 `complete_set.rs` harness conventions (address conversion +
//! program load + tx send + token helpers). "USDC" is a fee-free Token-2022 mint.

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
    instruction::Instruction as LegacyInstruction, program_pack::Pack,
    pubkey::Pubkey as LegacyPubkey, system_instruction, system_program,
};
use solana_signer::Signer;
use solana_transaction::{Transaction, TransactionError};
use spl_token_2022::extension::StateWithExtensions;
use spl_token_2022::state::{Account as SplTokenAccount, Mint as SplMint};
use std::path::{Path, PathBuf};
use wzrd_markets::{
    accounts as markets_accounts, instruction as markets_ix,
    state::{
        MarketMetric, Pool, LP_MINT_SEED, MARKETS_CONFIG_SEED, MARKET_SEED, MINT_AUTH_SEED,
        NO_MINT_SEED, POOL_SEED, VAULT_SEED, YES_MINT_SEED,
    },
    MarketsError, SwapDirection, ID as WZRD_MARKETS_PROGRAM_ID,
};

const USDC_DECIMALS: u8 = 6;
const MARKET_ID: u64 = 0;

/// Virtual-liquidity floor seeded at pool init (bounding phase).
const VIRTUAL_LIQUIDITY: u64 = 100_000_000; // 100 units @ 6 decimals
/// Real liquidity each LP seeds (> V on both sides → graduates the bounding phase).
const LP_YES: u64 = 1_000_000_000; // 1000 units
const LP_NO: u64 = 1_000_000_000;
/// Generous USDC funding so conservation (not balance) is the only constraint.
const DEPOSITOR_USDC_FUNDING: u64 = 100_000_000_000;

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
/// Pool PDA: `[POOL_SEED, market.key()]`.
fn pool_pda(market: &LegacyPubkey) -> (LegacyPubkey, u8) {
    let (addr, bump) =
        Pubkey::find_program_address(&[POOL_SEED, &market.to_bytes()], &WZRD_MARKETS_PROGRAM_ID);
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}
/// LP mint PDA: `[LP_MINT_SEED, market.key()]`.
fn lp_mint_pda(market: &LegacyPubkey) -> (LegacyPubkey, u8) {
    let (addr, bump) = Pubkey::find_program_address(
        &[LP_MINT_SEED, &market.to_bytes()],
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

// ─── instruction builders ─────────────────────────────────────────────────────

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
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializeMarketsConfig {
            usdc_mint: anchor_pubkey(usdc_mint),
            resolver_multisig: anchor_pubkey(resolver_multisig),
            // Carved into MarketsConfig in Phase 3 (resolver allow-list config).
            // Phase 2 pool/swap tests don't exercise the dispute window; these
            // just satisfy the IX guards (window > 0, threshold in 1..=MAX).
            default_dispute_window_slots: 54_000,
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

#[allow(clippy::too_many_arguments)]
fn build_initialize_pool_ix(
    payer: LegacyPubkey,
    market: LegacyPubkey,
    pool: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    lp_mint: LegacyPubkey,
    pool_yes: LegacyPubkey,
    pool_no: LegacyPubkey,
    virtual_liquidity: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::InitializePool {
            payer,
            market,
            pool,
            yes_mint,
            no_mint,
            lp_mint,
            pool_yes,
            pool_no,
            outcome_token_program: spl_token_2022::id(),
            lp_token_program: spl_token_2022::id(),
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializePool { virtual_liquidity }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_add_liquidity_ix(
    provider: LegacyPubkey,
    market: LegacyPubkey,
    pool: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    lp_mint: LegacyPubkey,
    pool_yes: LegacyPubkey,
    pool_no: LegacyPubkey,
    provider_yes: LegacyPubkey,
    provider_no: LegacyPubkey,
    provider_lp: LegacyPubkey,
    max_yes: u64,
    max_no: u64,
    min_lp: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::AddLiquidity {
            provider,
            market,
            pool,
            yes_mint,
            no_mint,
            lp_mint,
            pool_yes,
            pool_no,
            provider_yes,
            provider_no,
            provider_lp,
            outcome_token_program: spl_token_2022::id(),
            lp_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::AddLiquidity {
            max_yes,
            max_no,
            min_lp,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_remove_liquidity_ix(
    provider: LegacyPubkey,
    market: LegacyPubkey,
    pool: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    lp_mint: LegacyPubkey,
    pool_yes: LegacyPubkey,
    pool_no: LegacyPubkey,
    provider_yes: LegacyPubkey,
    provider_no: LegacyPubkey,
    provider_lp: LegacyPubkey,
    lp_amount: u64,
    min_yes: u64,
    min_no: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::RemoveLiquidity {
            provider,
            market,
            pool,
            yes_mint,
            no_mint,
            lp_mint,
            pool_yes,
            pool_no,
            provider_yes,
            provider_no,
            provider_lp,
            outcome_token_program: spl_token_2022::id(),
            lp_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::RemoveLiquidity {
            lp_amount,
            min_yes,
            min_no,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_swap_ix(
    trader: LegacyPubkey,
    market: LegacyPubkey,
    pool: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    pool_yes: LegacyPubkey,
    pool_no: LegacyPubkey,
    trader_yes: LegacyPubkey,
    trader_no: LegacyPubkey,
    amount_in: u64,
    min_amount_out: u64,
    direction: u8,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::Swap {
            trader,
            market,
            pool,
            yes_mint,
            no_mint,
            pool_yes,
            pool_no,
            trader_yes,
            trader_no,
            outcome_token_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: markets_ix::Swap {
            amount_in,
            min_amount_out,
            direction,
        }
        .data(),
    }
}

// ─── fixture ──────────────────────────────────────────────────────────────────

struct Fixture {
    svm: LiteSVM,
    #[allow(dead_code)]
    admin: Keypair,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    usdc_mint_authority: Keypair,
    market: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    mint_authority: LegacyPubkey,
    // pool
    pool: LegacyPubkey,
    lp_mint: LegacyPubkey,
    pool_yes: LegacyPubkey,
    pool_no: LegacyPubkey,
}

fn future_deadline_slot() -> u64 {
    1_000_000
}

/// Boot the SVM, init config + a fee-free Token-2022 "USDC" mint, create a market,
/// initialize its tokens, and initialize the pool (bounding phase, V seeded). Does
/// NOT add real liquidity — tests choose whether/how much to seed.
fn setup_pool() -> Fixture {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");
    load_token_2022_program(&mut svm);
    load_associated_token_program(&mut svm);

    let admin = Keypair::new();
    let usdc_mint_kp = Keypair::new();
    let usdc_mint_authority = Keypair::new();
    svm.airdrop(&admin.pubkey(), 1_000_000_000_000)
        .expect("airdrop admin");
    svm.airdrop(&usdc_mint_authority.pubkey(), 100_000_000_000)
        .expect("airdrop usdc mint authority");

    let (config, _) = markets_config_pda();
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);
    let resolver_multisig = legacy_from_signer(&Keypair::new());

    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_markets_config_ix(
            legacy_from_signer(&admin),
            config,
            usdc_mint,
            resolver_multisig,
        )],
    );

    create_plain_token_2022_mint(
        &mut svm,
        &admin,
        &usdc_mint_kp,
        &legacy_from_signer(&usdc_mint_authority),
    );

    let (market, _) = market_pda(MARKET_ID);
    send_tx(
        &mut svm,
        &[&admin],
        &[build_create_market_ix(
            legacy_from_signer(&admin),
            config,
            market,
            MARKET_ID,
            [7u8; 32],
            MarketMetric::AVG_VIEWERS,
            1_000,
            [9u8; 32],
            42,
            future_deadline_slot(),
            100,
        )],
    );

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

    // initialize_pool (bounding phase active, V seeded)
    let (pool, _) = pool_pda(&market);
    let (lp_mint, _) = lp_mint_pda(&market);
    let pool_yes = derive_ata(&pool, &yes_mint);
    let pool_no = derive_ata(&pool, &no_mint);
    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_pool_ix(
            legacy_from_signer(&admin),
            market,
            pool,
            yes_mint,
            no_mint,
            lp_mint,
            pool_yes,
            pool_no,
            VIRTUAL_LIQUIDITY,
        )],
    );

    Fixture {
        svm,
        admin,
        config,
        usdc_mint,
        usdc_mint_authority,
        market,
        yes_mint,
        no_mint,
        vault,
        mint_authority,
        pool,
        lp_mint,
        pool_yes,
        pool_no,
    }
}

/// Create a fresh, funded actor with USDC + YES/NO/LP ATAs, holding a complete set
/// of `set_amount` (minted through the Phase 1 rail). Returns the actor + its ATAs.
struct Actor {
    kp: Keypair,
    usdc: LegacyPubkey,
    yes: LegacyPubkey,
    no: LegacyPubkey,
    lp: LegacyPubkey,
}

fn new_actor(f: &mut Fixture, usdc_funding: u64) -> Actor {
    let kp = Keypair::new();
    f.svm
        .airdrop(&kp.pubkey(), 100_000_000_000)
        .expect("airdrop actor");
    let owner = legacy_from_signer(&kp);
    let usdc = create_ata(&mut f.svm, &kp, &owner, &f.usdc_mint);
    mint_token_2022(
        &mut f.svm,
        &f.usdc_mint_authority,
        &f.usdc_mint,
        &usdc,
        usdc_funding,
    );
    let yes = create_ata(&mut f.svm, &kp, &owner, &f.yes_mint);
    let no = create_ata(&mut f.svm, &kp, &owner, &f.no_mint);
    let lp = create_ata(&mut f.svm, &kp, &owner, &f.lp_mint);
    Actor {
        kp,
        usdc,
        yes,
        no,
        lp,
    }
}

/// Mint a complete set of `amount` to the actor (deposits `amount` USDC).
fn mint_set(f: &mut Fixture, actor: &Actor, amount: u64) {
    let ix = build_mint_complete_set_ix(
        legacy_from_signer(&actor.kp),
        f.market,
        f.config,
        f.usdc_mint,
        f.yes_mint,
        f.no_mint,
        f.vault,
        actor.usdc,
        actor.yes,
        actor.no,
        f.mint_authority,
        amount,
    );
    send_tx(&mut f.svm, &[&actor.kp], &[ix]);
}

/// Seed real liquidity from a fresh LP (mints a complete set of `max(yes,no)`,
/// then adds `yes_amt`/`no_amt`). Returns the LP actor so the test can remove
/// later. `min_lp = 0` (first LP / ratio not yet established).
fn seed_liquidity(f: &mut Fixture, yes_amt: u64, no_amt: u64) -> Actor {
    let lp = new_actor(f, DEPOSITOR_USDC_FUNDING);
    mint_set(f, &lp, yes_amt.max(no_amt));
    let ix = build_add_liquidity_ix(
        legacy_from_signer(&lp.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.lp_mint,
        f.pool_yes,
        f.pool_no,
        lp.yes,
        lp.no,
        lp.lp,
        yes_amt,
        no_amt,
        0,
    );
    send_tx(&mut f.svm, &[&lp.kp], &[ix]);
    lp
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
        TransactionError::InstructionError(0, InstructionError::Custom(markets_error_code(error))),
        "expected custom error {error:?} ({})",
        markets_error_code(error),
    );
}

fn pool_state(f: &Fixture) -> Pool {
    read_anchor_account::<Pool>(&f.svm, &f.pool)
}

/// k = yes_reserve * no_reserve as u128 (the constant-product invariant).
fn pool_k(f: &Fixture) -> u128 {
    let p = pool_state(f);
    (p.yes_reserve as u128) * (p.no_reserve as u128)
}

// ─── tests ────────────────────────────────────────────────────────────────────

/// THE ACCEPTANCE GATE.
///
/// A trader cannot extract free USDC by cycling the two rails through the pool.
/// Mint a complete set for N USDC → round-trip it through the pool (swap NO→YES,
/// then swap the YES back to NO) → redeem the equal complete set back to USDC →
/// assert final USDC <= N. Assert pool `k` never DECREASES across either swap.
#[test]
fn arb_coherence_no_free_usdc() {
    let mut f = setup_pool();

    // Real, balanced liquidity (graduates the bounding phase: both reserves > V).
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);
    assert!(
        !pool_state(&f).bounding_phase_active,
        "balanced LP > V on both sides graduates the bounding phase"
    );

    const N: u64 = 200_000_000; // 200 units — the trader's deposit
    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    let usdc_before = read_token_balance(&f.svm, &trader.usdc);

    // ── Mint a complete set for N USDC (N YES + N NO). ──
    mint_set(&mut f, &trader, N);
    assert_eq!(read_token_balance(&f.svm, &trader.yes), N);
    assert_eq!(read_token_balance(&f.svm, &trader.no), N);
    assert_eq!(read_token_balance(&f.svm, &trader.usdc), usdc_before - N);

    // ── Swap 1: NO → YES (sell some NO into the pool for YES). ──
    let k0 = pool_k(&f);
    let swap_no_in: u64 = 50_000_000;
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        swap_no_in,
        0,
        SwapDirection::NO_TO_YES,
    );
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    let k1 = pool_k(&f);
    assert!(
        k1 >= k0,
        "k must never decrease on swap 1 (NO→YES): k0={k0} k1={k1}"
    );

    // ── Swap 2: YES → NO (sell the YES we just got back into the pool). ──
    let yes_now = read_token_balance(&f.svm, &trader.yes);
    let yes_to_sell = yes_now - N; // sell only the swap proceeds, keep the original N YES
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        yes_to_sell,
        0,
        SwapDirection::YES_TO_NO,
    );
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    let k2 = pool_k(&f);
    assert!(
        k2 >= k1,
        "k must never decrease on swap 2 (YES→NO): k1={k1} k2={k2}"
    );

    // ── Redeem the equal complete set the trader can now form. ──
    let yes_bal = read_token_balance(&f.svm, &trader.yes);
    let no_bal = read_token_balance(&f.svm, &trader.no);
    let redeemable = yes_bal.min(no_bal);
    let ix = build_redeem_complete_set_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.config,
        f.usdc_mint,
        f.yes_mint,
        f.no_mint,
        f.vault,
        trader.usdc,
        trader.yes,
        trader.no,
        redeemable,
    );
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);

    // ── THE GATE: no free USDC. The trader may hold residual outcome tokens
    // (worth at most their face value if the OTHER side were also redeemable),
    // so we bound the BEST case: usdc_back + min(residual) <= N. ──
    let usdc_after = read_token_balance(&f.svm, &trader.usdc);
    let usdc_gained = usdc_after as i128 - usdc_before as i128;
    let residual_yes = read_token_balance(&f.svm, &trader.yes);
    let residual_no = read_token_balance(&f.svm, &trader.no);
    let best_case_residual_value = residual_yes.min(residual_no); // only paired sets redeem 1:1

    println!(
        "[arb gate] N={N} usdc_back_delta={usdc_gained} residual_yes={residual_yes} \
         residual_no={residual_no} k0={k0} k1={k1} k2={k2}"
    );

    // Net USDC change must be <= 0 (the trader spent N, got back <= N).
    assert!(
        usdc_after <= usdc_before,
        "ACCEPTANCE GATE FAILED: trader gained free USDC (before={usdc_before} after={usdc_after})"
    );
    // And even crediting the best-case residual at face value, no free money.
    let best_case_total = usdc_after as i128 + best_case_residual_value as i128;
    assert!(
        best_case_total <= usdc_before as i128,
        "ACCEPTANCE GATE FAILED: usdc_back + best-case residual ({best_case_total}) exceeds \
         deposit ({usdc_before})"
    );
}

/// A YES→NO swap measurably increases the implied price of NO and decreases YES.
/// In the CPMM-prediction model price(NO) = yes_reserve / (yes+no): YES in, NO out
/// → yes_reserve up, no_reserve down → NO price up.
#[test]
fn swap_moves_price() {
    let mut f = setup_pool();
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);

    let before = pool_state(&f);
    // Symmetric pool starts at price(NO) = 0.5.
    let no_price_before =
        (before.yes_reserve as u128) * 10_000 / ((before.yes_reserve + before.no_reserve) as u128);
    assert_eq!(no_price_before, 5_000, "balanced pool starts at NO=0.5");

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 300_000_000);

    // YES → NO swap.
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        100_000_000,
        0,
        SwapDirection::YES_TO_NO,
    );
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);

    let after = pool_state(&f);
    let no_price_after =
        (after.yes_reserve as u128) * 10_000 / ((after.yes_reserve + after.no_reserve) as u128);

    assert!(
        after.yes_reserve > before.yes_reserve,
        "YES reserve up (YES flowed in)"
    );
    assert!(
        after.no_reserve < before.no_reserve,
        "NO reserve down (NO flowed out)"
    );
    assert!(
        no_price_after > no_price_before,
        "implied NO price increased: {no_price_before} -> {no_price_after}"
    );
    println!("[price] NO bps {no_price_before} -> {no_price_after}");
}

/// The FIRST swap on a fresh pool (bounding phase, NO real liquidity except a
/// thin seed) gets a sane price near 0.5 — no revert, no div-by-zero, no 0/inf.
/// We seed a small real reserve so the pool can actually pay out, but keep it
/// below V so the bounding floor still dominates the price.
#[test]
fn bounding_phase_first_trade_sane() {
    let mut f = setup_pool();

    // Seed a small balanced real reserve, well below V (so bounding stays active
    // and the virtual floor dominates the first-trade price). It also gives the
    // pool real tokens to pay the first swap.
    let seed = VIRTUAL_LIQUIDITY / 10; // 10 units each, V = 100 units
    let _lp = seed_liquidity(&mut f, seed, seed);
    assert!(
        pool_state(&f).bounding_phase_active,
        "seed < V on both sides keeps the bounding phase active"
    );

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 50_000_000);

    // First real trade: a SMALL NO→YES swap. With V dominating, the marginal
    // price is ~0.5, so a small input returns ~the same output (sane, finite).
    let amount_in: u64 = 1_000_000; // 1 unit
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        amount_in,
        0,
        SwapDirection::NO_TO_YES,
    );
    let yes_before = read_token_balance(&f.svm, &trader.yes);
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    let yes_after = read_token_balance(&f.svm, &trader.yes);
    let out = yes_after - yes_before;

    // Sane: non-zero, finite, and within a sane band around the input (the floor
    // gives ~0.5 marginal price; with effective reserves ~V the output is close
    // to the input but strictly bounded by the curve, never exceeding it).
    assert!(out > 0, "first trade returned a non-zero output");
    assert!(
        out <= amount_in,
        "first-trade output never exceeds input at/under 0.5 marginal price (out={out})"
    );
    // Near-0.5 sanity: with V=100u dominating a 1u trade, out should be a large
    // fraction of the input (well above, say, 40%).
    assert!(
        out * 100 / amount_in >= 40,
        "first trade priced sanely near 0.5 (out={out} in={amount_in})"
    );
    println!("[bounding first trade] in={amount_in} out={out} (~0.5 expected)");
}

/// THE PHANTOM-PAYOUT GUARD.
///
/// The virtual floor lets the curve "price" an output larger than the pool's real
/// reserve. A swap demanding more than the real output reserve MUST revert with
/// `InsufficientPoolLiquidity` — the pool never pays tokens it does not hold.
#[test]
fn bounding_phase_solvency() {
    let mut f = setup_pool();

    // Seed a TINY real reserve while V is large. The bounding floor (V) makes the
    // curve compute a large `amount_out` for a large `amount_in`, but the pool
    // only holds `seed` real tokens on the output side.
    let seed: u64 = 1_000_000; // 1 unit each
    let _lp = seed_liquidity(&mut f, seed, seed);
    assert!(
        pool_state(&f).bounding_phase_active,
        "seed << V → bounding active"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.pool_yes),
        seed,
        "pool holds exactly `seed` real YES"
    );

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 1_000_000_000);

    // A large NO→YES swap. Effective reserves include V≈100u, so the curve's
    // calculated YES out can approach `amount_in` (~0.5+ marginal), which FAR
    // exceeds the pool's real YES balance of `seed` = 1 unit.
    let big_in: u64 = 500_000_000; // 500 units in — would "price" >> 1 unit out
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        big_in,
        0,
        SwapDirection::NO_TO_YES,
    );
    let result = try_send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    assert_markets_error(result, MarketsError::InsufficientPoolLiquidity);

    // The pool's real reserves are UNCHANGED (the swap fully reverted — no
    // phantom payout, no partial state mutation).
    assert_eq!(
        read_token_balance(&f.svm, &f.pool_yes),
        seed,
        "pool YES untouched after the reverted phantom swap"
    );
    println!("[phantom guard] big_in={big_in} reverted with InsufficientPoolLiquidity (pool held only {seed} YES)");
}

/// Add liquidity, then remove it — the LP gets back <= what they put in (dust
/// stays in the pool), and reserves/supply stay consistent.
#[test]
fn add_remove_liquidity_roundtrip() {
    let mut f = setup_pool();

    // First LP seeds the pool.
    let lp = seed_liquidity(&mut f, LP_YES, LP_NO);
    let lp_balance = read_token_balance(&f.svm, &lp.lp);
    assert!(lp_balance > 0, "first LP received LP tokens");

    let pool_before = pool_state(&f);
    assert_eq!(pool_before.yes_reserve, LP_YES);
    assert_eq!(pool_before.no_reserve, LP_NO);
    assert_eq!(pool_before.lp_supply, lp_balance);

    let yes_before = read_token_balance(&f.svm, &lp.yes);
    let no_before = read_token_balance(&f.svm, &lp.no);

    // Remove ALL LP.
    let ix = build_remove_liquidity_ix(
        legacy_from_signer(&lp.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.lp_mint,
        f.pool_yes,
        f.pool_no,
        lp.yes,
        lp.no,
        lp.lp,
        lp_balance,
        0,
        0,
    );
    send_tx(&mut f.svm, &[&lp.kp], &[ix]);

    let yes_back = read_token_balance(&f.svm, &lp.yes) - yes_before;
    let no_back = read_token_balance(&f.svm, &lp.no) - no_before;

    // LP gets back <= deposited (floor rounding leaves dust in the pool).
    assert!(yes_back <= LP_YES, "YES returned <= deposited");
    assert!(no_back <= LP_NO, "NO returned <= deposited");
    // For an untraded pool removing 100% of supply, floor rounding returns
    // exactly the reserves (no dust when lp_amount == lp_supply).
    assert_eq!(yes_back, LP_YES, "100% withdraw returns all YES");
    assert_eq!(no_back, LP_NO, "100% withdraw returns all NO");

    let pool_after = pool_state(&f);
    assert_eq!(pool_after.lp_supply, 0, "LP supply drained");
    assert_eq!(pool_after.yes_reserve, 0, "YES reserve drained");
    assert_eq!(pool_after.no_reserve, 0, "NO reserve drained");
    assert_eq!(read_mint_supply(&f.svm, &f.lp_mint), 0, "LP mint supply 0");
    println!("[add/remove] LP_in=({LP_YES},{LP_NO}) back=({yes_back},{no_back})");
}

/// A swap with `min_amount_out` higher than achievable reverts (SlippageExceeded).
#[test]
fn swap_slippage_guard() {
    let mut f = setup_pool();
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 200_000_000);

    let amount_in: u64 = 50_000_000;
    // For a balanced 1000/1000 pool, a 50-in swap returns < 50 out (slippage).
    // Demand 60 out → impossible → SlippageExceeded.
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        amount_in,
        amount_in + 10_000_000, // min_out > any achievable out
        SwapDirection::NO_TO_YES,
    );
    let result = try_send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    assert_markets_error(result, MarketsError::SlippageExceeded);
}

/// A large swap that nearly drains one side is bounded by the curve and the real
/// reserve — it never overpays. With real (graduated) liquidity and no virtual
/// floor, the curve alone guarantees `amount_out < output_reserve` (you cannot
/// drain a CPMM pool with a finite input), so a near-drain swap succeeds but
/// leaves a positive reserve.
#[test]
fn single_side_heavy_swap() {
    let mut f = setup_pool();
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);
    assert!(!pool_state(&f).bounding_phase_active);

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    // Mint a huge complete set so the trader can shove a massive NO in.
    mint_set(&mut f, &trader, 100_000_000_000);

    let yes_reserve_before = pool_state(&f).yes_reserve;
    let big_in: u64 = 50_000_000_000; // 50_000 units in vs 1000-unit YES reserve
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        big_in,
        0,
        SwapDirection::NO_TO_YES,
    );
    send_tx(&mut f.svm, &[&trader.kp], &[ix]);

    let pool_after = pool_state(&f);
    // The YES reserve is heavily drained but STRICTLY positive (CPMM can't be
    // fully drained by finite input), and the pool never paid more than it held.
    assert!(
        pool_after.yes_reserve > 0,
        "YES reserve never fully drained"
    );
    assert!(
        pool_after.yes_reserve < yes_reserve_before,
        "YES reserve heavily reduced by the large swap"
    );
    // k did not decrease.
    let k_after = (pool_after.yes_reserve as u128) * (pool_after.no_reserve as u128);
    let k_before = (yes_reserve_before as u128) * (LP_NO as u128);
    assert!(k_after >= k_before, "k held across the heavy swap");
    println!(
        "[heavy swap] yes_reserve {yes_reserve_before} -> {} (never 0)",
        pool_after.yes_reserve
    );
}

// ─── boundary / guard tests ─────────────────────────────────────────────────

/// Zero-amount swap / add / remove rejected with ZeroAmount.
#[test]
fn zero_amounts_rejected() {
    let mut f = setup_pool();
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);
    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 50_000_000);

    // zero-amount swap
    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        0,
        0,
        SwapDirection::NO_TO_YES,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&trader.kp], &[ix]),
        MarketsError::ZeroAmount,
    );

    // zero-amount add (max_yes = 0)
    let ix = build_add_liquidity_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.lp_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        trader.lp,
        0,
        1_000_000,
        0,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&trader.kp], &[ix]),
        MarketsError::ZeroAmount,
    );

    // zero-amount remove (lp_amount = 0)
    let ix = build_remove_liquidity_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.lp_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        trader.lp,
        0,
        0,
        0,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&trader.kp], &[ix]),
        MarketsError::ZeroAmount,
    );
}

/// A swap against an UNINITIALIZED pool is rejected (the pool PDA does not exist,
/// so the `seeds` constraint can't load it → an account/constraint error). We use
/// a second market whose pool was never created.
#[test]
fn swap_on_uninitialized_pool_rejected() {
    let mut f = setup_pool();

    // Create a SECOND market (id 1) with tokens but NO pool.
    let admin = &f.admin;
    let market1_id: u64 = 1;
    let (market1, _) = market_pda(market1_id);
    send_tx(
        &mut f.svm,
        &[admin],
        &[build_create_market_ix(
            legacy_from_signer(admin),
            f.config,
            market1,
            market1_id,
            [3u8; 32],
            MarketMetric::AVG_VIEWERS,
            500,
            [5u8; 32],
            1,
            future_deadline_slot(),
            10,
        )],
    );
    let (yes1, _) = yes_mint_pda(market1_id);
    let (no1, _) = no_mint_pda(market1_id);
    let (vault1, _) = vault_pda(market1_id);
    let (mint_auth1, _) = mint_auth_pda(market1_id);
    send_tx(
        &mut f.svm,
        &[admin],
        &[build_initialize_market_tokens_ix(
            legacy_from_signer(admin),
            f.config,
            market1,
            f.usdc_mint,
            yes1,
            no1,
            vault1,
            mint_auth1,
        )],
    );

    // Pool for market1 was never initialized.
    let (pool1, _) = pool_pda(&market1);
    let pool1_yes = derive_ata(&pool1, &yes1);
    let pool1_no = derive_ata(&pool1, &no1);

    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);

    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        market1,
        pool1,
        yes1,
        no1,
        pool1_yes,
        pool1_no,
        trader.yes, // wrong mint ATAs, but the missing pool fails first
        trader.no,
        1_000_000,
        0,
        SwapDirection::NO_TO_YES,
    );
    let result = try_send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    assert!(
        result.is_err(),
        "swap against an uninitialized pool must fail"
    );
}

/// A swap after the market is resolved is rejected (MarketTradingHalted). We
/// simulate resolution by flipping the Market `resolved` byte (no resolve_market
/// IX yet — Phase 3), exactly as the Phase 1 redeem-after-resolved test does.
#[test]
fn swap_after_resolved_rejected() {
    let mut f = setup_pool();
    let _lp = seed_liquidity(&mut f, LP_YES, LP_NO);
    let trader = new_actor(&mut f, DEPOSITOR_USDC_FUNDING);
    mint_set(&mut f, &trader, 100_000_000);

    // Flip `resolved` to true via a raw-byte write at the known Market offset
    // (same offset the Phase 1 harness uses):
    //   8 disc + 1 bump + 1 version + 8 market_id + 32 creator + 32 streamer_ref
    //   + 1 metric + 8 target + 32 resolution_root + 8 resolution_root_seq
    //   + 8 created_slot + 8 resolve_deadline_slot = 147 → byte 147 is `resolved`.
    const RESOLVED_OFFSET: usize = 8 + 1 + 1 + 8 + 32 + 32 + 1 + 8 + 32 + 8 + 8 + 8;
    let market_addr = address_from_legacy(&f.market);
    let mut market_account = f.svm.get_account(&market_addr).expect("market exists");
    assert_eq!(
        market_account.data[RESOLVED_OFFSET], 0,
        "offset 147 is the (false) resolved byte"
    );
    market_account.data[RESOLVED_OFFSET] = 1;
    f.svm
        .set_account(market_addr, market_account)
        .expect("set resolved market account");

    let ix = build_swap_ix(
        legacy_from_signer(&trader.kp),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.pool_yes,
        f.pool_no,
        trader.yes,
        trader.no,
        10_000_000,
        0,
        SwapDirection::YES_TO_NO,
    );
    let result = try_send_tx(&mut f.svm, &[&trader.kp], &[ix]);
    assert_markets_error(result, MarketsError::MarketTradingHalted);
}

/// `initialize_pool` twice on the same market is rejected (the Pool PDA `init`
/// constraint aborts on the second call — account already in use).
#[test]
fn double_initialize_pool_rejected() {
    let mut f = setup_pool(); // already initialized the pool once

    let ix = build_initialize_pool_ix(
        legacy_from_signer(&f.admin),
        f.market,
        f.pool,
        f.yes_mint,
        f.no_mint,
        f.lp_mint,
        f.pool_yes,
        f.pool_no,
        VIRTUAL_LIQUIDITY,
    );
    let result = try_send_tx(&mut f.svm, &[&f.admin], &[ix]);
    assert!(
        result.is_err(),
        "second initialize_pool must fail (Pool PDA already exists)"
    );
}

/// Pool state after init: bounding phase active, V seeded, reserves/supply zero,
/// lp_mint recorded, and the pool PDA bump stored (used as the signer bump — the
/// byte-identical seeds invariant). Also verifies the pool/lp_mint PDAs derive.
#[test]
fn initialize_pool_state() {
    let f = setup_pool();
    let p = pool_state(&f);

    let (pool, pool_bump) = pool_pda(&f.market);
    let (lp_mint, _) = lp_mint_pda(&f.market);
    assert_eq!(pool.to_bytes(), f.pool.to_bytes(), "pool PDA matches");
    assert_eq!(
        lp_mint.to_bytes(),
        f.lp_mint.to_bytes(),
        "lp_mint PDA matches"
    );

    assert_eq!(
        p.bump, pool_bump,
        "stored pool bump == derived bump (signer seed)"
    );
    assert_eq!(p.market.to_bytes(), f.market.to_bytes());
    assert_eq!(p.lp_mint.to_bytes(), f.lp_mint.to_bytes());
    assert_eq!(p.yes_reserve, 0);
    assert_eq!(p.no_reserve, 0);
    assert_eq!(p.lp_supply, 0);
    assert!(p.bounding_phase_active, "bounding phase active at init");
    assert_eq!(p.virtual_liquidity, VIRTUAL_LIQUIDITY);

    // Pool reserve ATAs exist and are empty.
    assert_eq!(read_token_balance(&f.svm, &f.pool_yes), 0);
    assert_eq!(read_token_balance(&f.svm, &f.pool_no), 0);
}
