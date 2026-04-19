#![cfg(feature = "localtest")]
//! LiteSVM integration coverage for `wzrd-rails`.
//!
//! Run with:
//!   anchor build --program-name wzrd_rails
//!   cargo test -p wzrd-rails --features localtest --test core_loop -- --nocapture

use anchor_lang::{error::ERROR_CODE_OFFSET, AccountDeserialize, InstructionData, ToAccountMetas};
use litesvm::{types::FailedTransactionMetadata, LiteSVM};
use solana_address::Address;
use solana_instruction::error::InstructionError;
use solana_keypair::Keypair;
use solana_message::Message;
use solana_signer::Signer;
use solana_transaction::{Transaction, TransactionError};
use solana_sdk::{
    instruction::Instruction as LegacyInstruction,
    program_pack::Pack,
    pubkey::Pubkey as LegacyPubkey,
    system_instruction, system_program, sysvar,
};
use spl_token_2022::state::{Account as TokenAccount, Mint};
use std::path::{Path, PathBuf};
use solana_keccak_hasher as keccak;
use wzrd_rails::{
    accounts as rail_accounts, instruction as rail_ix,
    state::{
        CompensationClaimed, Config, StakePool, UserStake, COMPENSATION_LEAF_DOMAIN,
        COMP_CLAIMED_SEED, COMP_VAULT_SEED, CONFIG_SEED, MAX_REWARD_RATE_PER_SLOT, POOL_SEED,
        REWARD_VAULT_SEED, STAKE_VAULT_SEED, USER_STAKE_SEED,
    },
    ID as WZRD_RAILS_PROGRAM_ID, RailsError,
};

const CCM_DECIMALS: u8 = 9;
const POOL_ID: u32 = 0;
const LOCK_DURATION_SLOTS: u64 = 1_000;
const DEFAULT_REWARD_RATE_PER_SLOT: u64 = 1_000;
const ADMIN_START_BALANCE: u64 = 20_000_000_000;
const USER_START_BALANCE: u64 = 10_000_000_000;
const GOLDEN_PATH_FUND_AMOUNT: u64 = 5_000_000_000;
const GOLDEN_PATH_STAKE_AMOUNT: u64 = 2_000_000_000;
const SMALL_STAKE_AMOUNT: u64 = 100;
const USER_B_STAKE_AMOUNT: u64 = 300;

struct UserFixture {
    signer: Keypair,
    ccm: LegacyPubkey,
    user_stake: LegacyPubkey,
    comp_claimed: LegacyPubkey,
}

impl UserFixture {
    fn pubkey(&self) -> LegacyPubkey {
        legacy_from_signer(&self.signer)
    }
}

struct TestEnv {
    svm: LiteSVM,
    admin: Keypair,
    ccm_mint: Keypair,
    config: LegacyPubkey,
    pool: LegacyPubkey,
    stake_vault: LegacyPubkey,
    reward_vault: LegacyPubkey,
    comp_vault: LegacyPubkey,
    admin_ccm: LegacyPubkey,
    user_a: UserFixture,
}

impl TestEnv {
    fn admin_pubkey(&self) -> LegacyPubkey {
        legacy_from_signer(&self.admin)
    }

    fn ccm_mint_pubkey(&self) -> LegacyPubkey {
        legacy_from_signer(&self.ccm_mint)
    }

    fn create_user(&mut self, starting_balance: u64) -> UserFixture {
        let ccm_mint = self.ccm_mint_pubkey();
        create_user_fixture(
            &mut self.svm,
            &self.admin,
            &ccm_mint,
            &self.pool,
            starting_balance,
        )
    }

    fn set_reward_rate(&mut self, new_rate: u64) {
        let ix = build_set_reward_rate_ix(self.config, self.pool, self.admin_pubkey(), new_rate);
        send_tx(&mut self.svm, &[&self.admin], &[ix]);
    }

    fn try_set_reward_rate_as(
        &mut self,
        signer: &Keypair,
        new_rate: u64,
    ) -> Result<(), FailedTransactionMetadata> {
        let ix = build_set_reward_rate_ix(self.config, self.pool, legacy_from_signer(signer), new_rate);
        try_send_tx(&mut self.svm, &[signer], &[ix])
    }

    fn try_set_reward_rate_as_admin(
        &mut self,
        new_rate: u64,
    ) -> Result<(), FailedTransactionMetadata> {
        let ix = build_set_reward_rate_ix(
            self.config,
            self.pool,
            self.admin_pubkey(),
            new_rate,
        );
        try_send_tx(&mut self.svm, &[&self.admin], &[ix])
    }

    fn fund_reward_pool(&mut self, amount: u64) {
        let ix = build_fund_reward_pool_ix(
            self.config,
            self.pool,
            self.admin_pubkey(),
            self.ccm_mint_pubkey(),
            self.admin_ccm,
            self.reward_vault,
            amount,
        );
        send_tx(&mut self.svm, &[&self.admin], &[ix]);
    }

    fn compensate_external_stakers(&mut self, merkle_root: [u8; 32]) {
        let ix = build_compensate_external_stakers_ix(
            self.config,
            self.admin_pubkey(),
            self.ccm_mint_pubkey(),
            self.comp_vault,
            merkle_root,
        );
        send_tx(&mut self.svm, &[&self.admin], &[ix]);
    }

    fn try_compensate_external_stakers(
        &mut self,
        merkle_root: [u8; 32],
    ) -> Result<(), FailedTransactionMetadata> {
        let ix = build_compensate_external_stakers_ix(
            self.config,
            self.admin_pubkey(),
            self.ccm_mint_pubkey(),
            self.comp_vault,
            merkle_root,
        );
        try_send_tx(&mut self.svm, &[&self.admin], &[ix])
    }

    fn fund_comp_vault(&mut self, amount: u64) {
        let ix = build_direct_token_transfer_ix(
            self.admin_pubkey(),
            self.admin_ccm,
            self.comp_vault,
            self.ccm_mint_pubkey(),
            amount,
        );
        send_tx(&mut self.svm, &[&self.admin], &[ix]);
    }

