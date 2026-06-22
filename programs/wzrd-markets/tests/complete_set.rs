#![cfg(feature = "localtest")]
//! LiteSVM integration coverage for `wzrd-markets` Phase 1 — the market
//! lifecycle (`create_market`, `initialize_market_tokens`) and the fixed-par
//! complete-set rail (`mint_complete_set`, `redeem_complete_set`).
//!
//! THE ACCEPTANCE GATE is `complete_set_roundtrip_preserves_solvency`: it mints
//! a complete set, asserts `vault.amount == yes_supply == no_supply == N`,
//! redeems it, and asserts the vault + supplies return to baseline and the
//! depositor is made whole in USDC.
//!
//! Run with (the `.so` must be built first via cargo-build-sbf):
//!   cargo-build-sbf --manifest-path programs/wzrd-markets/Cargo.toml
//!   cargo test -p wzrd-markets --features localtest --test complete_set -- --nocapture
//!
//! Mirrors the wzrd-rails `listen_payout_e2e.rs` harness: address conversion +
//! program load (markets + Token-2022 + Associated-Token) + tx send. "USDC" is
//! modeled here as a fee-free Token-2022 mint (6 decimals) — the program reaches
//! it through the token Interface, so a Token-2022 collateral mint exercises the
//! same path mainnet USDC would (and keeps the test to a single token program).

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
        Market, MarketMetric, MarketsConfig, MARKETS_CONFIG_SEED, MARKET_SEED, MINT_AUTH_SEED,
        NO_MINT_SEED, VAULT_SEED, YES_MINT_SEED,
    },
    MarketsError, ID as WZRD_MARKETS_PROGRAM_ID,
};

const USDC_DECIMALS: u8 = 6;
const MARKET_ID: u64 = 0;
/// 1_000 "USDC" at 6 decimals — the deposit/redeem amount used by the roundtrip.
const SET_AMOUNT: u64 = 1_000_000_000;
/// Depositor is funded with more than they deposit, to prove exact conservation.
const DEPOSITOR_USDC_FUNDING: u64 = 5_000_000_000;

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
/// cargo registry. Same lookup wzrd-rails' E2E harness uses.
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

/// Create a fee-free Token-2022 mint (6 decimals) standing in for USDC, with
/// `mint_authority` controlled by a test keypair so the harness can fund the
/// depositor.
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
            // Phase 1 complete-set tests don't exercise the dispute window; these
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

// ─── fixture ──────────────────────────────────────────────────────────────────

struct Fixture {
    svm: LiteSVM,
    admin: Keypair,
    depositor: Keypair,
    config: LegacyPubkey,
    usdc_mint: LegacyPubkey,
    usdc_mint_authority: Keypair,
    market: LegacyPubkey,
    yes_mint: LegacyPubkey,
    no_mint: LegacyPubkey,
    vault: LegacyPubkey,
    mint_authority: LegacyPubkey,
    depositor_usdc: LegacyPubkey,
    depositor_yes: LegacyPubkey,
    depositor_no: LegacyPubkey,
}

/// LiteSVM boots at slot 0; any large constant is comfortably in the future for
/// the `resolve_deadline_slot > current slot` guard.
fn future_deadline_slot(_svm: &LiteSVM) -> u64 {
    1_000_000
}

