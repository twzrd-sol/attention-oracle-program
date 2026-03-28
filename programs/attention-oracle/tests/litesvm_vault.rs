#![cfg(feature = "localtest")]
//! LiteSVM integration tests for the Market Vault deposit -> settle loop.
//!
//! Run with: `cargo test --package attention-oracle-token-2022 --test litesvm_vault -- --nocapture`
//!
//! Coverage:
//! - Full loop: deposit_market -> update_attention -> settle_market
//! - Paused protocol rejects deposits
//! - Multiplier cap enforcement (50000 BPS max)

use anchor_lang::prelude::AccountSerialize;
use litesvm::{types::TransactionResult, LiteSVM};
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_address::Address;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_sdk::{
    instruction::{AccountMeta as LegacyAccountMeta, Instruction as LegacyInstruction},
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey as LegacyPubkey,
};
use solana_signer::Signer;
use solana_system_interface::program as system_program;
use solana_transaction::Transaction;
use spl_token_2022::state::{Account as SplAccount, AccountState, Mint as SplMint};
use std::path::Path;

use token_2022::{MarketVault, ProtocolState};

// =============================================================================
// CONSTANTS & HELPERS
// =============================================================================

fn program_id() -> LegacyPubkey {
    "GnGzNdsQMxMpJfMeqnkGPsvHm8kwaDidiKjNU2dCVZop"
        .parse()
        .unwrap()
}

fn spl_token_program_id() -> LegacyPubkey {
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        .parse()
        .unwrap()
}

fn compute_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn serialize_anchor<T: AccountSerialize>(account: &T, len: usize) -> Vec<u8> {
    let mut data = vec![0u8; len];
    account.try_serialize(&mut data.as_mut_slice()).unwrap();
    data
}

fn address_from_legacy(pubkey: &LegacyPubkey) -> Address {
    Address::from(pubkey.to_bytes())
}

fn legacy_from_address(address: &Address) -> LegacyPubkey {
    LegacyPubkey::new_from_array(address.to_bytes())
}

fn legacy_from_signer(signer: &Keypair) -> LegacyPubkey {
    legacy_from_address(&signer.pubkey())
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

fn send_legacy_tx(
    svm: &mut LiteSVM,
    signers: &[&Keypair],
    payer: &Keypair,
    instructions: &[LegacyInstruction],
) -> TransactionResult {
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let tx = Transaction::new(
        signers,
        Message::new(&instructions, Some(&payer.pubkey())),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
}

fn get_account_legacy(svm: &LiteSVM, address: &LegacyPubkey) -> Account {
    svm.get_account(&address_from_legacy(address))
        .expect("Account not found")
}

// =============================================================================
// PROGRAM LOADING
// =============================================================================

fn load_program(svm: &mut LiteSVM) -> Result<(), Box<dyn std::error::Error>> {
    let program_path = Path::new("../../target/deploy/token_2022.so");
    if !program_path.exists() {
        return Err(format!(
            "Program not found at {:?}. Run `anchor build` first.",
            program_path
                .canonicalize()
                .unwrap_or(program_path.to_path_buf())
        )
        .into());
    }
    let program_bytes = std::fs::read(program_path)?;
    svm.add_program(address_from_legacy(&program_id()), &program_bytes)?;
    Ok(())
}

/// Search for SPL program ELF binaries shipped with litesvm in the cargo registry.
fn find_spl_elf(prefix: &str) -> Option<Vec<u8>> {
    let home = std::env::var("HOME").ok()?;
    let base = std::path::PathBuf::from(home).join(".cargo/registry/src");

    for index_entry in std::fs::read_dir(&base).ok()?.flatten() {
        for crate_entry in std::fs::read_dir(index_entry.path()).ok()?.flatten() {
            let name = crate_entry.file_name();
            if name.to_str().map_or(false, |s| s.starts_with("litesvm-")) {
                let elf_dir = crate_entry.path().join("src/programs/elf");
                if let Ok(entries) = std::fs::read_dir(&elf_dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name();
                        if fname
                            .to_str()
                            .map_or(false, |s| s.starts_with(prefix) && s.ends_with(".so"))
                        {
                            return std::fs::read(entry.path()).ok();
                        }
                    }
                }
            }
        }
    }
    None
}

fn load_token_2022_spl_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes = find_spl_elf("spl_token_2022").ok_or("Token-2022 ELF not found in litesvm")?;
    svm.add_program(address_from_legacy(&spl_token_2022::id()), &bytes)
        .map_err(|e| format!("{e:?}"))
}

fn load_standard_spl_token_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes = find_spl_elf("spl_token-").ok_or("SPL Token ELF not found in litesvm")?;
    svm.add_program(address_from_legacy(&spl_token_program_id()), &bytes)
        .map_err(|e| format!("{e:?}"))
}

// =============================================================================
// PDA DERIVATION
// =============================================================================

/// New vault instructions use `[b"protocol_state"]` (NOT the legacy `PROTOCOL_SEED`).
fn derive_protocol_state_v2() -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[b"protocol_state"], &program_id())
}