    fn stake_user_a(&mut self, amount: u64) {
        let user = &self.user_a;
        let ix = build_stake_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.stake_vault,
            user.user_stake,
            amount,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn stake_for_user(&mut self, user: &UserFixture, amount: u64) {
        let ix = build_stake_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.stake_vault,
            user.user_stake,
            amount,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn claim_user_a(&mut self) {
        let user = &self.user_a;
        let ix = build_claim_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.reward_vault,
            user.user_stake,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn claim_for_user(&mut self, user: &UserFixture) {
        let ix = build_claim_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.reward_vault,
            user.user_stake,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn claim_compensation_user_a(&mut self, amount: u64, proof: Vec<[u8; 32]>) {
        let user = &self.user_a;
        let ix = build_claim_compensation_ix(
            self.config,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.comp_vault,
            user.comp_claimed,
            amount,
            proof,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn try_claim_compensation_user_a(
        &mut self,
        amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<(), FailedTransactionMetadata> {
        let user = &self.user_a;
        let ix = build_claim_compensation_ix(
            self.config,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.comp_vault,
            user.comp_claimed,
            amount,
            proof,
        );
        try_send_tx(&mut self.svm, &[&user.signer], &[ix])
    }

    fn try_claim_compensation_custom(
        &mut self,
        signer: &Keypair,
        user_ccm: LegacyPubkey,
        claimed: LegacyPubkey,
        amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<(), FailedTransactionMetadata> {
        let ix = build_claim_compensation_ix(
            self.config,
            legacy_from_signer(signer),
            self.ccm_mint_pubkey(),
            user_ccm,
            self.comp_vault,
            claimed,
            amount,
            proof,
        );
        try_send_tx(&mut self.svm, &[signer], &[ix])
    }

    fn unstake_user_a(&mut self) {
        let user = &self.user_a;
        let ix = build_unstake_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.stake_vault,
            user.user_stake,
        );
        send_tx(&mut self.svm, &[&user.signer], &[ix]);
    }

    fn try_unstake_user_a(&mut self) -> Result<(), FailedTransactionMetadata> {
        let user = &self.user_a;
        let ix = build_unstake_ix(
            self.config,
            self.pool,
            user.pubkey(),
            self.ccm_mint_pubkey(),
            user.ccm,
            self.stake_vault,
            user.user_stake,
        );
        try_send_tx(&mut self.svm, &[&user.signer], &[ix])
    }
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

fn send_tx(svm: &mut LiteSVM, signers: &[&Keypair], instructions: &[LegacyInstruction]) {
    let payer = signers
        .first()
        .expect("at least one signer is required");
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());

    match svm.send_transaction(tx) {
        Ok(_) => {}
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
) -> Result<(), FailedTransactionMetadata> {
    let payer = signers
        .first()
        .expect("at least one signer is required");
    let instructions: Vec<_> = instructions.iter().map(convert_instruction).collect();
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(signers, message, svm.latest_blockhash());

    svm.send_transaction(tx).map(|_| ())
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

fn create_token_2022_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token_account: &Keypair,
    mint: &LegacyPubkey,
    owner: &LegacyPubkey,
) {
    let payer_pubkey = legacy_from_signer(payer);
    let token_account_pubkey = legacy_from_signer(token_account);
    let rent = svm.minimum_balance_for_rent_exemption(TokenAccount::LEN);
    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &token_account_pubkey,
        rent,
        TokenAccount::LEN as u64,
        &spl_token_2022::id(),
    );
    let init_ix = spl_token_2022::instruction::initialize_account3(
        &spl_token_2022::id(),
        &token_account_pubkey,
        mint,
        owner,
    )
    .unwrap();

    send_tx(svm, &[payer, token_account], &[create_ix, init_ix]);
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

fn derive_config() -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[CONFIG_SEED], &WZRD_RAILS_PROGRAM_ID)
}

fn derive_pool(pool_id: u32) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[POOL_SEED, &pool_id.to_le_bytes()], &WZRD_RAILS_PROGRAM_ID)
}

fn derive_stake_vault(pool: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[STAKE_VAULT_SEED, pool.as_ref()], &WZRD_RAILS_PROGRAM_ID)
}

fn derive_reward_vault(pool: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[REWARD_VAULT_SEED, pool.as_ref()], &WZRD_RAILS_PROGRAM_ID)
}

fn derive_comp_vault(config: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(&[COMP_VAULT_SEED, config.as_ref()], &WZRD_RAILS_PROGRAM_ID)
}

fn derive_user_stake(pool: &LegacyPubkey, user: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[USER_STAKE_SEED, pool.as_ref(), user.as_ref()],
        &WZRD_RAILS_PROGRAM_ID,
    )
}

fn derive_comp_claimed(user: &LegacyPubkey) -> (LegacyPubkey, u8) {
    LegacyPubkey::find_program_address(
        &[COMP_CLAIMED_SEED, user.as_ref()],
        &WZRD_RAILS_PROGRAM_ID,
    )
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
    TokenAccount::unpack(&account.data)
        .expect("failed to deserialize token account")
        .amount
}

fn expected_acc_reward_per_share(total_reward: u64, total_staked: u64) -> u128 {
    (total_reward as u128)
        .checked_mul(StakePool::REWARD_SCALE)
        .unwrap()
        .checked_div(total_staked as u128)
        .unwrap()
}

fn compensation_leaf(user: &LegacyPubkey, amount: u64) -> [u8; 32] {
    keccak::hashv(&[
        COMPENSATION_LEAF_DOMAIN,
        user.as_ref(),
        amount.to_le_bytes().as_ref(),
    ])
    .to_bytes()
}

fn sorted_pair_hash(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    keccak::hashv(&[first.as_ref(), second.as_ref()]).to_bytes()
}

fn two_leaf_merkle(
    left: (LegacyPubkey, u64),
    right: (LegacyPubkey, u64),
) -> ([u8; 32], Vec<[u8; 32]>, Vec<[u8; 32]>) {
    let left_leaf = compensation_leaf(&left.0, left.1);
    let right_leaf = compensation_leaf(&right.0, right.1);
    (
        sorted_pair_hash(left_leaf, right_leaf),
        vec![right_leaf],
        vec![left_leaf],
    )
}

fn rails_error_code(error: RailsError) -> u32 {
    ERROR_CODE_OFFSET + error as u32
}

fn assert_rails_error(result: Result<(), FailedTransactionMetadata>, error: RailsError) {
    let failure = result.expect_err("expected transaction to fail");
    assert_eq!(
        failure.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(rails_error_code(error)),
        )
    );
}

