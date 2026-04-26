#![cfg(feature = "localtest")]
//! Golden-path LiteSVM E2E for Listen payout allocation settlement.
//!
//! Run with:
//!   anchor build --program-name wzrd_rails --no-idl
//!   cargo test -p wzrd-rails --features localtest --test listen_payout_e2e -- --nocapture

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
use spl_token_2022::state::{Account as TokenAccount, Mint};
use std::path::{Path, PathBuf};
use wzrd_rails::{
    accounts as rail_accounts, instruction as rail_ix, listen_payout_node_hash_v1,
    state::{
        ClaimListenPayoutArgs, InitPayoutAuthorityConfigArgs, InitPayoutCapConfigArgs,
        InitPayoutVaultConfigArgs, PayoutWindow, PublishListenPayoutRootArgs, CONFIG_SEED,
        LISTEN_PAYOUT_AUTHORITY_CONFIG_SEED, LISTEN_PAYOUT_CAP_CONFIG_SEED,
        LISTEN_PAYOUT_VAULT_AUTHORITY_SEED, LISTEN_PAYOUT_VAULT_CONFIG_SEED,
        LISTEN_PAYOUT_WINDOW_SEED,
    },
    ListenPayoutError, PayoutAllocationLeafV1, ID as WZRD_RAILS_PROGRAM_ID,
    LISTEN_PAYOUT_LEAF_SCHEMA_V1,
};

const CCM_DECIMALS: u8 = 9;
const NUM_LEAVES: usize = 8;
const PER_WINDOW_CAP: u64 = 80_000_000_000;
const VAULT_INITIAL_BALANCE: u64 = 20_000_000_000;
const WINDOW_ID: u64 = 20_260_426;

struct E2EFixture {
    svm: LiteSVM,
    operator: Keypair,
    ccm_mint: LegacyPubkey,
    authority_config: LegacyPubkey,
    cap_config: LegacyPubkey,
    vault_config: LegacyPubkey,
    vault_authority: LegacyPubkey,
    vault_ata: LegacyPubkey,
    leaf_holders: Vec<Keypair>,
    leaves: Vec<PayoutAllocationLeafV1>,
    proofs: Vec<Vec<[u8; 32]>>,
    merkle_root: [u8; 32],
    total_amount: u64,
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

fn load_wzrd_rails_program(svm: &mut LiteSVM) -> Result<(), String> {
    let program_path = Path::new("../../target/deploy/wzrd_rails.so");
    if !program_path.exists() {
        return Err(format!(
            "program binary not found at {}",
            program_path.display()
        ));
    }

    let bytes = std::fs::read(program_path).map_err(|err| err.to_string())?;
    svm.add_program(address_from_legacy(&WZRD_RAILS_PROGRAM_ID), &bytes)
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

fn load_token_2022_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes = find_litesvm_elf("spl_token_2022")
        .ok_or("Token-2022 ELF not found in litesvm cargo registry")?;
    svm.add_program(address_from_legacy(&spl_token_2022::id()), &bytes)
        .map_err(|err| format!("{err:?}"))
}

fn load_associated_token_program(svm: &mut LiteSVM) -> Result<(), String> {
    let bytes = find_litesvm_elf("spl_associated_token_account")
        .ok_or("Associated Token Account ELF not found in litesvm cargo registry")?;
    svm.add_program(address_from_legacy(&ASSOCIATED_TOKEN_PROGRAM_ID), &bytes)
        .map_err(|err| format!("{err:?}"))
}

fn derive_config() -> LegacyPubkey {
    LegacyPubkey::find_program_address(&[CONFIG_SEED], &WZRD_RAILS_PROGRAM_ID).0
}

fn derive_payout_authority_config() -> LegacyPubkey {
    LegacyPubkey::find_program_address(
        &[LISTEN_PAYOUT_AUTHORITY_CONFIG_SEED],
        &WZRD_RAILS_PROGRAM_ID,
    )
    .0
}

fn derive_payout_cap_config() -> LegacyPubkey {
    LegacyPubkey::find_program_address(&[LISTEN_PAYOUT_CAP_CONFIG_SEED], &WZRD_RAILS_PROGRAM_ID).0
}

fn derive_payout_vault_config() -> LegacyPubkey {
    LegacyPubkey::find_program_address(&[LISTEN_PAYOUT_VAULT_CONFIG_SEED], &WZRD_RAILS_PROGRAM_ID).0
}

fn derive_payout_vault_authority() -> LegacyPubkey {
    LegacyPubkey::find_program_address(
        &[LISTEN_PAYOUT_VAULT_AUTHORITY_SEED],
        &WZRD_RAILS_PROGRAM_ID,
    )
    .0
}

fn derive_payout_window(window_id: u64) -> LegacyPubkey {
    LegacyPubkey::find_program_address(
        &[LISTEN_PAYOUT_WINDOW_SEED, &window_id.to_le_bytes()],
        &WZRD_RAILS_PROGRAM_ID,
    )
    .0
}

fn derive_ata(owner: &LegacyPubkey, mint: &LegacyPubkey) -> LegacyPubkey {
    get_associated_token_address_with_program_id(owner, mint, &spl_token_2022::id())
}

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
    StateWithExtensions::<TokenAccount>::unpack(&account.data)
        .expect("failed to deserialize token account")
        .base
        .amount
}

