#![cfg(feature = "localtest")]
//! LiteSVM integration tests for the AO v2 (Pinocchio) program.
//!
//! Proves byte-compatibility with the existing on-chain Anchor program by
//! exercising the full lifecycle through the Pinocchio binary:
//!
//!   initialize_protocol_state
//!   -> initialize_market_vault
//!   -> deposit_market
//!   -> update_attention
//!   -> settle_market
//!   -> publish_global_root
//!   -> claim_global_v2
//!
//! Run with:
//!   cargo test --package ao-v2 --features localtest --test litesvm_lifecycle -- --nocapture

use litesvm::{types::TransactionResult, LiteSVM};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
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

fn spl_token_2022_program_id() -> LegacyPubkey {
    "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
        .parse()
        .unwrap()
}

/// Compute an Anchor instruction discriminator: SHA-256("global:<name>")[..8].
fn compute_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

/// Compute an Anchor account discriminator: SHA-256("account:<name>")[..8].
fn compute_account_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("account:{}", name);
    let hash = Sha256::digest(preimage.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
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
    let program_path = Path::new("../../target/deploy/ao_v2.so");
    if !program_path.exists() {
        return Err(format!(
            "Program not found at {:?}. Run `cargo build-sbf` first.",
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
    svm.add_program(address_from_legacy(&spl_token_2022_program_id()), &bytes)
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

fn derive_protocol_state() -> (LegacyPubkey, u8) {
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

fn derive_global_root_config(mint: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[b"global_root", mint.as_ref()],
        &program_id(),
    )
}

fn derive_claim_state_global(
    mint: &LegacyPubkey,
    wallet: &LegacyPubkey,
) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[b"claim_global", mint.as_ref(), wallet.as_ref()],
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
// MERKLE TREE HELPERS (for claim_global_v2 test)
// =============================================================================

/// Keccak256 hash of concatenated slices (matches on-chain keccak_hashv).
fn keccak_hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    for p in parts {
        hasher.update(p);
    }
    let out = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out[..32]);
    arr
}

/// Compute V5 global leaf: keccak(domain || mint || root_seq || wallet || base_yield || attention_bonus)
fn compute_global_leaf_v5(
    mint: &[u8; 32],
    root_seq: u64,
    wallet: &[u8; 32],
    base_yield: u64,
    attention_bonus: u64,
) -> [u8; 32] {
    keccak_hashv(&[
        b"TWZRD:GLOBAL_V5",
        mint,
        &root_seq.to_le_bytes(),
        wallet,
        &base_yield.to_le_bytes(),
        &attention_bonus.to_le_bytes(),
    ])
}

/// Build a minimal 1-leaf merkle tree. Returns (root, proof).
/// For a single leaf, the root IS the leaf (proof is empty).
#[allow(dead_code)]
fn build_single_leaf_merkle(leaf: [u8; 32]) -> ([u8; 32], Vec<[u8; 32]>) {
    // Single-leaf tree: root = leaf, no proof siblings needed
    (leaf, vec![])
}

/// Build a 2-leaf merkle tree. Returns (root, proof_for_leaf_0).
fn build_two_leaf_merkle(leaf0: [u8; 32], leaf1: [u8; 32]) -> ([u8; 32], Vec<[u8; 32]>) {
    let (a, b) = if leaf0 <= leaf1 {
        (leaf0, leaf1)
    } else {
        (leaf1, leaf0)
    };
    let root = keccak_hashv(&[&a, &b]);
    // Proof for leaf0 is just leaf1
    (root, vec![leaf1])
}

// =============================================================================
// LIFECYCLE TEST ENVIRONMENT
// =============================================================================

struct LifecycleEnv {
    svm: LiteSVM,
    admin: Keypair,
    oracle_authority: Keypair,
    publisher: Keypair,
    user: Keypair,
    usdc_mint: LegacyPubkey,
    vlofi_mint: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    protocol_state_pda: LegacyPubkey,
    market_vault_pda: LegacyPubkey,
    user_position_pda: LegacyPubkey,
    vault_usdc_ata: LegacyPubkey,
    user_usdc_ata: LegacyPubkey,
    user_vlofi_ata: LegacyPubkey,
    market_id: u64,
    // Global root / claim fields
    global_root_config_pda: LegacyPubkey,
    treasury_ccm_ata: LegacyPubkey,
    user_ccm_ata: LegacyPubkey,
    claim_state_pda: LegacyPubkey,
}

/// Build the full lifecycle test environment. Returns None if binaries missing.
fn setup_lifecycle_env() -> Option<LifecycleEnv> {
    let mut svm = LiteSVM::new();

    if load_program(&mut svm).is_err() {
        println!("Skip: AO v2 program binary not found. Run `cargo build-sbf`.");
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
    let publisher = Keypair::new();
    let user = Keypair::new();
    let market_id: u64 = 1;

    // Airdrop SOL to all participants
    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();
    svm.airdrop(&oracle_authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&publisher.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&user.pubkey(), 100_000_000_000).unwrap();

    // -----------------------------------------------------------------
    // 1. Create mints (all standard SPL for test simplicity)
    // -----------------------------------------------------------------

    // USDC -- standard SPL Token, 6 decimals
    let usdc_mint_kp = Keypair::new();
    create_standard_spl_mint_via_cpi(
        &mut svm,
        &admin,
        &usdc_mint_kp,
        &legacy_from_signer(&admin),
        6,
    );
    let usdc_mint = legacy_from_signer(&usdc_mint_kp);

    // CCM -- use 9 decimals to match on-chain CCM assumptions in claim paths.
    // (Mainnet CCM is Token-2022 with 9 decimals; we use standard SPL here for
    // test harness simplicity but keep decimals identical.)
    let ccm_mint_kp = Keypair::new();
    create_standard_spl_mint_via_cpi(
        &mut svm,
        &admin,
        &ccm_mint_kp,
        &legacy_from_signer(&admin),
        9,
    );
    let ccm_mint = legacy_from_signer(&ccm_mint_kp);

    // vLOFI -- standard SPL Token, 6 decimals
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
    let (protocol_state_pda, _protocol_bump) = derive_protocol_state();
    let (market_vault_pda, _market_vault_bump) =
        derive_market_vault(&protocol_state_pda, market_id);
    let (user_position_pda, _) =
        derive_user_market_position(&market_vault_pda, &legacy_from_signer(&user));
    let (global_root_config_pda, _) = derive_global_root_config(&ccm_mint);
    let (claim_state_pda, _) =
        derive_claim_state_global(&ccm_mint, &legacy_from_signer(&user));

    // -----------------------------------------------------------------
    // 3. Transfer vLOFI mint authority to ProtocolState PDA
    //    (deposit_market needs PDA to mint vLOFI via MintTo CPI)
    // -----------------------------------------------------------------
    set_spl_mint_authority(&mut svm, &admin, &vlofi_mint, &protocol_state_pda);

    // CCM mint authority stays with admin for now (treasury funding)
    // We'll set it to protocol_state_pda later if needed for claims,
    // but claim_global uses transfer from treasury, not mint.

    // -----------------------------------------------------------------
    // 4. Create token accounts
    // -----------------------------------------------------------------

    // Vault USDC ATA (owned by market_vault PDA)
    let vault_usdc_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(
        &mut svm,
        &vault_usdc_ata,
        &usdc_mint,
        &market_vault_pda,
        0,
    );

    // User USDC ATA -- create via CPI so we can mint into it
    let user_usdc_ata_kp = Keypair::new();
    {
        let user_pubkey = legacy_from_signer(&user);
        let user_usdc_ata_pubkey = legacy_from_signer(&user_usdc_ata_kp);
        let user_usdc_ata_len = SplAccount::LEN;
        let user_usdc_ata_rent = svm.minimum_balance_for_rent_exemption(user_usdc_ata_len);
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

    // Fund user with 100 USDC
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

    // Treasury CCM ATA (owned by protocol_state_pda, holds CCM for claims)
    let treasury_ccm_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(
        &mut svm,
        &treasury_ccm_ata,
        &ccm_mint,
        &protocol_state_pda,
        0,
    );

    // Fund treasury with CCM (admin mints 1M CCM into treasury)
    mint_standard_spl_tokens(
        &mut svm,
        &admin,
        &ccm_mint,
        &treasury_ccm_ata,
        1_000_000_000_000,
    );

    // User CCM ATA
    let user_ccm_ata = LegacyPubkey::new_unique();
    create_standard_spl_token_account(
        &mut svm,
        &user_ccm_ata,
        &ccm_mint,
        &legacy_from_signer(&user),
        0,
    );

    Some(LifecycleEnv {
        svm,
        admin,
        oracle_authority,
        publisher,
        user,
        usdc_mint,
        vlofi_mint,
        ccm_mint,
        protocol_state_pda,
        market_vault_pda,
        user_position_pda,
        vault_usdc_ata,
        user_usdc_ata,
        user_vlofi_ata,
        market_id,
        global_root_config_pda,
        treasury_ccm_ata,
        user_ccm_ata,
        claim_state_pda,
    })
}

// =============================================================================
// INSTRUCTION BUILDERS
// =============================================================================

fn build_initialize_protocol_state_ix(
    admin: &Keypair,
    protocol_state_pda: &LegacyPubkey,
    publisher: &LegacyPubkey,
    treasury: &LegacyPubkey,
    oracle_authority: &LegacyPubkey,
    ccm_mint: &LegacyPubkey,
) -> LegacyInstruction {
    let disc = compute_discriminator("initialize_protocol_state");
    let mut data = disc.to_vec();
    // ix_data: publisher(32) + treasury(32) + oracle_authority(32) + ccm_mint(32)
    data.extend_from_slice(publisher.as_ref());
    data.extend_from_slice(treasury.as_ref());
    data.extend_from_slice(oracle_authority.as_ref());
    data.extend_from_slice(ccm_mint.as_ref());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(admin), true),         // admin (signer, mut)
            LegacyAccountMeta::new(*protocol_state_pda, false),              // protocol_state (mut)
            LegacyAccountMeta::new_readonly(system_program::ID, false),      // system_program
        ],
        data,
    }
}

fn build_initialize_market_vault_ix(
    admin: &Keypair,
    protocol_state_pda: &LegacyPubkey,
    market_vault_pda: &LegacyPubkey,
    deposit_mint: &LegacyPubkey,
    vlofi_mint: &LegacyPubkey,
    vault_ata: &LegacyPubkey,
    market_id: u64,
) -> LegacyInstruction {
    let disc = compute_discriminator("initialize_market_vault");
    let mut data = disc.to_vec();
    data.extend_from_slice(&market_id.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(admin), true),         // admin (signer, mut)
            LegacyAccountMeta::new_readonly(*protocol_state_pda, false),     // protocol_state
            LegacyAccountMeta::new(*market_vault_pda, false),                // market_vault (mut)
            LegacyAccountMeta::new_readonly(*deposit_mint, false),           // deposit_mint
            LegacyAccountMeta::new_readonly(*vlofi_mint, false),             // vlofi_mint
            LegacyAccountMeta::new_readonly(*vault_ata, false),              // vault_ata
            LegacyAccountMeta::new_readonly(system_program::ID, false),      // system_program
        ],
        data,
    }
}

