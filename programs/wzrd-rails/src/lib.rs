//! wzrd-rails — CCM productivity rail.
//!
//! Purpose: restore the SAVE loop for the TWZRD swarm after AO v2 phase2 features
//! were stripped from the immutable binary. Agents stake earned CCM here; a
//! revenue-router keeper funds the reward pool via USDC→CCM buyback; stakers
//! compound into a larger CCM position over time.
//!
//! Scope Day 1 (per project_solana_economy_plan.md v3.2):
//!   - single global pool (pool_id = 0)
//!   - 7-day weekly lockup
//!   - MasterChef-style `acc_reward_per_share` accumulator
//!   - CCM-denominated rewards (Token-2022)
//!   - one-time external compensation merkle drop

use anchor_lang::prelude::*;
use anchor_spl::token_2022::ID as TOKEN_2022_PROGRAM_ID;
use anchor_spl::token_interface::{
    self, Mint as MintInterface, TokenAccount, TokenInterface, TransferChecked,
};
use solana_keccak_hasher as keccak;

declare_id!("BdSv824hvYeGAWQZUcypRzAor8yJit2qeqCHty3CSZy9");

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;

#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "wzrd-rails",
    project_url: "https://github.com/twzrd-sol/attention-oracle-program",
    contacts: "email:security@twzrd.xyz",
    policy: "https://github.com/twzrd-sol/attention-oracle-program/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/twzrd-sol/attention-oracle-program"
}

pub mod error;
pub mod state;

pub use error::*;
pub use state::*;

#[program]
pub mod wzrd_rails {
    use super::*;