fn create_plain_token_2022_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    mint_authority: &LegacyPubkey,
) {
    let payer_pubkey = legacy_from_signer(payer);
    let mint_pubkey = legacy_from_signer(mint);
    let rent = svm.minimum_balance_for_rent_exemption(Mint::LEN);
    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &mint_pubkey,
        rent,
        Mint::LEN as u64,
        &spl_token_2022::id(),
    );
    let init_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        &mint_pubkey,
        mint_authority,
        None,
        CCM_DECIMALS,
    )
    .unwrap();
    send_tx(svm, &[payer, mint], &[create_ix, init_ix]);
}

fn create_associated_token_2022_account(
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

fn build_initialize_config_ix(
    signer: LegacyPubkey,
    config: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    treasury_ccm_ata: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::InitializeConfig {
            config,
            signer,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::InitializeConfig {
            ccm_mint,
            treasury_ccm_ata,
        }
        .data(),
    }
}

fn build_init_payout_authority_config_ix(
    config: LegacyPubkey,
    authority_config: LegacyPubkey,
    admin: LegacyPubkey,
    payout_admin: LegacyPubkey,
    initial_publisher: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::InitPayoutAuthorityConfig {
            config,
            authority_config,
            admin,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::InitPayoutAuthorityConfig {
            args: InitPayoutAuthorityConfigArgs {
                admin: anchor_pubkey(payout_admin),
                initial_publisher: anchor_pubkey(initial_publisher),
            },
        }
        .data(),
    }
}

fn build_init_payout_cap_config_ix(
    config: LegacyPubkey,
    cap_config: LegacyPubkey,
    admin: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::InitPayoutCapConfig {
            config,
            cap_config,
            admin,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::InitPayoutCapConfig {
            args: InitPayoutCapConfigArgs {
                admin: anchor_pubkey(admin),
                per_window_cap_ccm: PER_WINDOW_CAP,
            },
        }
        .data(),
    }
}

fn build_init_payout_vault_config_ix(
    config: LegacyPubkey,
    vault_config: LegacyPubkey,
    vault_authority: LegacyPubkey,
    admin: LegacyPubkey,
    ccm_mint: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::InitPayoutVaultConfig {
            config,
            vault_config,
            vault_authority,
            admin,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::InitPayoutVaultConfig {
            args: InitPayoutVaultConfigArgs {
                admin: anchor_pubkey(admin),
                ccm_mint: anchor_pubkey(ccm_mint),
            },
        }
        .data(),
    }
}

fn build_publish_listen_payout_root_ix(
    authority: LegacyPubkey,
    authority_config: LegacyPubkey,
    cap_config: LegacyPubkey,
    payout_window: LegacyPubkey,
    args: PublishListenPayoutRootArgs,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::PublishListenPayoutRoot {
            authority,
            authority_config,
            cap_config,
            payout_window,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::PublishListenPayoutRoot { args }.data(),
    }
}

fn build_claim_listen_payout_ix(
    claimer: LegacyPubkey,
    authority_config: LegacyPubkey,
    vault_config: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    listen_payout_vault: LegacyPubkey,
    vault_authority: LegacyPubkey,
    claimer_ata: LegacyPubkey,
    args: ClaimListenPayoutArgs,
) -> LegacyInstruction {
    let payout_window = derive_payout_window(args.leaf.window_id);
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::ClaimListenPayout {
            claimer,
            payout_window,
            authority_config,
            vault_config,
            ccm_mint,
            listen_payout_vault,
            vault_authority,
            claimer_ata,
            token_program: spl_token_2022::id(),
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::ClaimListenPayout { args }.data(),
    }
}

fn listen_payout_error_code(error: ListenPayoutError) -> u32 {
    ERROR_CODE_OFFSET + error as u32
}

fn assert_listen_payout_error(
    result: Result<TransactionMetadata, FailedTransactionMetadata>,
    error: ListenPayoutError,
) {
    let failure = result.expect_err("expected transaction to fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(listen_payout_error_code(error)),
        )
    );
}