/// Boot the SVM, init config + a fee-free Token-2022 "USDC" mint, fund the
/// depositor, and `create_market` + `initialize_market_tokens` (market_id 0).
/// Leaves the market ready for the complete-set rail.
fn setup() -> Fixture {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");
    load_token_2022_program(&mut svm);
    load_associated_token_program(&mut svm);

    let admin = Keypair::new();
    let depositor = Keypair::new();
    let usdc_mint_kp = Keypair::new();
    let usdc_mint_authority = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");
    svm.airdrop(&depositor.pubkey(), 100_000_000_000)
        .expect("airdrop depositor");
    svm.airdrop(&usdc_mint_authority.pubkey(), 100_000_000_000)
        .expect("airdrop usdc mint authority");

    let (config, _config_bump) = markets_config_pda();
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);
    let resolver_multisig = legacy_from_signer(&Keypair::new());

    // 1) config
    let ix = build_initialize_markets_config_ix(
        legacy_from_signer(&admin),
        config,
        usdc_mint,
        resolver_multisig,
    );
    send_tx(&mut svm, &[&admin], &[ix]);

    // 2) "USDC" mint + fund depositor's USDC ATA
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

    // 3) create_market (market_id 0)
    let (market, _market_bump) = market_pda(MARKET_ID);
    let deadline = future_deadline_slot(&svm);
    let ix = build_create_market_ix(
        legacy_from_signer(&admin),
        config,
        market,
        MARKET_ID,
        [7u8; 32],
        MarketMetric::AVG_VIEWERS,
        1_000,
        [9u8; 32], // non-zero resolution root
        42,
        deadline,
        100,
    );
    send_tx(&mut svm, &[&admin], &[ix]);

    // 4) initialize_market_tokens
    let (yes_mint, _) = yes_mint_pda(MARKET_ID);
    let (no_mint, _) = no_mint_pda(MARKET_ID);
    let (vault, _) = vault_pda(MARKET_ID);
    let (mint_authority, _) = mint_auth_pda(MARKET_ID);
    let ix = build_initialize_market_tokens_ix(
        legacy_from_signer(&admin),
        config,
        market,
        usdc_mint,
        yes_mint,
        no_mint,
        vault,
        mint_authority,
    );
    send_tx(&mut svm, &[&admin], &[ix]);

    // depositor outcome ATAs
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

    Fixture {
        svm,
        admin,
        depositor,
        config,
        usdc_mint,
        usdc_mint_authority,
        market,
        yes_mint,
        no_mint,
        vault,
        mint_authority,
        depositor_usdc,
        depositor_yes,
        depositor_no,
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

// ─── tests ────────────────────────────────────────────────────────────────────

/// THE ACCEPTANCE GATE.
#[test]
fn complete_set_roundtrip_preserves_solvency() {
    let mut f = setup();

    // Baseline: nothing minted yet.
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        0,
        "vault empty pre-mint"
    );
    assert_eq!(read_mint_supply(&f.svm, &f.yes_mint), 0, "yes supply 0");
    assert_eq!(read_mint_supply(&f.svm, &f.no_mint), 0, "no supply 0");

    let depositor_usdc_before = read_token_balance(&f.svm, &f.depositor_usdc);
    assert_eq!(depositor_usdc_before, DEPOSITOR_USDC_FUNDING);

    // ── mint a complete set of SET_AMOUNT ──
    let ix = build_mint_complete_set_ix(
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
        f.mint_authority,
        SET_AMOUNT,
    );
    send_tx(&mut f.svm, &[&f.depositor], &[ix]);

    // The solvency invariant: vault == yes_supply == no_supply == N.
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        SET_AMOUNT,
        "vault holds exactly the deposited USDC"
    );
    assert_eq!(
        read_mint_supply(&f.svm, &f.yes_mint),
        SET_AMOUNT,
        "YES supply == N"
    );
    assert_eq!(
        read_mint_supply(&f.svm, &f.no_mint),
        SET_AMOUNT,
        "NO supply == N"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_yes),
        SET_AMOUNT,
        "depositor received N YES"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_no),
        SET_AMOUNT,
        "depositor received N NO"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_usdc),
        DEPOSITOR_USDC_FUNDING - SET_AMOUNT,
        "depositor USDC debited by exactly N"
    );

    // ── redeem the complete set ──
    let ix = build_redeem_complete_set_ix(
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
    );
    send_tx(&mut f.svm, &[&f.depositor], &[ix]);

    // Back to baseline: vault == yes_supply == no_supply == 0.
    assert_eq!(
        read_token_balance(&f.svm, &f.vault),
        0,
        "vault back to 0 post-redeem"
    );
    assert_eq!(
        read_mint_supply(&f.svm, &f.yes_mint),
        0,
        "YES supply back to 0"
    );
    assert_eq!(
        read_mint_supply(&f.svm, &f.no_mint),
        0,
        "NO supply back to 0"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_yes),
        0,
        "depositor YES burned"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_no),
        0,
        "depositor NO burned"
    );
    assert_eq!(
        read_token_balance(&f.svm, &f.depositor_usdc),
        DEPOSITOR_USDC_FUNDING,
        "depositor made whole in USDC"
    );
}

#[test]
fn mint_complete_set_rejects_zero() {
    let mut f = setup();
    let ix = build_mint_complete_set_ix(
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
        f.mint_authority,
        0,
    );
    let result = try_send_tx(&mut f.svm, &[&f.depositor], &[ix]);
    assert_markets_error(result, MarketsError::ZeroAmount);
}