    /// Initialize the program's global config. One-time, per deployment.
    ///
    /// The signer becomes the initial admin. Admin can be transferred later via
    /// `set_admin` (e.g., to a Squads V4 vault PDA once production multisig is set up).
    ///
    /// Preconditions:
    ///   - Config PDA must not already exist (init constraint below enforces).
    ///   - Caller must sign and pay for rent.
    ///
    /// Postconditions:
    ///   - Config { admin = signer, ccm_mint, treasury_ccm_ata, comp_merkle_root = [0; 32], total_pools = 0, bump }
    ///
    /// Does NOT validate that `ccm_mint` is a real Token-2022 mint or that
    /// `treasury_ccm_ata` exists. These are trust-the-admin parameters — verified
    /// by the admin's off-chain process, not by the program. Rationale: avoiding
    /// an anchor_spl dependency at init keeps the compile surface minimal and
    /// matches the karpathy "simplicity first" discipline. Mint/ATA validity
    /// is re-checked at the point of use (funding, staking, claiming).
    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        ccm_mint: Pubkey,
        treasury_ccm_ata: Pubkey,
    ) -> Result<()> {
        let slot = Clock::get()?.slot;
        let config_key = ctx.accounts.config.key();
        let admin = ctx.accounts.signer.key();
        let config = &mut ctx.accounts.config;
        config.admin = admin;
        config.ccm_mint = ccm_mint;
        config.treasury_ccm_ata = treasury_ccm_ata;
        config.comp_merkle_root = [0u8; 32];
        config.total_pools = 0;
        config.bump = ctx.bumps.config;
        emit!(ConfigInitialized {
            config: config_key,
            admin,
            ccm_mint,
            treasury_ccm_ata,
            slot,
        });
        Ok(())
    }

    /// Transfer admin authority to a new pubkey.
    ///
    /// Use cases:
    ///   - Migrate from deployer key → Squads V4 vault PDA at production go-live
    ///   - Rotate admin after a key suspected compromise
    ///   - Retire admin role by transferring to a dead address (operational immutability)
    ///
    /// Preconditions: current admin signs.
    /// Postconditions: config.admin = new_admin.
    pub fn set_admin(ctx: Context<AdminOnly>, new_admin: Pubkey) -> Result<()> {
        let slot = Clock::get()?.slot;
        let config_key = ctx.accounts.config.key();
        let old_admin = ctx.accounts.config.admin;
        ctx.accounts.config.admin = new_admin;
        emit!(AdminChanged {
            config: config_key,
            old_admin,
            new_admin,
            slot,
        });
        Ok(())
    }

    /// Change a pool's reward emission rate. Admin-only.
    ///
    /// Semantic: emissions are measured in CCM base units per slot. At 400ms/slot,
    /// 1 CCM/slot ≈ 216,000 CCM/day. Start conservatively.
    ///
    /// Critical: accrue_rewards runs FIRST so stakers are credited at the old rate
    /// for the slot window up to `now`. The new rate only applies to slots after
    /// this IX lands. Without this, admin could retroactively amplify/squash
    /// historical accrual by changing the rate.
    ///
    /// Preconditions: admin signs; pool exists.
    /// Postconditions: pool.reward_rate_per_slot = new_rate; accumulator settled up to current slot.
    pub fn set_reward_rate(
        ctx: Context<SetRewardRate>,
        _pool_id: u32,
        new_rate: u64,
    ) -> Result<()> {
        require!(
            new_rate <= MAX_REWARD_RATE_PER_SLOT,
            RailsError::RewardRateTooHigh
        );
        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.pool;
        let old_rate = pool.reward_rate_per_slot;
        pool.accrue_rewards(clock.slot)
            .map_err(|_| error!(RailsError::MathOverflow))?;
        pool.reward_rate_per_slot = new_rate;
        emit!(RewardRateChanged {
            pool: ctx.accounts.pool.key(),
            pool_id: ctx.accounts.pool.pool_id,
            old_rate,
            new_rate,
            slot: clock.slot,
        });
        Ok(())
    }

    /// Set the one-time compensation merkle root and eagerly create the
    /// config-level compensation vault.
    ///
    /// This is admin-only and intentionally does not verify the root or require
    /// any funding. The vault is initialized even if it stays empty for now.
    pub fn compensate_external_stakers(
        ctx: Context<CompensateExternalStakers>,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        let slot = Clock::get()?.slot;
        require!(
            ctx.accounts.config.comp_merkle_root == [0u8; 32],
            RailsError::CompensationAlreadySet
        );
        require!(merkle_root != [0u8; 32], RailsError::CompensationInvalidProof);

        ctx.accounts.config.comp_merkle_root = merkle_root;
        emit!(CompensationRootSet {
            config: ctx.accounts.config.key(),
            admin: ctx.accounts.admin.key(),
            comp_vault: ctx.accounts.comp_vault.key(),
            merkle_root,
            slot,
        });
        Ok(())
    }

    /// Initialize a new stake pool. Admin-only.
    ///
    /// Day 1 creates pool_id = 0 (the global pool). Future per-channel pools
    /// reuse this IX with new ids — no IX changes required.
    ///
    /// Preconditions:
    ///   - admin signer matches config.admin
    ///   - pool_id must equal config.total_pools (enforces sequential numbering,
    ///     prevents gaps, makes off-chain enumeration predictable)
    ///   - pool PDA must not already exist (init constraint enforces)
    ///
    /// Postconditions:
    ///   - StakePool { pool_id, total_staked=0, acc_reward_per_share=0,
    ///                 reward_rate_per_slot=0, last_update_slot=Clock::slot,
    ///                 lock_duration_slots, bump }
    ///   - config.total_pools += 1
    ///
    /// Reward rate starts at 0 — emissions require an explicit `set_reward_rate`
    /// call by admin. This lets admin create the pool without committing to
    /// emissions until ready (e.g., post-audit, post-compensation, etc.).
    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        pool_id: u32,
        lock_duration_slots: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;

        // Enforce sequential numbering. Admin must pass pool_id == total_pools.
        require_eq!(pool_id, config.total_pools, RailsError::InvalidPoolId);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.pool;
        pool.pool_id = pool_id;
        pool.total_staked = 0;
        pool.acc_reward_per_share = 0;
        pool.reward_rate_per_slot = 0;
        pool.last_update_slot = clock.slot;
        pool.lock_duration_slots = lock_duration_slots;
        pool.bump = ctx.bumps.pool;

        config.total_pools = config
            .total_pools
            .checked_add(1)
            .ok_or(RailsError::MathOverflow)?;
        emit!(PoolInitialized {
            config: ctx.accounts.config.key(),
            pool: ctx.accounts.pool.key(),
            pool_id,
            stake_vault: ctx.accounts.stake_vault.key(),
            reward_vault: ctx.accounts.reward_vault.key(),
            lock_duration_slots,
            slot: clock.slot,
        });
        Ok(())
    }

    /// Stake CCM into a pool.
    ///
    /// Critical accounting detail: CCM is Token-2022 with a transfer fee, so the
    /// pool must credit `actual_received`, not the user-requested `amount`.
    /// The stake vault balance is sampled before/after transfer to compute the true
    /// post-fee principal added to the pool.
    ///
    /// Reward bookkeeping:
    ///   1. Accrue pool rewards to `now`
    ///   2. Carry forward any claimable rewards into `user_stake.pending_rewards`
    ///   3. Increase principal by `actual_received`
    ///   4. Re-anchor `reward_debt` at the new amount × current accumulator
    pub fn stake(ctx: Context<Stake>, _pool_id: u32, amount: u64) -> Result<()> {
        require!(amount > 0, RailsError::StakeAmountZero);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.pool;
        pool.accrue_rewards(clock.slot)
            .map_err(|_| error!(RailsError::MathOverflow))?;

        let user_stake = &mut ctx.accounts.user_stake;
        // `init_if_needed` zero-initializes a freshly-created UserStake account.
        // That makes `Pubkey::default()` the reliable "first stake" marker here,
        // while any pre-existing UserStake deserializes with its stored owner.
        let existing = user_stake.user != Pubkey::default();
        if existing {
            let pending = user_stake
                .total_claimable(pool.acc_reward_per_share)
                .map_err(|_| error!(RailsError::MathOverflow))?;
            user_stake.pending_rewards = pending;
        } else {
            user_stake.user = ctx.accounts.user.key();
            user_stake.pool = pool.key();
            user_stake.amount = 0;
            user_stake.reward_debt = 0;
            user_stake.pending_rewards = 0;
            user_stake.bump = ctx.bumps.user_stake;
        }

        let balance_before = ctx.accounts.stake_vault.amount;
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.user_ccm.to_account_info(),
                mint: ctx.accounts.ccm_mint.to_account_info(),
                to: ctx.accounts.stake_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        );
        token_interface::transfer_checked(transfer_ctx, amount, ctx.accounts.ccm_mint.decimals)?;

        ctx.accounts.stake_vault.reload()?;
        let actual_received = ctx
            .accounts
            .stake_vault
            .amount
            .checked_sub(balance_before)
            .ok_or(RailsError::MathOverflow)?;
        require!(actual_received > 0, RailsError::StakeAmountZero);

        pool.total_staked = pool
            .total_staked
            .checked_add(actual_received)
            .ok_or(RailsError::MathOverflow)?;

        user_stake.amount = user_stake
            .amount
            .checked_add(actual_received)
            .ok_or(RailsError::MathOverflow)?;
        user_stake.lock_end_slot = clock
            .slot
            .checked_add(pool.lock_duration_slots)
            .ok_or(RailsError::MathOverflow)?;
        user_stake.reward_debt = (user_stake.amount as u128)
            .checked_mul(pool.acc_reward_per_share)
            .ok_or(RailsError::MathOverflow)?
            .checked_div(StakePool::REWARD_SCALE)
            .ok_or(RailsError::MathOverflow)?;

        emit!(Staked {
            pool: ctx.accounts.pool.key(),
            user: ctx.accounts.user.key(),
            user_stake: ctx.accounts.user_stake.key(),
            requested_amount: amount,
            actual_received,
            total_staked: ctx.accounts.pool.total_staked,
            lock_end_slot: ctx.accounts.user_stake.lock_end_slot,
            slot: clock.slot,
        });

        Ok(())
    }

    /// Permissionlessly fund a pool's reward vault with CCM.
    ///
    /// This is pure token movement only:
    ///   - no admin/keeper gating
    ///   - no reward accrual
    ///   - no pool or user state mutation
    ///
    /// CCM is Token-2022 with transfer fees enabled, so the vault is credited
    /// based on the actual post-fee amount received, not the requested amount.
    pub fn fund_reward_pool(
        ctx: Context<FundRewardPool>,
        _pool_id: u32,
        ccm_amount: u64,
    ) -> Result<()> {
        let slot = Clock::get()?.slot;
        require!(ccm_amount > 0, RailsError::StakeAmountZero);

        let balance_before = ctx.accounts.reward_vault.amount;
        let transfer_ctx = CpiContext::new(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funder_ccm.to_account_info(),
                mint: ctx.accounts.ccm_mint.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        );
        token_interface::transfer_checked(
            transfer_ctx,
            ccm_amount,
            ctx.accounts.ccm_mint.decimals,
        )?;

        ctx.accounts.reward_vault.reload()?;
        let actual_received = ctx
            .accounts
            .reward_vault
            .amount
            .checked_sub(balance_before)
            .ok_or(RailsError::MathOverflow)?;
        require!(actual_received > 0, RailsError::StakeAmountZero);

        emit!(RewardPoolFunded {
            pool: ctx.accounts.pool.key(),
            funder: ctx.accounts.funder.key(),
            reward_vault: ctx.accounts.reward_vault.key(),
            requested_amount: ccm_amount,
            actual_received,
            slot,
        });

        Ok(())
    }

    /// Fully unstake a user's principal once the lock window has expired.
    ///
    /// Day 1 policy is intentionally simple:
    ///   - full-only unstake
    ///   - principal withdrawal must not depend on reward-vault solvency
    ///   - the UserStake PDA stays open for cheap future restakes
    ///
    /// Reward bookkeeping:
    ///   1. Accrue pool rewards to `now`
    ///   2. Carry current total claimable into `pending_rewards`
    ///   3. Transfer principal from stake_vault → user_ccm with pool PDA signer
    ///   4. Zero principal and reward_debt; keep `pending_rewards` for future claim
    ///
    /// Exit fee semantics:
    ///   CCM's Token-2022 transfer fee is borne by the withdrawing user. Pool
    ///   accounting is reduced by the sent principal amount, not the user's
    ///   post-fee received amount.
    pub fn unstake(ctx: Context<Unstake>, _pool_id: u32) -> Result<()> {
        let clock = Clock::get()?;
        let pool_id_bytes = ctx.accounts.pool.pool_id.to_le_bytes();
        let pool_bump = ctx.accounts.pool.bump;
        let pool_ai = ctx.accounts.pool.to_account_info();
        {
            let pool = &mut ctx.accounts.pool;
            pool.accrue_rewards(clock.slot)
                .map_err(|_| error!(RailsError::MathOverflow))?;
        }

        let user_stake = &mut ctx.accounts.user_stake;
        require!(user_stake.amount > 0, RailsError::NothingStaked);
        require!(clock.slot >= user_stake.lock_end_slot, RailsError::LockActive);

        let pending = user_stake
            .total_claimable(ctx.accounts.pool.acc_reward_per_share)
            .map_err(|_| error!(RailsError::MathOverflow))?;
        let unstake_amount = user_stake.amount;

        let signer_seeds: &[&[&[u8]]] =
            &[&[POOL_SEED, pool_id_bytes.as_ref(), &[pool_bump]]];
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.stake_vault.to_account_info(),
                mint: ctx.accounts.ccm_mint.to_account_info(),
                to: ctx.accounts.user_ccm.to_account_info(),
                authority: pool_ai,
            },
            signer_seeds,
        );
        token_interface::transfer_checked(
            transfer_ctx,
            unstake_amount,
            ctx.accounts.ccm_mint.decimals,
        )?;

        let pool = &mut ctx.accounts.pool;
        pool.total_staked = pool
            .total_staked
            .checked_sub(unstake_amount)
            .ok_or(RailsError::MathOverflow)?;

        user_stake.amount = 0;
        user_stake.reward_debt = 0;
        user_stake.pending_rewards = pending;
        user_stake.lock_end_slot = 0;

        emit!(Unstaked {
            pool: ctx.accounts.pool.key(),
            user: ctx.accounts.user.key(),
            user_stake: ctx.accounts.user_stake.key(),
            amount: unstake_amount,
            remaining_total_staked: ctx.accounts.pool.total_staked,
            pending_rewards: ctx.accounts.user_stake.pending_rewards,
            slot: clock.slot,
        });

        Ok(())
    }

    /// Claim accrued CCM rewards from the reward vault.
    ///
    /// Partial pay is intentional: if the reward vault is underfunded, the user
    /// receives whatever is currently liquid and the remainder stays in
    /// `pending_rewards` for a later claim. This must continue to work even after
    /// a full unstake, when `amount == 0` but `pending_rewards > 0`.
    pub fn claim(ctx: Context<Claim>, _pool_id: u32) -> Result<()> {
        let clock = Clock::get()?;
        let pool_id_bytes = ctx.accounts.pool.pool_id.to_le_bytes();
        let pool_bump = ctx.accounts.pool.bump;
        let pool_ai = ctx.accounts.pool.to_account_info();
        {
            let pool = &mut ctx.accounts.pool;
            pool.accrue_rewards(clock.slot)
                .map_err(|_| error!(RailsError::MathOverflow))?;
        }

        let acc_reward_per_share = ctx.accounts.pool.acc_reward_per_share;
        let owed = ctx
            .accounts
            .user_stake
            .total_claimable(acc_reward_per_share)
            .map_err(|_| error!(RailsError::MathOverflow))?;
        require!(owed > 0, RailsError::NoRewardsAvailable);

        let pay = owed.min(ctx.accounts.reward_vault.amount);
        require!(pay > 0, RailsError::NoRewardsAvailable);

        let signer_seeds: &[&[&[u8]]] =
            &[&[POOL_SEED, pool_id_bytes.as_ref(), &[pool_bump]]];
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.reward_vault.to_account_info(),
                mint: ctx.accounts.ccm_mint.to_account_info(),
                to: ctx.accounts.user_ccm.to_account_info(),
                authority: pool_ai,
            },
            signer_seeds,
        );
        token_interface::transfer_checked(transfer_ctx, pay, ctx.accounts.ccm_mint.decimals)?;

        let user_stake = &mut ctx.accounts.user_stake;
        user_stake.reward_debt = (user_stake.amount as u128)
            .checked_mul(acc_reward_per_share)
            .ok_or(RailsError::MathOverflow)?
            .checked_div(StakePool::REWARD_SCALE)
            .ok_or(RailsError::MathOverflow)?;
        user_stake.pending_rewards = owed.checked_sub(pay).ok_or(RailsError::MathOverflow)?;

        emit!(Claimed {
            pool: ctx.accounts.pool.key(),
            user: ctx.accounts.user.key(),
            user_stake: ctx.accounts.user_stake.key(),
            owed,
            paid: pay,
            pending_rewards: ctx.accounts.user_stake.pending_rewards,
            slot: clock.slot,
        });

        Ok(())
    }

    /// Claim the one-time external compensation allotment proved against the
    /// config-level merkle root.
    ///
    /// Day 1 policy is atomic, not streaming:
    ///   - proof must verify against the stored root
    ///   - replay is blocked by creating a `CompensationClaimed` PDA
    ///   - the claim reverts if the compensation vault cannot cover the amount
    ///
    /// Leaf convention:
    ///   leaf = keccak::hashv(&[
    ///       COMPENSATION_LEAF_DOMAIN,
    ///       user.as_ref(),
    ///       amount.to_le_bytes().as_ref(),
    ///   ])
    /// Internal nodes are sorted-pair keccak(min, max).
    pub fn claim_compensation(
        ctx: Context<ClaimCompensation>,
        amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        require!(amount > 0, RailsError::StakeAmountZero);
        require!(ctx.accounts.config.comp_root_set(), RailsError::CompensationInvalidProof);
        require!(
            verify_compensation_proof(
                &ctx.accounts.user.key(),
                amount,
                &proof,
                &ctx.accounts.config.comp_merkle_root,
            ),
            RailsError::CompensationInvalidProof
        );
        require!(
            ctx.accounts.comp_vault.amount >= amount,
            RailsError::CompensationUnavailable
        );

        let slot = Clock::get()?.slot;
        let config_ai = ctx.accounts.config.to_account_info();
        let signer_seeds: &[&[&[u8]]] = &[&[CONFIG_SEED, &[ctx.accounts.config.bump]]];
        let transfer_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_2022_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.comp_vault.to_account_info(),
                mint: ctx.accounts.ccm_mint.to_account_info(),
                to: ctx.accounts.user_ccm.to_account_info(),
                authority: config_ai,
            },
            signer_seeds,
        );
        token_interface::transfer_checked(transfer_ctx, amount, ctx.accounts.ccm_mint.decimals)?;

        let claimed = &mut ctx.accounts.claimed;
        claimed.user = ctx.accounts.user.key();
        claimed.amount = amount;
        claimed.bump = ctx.bumps.claimed;

        emit!(CompensationClaimedEvent {
            config: ctx.accounts.config.key(),
            user: ctx.accounts.user.key(),
            claimed_account: ctx.accounts.claimed.key(),
            comp_vault: ctx.accounts.comp_vault.key(),
            amount,
            slot,
        });

        Ok(())
    }
}