fn build_merkle_tree(leaves: &[[u8; 32]]) -> ([u8; 32], Vec<Vec<[u8; 32]>>) {
    assert!(!leaves.is_empty());

    let mut levels = vec![leaves.to_vec()];
    while levels.last().unwrap().len() > 1 {
        let previous = levels.last().unwrap();
        let mut next = Vec::with_capacity(previous.len().div_ceil(2));
        for pair in previous.chunks(2) {
            if pair.len() == 2 {
                next.push(listen_payout_node_hash_v1(&pair[0], &pair[1]));
            } else {
                next.push(pair[0]);
            }
        }
        levels.push(next);
    }

    let proofs = leaves
        .iter()
        .enumerate()
        .map(|(leaf_idx, _)| {
            let mut idx = leaf_idx;
            let mut proof = Vec::new();
            for level in levels.iter().take(levels.len() - 1) {
                let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
                if let Some(sibling) = level.get(sibling_idx) {
                    proof.push(*sibling);
                }
                idx /= 2;
            }
            proof
        })
        .collect();

    (levels.last().unwrap()[0], proofs)
}

fn build_leaf(wallet: LegacyPubkey, leaf_index: u32, amount_ccm: u64) -> PayoutAllocationLeafV1 {
    let mut allocation_id = [0u8; 16];
    allocation_id[0] = leaf_index as u8;
    allocation_id[15] = (leaf_index >> 8) as u8;

    let mut salt = [0u8; 16];
    salt[0] = 0xab;
    salt[15] = leaf_index as u8;

    PayoutAllocationLeafV1::new(
        [0x51; 32],
        WINDOW_ID,
        leaf_index,
        allocation_id,
        anchor_pubkey(wallet),
        amount_ccm,
        [0xa0 | (leaf_index as u8); 32],
        [0xb0 | (leaf_index as u8); 32],
        salt,
    )
}

fn claim_args(fixture: &E2EFixture, leaf_index: usize) -> ClaimListenPayoutArgs {
    ClaimListenPayoutArgs {
        leaf: fixture.leaves[leaf_index],
        proof: fixture.proofs[leaf_index].clone(),
    }
}

fn setup_fixture() -> E2EFixture {
    let mut svm = LiteSVM::new();
    load_wzrd_rails_program(&mut svm)
        .expect("run `anchor build --program-name wzrd_rails --no-idl` first");
    load_token_2022_program(&mut svm).expect("load Token-2022 program");
    load_associated_token_program(&mut svm).expect("load Associated Token program");

    let admin = Keypair::new();
    let operator = Keypair::new();
    let ccm_mint_keypair = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();
    svm.airdrop(&operator.pubkey(), 100_000_000_000).unwrap();

    let admin_pubkey = legacy_from_signer(&admin);
    let operator_pubkey = legacy_from_signer(&operator);
    let ccm_mint = legacy_from_signer(&ccm_mint_keypair);

    create_plain_token_2022_mint(&mut svm, &operator, &ccm_mint_keypair, &operator_pubkey);

    let config = derive_config();
    let authority_config = derive_payout_authority_config();
    let cap_config = derive_payout_cap_config();
    let vault_config = derive_payout_vault_config();
    let vault_authority = derive_payout_vault_authority();
    let operator_ata =
        create_associated_token_2022_account(&mut svm, &operator, &operator_pubkey, &ccm_mint);
    let vault_ata =
        create_associated_token_2022_account(&mut svm, &operator, &vault_authority, &ccm_mint);
    mint_token_2022(
        &mut svm,
        &operator,
        &ccm_mint,
        &vault_ata,
        VAULT_INITIAL_BALANCE,
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[
            build_initialize_config_ix(admin_pubkey, config, ccm_mint, operator_ata),
            build_init_payout_authority_config_ix(
                config,
                authority_config,
                admin_pubkey,
                admin_pubkey,
                operator_pubkey,
            ),
            build_init_payout_cap_config_ix(config, cap_config, admin_pubkey),
            build_init_payout_vault_config_ix(
                config,
                vault_config,
                vault_authority,
                admin_pubkey,
                ccm_mint,
            ),
        ],
    );

    let leaf_holders = (0..NUM_LEAVES)
        .map(|_| {
            let keypair = Keypair::new();
            svm.airdrop(&keypair.pubkey(), 10_000_000_000).unwrap();
            keypair
        })
        .collect::<Vec<_>>();
    let amounts = [
        1_000_000_000,
        2_000_000_000,
        750_000_000,
        1_250_000_000,
        500_000_000,
        1_500_000_000,
        2_250_000_000,
        3_000_000_000,
    ];
    let leaves = leaf_holders
        .iter()
        .zip(amounts)
        .enumerate()
        .map(|(idx, (holder, amount))| build_leaf(legacy_from_signer(holder), idx as u32, amount))
        .collect::<Vec<_>>();
    let leaf_hashes = leaves
        .iter()
        .map(PayoutAllocationLeafV1::hash)
        .collect::<Vec<_>>();
    let (merkle_root, proofs) = build_merkle_tree(&leaf_hashes);
    let total_amount = leaves.iter().map(|leaf| leaf.amount_ccm).sum();

    E2EFixture {
        svm,
        operator,
        ccm_mint,
        authority_config,
        cap_config,
        vault_config,
        vault_authority,
        vault_ata,
        leaf_holders,
        leaves,
        proofs,
        merkle_root,
        total_amount,
    }
}