#[test]
fn redeem_more_than_held_rejected() {
    let mut f = setup();

    // Mint exactly SET_AMOUNT, then try to redeem SET_AMOUNT + 1.
    let ix = build_mint_complete_set_ix(
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
        f.mint_authority,
        SET_AMOUNT,
    );
    send_tx(&mut f.svm, &[&f.depositor], &[ix]);

    let ix = build_redeem_complete_set_ix(
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
        SET_AMOUNT + 1,
    );
    let result = try_send_tx(&mut f.svm, &[&f.depositor], &[ix]);
    assert_markets_error(result, MarketsError::InsufficientOutcomeBalance);
}

/// Phase 3 has no `resolve_market` yet, so we simulate a resolved market by
/// writing `resolved = true` directly into the Market account, then assert the
/// redeem rail's pre-resolution guard rejects it.
#[test]
fn redeem_after_resolved_rejected() {
    let mut f = setup();

    // Mint a set so the redeemer holds outcome tokens (isolate the guard).
    let ix = build_mint_complete_set_ix(
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
        f.mint_authority,
        SET_AMOUNT,
    );
    send_tx(&mut f.svm, &[&f.depositor], &[ix]);

    // Flip `resolved` to true via a single raw-byte write (Phase-3 resolve
    // simulation — there is no `resolve_market` instruction yet). The `resolved`
    // bool sits at a fixed offset in the Market account:
    //   8 disc + 1 bump + 1 version + 8 market_id + 32 creator + 32 streamer_ref
    //   + 1 metric + 8 target + 32 resolution_root + 8 resolution_root_seq
    //   + 8 created_slot + 8 resolve_deadline_slot = 147  → byte 147 is `resolved`.
    const RESOLVED_OFFSET: usize = 8 + 1 + 1 + 8 + 32 + 32 + 1 + 8 + 32 + 8 + 8 + 8;
    let market_addr = address_from_legacy(&f.market);
    let mut market_account = f
        .svm
        .get_account(&market_addr)
        .expect("market account exists");
    // Sanity: confirm we're flipping the field we think we are.
    {
        let parsed: Market =
            Market::try_deserialize(&mut market_account.data.as_slice()).expect("deserialize");
        assert!(!parsed.resolved, "market starts unresolved");
        assert_eq!(
            market_account.data[RESOLVED_OFFSET], 0,
            "offset 147 is the (false) resolved byte"
        );
    }
    market_account.data[RESOLVED_OFFSET] = 1; // resolved = true
                                              // Write the SAME account object litesvm handed us back (correct field types).
    f.svm
        .set_account(market_addr, market_account)
        .expect("set resolved market account");
    // Confirm the flip round-trips.
    {
        let after = f.svm.get_account(&market_addr).expect("market exists");
        let parsed: Market =
            Market::try_deserialize(&mut after.data.as_slice()).expect("deserialize");
        assert!(parsed.resolved, "resolved flipped to true");
    }

    let ix = build_redeem_complete_set_ix(
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
    );
    let result = try_send_tx(&mut f.svm, &[&f.depositor], &[ix]);
    assert_markets_error(result, MarketsError::MarketResolved);
}

#[test]
fn double_initialize_market_tokens_rejected() {
    let mut f = setup();
    // setup() already called initialize_market_tokens once; a second call must
    // fail. The `init` of the mints/vault would also abort, but the program's
    // `!tokens_initialized` guard yields the typed MarketAlreadyHasTokens.
    let ix = build_initialize_market_tokens_ix(
        legacy_from_signer(&f.admin),
        f.config,
        f.market,
        f.usdc_mint,
        f.yes_mint,
        f.no_mint,
        f.vault,
        f.mint_authority,
    );
    let result = try_send_tx(&mut f.svm, &[&f.admin], &[ix]);
    assert!(
        result.is_err(),
        "second initialize_market_tokens must fail (account already exists / already initialized)"
    );
}

#[test]
fn non_sequential_market_id_rejected() {
    let mut f = setup();
    // setup() created market_id 0, so next_market_id == 1. Creating market_id 5
    // (a gap) must fail with InvalidMarketId.
    let bad_id: u64 = 5;
    let (bad_market, _) = market_pda(bad_id);
    let deadline = future_deadline_slot(&f.svm);
    let ix = build_create_market_ix(
        legacy_from_signer(&f.admin),
        f.config,
        bad_market,
        bad_id,
        [1u8; 32],
        MarketMetric::PEAK_VIEWERS,
        500,
        [3u8; 32],
        1,
        deadline,
        50,
    );
    let result = try_send_tx(&mut f.svm, &[&f.admin], &[ix]);
    assert_markets_error(result, MarketsError::InvalidMarketId);
}