fn warp_to_slot(env: &mut TestEnv, slot: u64) {
    env.svm.warp_to_slot(slot);
    env.svm.expire_blockhash();
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

fn build_initialize_pool_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    stake_vault: LegacyPubkey,
    reward_vault: LegacyPubkey,
    admin: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::InitializePool {
            config,
            pool,
            ccm_mint,
            stake_vault,
            reward_vault,
            admin,
            token_2022_program: spl_token_2022::id(),
            system_program: system_program::ID,
            rent: sysvar::rent::ID,
        }
        .to_account_metas(None),
        data: rail_ix::InitializePool {
            pool_id: POOL_ID,
            lock_duration_slots: LOCK_DURATION_SLOTS,
        }
        .data(),
    }
}

fn build_set_reward_rate_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    admin: LegacyPubkey,
    new_rate: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::SetRewardRate {
            config,
            pool,
            admin,
        }
        .to_account_metas(None),
        data: rail_ix::SetRewardRate {
            _pool_id: POOL_ID,
            new_rate,
        }
        .data(),
    }
}

fn build_fund_reward_pool_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    funder: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    funder_ccm: LegacyPubkey,
    reward_vault: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::FundRewardPool {
            config,
            pool,
            funder,
            ccm_mint,
            funder_ccm,
            reward_vault,
            token_2022_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: rail_ix::FundRewardPool {
            _pool_id: POOL_ID,
            ccm_amount: amount,
        }
        .data(),
    }
}

fn build_compensate_external_stakers_ix(
    config: LegacyPubkey,
    admin: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    comp_vault: LegacyPubkey,
    merkle_root: [u8; 32],
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::CompensateExternalStakers {
            config,
            admin,
            ccm_mint,
            comp_vault,
            token_2022_program: spl_token_2022::id(),
            system_program: system_program::ID,
            rent: sysvar::rent::ID,
        }
        .to_account_metas(None),
        data: rail_ix::CompensateExternalStakers { merkle_root }.data(),
    }
}

fn build_stake_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    user: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    user_ccm: LegacyPubkey,
    stake_vault: LegacyPubkey,
    user_stake: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::Stake {
            config,
            pool,
            user,
            ccm_mint,
            user_ccm,
            stake_vault,
            user_stake,
            token_2022_program: spl_token_2022::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::Stake {
            _pool_id: POOL_ID,
            amount,
        }
        .data(),
    }
}

fn build_claim_compensation_ix(
    config: LegacyPubkey,
    user: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    user_ccm: LegacyPubkey,
    comp_vault: LegacyPubkey,
    claimed: LegacyPubkey,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::ClaimCompensation {
            config,
            user,
            ccm_mint,
            user_ccm,
            comp_vault,
            claimed,
            token_2022_program: spl_token_2022::id(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: rail_ix::ClaimCompensation { amount, proof }.data(),
    }
}

fn build_direct_token_transfer_ix(
    authority: LegacyPubkey,
    from: LegacyPubkey,
    to: LegacyPubkey,
    mint: LegacyPubkey,
    amount: u64,
) -> LegacyInstruction {
    spl_token_2022::instruction::transfer_checked(
        &spl_token_2022::id(),
        &from,
        &mint,
        &to,
        &authority,
        &[],
        amount,
        CCM_DECIMALS,
    )
    .unwrap()
}

fn build_claim_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    user: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    user_ccm: LegacyPubkey,
    reward_vault: LegacyPubkey,
    user_stake: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::Claim {
            config,
            pool,
            user,
            ccm_mint,
            user_ccm,
            reward_vault,
            user_stake,
            token_2022_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: rail_ix::Claim { _pool_id: POOL_ID }.data(),
    }
}

fn build_unstake_ix(
    config: LegacyPubkey,
    pool: LegacyPubkey,
    user: LegacyPubkey,
    ccm_mint: LegacyPubkey,
    user_ccm: LegacyPubkey,
    stake_vault: LegacyPubkey,
    user_stake: LegacyPubkey,
) -> LegacyInstruction {
    LegacyInstruction {
        program_id: WZRD_RAILS_PROGRAM_ID,
        accounts: rail_accounts::Unstake {
            config,
            pool,
            user,
            ccm_mint,
            user_ccm,
            stake_vault,
            user_stake,
            token_2022_program: spl_token_2022::id(),
        }
        .to_account_metas(None),
        data: rail_ix::Unstake { _pool_id: POOL_ID }.data(),
    }
}

fn create_user_fixture(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    ccm_mint: &LegacyPubkey,
    pool: &LegacyPubkey,
    starting_balance: u64,
) -> UserFixture {
    let signer = Keypair::new();
    let token_account = Keypair::new();

    svm.airdrop(&signer.pubkey(), 100_000_000_000).unwrap();

    let user_pubkey = legacy_from_signer(&signer);
    create_token_2022_account(svm, &signer, &token_account, ccm_mint, &user_pubkey);

    let ccm = legacy_from_signer(&token_account);
    mint_token_2022(svm, mint_authority, ccm_mint, &ccm, starting_balance);

    let (user_stake, _) = derive_user_stake(pool, &user_pubkey);
    let (comp_claimed, _) = derive_comp_claimed(&user_pubkey);
    UserFixture {
        signer,
        ccm,
        user_stake,
        comp_claimed,
    }
}