#[test]
fn listen_payout_e2e_publish_then_claim_allocation_leaves() {
    let trivial = [[0u8; 32]; 2];
    assert_eq!(
        build_merkle_tree(&trivial).0,
        listen_payout_node_hash_v1(&[0u8; 32], &[0u8; 32])
    );

    let mut fixture = setup_fixture();
    assert_eq!(fixture.leaves.len(), NUM_LEAVES);
    assert!(fixture.total_amount <= PER_WINDOW_CAP);

    let publish_ix = build_publish_listen_payout_root_ix(
        legacy_from_signer(&fixture.operator),
        fixture.authority_config,
        fixture.cap_config,
        derive_payout_window(WINDOW_ID),
        PublishListenPayoutRootArgs {
            window_id: WINDOW_ID,
            merkle_root: fixture.merkle_root,
            leaf_count: NUM_LEAVES as u32,
            schema_version: LISTEN_PAYOUT_LEAF_SCHEMA_V1,
            total_amount_ccm: fixture.total_amount,
        },
    );
    send_tx(&mut fixture.svm, &[&fixture.operator], &[publish_ix]);

    for leaf_index in [5usize, 0, 7, 1, 6, 2, 4, 3] {
        let claimer = &fixture.leaf_holders[leaf_index];
        let claimer_pubkey = legacy_from_signer(claimer);
        let claimer_ata = derive_ata(&claimer_pubkey, &fixture.ccm_mint);
        let claim_ix = build_claim_listen_payout_ix(
            claimer_pubkey,
            fixture.authority_config,
            fixture.vault_config,
            fixture.ccm_mint,
            fixture.vault_ata,
            fixture.vault_authority,
            claimer_ata,
            claim_args(&fixture, leaf_index),
        );
        send_tx(&mut fixture.svm, &[claimer], &[claim_ix]);
        assert_eq!(
            read_token_balance(&fixture.svm, &claimer_ata),
            fixture.leaves[leaf_index].amount_ccm
        );
    }

    assert_eq!(
        read_token_balance(&fixture.svm, &fixture.vault_ata),
        VAULT_INITIAL_BALANCE - fixture.total_amount
    );

    let payout_window: PayoutWindow =
        read_anchor_account(&fixture.svm, &derive_payout_window(WINDOW_ID));
    assert_eq!(payout_window.claim_bitmap, vec![0xff]);

    fixture.svm.expire_blockhash();
    let replay_claimer = &fixture.leaf_holders[0];
    let replay_pubkey = legacy_from_signer(replay_claimer);
    let replay_ata = derive_ata(&replay_pubkey, &fixture.ccm_mint);
    let replay_ix = build_claim_listen_payout_ix(
        replay_pubkey,
        fixture.authority_config,
        fixture.vault_config,
        fixture.ccm_mint,
        fixture.vault_ata,
        fixture.vault_authority,
        replay_ata,
        claim_args(&fixture, 0),
    );
    assert_listen_payout_error(
        try_send_tx(&mut fixture.svm, &[replay_claimer], &[replay_ix]),
        ListenPayoutError::AlreadyClaimed,
    );
}
