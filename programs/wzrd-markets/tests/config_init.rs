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
use solana_sdk::{
    instruction::Instruction as LegacyInstruction, pubkey::Pubkey as LegacyPubkey, system_program,
};
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::path::Path;
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
    assert_eq!(parsed._reserved, [0u8; 64], "reserved zero-initialized");
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