fn build_deposit_market_ix(env: &LifecycleEnv, amount: u64) -> LegacyInstruction {
    let disc = compute_discriminator("deposit_market");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());
    data.extend_from_slice(&amount.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true),     // user (signer, mut)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false),  // protocol_state
            LegacyAccountMeta::new(env.market_vault_pda, false),             // market_vault (mut)
            LegacyAccountMeta::new(env.user_position_pda, false),            // user_market_position (mut)
            LegacyAccountMeta::new(env.user_usdc_ata, false),                // user_usdc_ata (mut)
            LegacyAccountMeta::new(env.vault_usdc_ata, false),               // vault_usdc_ata (mut)
            LegacyAccountMeta::new(env.vlofi_mint, false),                   // vlofi_mint (mut)
            LegacyAccountMeta::new(env.user_vlofi_ata, false),               // user_vlofi_ata (mut)
            LegacyAccountMeta::new_readonly(spl_token_program_id(), false),  // token_program
            LegacyAccountMeta::new_readonly(spl_token_2022_program_id(), false), // token_2022_program
            LegacyAccountMeta::new_readonly(system_program::ID, false),      // system_program
        ],
        data,
    }
}

fn build_update_attention_ix(
    env: &LifecycleEnv,
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
            LegacyAccountMeta::new(legacy_from_signer(&env.oracle_authority), true), // oracle_authority (signer, mut)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false),          // protocol_state
            LegacyAccountMeta::new_readonly(env.market_vault_pda, false),            // market_vault
            LegacyAccountMeta::new(env.user_position_pda, false),                    // user_market_position (mut)
        ],
        data,
    }
}

fn build_settle_market_ix(env: &LifecycleEnv) -> LegacyInstruction {
    let disc = compute_discriminator("settle_market");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true),     // user (signer, mut)
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false),  // protocol_state
            LegacyAccountMeta::new(env.market_vault_pda, false),             // market_vault (mut)
            LegacyAccountMeta::new(env.user_position_pda, false),            // user_market_position (mut)
            LegacyAccountMeta::new(env.vlofi_mint, false),                   // vlofi_mint (mut)
            LegacyAccountMeta::new(env.user_vlofi_ata, false),               // user_vlofi_ata (mut)
            LegacyAccountMeta::new(env.vault_usdc_ata, false),               // vault_usdc_ata (mut)
            LegacyAccountMeta::new(env.user_usdc_ata, false),                // user_usdc_ata (mut)
            LegacyAccountMeta::new_readonly(spl_token_program_id(), false),  // token_program
            LegacyAccountMeta::new_readonly(spl_token_2022_program_id(), false), // token_2022_program
        ],
        data,
    }
}