fn derive_market_vault(protocol_state: &LegacyPubkey, market_id: u64) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[
            b"market_vault",
            protocol_state.as_ref(),
            &market_id.to_le_bytes(),
        ],
        &program_id(),
    )
}

fn derive_user_market_position(
    market_vault: &LegacyPubkey,
    user: &LegacyPubkey,
) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[b"market_position", market_vault.as_ref(), user.as_ref()],
        &program_id(),
    )
}

// =============================================================================
// MINT / ACCOUNT CREATION HELPERS
// =============================================================================

/// Create a standard SPL Token mint via CPI (no extensions).
fn create_standard_spl_mint_via_cpi(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_kp: &Keypair,
    mint_authority: &LegacyPubkey,
    decimals: u8,
) {
    let mint_len = SplMint::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(mint_len);
    let payer_pubkey = legacy_from_signer(payer);
    let mint_pubkey = legacy_from_signer(mint_kp);

    let create_ix = solana_sdk::system_instruction::create_account(
        &payer_pubkey,
        &mint_pubkey,
        rent,
        mint_len as u64,
        &spl_token_program_id(),
    );

    // Use spl_token_2022 instruction builders which also work for the standard program
    // by swapping the program ID.
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_program_id(),
        &mint_pubkey,
        mint_authority,
        None,
        decimals,
    )
    .unwrap();

    send_legacy_tx(svm, &[payer, mint_kp], payer, &[create_ix, init_mint_ix])
        .expect("Failed to create standard SPL mint via CPI");
}

