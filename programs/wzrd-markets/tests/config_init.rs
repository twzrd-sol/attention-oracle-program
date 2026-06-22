#![cfg(feature = "localtest")]
//! LiteSVM integration coverage for `wzrd-markets` Phase 0.
//!
//! Proves the program loads, the `initialize_markets_config` instruction lands,
//! and the stored `MarketsConfig` fields match what was passed in.
//!
//! Run with (the `.so` must be built first via cargo-build-sbf):
//!   cargo-build-sbf --manifest-path programs/wzrd-markets/Cargo.toml
//!   cargo test -p wzrd-markets --features localtest --test config_init -- --nocapture
//!
//! Mirrors the wzrd-rails `core_loop.rs` harness (address conversion + program
//! load + tx send), trimmed to what config-init needs (no Token-2022 program —
//! Phase 0 moves no funds).

use anchor_lang::{
    error::ERROR_CODE_OFFSET, prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas,
};
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_address::Address;
use solana_instruction::error::InstructionError;
use solana_instruction::Instruction as ModernInstruction;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_sdk::{
    instruction::Instruction as LegacyInstruction, pubkey::Pubkey as LegacyPubkey, system_program,
};
use solana_signer::Signer;
use solana_transaction::{Transaction, TransactionError};
use std::path::Path;
use wzrd_markets::{
    accounts as markets_accounts,
    error::MarketsError,
    instruction as markets_ix,
    state::{MarketsConfig, MARKETS_CONFIG_SEED},
    ID as WZRD_MARKETS_PROGRAM_ID,
};

fn address_from_legacy(pubkey: &LegacyPubkey) -> Address {
    Address::from(pubkey.to_bytes())
}

fn legacy_from_address(address: &Address) -> LegacyPubkey {
    LegacyPubkey::new_from_array(address.to_bytes())
}

fn legacy_from_signer(signer: &Keypair) -> LegacyPubkey {
    legacy_from_address(&signer.pubkey())
}

fn convert_instruction(ix: &LegacyInstruction) -> ModernInstruction {
    ModernInstruction {
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

fn try_send_tx(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    instructions: &[LegacyInstruction],
) -> Result<(), FailedTransactionMetadata> {
    let payer = signers.first().expect("at least one signer is required");
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());
    svm.send_transaction(tx).map(|_| ())
}

fn markets_config_pda() -> (LegacyPubkey, u8) {
    let (addr, bump) =
        Pubkey::find_program_address(&[MARKETS_CONFIG_SEED], &WZRD_MARKETS_PROGRAM_ID);
    (LegacyPubkey::new_from_array(addr.to_bytes()), bump)
}

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
            usdc_mint: Pubkey::new_from_array(usdc_mint.to_bytes()),
            resolver_multisig: Pubkey::new_from_array(resolver_multisig.to_bytes()),
            // Carved into MarketsConfig in Phase 3 (see _reserved 56 -> 47). The
            // Phase-0 assertions below don't read these back, but the IX guards
            // require window > 0 and threshold in 1..=MAX_PUBLISHERS.
            default_dispute_window_slots: 54_000,
            resolver_threshold: 1,
        }
        .data(),
    }
}

#[test]
fn initialize_markets_config_works() {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");

    let (config, expected_bump) = markets_config_pda();
    // Distinct sentinel mints so the assertions catch any field cross-wiring.
    let usdc_mint = legacy_from_signer(&Keypair::new());
    let resolver_multisig = legacy_from_signer(&Keypair::new());

    let ix = build_initialize_markets_config_ix(
        legacy_from_signer(&admin),
        config,
        usdc_mint,
        resolver_multisig,
    );
    try_send_tx(&mut svm, &[&admin], &[ix]).expect("initialize_markets_config tx");

    // Read the account back and assert every stored field.
    let raw = svm
        .get_account(&address_from_legacy(&config))
        .expect("config account exists after init");
    let parsed = MarketsConfig::try_deserialize(&mut raw.data.as_slice())
        .expect("deserialize MarketsConfig");

    assert_eq!(parsed.bump, expected_bump, "stored bump matches PDA bump");
    assert_eq!(
        parsed.admin.to_bytes(),
        admin.pubkey().to_bytes(),
        "admin = signer"
    );
    assert_eq!(
        parsed.usdc_mint.to_bytes(),
        usdc_mint.to_bytes(),
        "usdc_mint stored"
    );
    assert_eq!(
        parsed.resolver_multisig.to_bytes(),
        resolver_multisig.to_bytes(),
        "resolver_multisig stored"
    );
    assert!(
        parsed.publisher_allowlist.is_empty(),
        "publisher allow-list starts empty"
    );
    assert_eq!(
        parsed.next_market_id, 0,
        "next_market_id starts at 0 (Phase 1 counter)"
    );
    // Audit C-02: no admin rotation in flight at init.
    assert_eq!(
        parsed.pending_admin,
        Pubkey::default(),
        "pending_admin starts at the zero sentinel (no rotation)"
    );
    assert_eq!(parsed._reserved, [0u8; 15], "reserved zero-initialized");
}