fn setup_rails() -> TestEnv {
    let mut svm = LiteSVM::new();

    if let Err(err) = load_wzrd_rails_program(&mut svm) {
        panic!(
            "Failed to load wzrd-rails program: {err}. Run `anchor build --program-name wzrd_rails` first."
        );
    }
    if let Err(err) = load_token_2022_program(&mut svm) {
        panic!("Failed to load Token-2022 program into LiteSVM: {err}");
    }

    let admin = Keypair::new();
    let ccm_mint = Keypair::new();
    let admin_ccm_account = Keypair::new();

    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();

    let admin_pubkey = legacy_from_signer(&admin);
    let ccm_mint_pubkey = legacy_from_signer(&ccm_mint);

    create_plain_token_2022_mint(&mut svm, &admin, &ccm_mint, &admin_pubkey);
    create_token_2022_account(
        &mut svm,
        &admin,
        &admin_ccm_account,
        &ccm_mint_pubkey,
        &admin_pubkey,
    );

    let admin_ccm = legacy_from_signer(&admin_ccm_account);
    mint_token_2022(
        &mut svm,
        &admin,
        &ccm_mint_pubkey,
        &admin_ccm,
        ADMIN_START_BALANCE,
    );

    let (config, _) = derive_config();
    let (pool, _) = derive_pool(POOL_ID);
    let (stake_vault, _) = derive_stake_vault(&pool);
    let (reward_vault, _) = derive_reward_vault(&pool);
    let (comp_vault, _) = derive_comp_vault(&config);

    let user_a = create_user_fixture(
        &mut svm,
        &admin,
        &ccm_mint_pubkey,
        &pool,
        USER_START_BALANCE,
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_config_ix(
            admin_pubkey,
            config,
            ccm_mint_pubkey,
            admin_ccm,
        )],
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_pool_ix(
            config,
            pool,
            ccm_mint_pubkey,
            stake_vault,
            reward_vault,
            admin_pubkey,
        )],
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_set_reward_rate_ix(
            config,
            pool,
            admin_pubkey,
            DEFAULT_REWARD_RATE_PER_SLOT,
        )],
    );

    TestEnv {
        svm,
        admin,
        ccm_mint,
        config,
        pool,
        stake_vault,
        reward_vault,
        comp_vault,
        admin_ccm,
        user_a,
    }
}

#[test]
fn happy_path_core_loop_runs_end_to_end() {
    let mut env = setup_rails();

    let config: Config = read_anchor_account(&env.svm, &env.config);
    assert_eq!(config.admin, env.admin_pubkey());
    assert_eq!(config.ccm_mint, env.ccm_mint_pubkey());
    assert_eq!(config.treasury_ccm_ata, env.admin_ccm);
    assert_eq!(config.comp_merkle_root, [0u8; 32]);
    assert_eq!(config.total_pools, 1);

    let pool_after_setup: StakePool = read_anchor_account(&env.svm, &env.pool);
    assert_eq!(pool_after_setup.pool_id, POOL_ID);
    assert_eq!(pool_after_setup.total_staked, 0);
    assert_eq!(pool_after_setup.acc_reward_per_share, 0);
    assert_eq!(
        pool_after_setup.reward_rate_per_slot,
        DEFAULT_REWARD_RATE_PER_SLOT
    );
    assert_eq!(pool_after_setup.lock_duration_slots, LOCK_DURATION_SLOTS);
    assert_eq!(read_token_balance(&env.svm, &env.stake_vault), 0);
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);

    env.fund_reward_pool(GOLDEN_PATH_FUND_AMOUNT);

    assert_eq!(
        read_token_balance(&env.svm, &env.reward_vault),
        GOLDEN_PATH_FUND_AMOUNT
    );
    assert_eq!(
        read_token_balance(&env.svm, &env.admin_ccm),
        ADMIN_START_BALANCE - GOLDEN_PATH_FUND_AMOUNT
    );

    env.stake_user_a(GOLDEN_PATH_STAKE_AMOUNT);

    let pool_after_stake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_stake_after_stake: UserStake =
        read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(pool_after_stake.total_staked, GOLDEN_PATH_STAKE_AMOUNT);
    assert_eq!(
        read_token_balance(&env.svm, &env.stake_vault),
        GOLDEN_PATH_STAKE_AMOUNT
    );
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - GOLDEN_PATH_STAKE_AMOUNT
    );
    assert_eq!(user_stake_after_stake.user, env.user_a.pubkey());
    assert_eq!(user_stake_after_stake.pool, env.pool);
    assert_eq!(user_stake_after_stake.amount, GOLDEN_PATH_STAKE_AMOUNT);
    assert_eq!(user_stake_after_stake.reward_debt, 0);
    assert_eq!(user_stake_after_stake.pending_rewards, 0);
    assert_eq!(
        user_stake_after_stake.lock_end_slot,
        pool_after_stake.last_update_slot + LOCK_DURATION_SLOTS
    );

    let claim_slot = user_stake_after_stake.lock_end_slot + 1;
    warp_to_slot(&mut env, claim_slot);

    let expected_reward = DEFAULT_REWARD_RATE_PER_SLOT
        .checked_mul(LOCK_DURATION_SLOTS + 1)
        .unwrap();
    let expected_acc_reward_per_share =
        expected_acc_reward_per_share(expected_reward, GOLDEN_PATH_STAKE_AMOUNT);

    env.claim_user_a();

    let pool_after_claim: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_stake_after_claim: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(pool_after_claim.last_update_slot, claim_slot);
    assert_eq!(
        pool_after_claim.acc_reward_per_share,
        expected_acc_reward_per_share
    );
    assert_eq!(
        read_token_balance(&env.svm, &env.reward_vault),
        GOLDEN_PATH_FUND_AMOUNT - expected_reward
    );
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - GOLDEN_PATH_STAKE_AMOUNT + expected_reward
    );
    assert_eq!(user_stake_after_claim.amount, GOLDEN_PATH_STAKE_AMOUNT);
    assert_eq!(user_stake_after_claim.pending_rewards, 0);
    assert_eq!(user_stake_after_claim.reward_debt, expected_reward as u128);

    env.unstake_user_a();

    let pool_after_unstake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_stake_after_unstake: UserStake =
        read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(pool_after_unstake.total_staked, 0);
    assert_eq!(pool_after_unstake.last_update_slot, claim_slot);
    assert_eq!(read_token_balance(&env.svm, &env.stake_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE + expected_reward
    );
    assert_eq!(user_stake_after_unstake.amount, 0);
    assert_eq!(user_stake_after_unstake.reward_debt, 0);
    assert_eq!(user_stake_after_unstake.pending_rewards, 0);
    assert_eq!(user_stake_after_unstake.lock_end_slot, 0);
}