/// Create a standard SPL token account via set_account injection.
fn create_standard_spl_token_account(
    svm: &mut LiteSVM,
    address: &LegacyPubkey,
    mint: &LegacyPubkey,
    owner: &LegacyPubkey,
    amount: u64,
) {
    let mut data = vec![0u8; SplAccount::LEN];
    SplAccount::pack(
        SplAccount {
            mint: *mint,
            owner: *owner,
            amount,
            delegate: COption::None,
            state: AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut data,
    )
    .unwrap();

    let lamports = svm.minimum_balance_for_rent_exemption(SplAccount::LEN);
    svm.set_account(
        address_from_legacy(address),
        Account {
            lamports,
            data,
            owner: address_from_legacy(&spl_token_program_id()),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

/// Mint standard SPL tokens to an account via CPI.
fn mint_standard_spl_tokens(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    mint: &LegacyPubkey,
    dest: &LegacyPubkey,
    amount: u64,
) {
    let mint_authority_pubkey = legacy_from_signer(mint_authority);
    let mint_ix = spl_token_2022::instruction::mint_to(
        &spl_token_program_id(),
        mint,
        dest,
        &mint_authority_pubkey,
        &[],
        amount,
    )
    .unwrap();

    send_legacy_tx(svm, &[mint_authority], mint_authority, &[mint_ix])
        .expect("Failed to mint standard SPL tokens");
}

/// Read the token balance from any SPL / Token-2022 token account.
/// The `amount` field is at byte offset 64 in both layouts.
fn read_token_amount(svm: &LiteSVM, address: &LegacyPubkey) -> u64 {
    let account = get_account_legacy(svm, address);
    assert!(
        account.data.len() >= 72,
        "Account too small to be a token account"
    );
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

/// Set the mint authority on a standard SPL mint via CPI.
fn set_spl_mint_authority(
    svm: &mut LiteSVM,
    current_authority: &Keypair,
    mint: &LegacyPubkey,
    new_authority: &LegacyPubkey,
) {
    let current_authority_pubkey = legacy_from_signer(current_authority);
    let ix = spl_token_2022::instruction::set_authority(
        &spl_token_program_id(),
        mint,
        Some(new_authority),
        spl_token_2022::instruction::AuthorityType::MintTokens,
        &current_authority_pubkey,
        &[],
    )
    .unwrap();

    send_legacy_tx(svm, &[current_authority], current_authority, &[ix])
        .expect("Failed to set SPL mint authority");
}

// =============================================================================
// VAULT TEST ENVIRONMENT
// =============================================================================

struct VaultTestEnv {
    svm: LiteSVM,
    #[allow(dead_code)]
    admin: Keypair,
    oracle_authority: Keypair,
    user: Keypair,
    #[allow(dead_code)]
    usdc_mint: LegacyPubkey,
    vlofi_mint: LegacyPubkey,
    protocol_state_pda: LegacyPubkey,
    market_vault_pda: LegacyPubkey,
    user_position_pda: LegacyPubkey,
    vault_usdc_ata: LegacyPubkey,
    user_usdc_ata: LegacyPubkey,
    user_vlofi_ata: LegacyPubkey,
    user_ccm_ata: LegacyPubkey,
    market_id: u64,
}

/// Build the full vault test environment.
/// Returns None if program binaries are missing (graceful skip).
fn setup_vault_env(paused: bool) -> Option<VaultTestEnv> {
    let mut svm = LiteSVM::new();

    if load_program(&mut svm).is_err() {
        println!("Skip: AO program binary not found. Run `anchor build`.");
        return None;
    }
    if load_token_2022_spl_program(&mut svm).is_err() {
        println!("Skip: Token-2022 ELF not found in litesvm.");
        return None;
    }
    if load_standard_spl_token_program(&mut svm).is_err() {
        println!("Skip: Standard SPL Token ELF not found in litesvm.");
        return None;
    }

    let admin = Keypair::new();
    let oracle_authority = Keypair::new();
    let user = Keypair::new();
    let market_id: u64 = 1;

    // Airdrop SOL to all participants
    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();
    svm.airdrop(&oracle_authority.pubkey(), 10_000_000_000)
        .unwrap();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    // -----------------------------------------------------------------
    // 1. Create mints
    // -----------------------------------------------------------------

    // USDC — standard SPL Token, 6 decimals, admin is initial authority
    let usdc_mint_kp = Keypair::new();
    create_standard_spl_mint_via_cpi(
        &mut svm,
        &admin,
        &usdc_mint_kp,
        &legacy_from_signer(&admin),
        6,
    );
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);

    // CCM — standard SPL Token, 6 decimals, admin is initial authority
    let ccm_mint_kp = Keypair::new();
    create_standard_spl_mint_via_cpi(
        &mut svm,
        &admin,
        &ccm_mint_kp,
        &legacy_from_signer(&admin),
        6,
    );
    let ccm_mint = legacy_from_signer(&ccm_mint_kp);

    // vLOFI — standard SPL Token, 6 decimals, admin is initial authority
    let vlofi_mint_kp = Keypair::new();
    create_standard_spl_mint_via_cpi(
        &mut svm,
        &admin,
        &vlofi_mint_kp,
        &legacy_from_signer(&admin),
        6,
    );
    let vlofi_mint = legacy_from_signer(&vlofi_mint_kp);

    // -----------------------------------------------------------------
    // 2. Derive PDAs
    // -----------------------------------------------------------------
    let (protocol_state_pda, protocol_bump) = derive_protocol_state_v2();
    let (market_vault_pda, market_vault_bump) = derive_market_vault(&protocol_state_pda, market_id);
    let (user_position_pda, _user_position_bump) =
        derive_user_market_position(&market_vault_pda, &legacy_from_signer(&user));

    // -----------------------------------------------------------------
    // 3. Transfer mint authorities to ProtocolState PDA.
    //    vLOFI mint_to uses this authority; CCM mint authority is intentionally
    //    held by the PDA but on-chain CCM distribution is merkle-claim based.
    // -----------------------------------------------------------------
    set_spl_mint_authority(&mut svm, &admin, &ccm_mint, &protocol_state_pda);
    set_spl_mint_authority(&mut svm, &admin, &vlofi_mint, &protocol_state_pda);

    // -----------------------------------------------------------------
    // 4. Create token accounts
    // -----------------------------------------------------------------

    // Vault USDC ATA (owned by market_vault PDA)
    let vault_usdc_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(&mut svm, &vault_usdc_ata, &usdc_mint, &market_vault_pda, 0);

    // User USDC ATA — create via CPI so we can mint into it
    let user_usdc_ata_kp = Keypair::new();
    let user_usdc_ata_len = SplAccount::LEN;
    let user_usdc_ata_rent = svm.minimum_balance_for_rent_exemption(user_usdc_ata_len);
    {
        let user_pubkey = legacy_from_signer(&user);
        let user_usdc_ata_pubkey = legacy_from_signer(&user_usdc_ata_kp);
        let create_ix = solana_sdk::system_instruction::create_account(
            &user_pubkey,
            &user_usdc_ata_pubkey,
            user_usdc_ata_rent,
            user_usdc_ata_len as u64,
            &spl_token_program_id(),
        );
        let init_ix = spl_token_2022::instruction::initialize_account3(
            &spl_token_program_id(),
            &user_usdc_ata_pubkey,
            &usdc_mint,
            &user_pubkey,
        )
        .unwrap();
        send_legacy_tx(
            &mut svm,
            &[&user, &user_usdc_ata_kp],
            &user,
            &[create_ix, init_ix],
        )
        .expect("Failed to create user USDC ATA");
    }
    let user_usdc_ata = legacy_from_signer(&user_usdc_ata_kp);

    // Fund user with 100 USDC (100_000_000 in 6 decimals)
    // We need to use admin as mint authority to mint USDC. But we already
    // transferred CCM mint authority away. USDC mint authority is still admin.
    // Wait -- we only transferred CCM and vLOFI. USDC mint authority is still admin.
    // Actually no: create_standard_spl_mint_via_cpi sets admin as mint_authority,
    // and we only called set_spl_mint_authority for CCM. USDC authority is still admin.
    mint_standard_spl_tokens(&mut svm, &admin, &usdc_mint, &user_usdc_ata, 100_000_000);

    // User vLOFI ATA (standard SPL, 0 balance)
    let user_vlofi_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(
        &mut svm,
        &user_vlofi_ata,
        &vlofi_mint,
        &legacy_from_signer(&user),
        0,
    );

    // User CCM ATA (standard SPL, 0 balance)
    let user_ccm_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(
        &mut svm,
        &user_ccm_ata,
        &ccm_mint,
        &legacy_from_signer(&user),
        0,
    );

    // -----------------------------------------------------------------
    // 5. Bootstrap ProtocolState PDA
    // -----------------------------------------------------------------
    let protocol_data = ProtocolState {
        is_initialized: true,
        version: 1,
        admin: legacy_from_signer(&admin),
        publisher: legacy_from_signer(&admin),
        treasury: legacy_from_signer(&admin),
        oracle_authority: legacy_from_signer(&oracle_authority),
        mint: ccm_mint,
        paused,
        require_receipt: false,
        bump: protocol_bump,
    };
    let protocol_bytes = serialize_anchor(&protocol_data, ProtocolState::LEN);
    let protocol_lam = svm.minimum_balance_for_rent_exemption(protocol_bytes.len());
    svm.set_account(
        address_from_legacy(&protocol_state_pda),
        Account {
            lamports: protocol_lam,
            data: protocol_bytes,
            owner: address_from_legacy(&program_id()),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // -----------------------------------------------------------------
    // 6. Bootstrap MarketVault PDA
    // -----------------------------------------------------------------
    let vault_data = MarketVault {
        bump: market_vault_bump,
        market_id,
        deposit_mint: usdc_mint,
        vlofi_mint,
        vault_ata: vault_usdc_ata,
        total_deposited: 0,
        total_shares: 0,
        created_slot: 0,
        nav_per_share_bps: 0,
        last_nav_update_slot: 0,
    };
    let vault_bytes = serialize_anchor(&vault_data, MarketVault::LEN);
    let vault_lam = svm.minimum_balance_for_rent_exemption(vault_bytes.len());
    svm.set_account(
        address_from_legacy(&market_vault_pda),
        Account {
            lamports: vault_lam,
            data: vault_bytes,
            owner: address_from_legacy(&program_id()),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    Some(VaultTestEnv {
        svm,
        admin,
        oracle_authority,
        user,
        usdc_mint,
        vlofi_mint,
        protocol_state_pda,
        market_vault_pda,
        user_position_pda,
        vault_usdc_ata,
        user_usdc_ata,
        user_vlofi_ata,
        user_ccm_ata,
        market_id,
    })
}

// =============================================================================
// INSTRUCTION BUILDERS
// =============================================================================

fn build_deposit_market_ix(env: &VaultTestEnv, amount: u64) -> LegacyInstruction {
    let disc = compute_discriminator("deposit_market");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());
    data.extend_from_slice(&amount.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true), // user (signer, writable)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false), // protocol_state
            LegacyAccountMeta::new(env.market_vault_pda, false),         // market_vault
            LegacyAccountMeta::new(env.user_position_pda, false),        // user_market_position
            LegacyAccountMeta::new(env.user_usdc_ata, false),            // user_usdc_ata
            LegacyAccountMeta::new(env.vault_usdc_ata, false),           // vault_usdc_ata
            LegacyAccountMeta::new(env.vlofi_mint, false),               // vlofi_mint
            LegacyAccountMeta::new(env.user_vlofi_ata, false),           // user_vlofi_ata
            LegacyAccountMeta::new_readonly(spl_token_program_id(), false), // token_program
            LegacyAccountMeta::new_readonly(spl_token_2022::id(), false), // token_2022_program
            LegacyAccountMeta::new_readonly(system_program::ID, false),  // system_program
        ],
        data,
    }
}

fn build_update_attention_ix(
    env: &VaultTestEnv,
    user_pubkey: &Address,
    multiplier_bps: u64,
) -> LegacyInstruction {
    let disc = compute_discriminator("update_attention");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());
    data.extend_from_slice(&user_pubkey.to_bytes());
    data.extend_from_slice(&multiplier_bps.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.oracle_authority), true), // oracle_authority (signer, writable)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false), // protocol_state
            LegacyAccountMeta::new_readonly(env.market_vault_pda, false),   // market_vault
            LegacyAccountMeta::new(env.user_position_pda, false),           // user_market_position
        ],
        data,
    }
}

fn build_settle_market_ix(env: &VaultTestEnv) -> LegacyInstruction {
    let disc = compute_discriminator("settle_market");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true), // user (signer, writable)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false), // protocol_state
            LegacyAccountMeta::new(env.market_vault_pda, false),         // market_vault
            LegacyAccountMeta::new(env.user_position_pda, false),        // user_market_position
            LegacyAccountMeta::new(env.vlofi_mint, false),               // vlofi_mint
            LegacyAccountMeta::new(env.user_vlofi_ata, false),           // user_vlofi_ata
            LegacyAccountMeta::new(env.vault_usdc_ata, false),           // vault_usdc_ata
            LegacyAccountMeta::new(env.user_usdc_ata, false),            // user_usdc_ata
            LegacyAccountMeta::new_readonly(spl_token_program_id(), false), // token_program
            LegacyAccountMeta::new_readonly(spl_token_2022::id(), false), // token_2022_program
        ],
        data,
    }
}

