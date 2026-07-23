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

use anchor_lang::{prelude::Pubkey, AccountDeserialize, InstructionData, ToAccountMetas};
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_address::Address;
use solana_instruction::Instruction as ModernInstruction;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_sdk::system_instruction;
use solana_sdk::{
    instruction::Instruction as LegacyInstruction, program_pack::Pack,
    pubkey::Pubkey as LegacyPubkey, system_program,
};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::path::{Path, PathBuf};
use wzrd_markets::{
    accounts as markets_accounts, instruction as markets_ix,
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

/// Locate a litesvm-bundled SPL ELF from the cargo registry (same lookup as
/// complete_set.rs). Needed since the M-01/L-01 fix: initialize_markets_config
/// validates the collateral mint (owner == token-2022), so a REAL Token-2022
/// mint must exist before config init even in Phase-0 tests.
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

/// Create a fee-free Token-2022 mint (6 decimals) standing in for USDC.
fn create_plain_token_2022_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair) {
    let payer_pubkey = legacy_from_signer(payer);
    let mint_pubkey = legacy_from_signer(mint);
    let rent = svm.minimum_balance_for_rent_exemption(spl_token_2022::state::Mint::LEN);
    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &mint_pubkey,
        rent,
        spl_token_2022::state::Mint::LEN as u64,
        &spl_token_2022::id(),
    );
    let init_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        &mint_pubkey,
        &payer_pubkey,
        None,
        6,
    )
    .unwrap();
    try_send_tx(svm, &[payer, mint], &[create_ix, init_ix]).expect("create token-2022 mint");
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
            usdc_mint,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: markets_ix::InitializeMarketsConfig {
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
    load_token_2022_program(&mut svm);

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");

    let (config, expected_bump) = markets_config_pda();
    // Distinct sentinel mints so the assertions catch any field cross-wiring.
    let usdc_mint_kp = Keypair::new();
    create_plain_token_2022_mint(&mut svm, &admin, &usdc_mint_kp);
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);
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
    load_token_2022_program(&mut svm);

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000)
        .expect("airdrop admin");

    let (config, _bump) = markets_config_pda();
    let usdc_mint_kp = Keypair::new();
    create_plain_token_2022_mint(&mut svm, &admin, &usdc_mint_kp);
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);
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