fn compensation_leaf(user: &Pubkey, amount: u64) -> [u8; 32] {
    keccak::hashv(&[
        COMPENSATION_LEAF_DOMAIN,
        user.as_ref(),
        amount.to_le_bytes().as_ref(),
    ])
    .to_bytes()
}

fn sorted_pair_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let (first, second) = if left <= right {
        (left.as_slice(), right.as_slice())
    } else {
        (right.as_slice(), left.as_slice())
    };
    keccak::hashv(&[first, second]).to_bytes()
}

#[inline(never)]
fn verify_compensation_proof(user: &Pubkey, amount: u64, proof: &[[u8; 32]], root: &[u8; 32]) -> bool {
    let mut computed = compensation_leaf(user, amount);
    for sibling in proof {
        computed = sorted_pair_hash(&computed, sibling);
    }
    &computed == root
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = signer,
        space = Config::LEN,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct InitializePool<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RailsError::Unauthorized,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(
        init,
        payer = admin,
        space = StakePool::LEN,
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump
    )]
    pub pool: Account<'info, StakePool>,
    /// CCM mint (Token-2022). Both vaults use this mint.
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    /// Principal vault: actual staked CCM lives here.
    #[account(
        init,
        payer = admin,
        seeds = [STAKE_VAULT_SEED, pool.key().as_ref()],
        bump,
        token::mint = ccm_mint,
        token::authority = pool,
        token::token_program = token_2022_program,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    /// Reward vault: keeper-funded emissions are paid out from here.
    #[account(
        init,
        payer = admin,
        seeds = [REWARD_VAULT_SEED, pool.key().as_ref()],
        bump,
        token::mint = ccm_mint,
        token::authority = pool,
        token::token_program = token_2022_program,
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Shared admin-gated context for config-only mutations (set_admin).
/// Does NOT include a system_program because no account is initialized here.
#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RailsError::Unauthorized
    )]
    pub config: Account<'info, Config>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct CompensateExternalStakers<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RailsError::Unauthorized,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub admin: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        init,
        payer = admin,
        seeds = [COMP_VAULT_SEED, config.key().as_ref()],
        bump,
        token::mint = ccm_mint,
        token::authority = config,
        token::token_program = token_2022_program,
    )]
    pub comp_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct SetRewardRate<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ RailsError::Unauthorized
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakePool>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct Stake<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakePool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ RailsError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, pool.key().as_ref()],
        bump,
        constraint = stake_vault.owner == pool.key() @ RailsError::Unauthorized,
        constraint = stake_vault.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        space = UserStake::LEN,
        seeds = [USER_STAKE_SEED, pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStake>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct FundRewardPool<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakePool>,
    #[account(mut)]
    pub funder: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        mut,
        constraint = funder_ccm.owner == funder.key() @ RailsError::Unauthorized,
        constraint = funder_ccm.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub funder_ccm: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [REWARD_VAULT_SEED, pool.key().as_ref()],
        bump,
        constraint = reward_vault.owner == pool.key() @ RailsError::Unauthorized,
        constraint = reward_vault.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct Unstake<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakePool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ RailsError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, pool.key().as_ref()],
        bump,
        constraint = stake_vault.owner == pool.key() @ RailsError::Unauthorized,
        constraint = stake_vault.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub stake_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [USER_STAKE_SEED, pool.key().as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ RailsError::Unauthorized,
        constraint = user_stake.pool == pool.key() @ RailsError::Unauthorized,
    )]
    pub user_stake: Account<'info, UserStake>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