fn build_claim_yield_ix(env: &VaultTestEnv) -> LegacyInstruction {
    let disc = compute_discriminator("claim_yield");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true), // user (signer, writable)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false), // protocol_state
            LegacyAccountMeta::new_readonly(env.market_vault_pda, false), // market_vault
            LegacyAccountMeta::new(env.user_position_pda, false),        // user_market_position
        ],
        data,
    }
}

/// Read `cumulative_claimed` from on-chain UserMarketPosition account data.
/// Offset = 8 (disc) + 1 (bump) + 32 (user) + 32 (market_vault) + 8 (deposited)
///        + 8 (shares) + 8 (multiplier) + 1 (settled) + 8 (entry_slot) = 106
fn read_cumulative_claimed(svm: &LiteSVM, position_pda: &LegacyPubkey) -> u64 {
    let account = get_account_legacy(svm, position_pda);
    let offset = 8 + 1 + 32 + 32 + 8 + 8 + 8 + 1 + 8;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

// =============================================================================
// TEST 1: FULL DEPOSIT -> UPDATE_ATTENTION -> SETTLE LOOP
// =============================================================================

#[test]
fn test_deposit_settle_full_loop() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000; // 100 USDC (6 decimals)

    // Confirm starting balances
    assert_eq!(
        read_token_amount(&env.svm, &env.user_usdc_ata),
        deposit_amount,
        "User should start with 100 USDC"
    );
    assert_eq!(
        read_token_amount(&env.svm, &env.vault_usdc_ata),
        0,
        "Vault should start empty"
    );
    println!("  Pre-deposit balances verified");

    // -----------------------------------------------------------------
    // Step 1: deposit_market — 100 USDC -> Vault, 100 vLOFI -> User
    // -----------------------------------------------------------------
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);

    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions. Run `anchor build`.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());
    println!("  deposit_market: OK");

    // Verify post-deposit balances
    let vault_usdc_balance = read_token_amount(&env.svm, &env.vault_usdc_ata);
    let user_vlofi_balance = read_token_amount(&env.svm, &env.user_vlofi_ata);
    assert_eq!(
        vault_usdc_balance, deposit_amount,
        "Vault USDC should be 100_000_000 after deposit"
    );
    assert_eq!(
        user_vlofi_balance, deposit_amount,
        "User vLOFI should be 100_000_000 after deposit (1:1)"
    );
    assert_eq!(
        read_token_amount(&env.svm, &env.user_usdc_ata),
        0,
        "User USDC should be 0 after full deposit"
    );
    println!(
        "  Post-deposit balances verified: vault_usdc={}, user_vlofi={}",
        vault_usdc_balance, user_vlofi_balance
    );

    // Verify position state was initialized
    let position_account = get_account_legacy(&env.svm, &env.user_position_pda);
    assert_eq!(
        position_account.owner,
        address_from_legacy(&program_id()),
        "Position owned by program"
    );
    println!("  UserMarketPosition PDA created and owned by program");

    // -----------------------------------------------------------------
    // Step 2: update_attention — Oracle sets 2.5x multiplier (25000 BPS)
    // -----------------------------------------------------------------
    let multiplier_bps: u64 = 25_000;
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), multiplier_bps);
    let result2 = send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    );
    assert!(
        result2.is_ok(),
        "update_attention failed: {:?}",
        result2.err()
    );
    println!("  update_attention: OK (multiplier={}bps)", multiplier_bps);

    // Verify the multiplier was written to the position
    let pos_account = get_account_legacy(&env.svm, &env.user_position_pda);
    // UserMarketPosition layout after 8-byte discriminator:
    //   bump(1) + user(32) + market_vault(32) + deposited_amount(8)
    //   + shares_minted(8) + attention_multiplier_bps(8) + settled(1) + entry_slot(8)
    // attention_multiplier_bps starts at offset: 8 + 1 + 32 + 32 + 8 + 8 = 89
    let multiplier_offset = 8 + 1 + 32 + 32 + 8 + 8;
    let stored_multiplier = u64::from_le_bytes(
        pos_account.data[multiplier_offset..multiplier_offset + 8]
            .try_into()
            .unwrap(),
    );
    assert_eq!(
        stored_multiplier, multiplier_bps,
        "Attention multiplier should be 25000 BPS"
    );
    println!("  Multiplier verified on-chain: {} bps", stored_multiplier);

    // -----------------------------------------------------------------
    // Step 3: settle_market — Burn vLOFI, return USDC, mint CCM yield
    // -----------------------------------------------------------------
    let settle_ix = build_settle_market_ix(&env);
    let result3 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix]);
    assert!(result3.is_ok(), "settle_market failed: {:?}", result3.err());
    println!("  settle_market: OK");

    // Verify post-settle balances
    // Principal returned: 100 USDC
    let user_usdc_final = read_token_amount(&env.svm, &env.user_usdc_ata);
    assert_eq!(
        user_usdc_final, deposit_amount,
        "User USDC should be 100_000_000 (principal returned)"
    );

    // vLOFI burned: should be 0
    let user_vlofi_final = read_token_amount(&env.svm, &env.user_vlofi_ata);
    assert_eq!(user_vlofi_final, 0, "User vLOFI should be 0 (all burned)");

    // CCM yield: 100_000_000 * 25000 / 10000 = 250_000_000
    // CCM is NOT minted — mint authority is revoked. settle_market only logs the
    // accrued yield; actual CCM distribution happens via merkle claims (claim_global).
    let expected_ccm_yield: u64 = deposit_amount
        .checked_mul(multiplier_bps)
        .unwrap()
        .checked_div(10_000)
        .unwrap();
    let user_ccm_final = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(
        user_ccm_final, 0,
        "User CCM should be 0 (settle no longer mints; CCM via merkle claims)"
    );
    // The yield is computed as ccm_yield = total_earned - cumulative_claimed.
    // Since no prior claim_yield was called, cumulative_claimed == 0.
    // settle_market does not update cumulative_claimed — it only logs the yield.
    let _ = expected_ccm_yield; // keep the math visible for documentation

    // Vault should be empty
    let vault_usdc_final = read_token_amount(&env.svm, &env.vault_usdc_ata);
    assert_eq!(vault_usdc_final, 0, "Vault USDC should be 0 after settle");

    // Position should be marked settled
    let pos_final = get_account_legacy(&env.svm, &env.user_position_pda);
    // settled field is at offset: 8 + 1 + 32 + 32 + 8 + 8 + 8 = 97
    let settled_offset = 8 + 1 + 32 + 32 + 8 + 8 + 8;
    let is_settled = pos_final.data[settled_offset] != 0;
    assert!(is_settled, "Position should be marked as settled");

    println!("  Post-settle verification complete:");
    println!("    User USDC:  {} (principal returned)", user_usdc_final);
    println!("    User vLOFI: {} (burned)", user_vlofi_final);
    println!(
        "    User CCM:   {} (merkle-only, no mint_to)",
        user_ccm_final
    );
    println!(
        "    Expected yield: {} (logged, claimed via merkle)",
        expected_ccm_yield
    );
    println!("    Vault USDC: {} (drained)", vault_usdc_final);
    println!("    Settled:    {}", is_settled);
    println!("\n  FULL DEPOSIT -> SETTLE LOOP: PASS");
}