#[test]
fn test_unstake_before_lock_reverts() {
    let mut env = setup_rails();

    env.stake_user_a(SMALL_STAKE_AMOUNT);

    let user_stake_after_stake: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    let attempted_slot = user_stake_after_stake.lock_end_slot - 1;
    warp_to_slot(&mut env, attempted_slot);

    assert_rails_error(env.try_unstake_user_a(), RailsError::LockActive);

    let pool_after_failed_unstake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_stake_after_failed_unstake: UserStake =
        read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(pool_after_failed_unstake.total_staked, SMALL_STAKE_AMOUNT);
    assert_eq!(read_token_balance(&env.svm, &env.stake_vault), SMALL_STAKE_AMOUNT);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - SMALL_STAKE_AMOUNT
    );
    assert_eq!(user_stake_after_failed_unstake.amount, SMALL_STAKE_AMOUNT);
    assert_eq!(user_stake_after_failed_unstake.reward_debt, 0);
    assert_eq!(user_stake_after_failed_unstake.pending_rewards, 0);
    assert_eq!(
        user_stake_after_failed_unstake.lock_end_slot,
        user_stake_after_stake.lock_end_slot
    );
}

#[test]
fn test_claim_with_underfunded_vault_partial_pays() {
    let mut env = setup_rails();

    env.stake_user_a(SMALL_STAKE_AMOUNT);
    let pool_after_stake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let claim_slot = pool_after_stake.last_update_slot + 10;
    warp_to_slot(&mut env, claim_slot);

    env.fund_reward_pool(9_999);
    env.claim_user_a();

    let user_after_partial: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - SMALL_STAKE_AMOUNT + 9_999
    );
    assert_eq!(user_after_partial.amount, SMALL_STAKE_AMOUNT);
    assert_eq!(user_after_partial.reward_debt, 10_000);
    assert_eq!(user_after_partial.pending_rewards, 1);

    env.fund_reward_pool(1);
    env.svm.expire_blockhash();
    env.claim_user_a();

    let user_after_full: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - SMALL_STAKE_AMOUNT + 10_000
    );
    assert_eq!(user_after_full.amount, SMALL_STAKE_AMOUNT);
    assert_eq!(user_after_full.reward_debt, 10_000);
    assert_eq!(user_after_full.pending_rewards, 0);
}

#[test]
fn test_set_reward_rate_admin_only() {
    let mut env = setup_rails();
    let outsider = Keypair::new();
    env.svm.airdrop(&outsider.pubkey(), 100_000_000_000).unwrap();

    assert_rails_error(
        env.try_set_reward_rate_as(&outsider, DEFAULT_REWARD_RATE_PER_SLOT * 2),
        RailsError::Unauthorized,
    );

    let pool_after_attempt: StakePool = read_anchor_account(&env.svm, &env.pool);
    assert_eq!(
        pool_after_attempt.reward_rate_per_slot,
        DEFAULT_REWARD_RATE_PER_SLOT
    );
    assert_eq!(pool_after_attempt.acc_reward_per_share, 0);
    assert_eq!(pool_after_attempt.total_staked, 0);
}

#[test]
fn test_set_reward_rate_above_cap_reverts() {
    let mut env = setup_rails();

    assert_rails_error(
        env.try_set_reward_rate_as_admin(MAX_REWARD_RATE_PER_SLOT + 1),
        RailsError::RewardRateTooHigh,
    );

    let pool_after_attempt: StakePool = read_anchor_account(&env.svm, &env.pool);
    assert_eq!(
        pool_after_attempt.reward_rate_per_slot,
        DEFAULT_REWARD_RATE_PER_SLOT
    );
    assert_eq!(pool_after_attempt.acc_reward_per_share, 0);
    assert_eq!(pool_after_attempt.total_staked, 0);
}

#[test]
fn test_reward_rate_change_mid_period_no_retroactive_effect() {
    let mut env = setup_rails();

    env.stake_user_a(SMALL_STAKE_AMOUNT);
    let pool_after_stake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let first_window_end = pool_after_stake.last_update_slot + 10;
    warp_to_slot(&mut env, first_window_end);

    env.set_reward_rate(DEFAULT_REWARD_RATE_PER_SLOT * 2);

    let pool_after_rate_change: StakePool = read_anchor_account(&env.svm, &env.pool);
    assert_eq!(pool_after_rate_change.last_update_slot, first_window_end);
    assert_eq!(
        pool_after_rate_change.acc_reward_per_share,
        expected_acc_reward_per_share(10_000, SMALL_STAKE_AMOUNT)
    );
    assert_eq!(
        pool_after_rate_change.reward_rate_per_slot,
        DEFAULT_REWARD_RATE_PER_SLOT * 2
    );

    warp_to_slot(&mut env, first_window_end + 10);
    env.fund_reward_pool(30_000);
    env.claim_user_a();

    let pool_after_claim: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_after_claim: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(
        pool_after_claim.acc_reward_per_share,
        expected_acc_reward_per_share(30_000, SMALL_STAKE_AMOUNT)
    );
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - SMALL_STAKE_AMOUNT + 30_000
    );
    assert_eq!(user_after_claim.reward_debt, 30_000);
    assert_eq!(user_after_claim.pending_rewards, 0);
}

#[test]
fn test_post_unstake_claim_drains_pending_rewards() {
    let mut env = setup_rails();

    env.stake_user_a(SMALL_STAKE_AMOUNT);
    let user_stake_after_stake: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    warp_to_slot(&mut env, user_stake_after_stake.lock_end_slot);

    env.unstake_user_a();

    let pool_after_unstake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_after_unstake: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(pool_after_unstake.total_staked, 0);
    assert_eq!(read_token_balance(&env.svm, &env.stake_vault), 0);
    assert_eq!(read_token_balance(&env.svm, &env.user_a.ccm), USER_START_BALANCE);
    assert_eq!(user_after_unstake.amount, 0);
    assert_eq!(user_after_unstake.reward_debt, 0);
    assert_eq!(user_after_unstake.pending_rewards, 1_000_000);
    assert_eq!(user_after_unstake.lock_end_slot, 0);

    env.fund_reward_pool(1_000_000);
    env.claim_user_a();

    let user_after_claim: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE + 1_000_000
    );
    assert_eq!(user_after_claim.amount, 0);
    assert_eq!(user_after_claim.reward_debt, 0);
    assert_eq!(user_after_claim.pending_rewards, 0);
    assert_eq!(user_after_claim.lock_end_slot, 0);
}