fn build_initialize_global_root_ix(
    payer: &Keypair,
    protocol_state_pda: &LegacyPubkey,
    global_root_config_pda: &LegacyPubkey,
) -> LegacyInstruction {
    let disc = compute_discriminator("initialize_global_root");
    let data = disc.to_vec();

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(payer), true),         // payer (signer, mut)
            LegacyAccountMeta::new_readonly(*protocol_state_pda, false),     // protocol_state
            LegacyAccountMeta::new(*global_root_config_pda, false),          // global_root_config (mut)
            LegacyAccountMeta::new_readonly(system_program::ID, false),      // system_program
        ],
        data,
    }
}

fn build_publish_global_root_ix(
    publisher: &Keypair,
    protocol_state_pda: &LegacyPubkey,
    global_root_config_pda: &LegacyPubkey,
    root_seq: u64,
    root: [u8; 32],
    dataset_hash: [u8; 32],
) -> LegacyInstruction {
    let disc = compute_discriminator("publish_global_root");
    let mut data = disc.to_vec();
    data.extend_from_slice(&root_seq.to_le_bytes());
    data.extend_from_slice(&root);
    data.extend_from_slice(&dataset_hash);

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(publisher), true),     // payer (signer, mut)
            LegacyAccountMeta::new_readonly(*protocol_state_pda, false),     // protocol_state
            LegacyAccountMeta::new(*global_root_config_pda, false),          // global_root_config (mut)
        ],
        data,
    }
}

fn build_claim_global_v2_ix(
    claimer: &Keypair,
    protocol_state_pda: &LegacyPubkey,
    global_root_config_pda: &LegacyPubkey,
    claim_state_pda: &LegacyPubkey,
    ccm_mint: &LegacyPubkey,
    treasury_ata: &LegacyPubkey,
    claimer_ata: &LegacyPubkey,
    root_seq: u64,
    base_yield: u64,
    attention_bonus: u64,
    proof: &[[u8; 32]],
) -> LegacyInstruction {
    let disc = compute_discriminator("claim_global_v2");
    let mut data = disc.to_vec();
    data.extend_from_slice(&root_seq.to_le_bytes());
    data.extend_from_slice(&base_yield.to_le_bytes());
    data.extend_from_slice(&attention_bonus.to_le_bytes());
    // Borsh Vec prefix: u32 LE length
    data.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    for node in proof {
        data.extend_from_slice(node);
    }

    // Associated token program ID (passed but not used for CPI in test path)
    let associated_token_program: LegacyPubkey =
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
            .parse()
            .unwrap();

    LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(claimer), true),       // claimer (signer, mut)
            LegacyAccountMeta::new(*protocol_state_pda, false),              // protocol_state (mut for PDA signer)
            LegacyAccountMeta::new_readonly(*global_root_config_pda, false), // global_root_config
            LegacyAccountMeta::new(*claim_state_pda, false),                 // claim_state (mut, init_if_needed)
            LegacyAccountMeta::new_readonly(*ccm_mint, false),               // mint
            LegacyAccountMeta::new(*treasury_ata, false),                    // treasury_ata (mut)
            LegacyAccountMeta::new(*claimer_ata, false),                     // claimer_ata (mut)
            LegacyAccountMeta::new_readonly(spl_token_program_id(), false),  // token_program
            LegacyAccountMeta::new_readonly(associated_token_program, false), // associated_token_program
            LegacyAccountMeta::new_readonly(system_program::ID, false),      // system_program
        ],
        data,
    }
}

// =============================================================================
// ACCOUNT DATA READERS (byte-offset based, matching state.rs layouts)
// =============================================================================

/// ProtocolState layout:
///   0: disc(8), 8: is_initialized(1), 9: version(1), 10: admin(32),
///   42: publisher(32), 74: treasury(32), 106: oracle_authority(32),
///   138: mint(32), 170: paused(1), 171: require_receipt(1), 172: bump(1)
#[allow(dead_code)]
mod ps_offsets {
    pub const DISC: usize = 0;
    pub const IS_INITIALIZED: usize = 8;
    pub const VERSION: usize = 9;
    pub const ADMIN: usize = 10;
    pub const PUBLISHER: usize = 42;
    pub const TREASURY: usize = 74;
    pub const ORACLE_AUTHORITY: usize = 106;
    pub const MINT: usize = 138;
    pub const PAUSED: usize = 170;
    pub const BUMP: usize = 172;
    pub const LEN: usize = 173;
}

/// MarketVault layout:
///   0: disc(8), 8: bump(1), 9: market_id(8), 17: deposit_mint(32),
///   49: vlofi_mint(32), 81: vault_ata(32), 113: total_deposited(8),
///   121: total_shares(8), 129: created_slot(8), 137: nav_per_share_bps(8),
///   145: last_nav_update_slot(8)
#[allow(dead_code)]
mod mv_offsets {
    pub const DISC: usize = 0;
    pub const BUMP: usize = 8;
    pub const MARKET_ID: usize = 9;
    pub const DEPOSIT_MINT: usize = 17;
    pub const VLOFI_MINT: usize = 49;
    pub const VAULT_ATA: usize = 81;
    pub const TOTAL_DEPOSITED: usize = 113;
    pub const TOTAL_SHARES: usize = 121;
    pub const CREATED_SLOT: usize = 129;
    pub const LEN: usize = 153;
}

/// UserMarketPosition layout:
///   0: disc(8), 8: bump(1), 9: user(32), 41: market_vault(32),
///   73: deposited_amount(8), 81: shares_minted(8),
///   89: attention_multiplier_bps(8), 97: settled(1),
///   98: entry_slot(8), 106: cumulative_claimed(8)
#[allow(dead_code)]
mod pos_offsets {
    pub const USER: usize = 9;
    pub const MARKET_VAULT: usize = 41;
    pub const DEPOSITED_AMOUNT: usize = 73;
    pub const SHARES_MINTED: usize = 81;
    pub const ATTENTION_MULTIPLIER_BPS: usize = 89;
    pub const SETTLED: usize = 97;
    pub const ENTRY_SLOT: usize = 98;
    pub const LEN: usize = 114;
}

/// GlobalRootConfig layout:
///   0: disc(8), 8: version(1), 9: bump(1), 10: mint(32),
///   42: latest_root_seq(8), 50: roots(4 x 80 = 320)
#[allow(dead_code)]
mod grc_offsets {
    pub const VERSION: usize = 8;
    pub const MINT: usize = 10;
    pub const LATEST_ROOT_SEQ: usize = 42;
    pub const ROOTS_START: usize = 50;
    pub const ROOT_ENTRY_SIZE: usize = 80;
    pub const LEN: usize = 370;
}

/// ClaimStateGlobal layout:
///   0: disc(8), 8: version(1), 9: bump(1), 10: mint(32),
///   42: wallet(32), 74: claimed_total(8), 82: last_claim_seq(8)
#[allow(dead_code)]
mod csg_offsets {
    pub const CLAIMED_TOTAL: usize = 74;
    pub const LAST_CLAIM_SEQ: usize = 82;
    pub const LEN: usize = 90;
}

fn read_u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

fn read_pubkey_at(data: &[u8], offset: usize) -> [u8; 32] {
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&data[offset..offset + 32]);
    pk
}