// =============================================================================
// TEST 2: DEPOSIT FAILS WHEN PROTOCOL IS PAUSED
// =============================================================================

#[test]
fn test_deposit_paused_fails() {
    let Some(mut env) = setup_vault_env(true) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000;

    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);

    if let Ok(_) = result {
        // Check if program binary predates vault instructions
        panic!("deposit_market should have failed with ProtocolPaused, but succeeded");
    }

    let err_str = format!("{:?}", result.err().unwrap());

    // Graceful skip if program binary is too old
    if err_str.contains("101") || err_str.contains("FallbackNotFound") {
        println!("Skip: program binary predates vault instructions. Run `anchor build`.");
        return;
    }

    // OracleError::ProtocolPaused is variant index 2, so error code = 6000 + 2 = 6002
    assert!(
        err_str.contains("6002") || err_str.contains("ProtocolPaused"),
        "Expected ProtocolPaused (6002) error, got: {}",
        err_str
    );
    println!("  deposit_market correctly rejected with ProtocolPaused");
    println!("  PAUSED DEPOSIT REJECTION: PASS");
}

// =============================================================================
// TEST 3: MULTIPLIER EXCEEDS MAX FAILS
// =============================================================================

#[test]
fn test_multiplier_exceeds_max_fails() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000;

    // First deposit so there is a position to update
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);

    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions. Run `anchor build`.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());
    println!("  deposit_market: OK (setup for multiplier test)");

    // Attempt to set multiplier above MAX_MULTIPLIER_BPS (50000)
    let excessive_multiplier: u64 = 60_000;
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), excessive_multiplier);
    let result2 = send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    );

    assert!(
        result2.is_err(),
        "update_attention with 60000 BPS should have failed"
    );

    let err_str = format!("{:?}", result2.err().unwrap());

    // OracleError::MaxMultiplierExceeded — count the variant index in errors.rs
    // Looking at the enum: it is the last variant. Let us count...
    // Unauthorized=0, AlreadyInitialized=1, ProtocolPaused=2, InvalidPubkey=3,
    // InvalidProof=4, InvalidProofLength=5, InvalidRootSeq=6, RootTooOldOrMissing=7,
    // InvalidClaimState=8, InvalidChannelState=9, ChannelNotInitialized=10,
    // SlotMismatch=11, InvalidFeeBps=12, InvalidFeeSplit=13, CreatorFeeTooHigh=14,
    // MissingCreatorAta=15, InvalidMint=16, InvalidMintData=17,
    // MissingTransferFeeExtension=18, InvalidTokenProgram=19,
    // InsufficientTreasuryBalance=20, InsufficientTreasuryFunding=21,
    // InsufficientStake=22, TokensLocked=23, StakeBelowMinimum=24,
    // LockPeriodTooLong=25, NoPendingRewards=26,
    // ChannelStakePoolNotInitialized=27, ChannelStakePoolExists=28,
    // StakePoolNotEmpty=29, NftAlreadyMinted=30, NftNotMinted=31,
    // NftHolderMismatch=32, LockNotExpired=33, LockExpiredUseStandardUnstake=34,
    // LockReductionNotAllowed=35, SubjectMismatch=36,
    // InvalidUserHash=37, DowngradeNotAllowed=38, InvalidTier=39,
    // InvalidInputLength=40, MathOverflow=41, NoRewardsToClaim=42,
    // InvalidChannelName=43, RewardRateExceedsMaxApr=44, PoolIsShutdown=45,
    // PendingRewardsOnUnstake=46, ClaimExceedsAvailableRewards=47,
    // PoolNotShutdown=48, StakeSnapshotMismatch=49, ProofExpired=50,
    // V2ClaimsDisabled=51, GlobalRootNotInitialized=52,
    // InvalidMarketState=53, UnsupportedMarketMetric=54,
    // MarketAlreadyResolved=55, MarketNotResolvableYet=56,
    // MarketTokensNotInitialized=57, MarketTokensAlreadyInitialized=58,
    // ZeroSharesMinted=59, UnequalShareAmounts=60, MarketNotResolved=61,
    // WrongOutcomeToken=62, InsufficientVaultBalance=63,
    // WinningSharesStillOutstanding=64, VaultNotEmpty=65,
    // AlreadySettled=66, MaxMultiplierExceeded=67
    // So error code = 6000 + 67 = 6067
    assert!(
        err_str.contains("6067") || err_str.contains("MaxMultiplierExceeded"),
        "Expected MaxMultiplierExceeded (6067) error, got: {}",
        err_str
    );
    println!(
        "  update_attention correctly rejected with MaxMultiplierExceeded (60000 > 50000 cap)"
    );
    println!("  MAX MULTIPLIER ENFORCEMENT: PASS");
}