#[test]
fn test_two_users_proportional_distribution() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);

    env.stake_user_a(SMALL_STAKE_AMOUNT);
    let pool_after_user_a_stake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_a_start_slot = pool_after_user_a_stake.last_update_slot;

    warp_to_slot(&mut env, user_a_start_slot + 10);
    env.stake_for_user(&user_b, USER_B_STAKE_AMOUNT);

    let pool_after_user_b_stake: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_b_after_stake: UserStake = read_anchor_account(&env.svm, &user_b.user_stake);
    assert_eq!(pool_after_user_b_stake.total_staked, SMALL_STAKE_AMOUNT + USER_B_STAKE_AMOUNT);
    assert_eq!(
        pool_after_user_b_stake.acc_reward_per_share,
        expected_acc_reward_per_share(10_000, SMALL_STAKE_AMOUNT)
    );
    assert_eq!(user_b_after_stake.amount, USER_B_STAKE_AMOUNT);
    assert_eq!(user_b_after_stake.reward_debt, 30_000);
    assert_eq!(user_b_after_stake.pending_rewards, 0);

    warp_to_slot(&mut env, user_a_start_slot + 110);
    env.fund_reward_pool(110_000);
    env.claim_user_a();
    env.claim_for_user(&user_b);

    let pool_after_claims: StakePool = read_anchor_account(&env.svm, &env.pool);
    let user_a_after_claim: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    let user_b_after_claim: UserStake = read_anchor_account(&env.svm, &user_b.user_stake);
    assert_eq!(
        pool_after_claims.acc_reward_per_share,
        expected_acc_reward_per_share(35_000, SMALL_STAKE_AMOUNT)
    );
    assert_eq!(read_token_balance(&env.svm, &env.reward_vault), 0);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE - SMALL_STAKE_AMOUNT + 35_000
    );
    assert_eq!(
        read_token_balance(&env.svm, &user_b.ccm),
        USER_START_BALANCE - USER_B_STAKE_AMOUNT + 75_000
    );
    assert_eq!(user_a_after_claim.reward_debt, 35_000);
    assert_eq!(user_a_after_claim.pending_rewards, 0);
    assert_eq!(user_b_after_claim.reward_debt, 105_000);
    assert_eq!(user_b_after_claim.pending_rewards, 0);
}

#[test]
fn test_claim_compensation_happy_path() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);
    let compensation_amount = 123_456;
    let (root, user_a_proof, _) =
        two_leaf_merkle((env.user_a.pubkey(), compensation_amount), (user_b.pubkey(), 777_777));

    env.compensate_external_stakers(root);
    env.fund_comp_vault(500_000);
    env.claim_compensation_user_a(compensation_amount, user_a_proof);

    let config: Config = read_anchor_account(&env.svm, &env.config);
    let claimed: CompensationClaimed = read_anchor_account(&env.svm, &env.user_a.comp_claimed);
    assert_eq!(config.comp_merkle_root, root);
    assert_eq!(claimed.user, env.user_a.pubkey());
    assert_eq!(claimed.amount, compensation_amount);
    assert_eq!(read_token_balance(&env.svm, &env.comp_vault), 500_000 - compensation_amount);
    assert_eq!(
        read_token_balance(&env.svm, &env.user_a.ccm),
        USER_START_BALANCE + compensation_amount
    );
}

#[test]
fn test_claim_compensation_already_claimed_reverts() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);
    let compensation_amount = 10_000;
    let (root, user_a_proof, _) =
        two_leaf_merkle((env.user_a.pubkey(), compensation_amount), (user_b.pubkey(), 20_000));

    env.compensate_external_stakers(root);
    env.fund_comp_vault(50_000);
    env.claim_compensation_user_a(compensation_amount, user_a_proof.clone());

    let balance_before = read_token_balance(&env.svm, &env.user_a.ccm);
    assert!(env
        .try_claim_compensation_user_a(compensation_amount, user_a_proof)
        .is_err());
    assert_eq!(read_token_balance(&env.svm, &env.user_a.ccm), balance_before);
}

#[test]
fn test_claim_compensation_wrong_proof_reverts() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);
    let (root, _, user_b_proof) =
        two_leaf_merkle((env.user_a.pubkey(), 33_333), (user_b.pubkey(), 44_444));

    env.compensate_external_stakers(root);
    env.fund_comp_vault(100_000);

    assert_rails_error(
        env.try_claim_compensation_user_a(33_334, user_b_proof),
        RailsError::CompensationInvalidProof,
    );
}

#[test]
fn test_compensation_root_already_set_reverts() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);
    let (root_one, _, _) =
        two_leaf_merkle((env.user_a.pubkey(), 1_000), (user_b.pubkey(), 2_000));
    let (root_two, _, _) =
        two_leaf_merkle((env.user_a.pubkey(), 3_000), (user_b.pubkey(), 4_000));

    env.compensate_external_stakers(root_one);
    assert!(env.try_compensate_external_stakers(root_two).is_err());

    let config: Config = read_anchor_account(&env.svm, &env.config);
    assert_eq!(config.comp_merkle_root, root_one);
}

#[test]
fn test_claim_compensation_bad_signer_reverts() {
    let mut env = setup_rails();
    let user_b = env.create_user(USER_START_BALANCE);
    let compensation_amount = 55_555;
    let (root, user_a_proof, _) =
        two_leaf_merkle((env.user_a.pubkey(), compensation_amount), (user_b.pubkey(), 66_666));

    env.compensate_external_stakers(root);
    env.fund_comp_vault(100_000);

    assert_rails_error(
        env.try_claim_compensation_custom(
            &user_b.signer,
            env.user_a.ccm,
            user_b.comp_claimed,
            compensation_amount,
            user_a_proof,
        ),
        RailsError::Unauthorized,
    );
}