// =============================================================================
// TEST 1: INITIALIZE PROTOCOL STATE
// =============================================================================

#[test]
fn test_initialize_protocol_state() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    let ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin), // treasury = admin for test
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ix]);
    assert!(
        result.is_ok(),
        "initialize_protocol_state failed: {:?}",
        result.err()
    );
    println!("  initialize_protocol_state: OK");

    // Verify ProtocolState PDA data
    let account = get_account_legacy(&env.svm, &env.protocol_state_pda);
    assert_eq!(
        account.owner,
        address_from_legacy(&program_id()),
        "ProtocolState must be owned by AO program"
    );
    assert!(
        account.data.len() >= ps_offsets::LEN,
        "ProtocolState account too small: {} < {}",
        account.data.len(),
        ps_offsets::LEN
    );

    let data = &account.data;

    // Check discriminator
    let expected_disc = compute_account_discriminator("ProtocolState");
    assert_eq!(
        &data[0..8],
        &expected_disc,
        "ProtocolState discriminator mismatch"
    );

    // Check fields
    assert_eq!(data[ps_offsets::IS_INITIALIZED], 1, "is_initialized should be 1");
    assert_eq!(data[ps_offsets::VERSION], 1, "version should be 1");
    assert_eq!(
        read_pubkey_at(data, ps_offsets::ADMIN),
        legacy_from_signer(&env.admin).to_bytes(),
        "admin mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, ps_offsets::PUBLISHER),
        legacy_from_signer(&env.publisher).to_bytes(),
        "publisher mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, ps_offsets::ORACLE_AUTHORITY),
        legacy_from_signer(&env.oracle_authority).to_bytes(),
        "oracle_authority mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, ps_offsets::MINT),
        env.ccm_mint.to_bytes(),
        "CCM mint mismatch"
    );
    assert_eq!(data[ps_offsets::PAUSED], 0, "should not be paused");
    assert_ne!(data[ps_offsets::BUMP], 0, "bump should be nonzero");

    println!("  ProtocolState PDA verification: PASS");
    println!("  INITIALIZE PROTOCOL STATE: PASS\n");
}

// =============================================================================
// TEST 2: INITIALIZE MARKET VAULT
// =============================================================================

#[test]
fn test_initialize_market_vault() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // First initialize protocol state
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed in setup");
    println!("  Protocol state initialized (setup)");

    // Now initialize market vault
    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix]);
    assert!(
        result.is_ok(),
        "initialize_market_vault failed: {:?}",
        result.err()
    );
    println!("  initialize_market_vault: OK");

    // Verify MarketVault PDA data
    let account = get_account_legacy(&env.svm, &env.market_vault_pda);
    assert_eq!(
        account.owner,
        address_from_legacy(&program_id()),
        "MarketVault must be owned by AO program"
    );
    assert!(
        account.data.len() >= mv_offsets::LEN,
        "MarketVault account too small: {} < {}",
        account.data.len(),
        mv_offsets::LEN
    );

    let data = &account.data;

    // Check discriminator
    let expected_disc = compute_account_discriminator("MarketVault");
    assert_eq!(
        &data[0..8],
        &expected_disc,
        "MarketVault discriminator mismatch"
    );

    // Check fields
    assert_ne!(data[mv_offsets::BUMP], 0, "bump should be nonzero");
    assert_eq!(
        read_u64_at(data, mv_offsets::MARKET_ID),
        env.market_id,
        "market_id mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, mv_offsets::DEPOSIT_MINT),
        env.usdc_mint.to_bytes(),
        "deposit_mint mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, mv_offsets::VLOFI_MINT),
        env.vlofi_mint.to_bytes(),
        "vlofi_mint mismatch"
    );
    assert_eq!(
        read_pubkey_at(data, mv_offsets::VAULT_ATA),
        env.vault_usdc_ata.to_bytes(),
        "vault_ata mismatch"
    );
    assert_eq!(
        read_u64_at(data, mv_offsets::TOTAL_DEPOSITED),
        0,
        "total_deposited should be 0"
    );
    assert_eq!(
        read_u64_at(data, mv_offsets::TOTAL_SHARES),
        0,
        "total_shares should be 0"
    );

    println!("  MarketVault PDA verification: PASS");
    println!("  INITIALIZE MARKET VAULT: PASS\n");
}

// =============================================================================
// TEST 3: FULL LIFECYCLE — deposit -> update_attention -> settle
// =============================================================================