// =============================================================================
// TEST 4: CLAIM_YIELD — deprecated direct claim is rejected
// =============================================================================

#[test]
fn test_claim_yield_basic() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000; // 100 USDC
    let multiplier_bps: u64 = 20_000; // 2.0x

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);
    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());

    // Oracle sets 2.0x multiplier
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), multiplier_bps);
    assert!(
        send_legacy_tx(
            &mut env.svm,
            &[&env.oracle_authority],
            &env.oracle_authority,
            &[update_ix],
        )
        .is_ok(),
        "update_attention failed"
    );

    // Direct claim is deprecated
    let claim_ix = build_claim_yield_ix(&env);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(result.is_err(), "claim_yield should be deprecated");
    let err_str = format!("{:?}", result.err().unwrap());
    assert!(
        err_str.contains("ClaimYieldDeprecated") || err_str.contains("custom program error"),
        "Expected ClaimYieldDeprecated-style error, got: {}",
        err_str
    );

    // Position accounting remains unchanged.
    let user_ccm = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(
        user_ccm, 0,
        "CCM token balance should be 0 (no mint_to; CCM via merkle claims)"
    );

    // cumulative_claimed should remain 0 because direct claim path is disabled.
    let claimed = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(claimed, 0, "cumulative_claimed should remain unchanged");

    // vLOFI should NOT be burned (position stays open)
    let user_vlofi = read_token_amount(&env.svm, &env.user_vlofi_ata);
    assert_eq!(user_vlofi, deposit_amount, "vLOFI should be untouched");

    // USDC should still be in vault (principal not returned)
    let vault_usdc = read_token_amount(&env.svm, &env.vault_usdc_ata);
    assert_eq!(vault_usdc, deposit_amount, "Vault USDC should be untouched");

    println!("  claim_yield basic: PASS (deprecated path rejected; state unchanged)");
}