// ═══════════════════════════════════════════════════════════════════════
// Token-2022 TransferFee path (shift-left fee-accounting validation)
// ═══════════════════════════════════════════════════════════════════════
// Added 2026-04-19. The 13 tests above use a plain Token-2022 mint with no
// fee extension, so the stake IX's balance-diff crediting logic
// (actual_received = balance_after - balance_before) is exercised only in
// the degenerate case where fee = 0 and actual_received == amount. This
// block adds a single test that re-runs the stake path against a mint
// with TransferFeeConfig initialized, asserting the production fee math.
// Strictly additive: no existing helper or test is modified.

/// Mirror of [`create_plain_token_2022_mint`] but pre-initializes a
/// `TransferFeeConfig` extension on the same mint account. Ordering
/// matters: the extension IX must precede `initialize_mint2` in the same
/// tx. The account is sized for `Mint` base + `TransferFeeConfig` extension.
/// Authorities (config + withheld) are set to `None` — the extension is
/// effectively immutable, matching mainnet CCM's post-launch config.
fn create_token_2022_mint_with_transfer_fee(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Keypair,
    mint_authority: &LegacyPubkey,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) {
    use spl_token_2022::extension::transfer_fee::instruction::initialize_transfer_fee_config;
    use spl_token_2022::extension::ExtensionType;

    let payer_pubkey = legacy_from_signer(payer);
    let mint_pubkey = legacy_from_signer(mint);
    let mint_size = ExtensionType::try_calculate_account_len::<Mint>(&[
        ExtensionType::TransferFeeConfig,
    ])
    .expect("failed to calculate mint size with TransferFee extension");
    let rent = svm.minimum_balance_for_rent_exemption(mint_size);

    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &mint_pubkey,
        rent,
        mint_size as u64,
        &spl_token_2022::id(),
    );
    let fee_config_ix = initialize_transfer_fee_config(
        &spl_token_2022::id(),
        &mint_pubkey,
        None,
        None,
        transfer_fee_basis_points,
        maximum_fee,
    )
    .expect("failed to build initialize_transfer_fee_config ix");
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        &mint_pubkey,
        mint_authority,
        None,
        CCM_DECIMALS,
    )
    .expect("failed to build initialize_mint2 ix");

    send_tx(svm, &[payer, mint], &[create_ix, fee_config_ix, init_mint_ix]);
}

/// Mirror of [`create_token_2022_account`] but sized for the
/// `TransferFeeAmount` extension, required when holding a mint that has
/// `TransferFeeConfig` initialized. The extension is auto-initialized by
/// `initialize_account3` when the account has sufficient space allocated
/// for it; no separate extension-init IX is needed on the account side
/// (the withheld-fee counter starts at 0).
fn create_token_2022_account_with_fee_amount(
    svm: &mut LiteSVM,
    payer: &Keypair,
    token_account: &Keypair,
    mint: &LegacyPubkey,
    owner: &LegacyPubkey,
) {
    use spl_token_2022::extension::ExtensionType;

    let payer_pubkey = legacy_from_signer(payer);
    let token_account_pubkey = legacy_from_signer(token_account);
    let account_size = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
        ExtensionType::TransferFeeAmount,
    ])
    .expect("failed to calculate token account size with TransferFeeAmount");
    let rent = svm.minimum_balance_for_rent_exemption(account_size);

    let create_ix = system_instruction::create_account(
        &payer_pubkey,
        &token_account_pubkey,
        rent,
        account_size as u64,
        &spl_token_2022::id(),
    );
    let init_ix = spl_token_2022::instruction::initialize_account3(
        &spl_token_2022::id(),
        &token_account_pubkey,
        mint,
        owner,
    )
    .expect("failed to build initialize_account3 ix");

    send_tx(svm, &[payer, token_account], &[create_ix, init_ix]);
}

/// Mirror of [`read_token_balance`] that handles Token-2022 extension
/// accounts. Plain `TokenAccount::unpack` requires the data length to equal
/// `TokenAccount::LEN` (165 bytes) and fails on fee-extended accounts,
/// which carry extra bytes for `TransferFeeAmount`. `StateWithExtensions`
/// parses the base-account slice plus the TLV extension tail.
fn read_token_balance_with_extensions(svm: &LiteSVM, address: &LegacyPubkey) -> u64 {
    use spl_token_2022::extension::StateWithExtensions;

    let account = svm
        .get_account(&address_from_legacy(address))
        .unwrap_or_else(|| panic!("missing token account: {address}"));
    let state = StateWithExtensions::<TokenAccount>::unpack(&account.data)
        .expect("failed to unpack token account with extensions");
    state.base.amount
}

/// Mirror of [`create_user_fixture`] that uses the fee-aware account
/// helper. Necessary because plain token accounts cannot hold a mint with
/// `TransferFeeConfig` — `InitializeAccount3` returns `InvalidAccountData`
/// without the extension space.
fn create_user_fixture_with_fee_amount(
    svm: &mut LiteSVM,
    mint_authority: &Keypair,
    ccm_mint: &LegacyPubkey,
    pool: &LegacyPubkey,
    starting_balance: u64,
) -> UserFixture {
    let signer = Keypair::new();
    let token_account = Keypair::new();

    svm.airdrop(&signer.pubkey(), 100_000_000_000).unwrap();

    let user_pubkey = legacy_from_signer(&signer);
    create_token_2022_account_with_fee_amount(svm, &signer, &token_account, ccm_mint, &user_pubkey);

    let ccm = legacy_from_signer(&token_account);
    mint_token_2022(svm, mint_authority, ccm_mint, &ccm, starting_balance);

    let (user_stake, _) = derive_user_stake(pool, &user_pubkey);
    let (comp_claimed, _) = derive_comp_claimed(&user_pubkey);
    UserFixture {
        signer,
        ccm,
        user_stake,
        comp_claimed,
    }
}