#[test]
fn test_deposit_update_settle_lifecycle() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup: initialize protocol state + market vault
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");
    println!("  Setup: protocol_state + market_vault initialized");

    let deposit_amount: u64 = 100_000_000; // 100 USDC

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
    // Step 1: deposit_market -- 100 USDC -> Vault, 100 vLOFI -> User
    // -----------------------------------------------------------------
    let deposit_ix = build_deposit_market_ix(&env, deposit_amount);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);
    assert!(
        result.is_ok(),
        "deposit_market failed: {:?}",
        result.err()
    );
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
        "User vLOFI should be 100_000_000 after deposit (1:1 at NAV=10000)"
    );
    assert_eq!(
        read_token_amount(&env.svm, &env.user_usdc_ata),
        0,
        "User USDC should be 0 after full deposit"
    );
    println!(
        "  Post-deposit: vault_usdc={}, user_vlofi={}",
        vault_usdc_balance, user_vlofi_balance
    );

    // Verify position state
    let position_account = get_account_legacy(&env.svm, &env.user_position_pda);
    assert_eq!(
        position_account.owner,
        address_from_legacy(&program_id()),
        "Position must be owned by AO program"
    );
    let pos_data = &position_account.data;

    let expected_pos_disc = compute_account_discriminator("UserMarketPosition");
    assert_eq!(
        &pos_data[0..8],
        &expected_pos_disc,
        "UserMarketPosition discriminator mismatch"
    );
    assert_eq!(
        read_pubkey_at(pos_data, pos_offsets::USER),
        legacy_from_signer(&env.user).to_bytes(),
        "Position user mismatch"
    );
    assert_eq!(
        read_u64_at(pos_data, pos_offsets::DEPOSITED_AMOUNT),
        deposit_amount,
        "Position deposited_amount mismatch"
    );
    assert_eq!(
        read_u64_at(pos_data, pos_offsets::SHARES_MINTED),
        deposit_amount,
        "Position shares_minted should match deposit at 1:1 NAV"
    );
    assert_eq!(
        pos_data[pos_offsets::SETTLED],
        0,
        "Position should not be settled"
    );
    println!("  UserMarketPosition PDA verified");

    // Verify MarketVault accounting updated
    let mv_account = get_account_legacy(&env.svm, &env.market_vault_pda);
    assert_eq!(
        read_u64_at(&mv_account.data, mv_offsets::TOTAL_DEPOSITED),
        deposit_amount,
        "MarketVault total_deposited should equal deposit"
    );
    assert_eq!(
        read_u64_at(&mv_account.data, mv_offsets::TOTAL_SHARES),
        deposit_amount,
        "MarketVault total_shares should equal deposit at 1:1"
    );

    // -----------------------------------------------------------------
    // Step 2: update_attention -- Oracle sets 2.5x multiplier (25000 BPS)
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

    // Verify multiplier was written
    let pos_account2 = get_account_legacy(&env.svm, &env.user_position_pda);
    let stored_multiplier = read_u64_at(
        &pos_account2.data,
        pos_offsets::ATTENTION_MULTIPLIER_BPS,
    );
    assert_eq!(
        stored_multiplier, multiplier_bps,
        "Attention multiplier should be 25000 BPS"
    );
    println!("  Multiplier verified on-chain: {} bps", stored_multiplier);

    // -----------------------------------------------------------------
    // Step 3: settle_market -- Burn vLOFI, return USDC
    // -----------------------------------------------------------------
    let settle_ix = build_settle_market_ix(&env);
    let result3 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix]);
    assert!(
        result3.is_ok(),
        "settle_market failed: {:?}",
        result3.err()
    );
    println!("  settle_market: OK");

    // Verify post-settle balances
    let user_usdc_final = read_token_amount(&env.svm, &env.user_usdc_ata);
    assert_eq!(
        user_usdc_final, deposit_amount,
        "User USDC should be 100_000_000 (principal returned)"
    );

    let user_vlofi_final = read_token_amount(&env.svm, &env.user_vlofi_ata);
    assert_eq!(
        user_vlofi_final, 0,
        "User vLOFI should be 0 (all burned)"
    );

    let vault_usdc_final = read_token_amount(&env.svm, &env.vault_usdc_ata);
    assert_eq!(
        vault_usdc_final, 0,
        "Vault USDC should be 0 after settle"
    );

    // Position should be marked settled
    let pos_final = get_account_legacy(&env.svm, &env.user_position_pda);
    assert_ne!(
        pos_final.data[pos_offsets::SETTLED],
        0,
        "Position should be marked as settled"
    );

    // Vault accounting should be zeroed
    let mv_final = get_account_legacy(&env.svm, &env.market_vault_pda);
    assert_eq!(
        read_u64_at(&mv_final.data, mv_offsets::TOTAL_DEPOSITED),
        0,
        "MarketVault total_deposited should be 0 after settle"
    );
    assert_eq!(
        read_u64_at(&mv_final.data, mv_offsets::TOTAL_SHARES),
        0,
        "MarketVault total_shares should be 0 after settle"
    );

    println!("  Post-settle verification:");
    println!("    User USDC:  {} (principal returned)", user_usdc_final);
    println!("    User vLOFI: {} (burned)", user_vlofi_final);
    println!("    Vault USDC: {} (drained)", vault_usdc_final);
    println!("    Settled:    true");
    println!("  FULL DEPOSIT -> UPDATE -> SETTLE LIFECYCLE: PASS\n");
}

// =============================================================================
// TEST 4: DEPOSIT FAILS WHEN PROTOCOL IS PAUSED
// =============================================================================

#[test]
fn test_deposit_paused_fails() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Initialize protocol state with paused=false, then pause it
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    // Manually set paused=1 in the ProtocolState PDA
    let mut ps_account = get_account_legacy(&env.svm, &env.protocol_state_pda);
    ps_account.data[ps_offsets::PAUSED] = 1;
    env.svm
        .set_account(
            address_from_legacy(&env.protocol_state_pda),
            ps_account,
        )
        .unwrap();

    // Initialize market vault
    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");

    // Try to deposit -- should fail with ProtocolPaused
    let deposit_ix = build_deposit_market_ix(&env, 100_000_000);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix]);

    assert!(
        result.is_err(),
        "deposit_market should have failed with ProtocolPaused"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::ProtocolPaused = variant 2, error code = 6002
    assert!(
        err_str.contains("6002") || err_str.contains("ProtocolPaused"),
        "Expected ProtocolPaused (6002) error, got: {}",
        err_str
    );

    println!("  deposit_market correctly rejected with ProtocolPaused (6002)");
    println!("  PAUSED DEPOSIT REJECTION: PASS\n");
}

// =============================================================================
// TEST 5: MULTIPLIER EXCEEDS MAX FAILS
// =============================================================================

#[test]
fn test_multiplier_exceeds_max_fails() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");

    // Deposit first so there is a position to update
    let deposit_ix = build_deposit_market_ix(&env, 100_000_000);
    send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix])
        .expect("deposit_market failed");
    println!("  Setup: deposited 100 USDC");

    // Attempt multiplier above MAX_MULTIPLIER_BPS (50000)
    let excessive_multiplier: u64 = 60_000;
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), excessive_multiplier);
    let result = send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    );

    assert!(
        result.is_err(),
        "update_attention with 60000 BPS should have failed"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::MaxMultiplierExceeded = variant 67, code = 6067
    assert!(
        err_str.contains("6067") || err_str.contains("MaxMultiplierExceeded"),
        "Expected MaxMultiplierExceeded (6067), got: {}",
        err_str
    );

    println!("  update_attention correctly rejected 60000 BPS (max 50000)");
    println!("  MAX MULTIPLIER ENFORCEMENT: PASS\n");
}

// =============================================================================
// TEST 6: MULTIPLIER BELOW MIN FAILS
// =============================================================================

#[test]
fn test_multiplier_below_min_fails() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, 100_000_000);
    send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix])
        .expect("deposit_market failed");

    // Attempt multiplier below MIN_MULTIPLIER_BPS (10000)
    let low_multiplier: u64 = 5_000;
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), low_multiplier);
    let result = send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    );

    assert!(
        result.is_err(),
        "update_attention with 5000 BPS should have failed"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::MultiplierBelowMinimum = variant 79, code = 6079
    assert!(
        err_str.contains("6079") || err_str.contains("MultiplierBelowMinimum"),
        "Expected MultiplierBelowMinimum (6079), got: {}",
        err_str
    );

    println!("  update_attention correctly rejected 5000 BPS (min 10000)");
    println!("  MIN MULTIPLIER ENFORCEMENT: PASS\n");
}

// =============================================================================
// TEST 7: CLAIM_YIELD IS DEPRECATED
// =============================================================================

#[test]
fn test_claim_yield_deprecated() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");

    // Deposit + set multiplier
    let deposit_ix = build_deposit_market_ix(&env, 100_000_000);
    send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix])
        .expect("deposit_market failed");

    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), 20_000);
    send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    )
    .expect("update_attention failed");

    // Try claim_yield -- should fail (deprecated)
    let disc = compute_discriminator("claim_yield");
    let mut data = disc.to_vec();
    data.extend_from_slice(&env.market_id.to_le_bytes());

    let claim_ix = LegacyInstruction {
        program_id: program_id(),
        accounts: vec![
            LegacyAccountMeta::new(legacy_from_signer(&env.user), true),
            LegacyAccountMeta::new_readonly(env.protocol_state_pda, false),
            LegacyAccountMeta::new_readonly(env.market_vault_pda, false),
            LegacyAccountMeta::new(env.user_position_pda, false),
        ],
        data,
    };

    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(
        result.is_err(),
        "claim_yield should be deprecated and rejected"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::ClaimYieldDeprecated = variant 83, code = 6083
    assert!(
        err_str.contains("6083") || err_str.contains("ClaimYieldDeprecated"),
        "Expected ClaimYieldDeprecated (6083), got: {}",
        err_str
    );

    println!("  claim_yield correctly rejected (deprecated, code 6083)");
    println!("  CLAIM_YIELD DEPRECATED: PASS\n");
}