#[test]
fn initialize_markets_config_is_one_time() {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");

    let (config, _bump) = markets_config_pda();
    let usdc_mint = legacy_from_signer(&Keypair::new());
    let resolver_multisig = legacy_from_signer(&Keypair::new());

    let ix = build_initialize_markets_config_ix(
        legacy_from_signer(&admin),
        config,
        usdc_mint,
        resolver_multisig,
    );
    try_send_tx(&mut svm, &[&admin], &[ix.clone()]).expect("first init succeeds");

    // Second init against the same PDA must fail (Anchor `init` on an
    // already-initialized account).
    let err = try_send_tx(&mut svm, &[&admin], &[ix]);
    assert!(err.is_err(), "second initialize_markets_config must fail");
}

// ─── Audit C-02: 2-step admin rotation (set_admin / accept_admin) ──────────────
//
// The re-audit of the C-01/C-02/C-03 fix deltas flagged that C-02 had ZERO
// behavioral coverage — only the LEN/layout invariant was tested. The prior
// audit's AC-1 concern was literally "admin-rotation bricks the admin setter",
// so the regression a rotation integration test catches is exactly the one that
// motivated the finding. This test exercises: propose -> accept -> admin changed,
// the rotated admin retains an admin-gated capability (add_publisher), and the
// two negative cases (non-admin can't propose, wrong key can't accept).

fn markets_error_code(error: MarketsError) -> u32 {
    ERROR_CODE_OFFSET + error as u32
}

/// Assert a `try_send_tx` failure carries the expected program error at ix 0.
fn assert_markets_error(result: Result<(), FailedTransactionMetadata>, error: MarketsError) {
    let failure = result.expect_err("expected transaction to fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(0, InstructionError::Custom(markets_error_code(error))),
        "expected custom error {error:?} ({})",
        markets_error_code(error),
    );
}

fn build_set_admin_ix(
    admin: LegacyPubkey,
    config: LegacyPubkey,
    new_admin: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::AdminConfig { admin, config }.to_account_metas(None),
        data: markets_ix::SetAdmin {
            new_admin: Pubkey::new_from_array(new_admin.to_bytes()),
        }
        .data(),
    }
}

fn build_accept_admin_ix(new_admin: LegacyPubkey, config: LegacyPubkey) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_MARKETS_PROGRAM_ID,
        accounts: markets_accounts::AcceptAdmin { new_admin, config }.to_account_metas(None),
        data: markets_ix::AcceptAdmin {}.data(),
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
            publisher: Pubkey::new_from_array(publisher.to_bytes()),
        }
        .data(),
    }
}

fn read_config(svm: &LiteSVM, config: &LegacyPubkey) -> MarketsConfig {
    let raw = svm
        .get_account(&address_from_legacy(config))
        .expect("config account exists");
    MarketsConfig::try_deserialize(&mut raw.data.as_slice()).expect("deserialize MarketsConfig")
}

/// Spin up an initialized config owned by a fresh admin. Returns (svm, config, admin).
fn setup_initialized_config() -> (LiteSVM, LegacyPubkey, Keypair) {
    let mut svm = LiteSVM::new();
    load_wzrd_markets_program(&mut svm).expect("load wzrd-markets program");
    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");
    let (config, _bump) = markets_config_pda();
    let ix = build_initialize_markets_config_ix(
        legacy_from_signer(&admin),
        config,
        legacy_from_signer(&Keypair::new()),
        legacy_from_signer(&Keypair::new()),
    );
    try_send_tx(&mut svm, &[&admin], &[ix]).expect("initialize_markets_config");
    (svm, config, admin)
}