/// Mirror of [`setup_rails`] that uses a fee-enabled CCM mint. All other
/// initialization (Config, Pool at pool_id=0, StakeVault, RewardVault,
/// User A) is identical to the plain-mint setup. Only the mint creation
/// step differs.
fn setup_rails_with_transfer_fee(
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> TestEnv {
    let mut svm = LiteSVM::new();

    if let Err(err) = load_wzrd_rails_program(&mut svm) {
        panic!(
            "Failed to load wzrd-rails program: {err}. Run `anchor build --program-name wzrd_rails` first."
        );
    }
    if let Err(err) = load_token_2022_program(&mut svm) {
        panic!("Failed to load Token-2022 program into LiteSVM: {err}");
    }

    let admin = Keypair::new();
    let ccm_mint = Keypair::new();
    let admin_ccm_account = Keypair::new();

    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();

    let admin_pubkey = legacy_from_signer(&admin);
    let ccm_mint_pubkey = legacy_from_signer(&ccm_mint);

    create_token_2022_mint_with_transfer_fee(
        &mut svm,
        &admin,
        &ccm_mint,
        &admin_pubkey,
        transfer_fee_basis_points,
        maximum_fee,
    );
    create_token_2022_account_with_fee_amount(
        &mut svm,
        &admin,
        &admin_ccm_account,
        &ccm_mint_pubkey,
        &admin_pubkey,
    );

    let admin_ccm = legacy_from_signer(&admin_ccm_account);
    mint_token_2022(
        &mut svm,
        &admin,
        &ccm_mint_pubkey,
        &admin_ccm,
        ADMIN_START_BALANCE,
    );

    let (config, _) = derive_config();
    let (pool, _) = derive_pool(POOL_ID);
    let (stake_vault, _) = derive_stake_vault(&pool);
    let (reward_vault, _) = derive_reward_vault(&pool);
    let (comp_vault, _) = derive_comp_vault(&config);

    let user_a = create_user_fixture_with_fee_amount(
        &mut svm,
        &admin,
        &ccm_mint_pubkey,
        &pool,
        USER_START_BALANCE,
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_config_ix(
            admin_pubkey,
            config,
            ccm_mint_pubkey,
            admin_ccm,
        )],
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_initialize_pool_ix(
            config,
            pool,
            ccm_mint_pubkey,
            stake_vault,
            reward_vault,
            admin_pubkey,
        )],
    );

    send_tx(
        &mut svm,
        &[&admin],
        &[build_set_reward_rate_ix(
            config,
            pool,
            admin_pubkey,
            DEFAULT_REWARD_RATE_PER_SLOT,
        )],
    );

    TestEnv {
        svm,
        admin,
        ccm_mint,
        config,
        pool,
        stake_vault,
        reward_vault,
        comp_vault,
        admin_ccm,
        user_a,
    }
}

#[test]
fn stake_with_transfer_fee_credits_actual_received() {
    // 50 basis points = 0.5% fee. u64::MAX = no cap. Matches mainnet CCM config.
    const FEE_BASIS_POINTS: u16 = 50;
    const MAX_FEE: u64 = u64::MAX;
    const STAKE_REQUEST: u64 = 1_000_000_000; // 1 CCM at 9 decimals

    let mut env = setup_rails_with_transfer_fee(FEE_BASIS_POINTS, MAX_FEE);

    // Expected fee: 1_000_000_000 * 50 / 10_000 = 5_000_000 units.
    // Expected received: 1_000_000_000 - 5_000_000 = 995_000_000 units.
    let expected_fee: u64 =
        ((STAKE_REQUEST as u128) * (FEE_BASIS_POINTS as u128) / 10_000u128) as u64;
    let expected_received: u64 = STAKE_REQUEST - expected_fee;

    // Sanity: mint_to does NOT apply TransferFee (fee only applies between
    // non-mint accounts), so user's ATA holds the full USER_START_BALANCE.
    let user_balance_before = read_token_balance_with_extensions(&env.svm, &env.user_a.ccm);
    assert!(
        user_balance_before >= STAKE_REQUEST,
        "user_a needs >= {STAKE_REQUEST} pre-stake, has {user_balance_before}"
    );

    env.stake_user_a(STAKE_REQUEST);

    // Primary assertion: the stake vault holds only what actually arrived
    // after the mint deducted the TransferFee in-flight. This is the bug
    // surface `actual_received = balance_after - balance_before` exists for.
    let stake_vault_balance = read_token_balance_with_extensions(&env.svm, &env.stake_vault);
    assert_eq!(
        stake_vault_balance, expected_received,
        "stake_vault must hold post-fee amount ({expected_received}), has {stake_vault_balance}"
    );

    // Pool accounting must match vault balance — the pool is credited with
    // `actual_received`, not `requested_amount`.
    let pool: StakePool = read_anchor_account(&env.svm, &env.pool);
    assert_eq!(
        pool.total_staked, expected_received,
        "pool.total_staked must equal post-fee received amount"
    );

    // User stake principal must also match vault balance (the user's share
    // of the pool is what actually arrived, not what they asked for).
    let user_stake: UserStake = read_anchor_account(&env.svm, &env.user_a.user_stake);
    assert_eq!(
        user_stake.amount, expected_received,
        "user_stake.amount must equal post-fee received amount"
    );
    assert!(
        user_stake.amount < STAKE_REQUEST,
        "user_stake.amount ({}) must be strictly less than stake request ({}) -- fee not absorbed",
        user_stake.amount,
        STAKE_REQUEST
    );

    // At first stake in an empty pool, acc_reward_per_share = 0, so
    // reward_debt (= amount * acc / SCALE) must be 0 regardless of the
    // fee delta.
    assert_eq!(
        user_stake.reward_debt, 0u128,
        "reward_debt at first stake in empty pool must be 0"
    );

    // The user's own balance decreased by the full STAKE_REQUEST — they
    // paid the fee from their side, not from the pool's side.
    let user_balance_after = read_token_balance_with_extensions(&env.svm, &env.user_a.ccm);
    assert_eq!(
        user_balance_before - user_balance_after,
        STAKE_REQUEST,
        "user should have sent STAKE_REQUEST ({STAKE_REQUEST}) units from their ATA"
    );
}