// =============================================================================
// TEST 8: PUBLISH GLOBAL ROOT + VERIFY RING BUFFER
// =============================================================================

#[test]
fn test_publish_global_root() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Initialize protocol state (publisher = admin for simplicity, use
    // admin as the signer for publish)
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.admin), // publisher = admin
        &legacy_from_signer(&env.admin), // treasury
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");
    println!("  Protocol state initialized (publisher=admin)");

    // Initialize global root config
    let grc_ix = build_initialize_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
    );
    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[grc_ix]);
    assert!(
        result.is_ok(),
        "initialize_global_root failed: {:?}",
        result.err()
    );
    println!("  initialize_global_root: OK");

    // Verify initial state
    let grc_account = get_account_legacy(&env.svm, &env.global_root_config_pda);
    assert_eq!(
        grc_account.owner,
        address_from_legacy(&program_id()),
        "GlobalRootConfig must be owned by AO program"
    );
    assert!(
        grc_account.data.len() >= grc_offsets::LEN,
        "GlobalRootConfig too small"
    );

    let expected_grc_disc = compute_account_discriminator("GlobalRootConfig");
    assert_eq!(
        &grc_account.data[0..8],
        &expected_grc_disc,
        "GlobalRootConfig discriminator mismatch"
    );
    assert_eq!(
        grc_account.data[grc_offsets::VERSION],
        1,
        "GlobalRootConfig version should be 1"
    );
    assert_eq!(
        read_u64_at(&grc_account.data, grc_offsets::LATEST_ROOT_SEQ),
        0,
        "Initial latest_root_seq should be 0"
    );

    // Publish root #1
    let root_1: [u8; 32] = [0xAA; 32];
    let dataset_hash_1: [u8; 32] = [0xBB; 32];
    let pub_ix_1 = build_publish_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        1, // root_seq = 1 (must be current + 1)
        root_1,
        dataset_hash_1,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[pub_ix_1]);
    assert!(
        result.is_ok(),
        "publish_global_root (seq=1) failed: {:?}",
        result.err()
    );
    println!("  publish_global_root (seq=1): OK");

    // Verify ring buffer entry
    let grc_account = get_account_legacy(&env.svm, &env.global_root_config_pda);
    let grc_data = &grc_account.data;

    assert_eq!(
        read_u64_at(grc_data, grc_offsets::LATEST_ROOT_SEQ),
        1,
        "latest_root_seq should be 1"
    );

    // Entry at index 1 % 4 = 1
    let entry_offset = grc_offsets::ROOTS_START + 1 * grc_offsets::ROOT_ENTRY_SIZE;
    assert_eq!(
        read_u64_at(grc_data, entry_offset),
        1,
        "Root entry seq should be 1"
    );
    assert_eq!(
        &grc_data[entry_offset + 8..entry_offset + 40],
        &root_1,
        "Root hash mismatch"
    );
    assert_eq!(
        &grc_data[entry_offset + 40..entry_offset + 72],
        &dataset_hash_1,
        "Dataset hash mismatch"
    );
    let first_published_slot = read_u64_at(grc_data, entry_offset + 72);

    // Publish root #2 to verify sequencing
    let root_2: [u8; 32] = [0xCC; 32];
    let dataset_hash_2: [u8; 32] = [0xDD; 32];
    let pub_ix_2 = build_publish_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        2,
        root_2,
        dataset_hash_2,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[pub_ix_2]);
    assert!(
        result.is_ok(),
        "publish_global_root (seq=2) failed: {:?}",
        result.err()
    );
    println!("  publish_global_root (seq=2): OK");

    // Verify seq=2 landed and slot bookkeeping is monotonic.
    let grc_account = get_account_legacy(&env.svm, &env.global_root_config_pda);
    let grc_data = &grc_account.data;
    assert_eq!(
        read_u64_at(grc_data, grc_offsets::LATEST_ROOT_SEQ),
        2,
        "latest_root_seq should advance to 2"
    );
    let entry2_offset = grc_offsets::ROOTS_START + 2 * grc_offsets::ROOT_ENTRY_SIZE;
    assert_eq!(
        read_u64_at(grc_data, entry2_offset),
        2,
        "Root entry seq should be 2"
    );
    assert_eq!(
        &grc_data[entry2_offset + 8..entry2_offset + 40],
        &root_2,
        "Root #2 hash mismatch"
    );
    assert_eq!(
        &grc_data[entry2_offset + 40..entry2_offset + 72],
        &dataset_hash_2,
        "Root #2 dataset hash mismatch"
    );
    let second_published_slot = read_u64_at(grc_data, entry2_offset + 72);
    assert!(
        second_published_slot >= first_published_slot,
        "Published slot should be monotonic ({} -> {})",
        first_published_slot,
        second_published_slot
    );

    // Verify out-of-order seq fails
    let bad_pub_ix = build_publish_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        5, // should be 3, not 5
        [0xFF; 32],
        [0xFF; 32],
    );
    let result = send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[bad_pub_ix]);
    assert!(
        result.is_err(),
        "publish_global_root with wrong seq should fail"
    );
    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::InvalidRootSeq = variant 6, code = 6006
    assert!(
        err_str.contains("6006") || err_str.contains("InvalidRootSeq"),
        "Expected InvalidRootSeq (6006), got: {}",
        err_str
    );

    println!("  Out-of-sequence publish correctly rejected (6006)");
    println!("  PUBLISH GLOBAL ROOT + RING BUFFER: PASS\n");
}

// =============================================================================
// TEST 9: CLAIM GLOBAL V2 — Merkle proof claim
// =============================================================================