/// create_market with a future-id rejected; also covers the zero-root and
/// past-deadline guards and the metric-range guard in one place.
#[test]
fn create_market_guards() {
    let mut f = setup();
    let next_id: u64 = 1; // setup() consumed id 0
    let (market1, _) = market_pda(next_id);
    let deadline = future_deadline_slot(&f.svm);

    // zero resolution root → ZeroResolutionRoot
    let ix = build_create_market_ix(
        legacy_from_signer(&f.admin),
        f.config,
        market1,
        next_id,
        [1u8; 32],
        MarketMetric::AVG_VIEWERS,
        100,
        [0u8; 32], // zero root
        1,
        deadline,
        10,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&f.admin], &[ix]),
        MarketsError::ZeroResolutionRoot,
    );

    // deadline in the past (slot 0) → DeadlineInPast
    let ix = build_create_market_ix(
        legacy_from_signer(&f.admin),
        f.config,
        market1,
        next_id,
        [1u8; 32],
        MarketMetric::AVG_VIEWERS,
        100,
        [4u8; 32],
        1,
        0, // deadline <= current slot
        10,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&f.admin], &[ix]),
        MarketsError::DeadlineInPast,
    );

    // invalid metric (4 > MAX=3) → InvalidMetric
    let ix = build_create_market_ix(
        legacy_from_signer(&f.admin),
        f.config,
        market1,
        next_id,
        [1u8; 32],
        4, // out of range
        100,
        [4u8; 32],
        1,
        deadline,
        10,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&f.admin], &[ix]),
        MarketsError::InvalidMetric,
    );

    // non-admin signer → Unauthorized
    let stranger = Keypair::new();
    f.svm
        .airdrop(&stranger.pubkey(), 100_000_000_000)
        .expect("airdrop stranger");
    let ix = build_create_market_ix(
        legacy_from_signer(&stranger),
        f.config,
        market1,
        next_id,
        [1u8; 32],
        MarketMetric::AVG_VIEWERS,
        100,
        [4u8; 32],
        1,
        deadline,
        10,
    );
    assert_markets_error(
        try_send_tx(&mut f.svm, &[&stranger], &[ix]),
        MarketsError::Unauthorized,
    );
}

/// PDA derivation sanity: the market / mint / vault / mint-auth PDAs derive and
/// are pairwise distinct, and the stored Market fields match what create_market
/// committed (H-01 snapshot).
#[test]
fn pda_derivation_and_market_state() {
    let f = setup();

    let (market, market_bump) = market_pda(MARKET_ID);
    let (yes_mint, _) = yes_mint_pda(MARKET_ID);
    let (no_mint, _) = no_mint_pda(MARKET_ID);
    let (vault, _) = vault_pda(MARKET_ID);
    let (mint_authority, _) = mint_auth_pda(MARKET_ID);

    // All five PDAs are distinct.
    let all = [market, yes_mint, no_mint, vault, mint_authority];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "PDAs must be pairwise distinct ({i},{j})");
        }
    }

    // Stored Market matches the create_market commitment.
    let market_state: Market = read_anchor_account(&f.svm, &f.market);
    assert_eq!(market_state.market_id, MARKET_ID);
    assert_eq!(market_state.bump, market_bump);
    assert_eq!(market_state.version, Market::VERSION);
    assert_eq!(market_state.metric, MarketMetric::AVG_VIEWERS);
    assert_eq!(market_state.target, 1_000);
    assert_eq!(market_state.streamer_ref, [7u8; 32]);
    assert_eq!(
        market_state.resolution_root, [9u8; 32],
        "H-01 root snapshot"
    );
    assert_eq!(market_state.resolution_root_seq, 42, "H-01 seq snapshot");
    assert!(!market_state.resolved);
    assert!(
        market_state.tokens_initialized,
        "tokens initialized in setup"
    );
    assert_eq!(market_state.yes_mint.to_bytes(), yes_mint.to_bytes());
    assert_eq!(market_state.no_mint.to_bytes(), no_mint.to_bytes());
    assert_eq!(market_state.vault.to_bytes(), vault.to_bytes());

    // next_market_id advanced to 1 after creating market 0.
    let config_state: MarketsConfig = read_anchor_account(&f.svm, &f.config);
    assert_eq!(config_state.next_market_id, 1, "counter advanced");
    let _ = (f.mint_authority, mint_authority);
}