#[instruction(pool_id: u32)]
pub struct Claim<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [POOL_SEED, &pool_id.to_le_bytes()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakePool>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ RailsError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [REWARD_VAULT_SEED, pool.key().as_ref()],
        bump,
        constraint = reward_vault.owner == pool.key() @ RailsError::Unauthorized,
        constraint = reward_vault.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [USER_STAKE_SEED, pool.key().as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        constraint = user_stake.user == user.key() @ RailsError::Unauthorized,
        constraint = user_stake.pool == pool.key() @ RailsError::Unauthorized,
    )]
    pub user_stake: Account<'info, UserStake>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct ClaimCompensation<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = ccm_mint @ RailsError::InvalidMint
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(address = config.ccm_mint)]
    pub ccm_mint: Box<InterfaceAccount<'info, MintInterface>>,
    #[account(
        mut,
        constraint = user_ccm.owner == user.key() @ RailsError::Unauthorized,
        constraint = user_ccm.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub user_ccm: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        mut,
        seeds = [COMP_VAULT_SEED, config.key().as_ref()],
        bump,
        constraint = comp_vault.owner == config.key() @ RailsError::Unauthorized,
        constraint = comp_vault.mint == ccm_mint.key() @ RailsError::InvalidMint,
    )]
    pub comp_vault: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(
        init,
        payer = user,
        space = CompensationClaimed::LEN,
        seeds = [COMP_CLAIMED_SEED, user.key().as_ref()],
        bump
    )]
    pub claimed: Account<'info, CompensationClaimed>,
    #[account(address = TOKEN_2022_PROGRAM_ID @ RailsError::InvalidTokenProgram)]
    pub token_2022_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