#[test]
fn c02_admin_rotation_full_cycle() {
    let (mut svm, config, admin) = setup_initialized_config();

    let new_admin = Keypair::new();
    svm.airdrop(&new_admin.pubkey(), 100_000_000_000)
        .expect("airdrop new_admin");

    // Step 1: the CURRENT admin proposes the new admin. `admin` is unchanged;
    // `pending_admin` now holds the proposal.
    try_send_tx(
        &mut svm,
        &[&admin],
        &[build_set_admin_ix(
            legacy_from_signer(&admin),
            config,
            legacy_from_signer(&new_admin),
        )],
    )
    .expect("set_admin (propose) by current admin");
    let cfg = read_config(&svm, &config);
    assert_eq!(
        cfg.admin.to_bytes(),
        admin.pubkey().to_bytes(),
        "admin unchanged until accept"
    );
    assert_eq!(
        cfg.pending_admin.to_bytes(),
        new_admin.pubkey().to_bytes(),
        "pending_admin holds the proposal"
    );

    // Step 2: the PROPOSED admin accepts (signs for itself). `admin` becomes
    // new_admin; `pending_admin` clears back to the zero sentinel.
    try_send_tx(
        &mut svm,
        &[&new_admin],
        &[build_accept_admin_ix(
            legacy_from_signer(&new_admin),
            config,
        )],
    )
    .expect("accept_admin by the proposed admin");
    let cfg = read_config(&svm, &config);
    assert_eq!(
        cfg.admin.to_bytes(),
        new_admin.pubkey().to_bytes(),
        "admin is now the rotated key"
    );
    assert_eq!(
        cfg.pending_admin,
        Pubkey::default(),
        "pending_admin cleared after accept"
    );

    // AC-1 regression guard: the rotated admin must retain admin-gated powers.
    // (The original audit concern was rotation BRICKING the admin setter.)
    try_send_tx(
        &mut svm,
        &[&new_admin],
        &[build_add_publisher_ix(
            legacy_from_signer(&new_admin),
            config,
            legacy_from_signer(&Keypair::new()),
        )],
    )
    .expect("rotated admin can call an admin-gated instruction (add_publisher)");

    // ...and the OLD admin must no longer be honored.
    let old_admin_denied = try_send_tx(
        &mut svm,
        &[&admin],
        &[build_add_publisher_ix(
            legacy_from_signer(&admin),
            config,
            legacy_from_signer(&Keypair::new()),
        )],
    );
    assert_markets_error(old_admin_denied, MarketsError::Unauthorized);
}

#[test]
fn c02_non_admin_cannot_propose() {
    let (mut svm, config, _admin) = setup_initialized_config();
    let stranger = Keypair::new();
    svm.airdrop(&stranger.pubkey(), 100_000_000_000)
        .expect("airdrop stranger");

    // A non-admin signer proposing a rotation must be rejected (Unauthorized),
    // and `pending_admin` must remain the zero sentinel.
    let denied = try_send_tx(
        &mut svm,
        &[&stranger],
        &[build_set_admin_ix(
            legacy_from_signer(&stranger),
            config,
            legacy_from_signer(&Keypair::new()),
        )],
    );
    assert_markets_error(denied, MarketsError::Unauthorized);
    assert_eq!(
        read_config(&svm, &config).pending_admin,
        Pubkey::default(),
        "no pending rotation after a rejected propose"
    );
}

#[test]
fn c02_wrong_key_cannot_accept() {
    let (mut svm, config, admin) = setup_initialized_config();
    let new_admin = Keypair::new();
    let imposter = Keypair::new();
    for kp in [&new_admin, &imposter] {
        svm.airdrop(&kp.pubkey(), 100_000_000_000).expect("airdrop");
    }

    // Admin proposes new_admin.
    try_send_tx(
        &mut svm,
        &[&admin],
        &[build_set_admin_ix(
            legacy_from_signer(&admin),
            config,
            legacy_from_signer(&new_admin),
        )],
    )
    .expect("propose");

    // An imposter (not the pending key) cannot accept -> Unauthorized; admin
    // stays the original and the pending proposal is untouched.
    let denied = try_send_tx(
        &mut svm,
        &[&imposter],
        &[build_accept_admin_ix(legacy_from_signer(&imposter), config)],
    );
    assert_markets_error(denied, MarketsError::Unauthorized);
    let cfg = read_config(&svm, &config);
    assert_eq!(
        cfg.admin.to_bytes(),
        admin.pubkey().to_bytes(),
        "admin unchanged after a rejected accept"
    );
    assert_eq!(
        cfg.pending_admin.to_bytes(),
        new_admin.pubkey().to_bytes(),
        "pending proposal still stands after a rejected accept"
    );
}