// =============================================================================
// TEST 5: CLAIM_YIELD — repeated direct claims are consistently rejected
// =============================================================================

#[test]
fn test_claim_yield_double_claim_nothing() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 50_000_000;
    let multiplier_bps: u64 = 10_000; // 1.0x

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);
    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());

    // Set multiplier
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), multiplier_bps);
    assert!(send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    )
    .is_ok());

    // First direct claim — should fail (deprecated path)
    let claim_ix = build_claim_yield_ix(&env);
    let first = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(
        first.is_err(),
        "First direct claim should fail (deprecated)"
    );

    // Expire blockhash so the second (identical) claim_yield tx gets a fresh signature
    env.svm.expire_blockhash();

    // Second direct claim — should fail again
    let claim_ix2 = build_claim_yield_ix(&env);
    let result2 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix2]);

    assert!(result2.is_err(), "Second direct claim should fail");
    let err_str = format!("{:?}", result2.err().unwrap());
    assert!(
        err_str.contains("ClaimYieldDeprecated") || err_str.contains("custom program error"),
        "Expected ClaimYieldDeprecated-style error, got: {}",
        err_str
    );

    // CCM balance should still be 0, cumulative_claimed unchanged.
    let final_ccm = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(final_ccm, 0, "CCM token should still be 0");
    let final_claimed = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(
        final_claimed, 0,
        "cumulative_claimed should remain unchanged"
    );

    println!("  claim_yield double claim: PASS (deprecated path rejected consistently)");
}