#[test]
fn test_claim_global_v2() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup: protocol state (publisher = admin for simplicity)
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.admin), // publisher = admin
        &legacy_from_signer(&env.admin), // treasury = admin
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    // Initialize global root config
    let grc_ix = build_initialize_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[grc_ix])
        .expect("initialize_global_root failed");
    println!("  Setup: protocol_state + global_root_config initialized");

    // Build a merkle tree with a single user's claim
    let root_seq: u64 = 1;
    let base_yield: u64 = 500_000; // 0.5 CCM
    let attention_bonus: u64 = 250_000; // 0.25 CCM
    let total_claim = base_yield + attention_bonus; // 750_000 total

    let user_pubkey_bytes = legacy_from_signer(&env.user).to_bytes();
    let ccm_mint_bytes = env.ccm_mint.to_bytes();

    // Compute the V5 leaf for this user's claim
    let user_leaf = compute_global_leaf_v5(
        &ccm_mint_bytes,
        root_seq,
        &user_pubkey_bytes,
        base_yield,
        attention_bonus,
    );

    // Build a 2-leaf tree (need at least 2 leaves for a non-trivial proof)
    // Create a dummy second leaf
    let dummy_wallet = [0x42u8; 32];
    let dummy_leaf = compute_global_leaf_v5(
        &ccm_mint_bytes,
        root_seq,
        &dummy_wallet,
        100_000,
        50_000,
    );

    let (root, proof) = build_two_leaf_merkle(user_leaf, dummy_leaf);
    let dataset_hash: [u8; 32] = [0xAA; 32]; // arbitrary

    // Publish this root
    let pub_ix = build_publish_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        root_seq,
        root,
        dataset_hash,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[pub_ix])
        .expect("publish_global_root failed");
    println!("  Published root (seq={}, 2-leaf merkle tree)", root_seq);

    // The claim CPI does a transfer from treasury_ata to claimer_ata,
    // signed by the protocol_state PDA. The treasury_ata must be owned
    // by the protocol_state PDA and hold enough CCM.

    // Verify treasury has CCM
    let treasury_balance = read_token_amount(&env.svm, &env.treasury_ccm_ata);
    assert!(
        treasury_balance >= total_claim,
        "Treasury CCM balance ({}) should be >= total_claim ({})",
        treasury_balance,
        total_claim
    );

    // Build and send claim_global_v2
    let claim_ix = build_claim_global_v2_ix(
        &env.user,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        &env.claim_state_pda,
        &env.ccm_mint,
        &env.treasury_ccm_ata,
        &env.user_ccm_ata,
        root_seq,
        base_yield,
        attention_bonus,
        &proof,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(
        result.is_ok(),
        "claim_global_v2 failed: {:?}",
        result.err()
    );
    println!("  claim_global_v2: OK (claimed {} CCM)", total_claim);

    // Verify CCM transferred to user
    let user_ccm_balance = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(
        user_ccm_balance, total_claim,
        "User CCM should equal total_claim ({}) after claim",
        total_claim
    );

    // Verify treasury debited
    let treasury_balance_after = read_token_amount(&env.svm, &env.treasury_ccm_ata);
    assert_eq!(
        treasury_balance_after,
        treasury_balance - total_claim,
        "Treasury should be debited by total_claim"
    );

    // Verify ClaimStateGlobal PDA created and updated
    let cs_account = get_account_legacy(&env.svm, &env.claim_state_pda);
    assert_eq!(
        cs_account.owner,
        address_from_legacy(&program_id()),
        "ClaimStateGlobal must be owned by AO program"
    );
    assert!(
        cs_account.data.len() >= csg_offsets::LEN,
        "ClaimStateGlobal too small"
    );

    let expected_csg_disc = compute_account_discriminator("ClaimStateGlobal");
    assert_eq!(
        &cs_account.data[0..8],
        &expected_csg_disc,
        "ClaimStateGlobal discriminator mismatch"
    );

    let claimed_total = read_u64_at(&cs_account.data, csg_offsets::CLAIMED_TOTAL);
    assert_eq!(
        claimed_total, total_claim,
        "claimed_total should equal total_claim"
    );

    let last_claim_seq = read_u64_at(&cs_account.data, csg_offsets::LAST_CLAIM_SEQ);
    assert_eq!(
        last_claim_seq, root_seq,
        "last_claim_seq should equal root_seq"
    );

    // Verify idempotent re-claim (same amounts) is a no-op
    env.svm.expire_blockhash();
    let claim_ix_2 = build_claim_global_v2_ix(
        &env.user,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        &env.claim_state_pda,
        &env.ccm_mint,
        &env.treasury_ccm_ata,
        &env.user_ccm_ata,
        root_seq,
        base_yield,
        attention_bonus,
        &proof,
    );
    let result2 = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix_2]);
    assert!(
        result2.is_ok(),
        "Idempotent re-claim should succeed (no-op)"
    );

    // Balance should not change
    let user_ccm_after_reclaim = read_token_amount(&env.svm, &env.user_ccm_ata);
    assert_eq!(
        user_ccm_after_reclaim, total_claim,
        "Re-claim should not double-pay"
    );

    println!("  Idempotent re-claim: OK (no double-pay)");
    println!("  ClaimStateGlobal verification:");
    println!("    claimed_total:  {}", claimed_total);
    println!("    last_claim_seq: {}", last_claim_seq);
    println!("  CLAIM GLOBAL V2: PASS\n");
}

// =============================================================================
// TEST 10: INVALID MERKLE PROOF FAILS
// =============================================================================

#[test]
fn test_claim_global_v2_invalid_proof_fails() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Setup
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let grc_ix = build_initialize_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[grc_ix])
        .expect("initialize_global_root failed");

    // Publish a root
    let root_seq: u64 = 1;
    let root: [u8; 32] = [0xAA; 32]; // arbitrary root that won't match
    let dataset_hash: [u8; 32] = [0xBB; 32];

    let pub_ix = build_publish_global_root_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        root_seq,
        root,
        dataset_hash,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[pub_ix])
        .expect("publish_global_root failed");

    // Try to claim with a bogus proof (proof does not match root)
    let bogus_proof: Vec<[u8; 32]> = vec![[0xFF; 32]];
    let claim_ix = build_claim_global_v2_ix(
        &env.user,
        &env.protocol_state_pda,
        &env.global_root_config_pda,
        &env.claim_state_pda,
        &env.ccm_mint,
        &env.treasury_ccm_ata,
        &env.user_ccm_ata,
        root_seq,
        500_000,  // base_yield
        250_000,  // attention_bonus
        &bogus_proof,
    );

    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[claim_ix]);
    assert!(
        result.is_err(),
        "claim_global_v2 with invalid proof should fail"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::InvalidProof = variant 4, code = 6004
    assert!(
        err_str.contains("6004") || err_str.contains("InvalidProof"),
        "Expected InvalidProof (6004), got: {}",
        err_str
    );

    println!("  claim_global_v2 correctly rejected with InvalidProof (6004)");
    println!("  INVALID PROOF REJECTION: PASS\n");
}

// =============================================================================
// TEST 11: DOUBLE SETTLE FAILS
// =============================================================================

#[test]
fn test_double_settle_fails() {
    let Some(mut env) = setup_lifecycle_env() else {
        return;
    };

    // Full setup
    let ps_ix = build_initialize_protocol_state_ix(
        &env.admin,
        &env.protocol_state_pda,
        &legacy_from_signer(&env.publisher),
        &legacy_from_signer(&env.admin),
        &legacy_from_signer(&env.oracle_authority),
        &env.ccm_mint,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[ps_ix])
        .expect("initialize_protocol_state failed");

    let mv_ix = build_initialize_market_vault_ix(
        &env.admin,
        &env.protocol_state_pda,
        &env.market_vault_pda,
        &env.usdc_mint,
        &env.vlofi_mint,
        &env.vault_usdc_ata,
        env.market_id,
    );
    send_legacy_tx(&mut env.svm, &[&env.admin], &env.admin, &[mv_ix])
        .expect("initialize_market_vault failed");

    // Deposit
    let deposit_ix = build_deposit_market_ix(&env, 100_000_000);
    send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[deposit_ix])
        .expect("deposit_market failed");

    // Set multiplier
    let update_ix = build_update_attention_ix(&env, &env.user.pubkey(), 15_000);
    send_legacy_tx(
        &mut env.svm,
        &[&env.oracle_authority],
        &env.oracle_authority,
        &[update_ix],
    )
    .expect("update_attention failed");

    // First settle -- should succeed
    let settle_ix = build_settle_market_ix(&env);
    send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix])
        .expect("First settle_market failed");
    println!("  First settle: OK");

    // Second settle -- should fail with AlreadySettled
    env.svm.expire_blockhash();
    let settle_ix_2 = build_settle_market_ix(&env);
    let result = send_legacy_tx(&mut env.svm, &[&env.user], &env.user, &[settle_ix_2]);
    assert!(
        result.is_err(),
        "Second settle should fail with AlreadySettled"
    );

    let err_str = format!("{:?}", result.err().unwrap());
    // OracleError::AlreadySettled = variant 66, code = 6066
    assert!(
        err_str.contains("6066") || err_str.contains("AlreadySettled"),
        "Expected AlreadySettled (6066), got: {}",
        err_str
    );

    println!("  Double settle correctly rejected (6066)");
    println!("  DOUBLE SETTLE REJECTION: PASS\n");
}

// =============================================================================
// TEST 12: BYTE-LEVEL DISCRIMINATOR COMPATIBILITY
// =============================================================================

#[test]
fn test_discriminator_compatibility() {
    // Verify that the discriminators computed by the test helpers match
    // the hardcoded discriminators in lib.rs (which are the on-chain values).
    // This proves byte-compatibility between the Pinocchio program and the
    // Anchor-based program.

    // Instruction discriminators (SHA-256("global:<name>")[..8])
    assert_eq!(
        compute_discriminator("initialize_protocol_state"),
        [0xe5, 0xa8, 0x78, 0xa6, 0x07, 0x1f, 0x3b, 0xed],
        "initialize_protocol_state disc mismatch"
    );
    assert_eq!(
        compute_discriminator("initialize_market_vault"),
        [0x19, 0x66, 0xcb, 0x77, 0x97, 0x14, 0x8f, 0xde],
        "initialize_market_vault disc mismatch"
    );
    assert_eq!(
        compute_discriminator("deposit_market"),
        [0xd4, 0x35, 0xba, 0xc1, 0x93, 0x35, 0x8f, 0x7b],
        "deposit_market disc mismatch"
    );
    assert_eq!(
        compute_discriminator("update_attention"),
        [0x7b, 0xf7, 0x75, 0x86, 0xd0, 0x6b, 0x6c, 0x32],
        "update_attention disc mismatch"
    );
    assert_eq!(
        compute_discriminator("settle_market"),
        [0xc1, 0x99, 0x5f, 0xd8, 0xa6, 0x06, 0x90, 0xd9],
        "settle_market disc mismatch"
    );
    assert_eq!(
        compute_discriminator("claim_yield"),
        [0x31, 0x4a, 0x6f, 0x07, 0xba, 0x16, 0x3d, 0xa5],
        "claim_yield disc mismatch"
    );
    assert_eq!(
        compute_discriminator("initialize_global_root"),
        [0xca, 0x36, 0x6b, 0xf6, 0x18, 0xf7, 0x4b, 0xfd],
        "initialize_global_root disc mismatch"
    );
    assert_eq!(
        compute_discriminator("publish_global_root"),
        [0x51, 0x8d, 0xe2, 0x16, 0xfe, 0xa7, 0x62, 0xff],
        "publish_global_root disc mismatch"
    );
    assert_eq!(
        compute_discriminator("claim_global_v2"),
        [0xf8, 0x2c, 0xaa, 0x65, 0x31, 0xaa, 0x8c, 0x7e],
        "claim_global_v2 disc mismatch"
    );

    // Account discriminators (SHA-256("account:<name>")[..8])
    // These must match the on-chain DISC_* constants in state.rs
    println!("  Instruction discriminators verified against lib.rs");

    // Verify account discriminators match the const fn computed values
    // by importing the ao-v2 crate itself (which the test binary links against)
    let ps_disc = compute_account_discriminator("ProtocolState");
    let mv_disc = compute_account_discriminator("MarketVault");
    let pos_disc = compute_account_discriminator("UserMarketPosition");
    let grc_disc = compute_account_discriminator("GlobalRootConfig");
    let csg_disc = compute_account_discriminator("ClaimStateGlobal");

    // Cross-check with the const values from the ao-v2 state module
    assert_eq!(
        ps_disc,
        ao_v2::state::DISC_PROTOCOL_STATE,
        "ProtocolState account disc mismatch vs state.rs const"
    );
    assert_eq!(
        mv_disc,
        ao_v2::state::DISC_MARKET_VAULT,
        "MarketVault account disc mismatch vs state.rs const"
    );
    assert_eq!(
        pos_disc,
        ao_v2::state::DISC_USER_MARKET_POSITION,
        "UserMarketPosition account disc mismatch vs state.rs const"
    );
    assert_eq!(
        grc_disc,
        ao_v2::state::DISC_GLOBAL_ROOT_CONFIG,
        "GlobalRootConfig account disc mismatch vs state.rs const"
    );
    assert_eq!(
        csg_disc,
        ao_v2::state::DISC_CLAIM_STATE_GLOBAL,
        "ClaimStateGlobal account disc mismatch vs state.rs const"
    );

    println!("  Account discriminators verified against state.rs consts");
    println!("  DISCRIMINATOR COMPATIBILITY: PASS\n");
}

// =============================================================================
// TEST 13: STRUCT SIZE COMPATIBILITY
// =============================================================================

#[test]
fn test_struct_sizes() {
    // Verify that the struct sizes match expected values.
    // These are critical for byte-compatibility with existing on-chain data.
    assert_eq!(
        core::mem::size_of::<ao_v2::state::ProtocolState>(),
        173,
        "ProtocolState size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::MarketVault>(),
        153,
        "MarketVault size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::UserMarketPosition>(),
        114,
        "UserMarketPosition size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::GlobalRootConfig>(),
        370,
        "GlobalRootConfig size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::ClaimStateGlobal>(),
        90,
        "ClaimStateGlobal size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::RootEntry>(),
        80,
        "RootEntry size mismatch"
    );
    assert_eq!(
        core::mem::size_of::<ao_v2::state::FeeConfig>(),
        55,
        "FeeConfig size mismatch"
    );

    println!("  All struct sizes match expected values:");
    println!("    ProtocolState:      173 bytes");
    println!("    MarketVault:        153 bytes");
    println!("    UserMarketPosition: 114 bytes");
    println!("    GlobalRootConfig:   370 bytes");
    println!("    ClaimStateGlobal:    90 bytes");
    println!("    RootEntry:           80 bytes");
    println!("    FeeConfig:           55 bytes");
    println!("  STRUCT SIZE COMPATIBILITY: PASS\n");
}