// =============================================================================
// TEST 6: CLAIM_YIELD → SETTLE — rejected direct claim does not block settle
// =============================================================================

#[test]
fn test_claim_yield_partial_then_settle() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000;

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);
    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());

    // Oracle sets 2.5x (25000 BPS)
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), 25_000);
    assert!(send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    )
    .is_ok());

    // Direct claim path is deprecated and should fail.
    let claim_ix = build_claim_yield_ix(&env);
    let claim_result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(
        claim_result.is_err(),
        "claim_yield should fail (deprecated direct path)"
    );
    let claimed_after = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(
        claimed_after, 0,
        "cumulative_claimed should remain unchanged after rejected direct claim"
    );

    // Settle returns principal and closes position.
    let settle_ix = build_settle_market_ix(&env);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix]);
    assert!(result.is_ok(), "settle_market failed: {:?}", result.err());

    // settle_market does not mint CCM; CCM remains merkle-claimed.
    let final_ccm = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(final_ccm, 0, "CCM token should still be 0 (merkle-only)");
    // cumulative_claimed remains unchanged by settle.
    let final_claimed = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(
        final_claimed, 0,
        "cumulative_claimed should stay unchanged (settle does not update it)"
    );

    // vLOFI burned, USDC returned
    assert_eq!(read_token_amount(&env.svm, &env.user_vlofi_ata), 0);
    assert_eq!(
        read_token_amount(&env.svm, &env.user_usdc_ata),
        deposit_amount
    );

    println!("  claim_yield partial → settle: PASS (direct claim rejected, settle unaffected)");
}

// =============================================================================
// TEST 7: CLAIM_YIELD — multiplier changes do not affect deprecated direct-claim safety
// =============================================================================

#[test]
fn test_claim_yield_multiplier_drops_no_panic() {
    let Some(mut env) = setup_vault_env(false) else {
        return;
    };

    let deposit_amount: u64 = 100_000_000;

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);
    if let Err(ref e) = result {
        let err_str = format!("{e:?}");
        if err_str.contains("101") || err_str.contains("FallbackNotFound") {
            println!("Skip: program binary predates vault instructions.");
            return;
        }
    }
    assert!(result.is_ok(), "deposit_market failed: {:?}", result.err());

    // Oracle sets high multiplier: 3.0x (30000 BPS)
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), 30_000);
    assert!(send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    )
    .is_ok());

    // Direct claim at 3.0x — should fail (deprecated)
    let claim_ix = build_claim_yield_ix(&env);
    let first_claim = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(first_claim.is_err(), "claim_yield at 3.0x should fail");
    let claimed_first = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(
        claimed_first, 0,
        "cumulative_claimed should remain unchanged"
    );

    // Oracle DROPS multiplier to 1.0x (10000 BPS)
    // This simulates a scenario where the oracle corrects a score downward.
    let update_ix2 = build_update_attention_ix(&env, &env.user.pubkey(), 10_000);
    assert!(send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix2],
    )
    .is_ok());

    // Expire blockhash so the second claim_yield tx gets a fresh signature
    env.svm.expire_blockhash();

    // Attempt claim again at 1.0x — still deprecated.
    let claim_ix2 = build_claim_yield_ix(&env);
    let result2 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix2]);

    assert!(result2.is_err(), "Claim after multiplier drop should fail");
    let err_str = format!("{:?}", result2.err().unwrap());
    assert!(
        err_str.contains("ClaimYieldDeprecated") || err_str.contains("custom program error"),
        "Expected ClaimYieldDeprecated-style error, got: {}",
        err_str
    );

    // Settle at 1.0x should still succeed and return principal.
    let settle_ix = build_settle_market_ix(&env);
    let result3 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix]);
    assert!(
        result3.is_ok(),
        "settle_market should succeed even with 0 CCM yield: {:?}",
        result3.err()
    );

    // CCM token stays 0, cumulative_claimed stays unchanged.
    let final_ccm = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(final_ccm, 0, "CCM token should still be 0");
    let final_claimed = read_cumulative_claimed(&env.svm, &env.user_position_pda);
    assert_eq!(
        final_claimed, 0,
        "cumulative_claimed should stay unchanged (settle does not update it)"
    );

    // USDC returned, vLOFI burned
    assert_eq!(read_token_amount(&env.svm, &env.user_vlofi_ata), 0);
    assert_eq!(
        read_token_amount(&env.svm, &env.user_usdc_ata),
        deposit_amount
    );

    println!("  claim_yield multiplier drop: PASS");
    println!("    Direct claim rejected at both multipliers (deprecated path)");
    println!("    Settle succeeded and principal returned safely");
}
